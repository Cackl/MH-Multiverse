use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::store::DisplayNameState;

// ── Pak format ────────────────────────────────────────────────────────────────

const PAK_SIGNATURE: u32 = 1196441931; // "KAPG" little-endian
const PAK_VERSION: u32 = 1;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PrototypeMatch {
    pub path: String,
    pub blueprint: String,
    pub display_name: String,
    pub leaf: String,
}

struct PrototypeRecord {
    prototype_id: u64, // runtime prototype ID; used to build the by_id index
    blueprint_id: u64,
    path: String,
    is_abstract: bool,
}

pub(crate) struct PrototypeCatalogue {
    /// blueprint id → blueprint file path (e.g. "Entity/Avatar.blueprint")
    blueprints: HashMap<u64, String>,
    prototypes: Vec<PrototypeRecord>,
    /// prototype runtime id → prototype file path; built once in build_catalogue.
    by_id: HashMap<u64, String>,
    /// prototype file path → prototype runtime id; inverse of by_id.
    by_path: HashMap<String, u64>,
}

/// Cached catalogue keyed by the sip path it was built from.
/// If server_exe changes, the path changes and we rebuild automatically.
pub struct CatalogueState(pub Mutex<Option<(String, PrototypeCatalogue)>>);

// ── Binary helpers ────────────────────────────────────────────────────────────

fn read_u8(data: &[u8], pos: &mut usize) -> u8 {
    let v = data[*pos];
    *pos += 1;
    v
}

fn read_u16_le(data: &[u8], pos: &mut usize) -> u16 {
    let v = u16::from_le_bytes(data[*pos..*pos + 2].try_into().unwrap());
    *pos += 2;
    v
}

fn read_i32_le(data: &[u8], pos: &mut usize) -> i32 {
    let v = i32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    v
}

fn read_u32_le(data: &[u8], pos: &mut usize) -> u32 {
    let v = u32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    v
}

fn read_u64_le(data: &[u8], pos: &mut usize) -> u64 {
    let v = u64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;
    v
}

/// FixedString32: i32 byte-length prefix + UTF-8 bytes (used in pak entry table)
fn read_fixed_string32(data: &[u8], pos: &mut usize) -> String {
    let len = read_i32_le(data, pos) as usize;
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .unwrap_or("")
        .to_string();
    *pos += len;
    s
}

/// FixedString16: u16 byte-length prefix + UTF-8 bytes (used in directory records)
fn read_fixed_string16(data: &[u8], pos: &mut usize) -> String {
    let len = read_u16_le(data, pos) as usize;
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .unwrap_or("")
        .to_string();
    *pos += len;
    s
}

// ── Pak reader ────────────────────────────────────────────────────────────────

struct PakEntry {
    offset: usize,
    compressed_size: usize,
    uncompressed_size: usize,
}

/// Reads the pak header and entry table. Returns (raw file bytes, entry map, data section start).
fn load_pak(sip_path: &str) -> Result<(Vec<u8>, HashMap<String, PakEntry>, usize), String> {
    let data = std::fs::read(sip_path)
        .map_err(|e| format!("Cannot read Calligraphy.sip: {e}"))?;

    let mut pos = 0;

    let signature = read_u32_le(&data, &mut pos);
    if signature != PAK_SIGNATURE {
        return Err(format!(
            "Invalid Calligraphy.sip signature 0x{signature:X} — expected 0x{PAK_SIGNATURE:X}"
        ));
    }

    let version = read_u32_le(&data, &mut pos);
    if version != PAK_VERSION {
        return Err(format!(
            "Unexpected Calligraphy.sip version {version} — expected {PAK_VERSION}"
        ));
    }

    let num_entries = read_i32_le(&data, &mut pos) as usize;
    let mut entries = HashMap::with_capacity(num_entries);

    for _ in 0..num_entries {
        let _hash = read_u64_le(&data, &mut pos);
        let file_path = read_fixed_string32(&data, &mut pos);
        let _mod_time = read_i32_le(&data, &mut pos);
        let offset = read_i32_le(&data, &mut pos) as usize;
        let compressed_size = read_i32_le(&data, &mut pos) as usize;
        let uncompressed_size = read_i32_le(&data, &mut pos) as usize;

        entries.insert(
            file_path,
            PakEntry { offset, compressed_size, uncompressed_size },
        );
    }

    let data_section_start = pos;
    Ok((data, entries, data_section_start))
}

/// Decompresses and returns a file stored in the pak.
/// LZ4 block format (no frame header) with the uncompressed size stored in the entry table.
fn extract_pak_file(
    pak_data: &[u8],
    entries: &HashMap<String, PakEntry>,
    data_section_start: usize,
    file_path: &str,
) -> Result<Vec<u8>, String> {
    let entry = entries
        .get(file_path)
        .ok_or_else(|| format!("'{file_path}' not found in Calligraphy.sip"))?;

    let start = data_section_start + entry.offset;
    let compressed = &pak_data[start..start + entry.compressed_size];

    lz4_flex::block::decompress(compressed, entry.uncompressed_size)
        .map_err(|e| format!("LZ4 decompression of '{file_path}' failed: {e}"))
}

// ── Calligraphy directory parsers ─────────────────────────────────────────────

/// Reads the 4-byte CalligraphyHeader (3 bytes magic + 1 byte version).
/// Returns version so the caller can choose record count width.
fn read_calligraphy_header(data: &[u8], pos: &mut usize) -> u8 {
    *pos += 3; // magic bytes ("Cal" or similar — not used)
    let version = data[*pos];
    *pos += 1;
    version
}

/// Version >= 11 uses i32 record count; earlier uses u16.
fn read_record_count(data: &[u8], pos: &mut usize, version: u8) -> usize {
    if version >= 11 {
        read_i32_le(data, pos) as usize
    } else {
        read_u16_le(data, pos) as usize
    }
}

/// Parses Blueprint.directory.
/// Each record: u64 id + u64 guid + u8 flags + FixedString16 filePath
fn parse_blueprint_directory(data: &[u8]) -> Result<HashMap<u64, String>, String> {
    let mut pos = 0;
    let version = read_calligraphy_header(data, &mut pos);
    let count = read_record_count(data, &mut pos, version);
    let mut blueprints = HashMap::with_capacity(count);

    for _ in 0..count {
        let id = read_u64_le(data, &mut pos);
        let _guid = read_u64_le(data, &mut pos);
        let _flags = read_u8(data, &mut pos);
        let path = read_fixed_string16(data, &mut pos).replace('\\', "/");
        blueprints.insert(id, path);
    }

    Ok(blueprints)
}

/// Parses Prototype.directory.
/// Each record: u64 prototypeId + u64 prototypeGuid + u64 blueprintId + u8 flags + FixedString16 filePath
fn parse_prototype_directory(data: &[u8]) -> Result<Vec<PrototypeRecord>, String> {
    let mut pos = 0;
    let version = read_calligraphy_header(data, &mut pos);
    let count = read_record_count(data, &mut pos, version);
    let mut prototypes = Vec::with_capacity(count);

    for _ in 0..count {
        let prototype_id = read_u64_le(data, &mut pos); // stored; used to build by_id index
        let _prototype_guid = read_u64_le(data, &mut pos);
        let blueprint_id = read_u64_le(data, &mut pos);
        let flags = read_u8(data, &mut pos);
        let path = read_fixed_string16(data, &mut pos).replace('\\', "/");

        // PrototypeRecordFlags::Abstract = bit 0
        let is_abstract = flags & 0x01 != 0;

        prototypes.push(PrototypeRecord { prototype_id, blueprint_id, path, is_abstract });
    }

    Ok(prototypes)
}

// ── Catalogue builder ─────────────────────────────────────────────────────────

fn build_catalogue(sip_path: &str) -> Result<PrototypeCatalogue, String> {
    let (pak_data, entries, data_section_start) = load_pak(sip_path)?;

    let blueprint_data = extract_pak_file(
        &pak_data, &entries, data_section_start,
        "Calligraphy/Blueprint.directory",
    )?;

    let prototype_data = extract_pak_file(
        &pak_data, &entries, data_section_start,
        "Calligraphy/Prototype.directory",
    )?;

    let blueprints = parse_blueprint_directory(&blueprint_data)?;
    let prototypes = parse_prototype_directory(&prototype_data)?;

    // Build runtime-ID ↔ path indices; by_id for display name resolution,
    // by_path for the inverse lookup needed when adding GuidItems from a path search.
    let by_id: HashMap<u64, String> = prototypes
        .iter()
        .map(|p| (p.prototype_id, p.path.clone()))
        .collect();

    let by_path: HashMap<String, u64> = prototypes
        .iter()
        .map(|p| (p.path.clone(), p.prototype_id))
        .collect();

    Ok(PrototypeCatalogue { blueprints, prototypes, by_id, by_path })
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Look up the prototype file path for a given runtime prototype ID.
///
/// Returns `None` if the ID is not present in the catalogue (catalogue not yet
/// loaded, or the ID is from a file not present in `Calligraphy.sip`).
///
/// Used by `store.rs` for display name resolution without duplicating catalogue
/// access logic.
pub(crate) fn path_for_id(catalogue: &PrototypeCatalogue, id: u64) -> Option<&str> {
    catalogue.by_id.get(&id).map(|s| s.as_str())
}

/// Look up the prototype runtime ID for a given prototype file path.
///
/// Returns `None` if the path is not in the catalogue.
/// Used by `lookup_prototype_id` in `store.rs` to resolve a selected prototype
/// path back to the `ItemPrototypeRuntimeIdForClient` needed by catalog entries.
pub(crate) fn id_for_path(catalogue: &PrototypeCatalogue, path: &str) -> Option<u64> {
    catalogue.by_path.get(path).copied()
}

// ── Tauri command ─────────────────────────────────────────────────────────────

/// Search prototype paths in Calligraphy.sip.
///
/// - `query`: substring to match against prototype file paths (case-insensitive).
///   Must be at least 2 characters; returns an empty list otherwise.
/// - `blueprint_hint`: optional case-insensitive substring matched against the
///   blueprint path associated with each prototype. Use this to restrict results
///   to a specific category (e.g. "LootTable", "Avatar", "Region").
///   When None, all blueprint types are searched.
///
/// Returns up to 100 matches. The catalogue is loaded lazily on first call and
/// cached for subsequent calls. If server_exe changes, the sip path changes and
/// the catalogue is rebuilt automatically.
#[tauri::command]
pub fn search_prototypes(
    state: tauri::State<CatalogueState>,
    dn_state: tauri::State<DisplayNameState>,
    server_exe: String,
    query: String,
    blueprint_hint: Option<String>,
) -> Result<Vec<PrototypeMatch>, String> {
    if query.len() < 2 && blueprint_hint.is_none() {
        return Ok(vec![]);
    }

    let sip_path = Path::new(&server_exe)
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())?
        .join("Data")
        .join("Game")
        .join("Calligraphy.sip");

    let sip_path_str = sip_path
        .to_str()
        .ok_or_else(|| "Calligraphy.sip path contains invalid UTF-8".to_string())?
        .to_string();

    let server_dir = Path::new(&server_exe)
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())?
        .to_string_lossy()
        .to_string();

    if !sip_path.exists() {
        return Err(format!("Calligraphy.sip not found at {sip_path_str}"));
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "Catalogue state lock is poisoned".to_string())?;

    // Build or rebuild if sip path has changed
    let needs_build = guard
        .as_ref()
        .map(|(cached_path, _)| cached_path != &sip_path_str)
        .unwrap_or(true);

    if needs_build {
        *guard = Some((sip_path_str.clone(), build_catalogue(&sip_path_str)?));
    }

    let (_, catalogue) = guard.as_ref().unwrap();

    let query_lower = query.to_lowercase();
    let hint_lower = blueprint_hint.as_deref().map(str::to_lowercase);

    let max_results: usize = if query.is_empty() { 500 } else { 100 };

    let results = catalogue
        .prototypes
        .iter()
        .filter(|p| !p.is_abstract)
        .filter(|p| {
            // Blueprint hint filter — substring match against blueprint path
            if let Some(ref hint) = hint_lower {
                let bp_path = catalogue
                    .blueprints
                    .get(&p.blueprint_id)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if !bp_path.contains(hint.as_str()) {
                    return false;
                }
            }
            true
        })
        .filter(|p| p.path.to_lowercase().contains(&query_lower))
        .take(max_results)
        .map(|p| PrototypeMatch {
            path: p.path.clone(),
            blueprint: catalogue
                .blueprints
                .get(&p.blueprint_id)
                .cloned()
                .unwrap_or_default(),
            display_name: dn_state.lookup(&server_dir, &p.path),
            leaf: p.path.rsplit('/').next().unwrap_or(&p.path).to_string(),
        })
        .collect();

    Ok(results)
}

/// Resolve a prototype file path to its runtime ID (as a decimal string).
///
/// This is the inverse of `path_for_id` — given a path returned by
/// `search_prototypes`, it returns the `ItemPrototypeRuntimeIdForClient`
/// value needed to construct a `GuidItem` in a catalog entry.
///
/// Returns as a String to safely transport the full u64 range across the
/// JS boundary without precision loss.
#[tauri::command]
pub fn lookup_prototype_id(
    state: tauri::State<CatalogueState>,
    server_exe: String,
    prototype_path: String,
) -> Result<String, String> {
    let sip_path = Path::new(&server_exe)
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())?
        .join("Data")
        .join("Game")
        .join("Calligraphy.sip");

    let sip_path_str = sip_path
        .to_str()
        .ok_or_else(|| "Calligraphy.sip path contains invalid UTF-8".to_string())?
        .to_string();

    if !sip_path.exists() {
        return Err(format!("Calligraphy.sip not found at {sip_path_str}"));
    }

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "Catalogue state lock is poisoned".to_string())?;

    let needs_build = guard
        .as_ref()
        .map(|(cached_path, _)| cached_path != &sip_path_str)
        .unwrap_or(true);

    if needs_build {
        *guard = Some((sip_path_str.clone(), build_catalogue(&sip_path_str)?));
    }

    let (_, catalogue) = guard.as_ref().unwrap();

    id_for_path(catalogue, &prototype_path)
        .map(|id| id.to_string())
        .ok_or_else(|| format!("Prototype '{}' not found in catalogue", prototype_path))
}