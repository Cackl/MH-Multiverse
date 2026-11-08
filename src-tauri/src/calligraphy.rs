use serde::Serialize;
use std::collections::HashMap;
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
    /// The prototype's class (its blueprint's RuntimeBinding, e.g. "AvatarPrototype").
    pub class: String,
    /// Where `display_name` came from: "custom", "game", "bundled" or "path" (see `resolve_name`).
    pub name_source: String,
    /// Runtime prototype ID (PrototypeId / PrototypeDataRef in patch files).
    /// Serialised as a decimal string to preserve full u64 precision across the JS boundary.
    pub prototype_id: String,
    /// Prototype GUID (PrototypeGuid in patch files).
    /// Serialised as a decimal string for the same reason.
    pub prototype_guid: String,
}

struct PrototypeRecord {
    prototype_id: u64,
    prototype_guid: u64,
    blueprint_id: u64,
    path: String,
    is_abstract: bool,
    /// English name from the game client's strings (own or inherited `DisplayName`-style field).
    game_name: Option<String>,
    /// Display name after the fallback order in `resolve_name`, and where it came from.
    name: String,
    name_source: &'static str,
    /// Lowercase name and path, precomputed for search.
    search_name: String,
    search_path: String,
}

pub(crate) struct PrototypeCatalogue {
    /// blueprint id → blueprint file path (e.g. "Entity/Avatar.blueprint")
    blueprints: HashMap<u64, String>,
    /// blueprint id → its RuntimeBinding class followed by its ancestors' (e.g. AvatarPrototype, AgentPrototype, …)
    classes: HashMap<u64, Vec<String>>,
    prototypes: Vec<PrototypeRecord>,
    /// prototype runtime id → index into `prototypes`; built once in build_catalogue.
    by_id: HashMap<u64, usize>,
    /// prototype file path → prototype runtime id; inverse of by_id.
    by_path: HashMap<String, u64>,
}

/// Cached catalogue keyed by the sip path it was built from.
/// If server_exe changes, the path changes and we rebuild automatically.
pub struct CatalogueState(pub Mutex<Option<(String, PrototypeCatalogue)>>);

// ── Binary helpers ────────────────────────────────────────────────────────────

fn eod_err(pos: usize) -> String {
    format!("Unexpected end of data at offset {pos}. File may be truncated or corrupt")
}

fn read_u8(data: &[u8], pos: &mut usize) -> Result<u8, String> {
    let v = *data.get(*pos).ok_or_else(|| eod_err(*pos))?;
    *pos += 1;
    Ok(v)
}

fn read_u16_le(data: &[u8], pos: &mut usize) -> Result<u16, String> {
    let end = *pos + 2;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let v = u16::from_le_bytes(bytes.try_into().unwrap());
    *pos = end;
    Ok(v)
}

fn read_i32_le(data: &[u8], pos: &mut usize) -> Result<i32, String> {
    let end = *pos + 4;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let v = i32::from_le_bytes(bytes.try_into().unwrap());
    *pos = end;
    Ok(v)
}

fn read_u32_le(data: &[u8], pos: &mut usize) -> Result<u32, String> {
    let end = *pos + 4;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let v = u32::from_le_bytes(bytes.try_into().unwrap());
    *pos = end;
    Ok(v)
}

fn read_u64_le(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let end = *pos + 8;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let v = u64::from_le_bytes(bytes.try_into().unwrap());
    *pos = end;
    Ok(v)
}

/// FixedString32: i32 byte-length prefix + UTF-8 bytes (used in pak entry table)
fn read_fixed_string32(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_i32_le(data, pos)? as usize;
    let end = pos.checked_add(len).ok_or_else(|| eod_err(*pos))?;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let s = std::str::from_utf8(bytes).unwrap_or("").to_string();
    *pos = end;
    Ok(s)
}

/// FixedString16: u16 byte-length prefix + UTF-8 bytes (used in directory records)
fn read_fixed_string16(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_u16_le(data, pos)? as usize;
    let end = *pos + len;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let s = std::str::from_utf8(bytes).unwrap_or("").to_string();
    *pos = end;
    Ok(s)
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

    let signature = read_u32_le(&data, &mut pos)?;
    if signature != PAK_SIGNATURE {
        return Err(format!(
            "Invalid Calligraphy.sip signature 0x{signature:X}, expected 0x{PAK_SIGNATURE:X}"
        ));
    }

    let version = read_u32_le(&data, &mut pos)?;
    if version != PAK_VERSION {
        return Err(format!(
            "Unexpected Calligraphy.sip version {version}, expected {PAK_VERSION}"
        ));
    }

    let num_entries = read_i32_le(&data, &mut pos)? as usize;
    let mut entries = HashMap::new();

    for _ in 0..num_entries {
        let _hash = read_u64_le(&data, &mut pos)?;
        let file_path = read_fixed_string32(&data, &mut pos)?;
        let _mod_time = read_i32_le(&data, &mut pos)?;
        let offset = read_i32_le(&data, &mut pos)? as usize;
        let compressed_size = read_i32_le(&data, &mut pos)? as usize;
        let uncompressed_size = read_i32_le(&data, &mut pos)? as usize;

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

    let start = data_section_start
        .checked_add(entry.offset)
        .ok_or_else(|| format!("'{file_path}' entry offset overflow"))?;
    let end = start
        .checked_add(entry.compressed_size)
        .ok_or_else(|| format!("'{file_path}' entry size overflow"))?;
    let compressed = pak_data
        .get(start..end)
        .ok_or_else(|| format!("'{file_path}' entry extends beyond Calligraphy.sip data"))?;

    lz4_flex::block::decompress(compressed, entry.uncompressed_size)
        .map_err(|e| format!("LZ4 decompression of '{file_path}' failed: {e}"))
}

// ── Calligraphy directory parsers ─────────────────────────────────────────────

/// Reads the 4-byte CalligraphyHeader (3 bytes magic + 1 byte version).
/// Returns version so the caller can choose record count width.
fn read_calligraphy_header(data: &[u8], pos: &mut usize) -> Result<u8, String> {
    let end = *pos + 4;
    let bytes = data.get(*pos..end).ok_or_else(|| eod_err(*pos))?;
    let version = bytes[3]; // first 3 bytes are magic ("Cal" or similar — not used)
    *pos = end;
    Ok(version)
}

/// Version >= 11 uses i32 record count; earlier uses u16.
fn read_record_count(data: &[u8], pos: &mut usize, version: u8) -> Result<usize, String> {
    if version >= 11 {
        Ok(read_i32_le(data, pos)? as usize)
    } else {
        Ok(read_u16_le(data, pos)? as usize)
    }
}

/// Parses Blueprint.directory.
/// Each record: u64 id + u64 guid + u8 flags + FixedString16 filePath
fn parse_blueprint_directory(data: &[u8]) -> Result<HashMap<u64, String>, String> {
    let mut pos = 0;
    let version = read_calligraphy_header(data, &mut pos)?;
    let count = read_record_count(data, &mut pos, version)?;
    let mut blueprints = HashMap::new();

    for _ in 0..count {
        let id = read_u64_le(data, &mut pos)?;
        let _guid = read_u64_le(data, &mut pos)?;
        let _flags = read_u8(data, &mut pos)?;
        let path = read_fixed_string16(data, &mut pos)?.replace('\\', "/");
        blueprints.insert(id, path);
    }

    Ok(blueprints)
}

/// Parses Prototype.directory.
/// Each record: u64 prototypeId + u64 prototypeGuid + u64 blueprintId + u8 flags + FixedString16 filePath
fn parse_prototype_directory(data: &[u8]) -> Result<Vec<PrototypeRecord>, String> {
    let mut pos = 0;
    let version = read_calligraphy_header(data, &mut pos)?;
    let count = read_record_count(data, &mut pos, version)?;
    let mut prototypes = Vec::new();

    for _ in 0..count {
        let prototype_id = read_u64_le(data, &mut pos)?;
        let prototype_guid = read_u64_le(data, &mut pos)?;
        let blueprint_id = read_u64_le(data, &mut pos)?;
        let flags = read_u8(data, &mut pos)?;
        let path = read_fixed_string16(data, &mut pos)?.replace('\\', "/");

        // PrototypeRecordFlags::Abstract = bit 0
        let is_abstract = flags & 0x01 != 0;

        prototypes.push(PrototypeRecord { prototype_id, prototype_guid, blueprint_id, path, is_abstract, game_name: None, name: String::new(), name_source: "", search_name: String::new(), search_path: String::new() });
    }

    Ok(prototypes)
}

// ── Game names: blueprint / prototype skip-scanner and client strings ────────
//
// Format: MHServerEmu docs/GameData/Calligraphy.md and Locale.md. A prototype file stores only
// the fields it overrides, each tagged with its field id and base type, so the scanner can skip
// every value without knowing the schema; the only lookup is which field ids are name fields.

/// LocaleStringId fields that hold a prototype's name, in priority order.
const NAME_FIELDS: &[&str] = &["DisplayName", "Name", "RegionName", "AreaName"];

/// Parses a `.blueprint` file: returns (RuntimeBinding, parent blueprint ids) and records the
/// field ids of its name fields (field id → priority) in `name_fields`.
fn parse_blueprint_file(d: &[u8], name_fields: &mut HashMap<u64, usize>) -> Result<(String, Vec<u64>), String> {
    let mut pos = 4; // CalligraphyHeader
    let binding = read_fixed_string16(d, &mut pos)?;
    read_u64_le(d, &mut pos)?; // default prototype id
    let mut parents = Vec::new();
    for _ in 0..read_u16_le(d, &mut pos)? {
        parents.push(read_u64_le(d, &mut pos)?);
        read_u8(d, &mut pos)?; // copies
    }
    for _ in 0..read_u16_le(d, &mut pos)? {
        read_u64_le(d, &mut pos)?; // contributing blueprint
        read_u8(d, &mut pos)?;
    }
    for _ in 0..read_u16_le(d, &mut pos)? {
        let field_id = read_u64_le(d, &mut pos)?;
        let field_name = read_fixed_string16(d, &mut pos)?;
        let base_type = read_u8(d, &mut pos)?;
        read_u8(d, &mut pos)?; // structure type
        if matches!(base_type, b'A' | b'C' | b'P' | b'R') {
            read_u64_le(d, &mut pos)?; // subtype
        }
        if base_type == b'S' {
            if let Some(rank) = NAME_FIELDS.iter().position(|n| *n == field_name) {
                name_fields.insert(field_id, rank);
            }
        }
    }
    Ok((binding, parents))
}

/// What a prototype file says about itself: its parent and the best name field it sets.
#[derive(Default)]
struct PrototypeScan {
    parent: u64,
    /// (priority, LocaleStringId); a LocaleStringId of 0 means "explicitly no name".
    name: Option<(usize, u64)>,
}

/// Skips one prototype body (header + field groups) starting at `pos`. Names are only recorded
/// at the top level (`out` is `None` inside embedded structs), so a sub-object's DisplayName
/// never becomes the prototype's own. Property-mixin groups share this layout on disk.
fn scan_prototype(
    d: &[u8],
    pos: &mut usize,
    name_fields: &HashMap<u64, usize>,
    mut out: Option<&mut PrototypeScan>,
) -> Result<(), String> {
    let flags = read_u8(d, pos)?;
    let reference = if flags & 1 != 0 { read_u64_le(d, pos)? } else { 0 };
    if let Some(o) = out.as_deref_mut() {
        o.parent = reference;
    }
    if flags & 2 == 0 {
        return Ok(()); // no instance data
    }
    for _ in 0..read_u16_le(d, pos)? {
        read_u64_le(d, pos)?; // declaring blueprint
        read_u8(d, pos)?; // copy number
        for _ in 0..read_u16_le(d, pos)? {
            let field_id = read_u64_le(d, pos)?;
            let base_type = read_u8(d, pos)?;
            if let (b'S', Some(o), Some(&rank)) = (base_type, out.as_deref_mut(), name_fields.get(&field_id)) {
                let mut peek = *pos;
                let string_id = read_u64_le(d, &mut peek)?;
                if o.name.map_or(true, |(r, _)| rank < r) {
                    o.name = Some((rank, string_id));
                }
            }
            skip_value(d, pos, base_type, name_fields)?;
        }
        for _ in 0..read_u16_le(d, pos)? {
            read_u64_le(d, pos)?; // field id
            let base_type = read_u8(d, pos)?;
            for _ in 0..read_u16_le(d, pos)? {
                skip_value(d, pos, base_type, name_fields)?;
            }
        }
    }
    Ok(())
}

fn skip_value(d: &[u8], pos: &mut usize, base_type: u8, name_fields: &HashMap<u64, usize>) -> Result<(), String> {
    match base_type {
        b'R' => scan_prototype(d, pos, name_fields, None),
        b'A' | b'B' | b'C' | b'D' | b'L' | b'P' | b'S' | b'T' => read_u64_le(d, pos).map(|_| ()),
        other => Err(format!("Unknown Calligraphy base type 0x{other:02X} at offset {}", *pos - 1)),
    }
}

/// Scans a whole `.prototype` file; errors unless every byte is accounted for, so a wrong skip
/// can't silently produce a wrong name.
fn scan_prototype_file(d: &[u8], name_fields: &HashMap<u64, usize>) -> Result<PrototypeScan, String> {
    let mut scan = PrototypeScan::default();
    let mut pos = 4; // CalligraphyHeader
    scan_prototype(d, &mut pos, name_fields, Some(&mut scan))?;
    if pos != d.len() {
        return Err(format!("{} unread bytes after prototype data", d.len() - pos));
    }
    Ok(scan)
}

/// Parses a client `.string` file (Locale.md) into `out`. Entry: u64 id, u16 variants,
/// u16 flags, u32 offset, then (variants - 1) × (u64 flags consumed, u16 flags produced,
/// u32 offset); the default text is a null-terminated UTF-8 string at the absolute offset.
fn parse_string_file(d: &[u8], out: &mut HashMap<u64, String>) -> Result<(), String> {
    let mut pos = 4; // CalligraphyHeader
    for _ in 0..read_u16_le(d, &mut pos)? {
        let id = read_u64_le(d, &mut pos)?;
        let variants = read_u16_le(d, &mut pos)?;
        read_u16_le(d, &mut pos)?; // flags produced
        let offset = read_u32_le(d, &mut pos)? as usize;
        for _ in 1..variants {
            read_u64_le(d, &mut pos)?;
            read_u16_le(d, &mut pos)?;
            read_u32_le(d, &mut pos)?;
        }
        let tail = d.get(offset..).ok_or_else(|| eod_err(offset))?;
        let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        out.insert(id, String::from_utf8_lossy(&tail[..end]).into_owned());
    }
    Ok(())
}

static MARKUP_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"#/?[A-Za-z0-9_]+#").unwrap());
static TEMPLATE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\$[A-Za-z0-9_]+\$").unwrap());

/// Client text → a plain name: strips colour markup (`#orange#…#/orange#`); a template
/// (`$RegionName$`) or blank text is no name.
fn clean_game_name(text: &str) -> Option<String> {
    let plain = MARKUP_RE.replace_all(text, "");
    let plain = plain.trim();
    (!plain.is_empty() && !TEMPLATE_RE.is_match(plain)).then(|| plain.to_string())
}

/// Reads every `*.string` file in the client's `Loco/eng.all` folder. Unreadable files are
/// skipped: names are a nicety and every caller has a fallback.
fn load_strings(loco_dir: &std::path::Path) -> HashMap<u64, String> {
    let mut strings = HashMap::new();
    for entry in std::fs::read_dir(loco_dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "string") {
            if let Ok(d) = std::fs::read(&path) {
                let _ = parse_string_file(&d, &mut strings);
            }
        }
    }
    strings
}

/// The name a prototype gets in game: the first prototype up its parent chain that sets a name
/// field decides (a 0 id there means no name, as on the server).
fn inherited_game_name(id: u64, scans: &HashMap<u64, PrototypeScan>, strings: &HashMap<u64, String>) -> Option<String> {
    let mut id = id;
    for _ in 0..64 {
        let scan = scans.get(&id)?;
        if let Some((_, string_id)) = scan.name {
            return strings.get(&string_id).and_then(|t| clean_game_name(t));
        }
        if scan.parent == 0 {
            return None;
        }
        id = scan.parent;
    }
    None
}

/// blueprint id → [own RuntimeBinding, ancestors' bindings…] (depth-first, deduplicated).
fn class_ancestry(bindings: &HashMap<u64, (String, Vec<u64>)>) -> HashMap<u64, Vec<String>> {
    fn walk(id: u64, bindings: &HashMap<u64, (String, Vec<u64>)>, out: &mut Vec<String>, depth: usize) {
        let Some((binding, parents)) = bindings.get(&id) else { return };
        if depth > 64 {
            return;
        }
        if !out.contains(binding) {
            out.push(binding.clone());
        }
        for p in parents {
            walk(*p, bindings, out, depth + 1);
        }
    }
    bindings
        .keys()
        .map(|id| {
            let mut out = Vec::new();
            walk(*id, bindings, &mut out, 0);
            (*id, out)
        })
        .collect()
}

// ── Catalogue builder ─────────────────────────────────────────────────────────

fn build_catalogue(sip_path: &str, loco_dir: Option<&std::path::Path>) -> Result<PrototypeCatalogue, String> {
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
    let mut prototypes = parse_prototype_directory(&prototype_data)?;

    // Names and classes are best effort: a file that is missing or fails to parse just leaves
    // its prototypes unnamed, and the directories above still give search and id lookups.
    let mut name_fields = HashMap::new();
    let bindings: HashMap<u64, (String, Vec<u64>)> = blueprints
        .iter()
        .filter_map(|(id, path)| {
            let d = extract_pak_file(&pak_data, &entries, data_section_start, &format!("Calligraphy/{path}")).ok()?;
            Some((*id, parse_blueprint_file(&d, &mut name_fields).ok()?))
        })
        .collect();
    let classes = class_ancestry(&bindings);

    let strings = loco_dir.map(load_strings).unwrap_or_default();
    if !strings.is_empty() {
        let scans: HashMap<u64, PrototypeScan> = prototypes
            .iter()
            .filter_map(|p| {
                let d = extract_pak_file(&pak_data, &entries, data_section_start, &format!("Calligraphy/{}", p.path)).ok()?;
                Some((p.prototype_id, scan_prototype_file(&d, &name_fields).ok()?))
            })
            .collect();
        for p in &mut prototypes {
            p.game_name = inherited_game_name(p.prototype_id, &scans, &strings);
        }
    }

    // Build runtime-ID ↔ path indices; by_id for display name resolution,
    // by_path for the inverse lookup needed when adding GuidItems from a path search.
    let by_id: HashMap<u64, usize> = prototypes
        .iter()
        .enumerate()
        .map(|(i, p)| (p.prototype_id, i))
        .collect();

    let by_path: HashMap<String, u64> = prototypes
        .iter()
        .map(|p| (p.path.clone(), p.prototype_id))
        .collect();

    Ok(PrototypeCatalogue { blueprints, classes, prototypes, by_id, by_path })
}

/// Blueprint types (RuntimeBinding) of every class deriving from EntityPrototype. The game's
/// EntityPrototype enum indexes exactly the prototypes of these types.
const ENTITY_BLUEPRINT_TYPES: &[&str] = &[
    "AgentPrototype", "AgentTeamUpPrototype", "ArmorPrototype", "ArtifactPrototype", "AvatarPrototype",
    "BagItemPrototype", "CharacterTokenPrototype", "CostumeCorePrototype", "CostumePrototype",
    "CraftingIngredientPrototype", "CraftingRecipePrototype", "DestructiblePropPrototype",
    "DestructibleSmartPropPrototype", "EmoteTokenPrototype", "EntityPrototype", "HotspotPrototype",
    "InventoryStashTokenPrototype", "ItemPrototype", "KismetSequenceEntityPrototype", "LegendaryPrototype",
    "MatchMetaGamePrototype", "MedalPrototype", "MetaGamePrototype", "MissilePrototype",
    "MissionMetaGamePrototype", "OrbPrototype", "PlayerPrototype", "PropPrototype", "PvPPrototype",
    "RelicPrototype", "SmartPropPrototype", "SpawnerPrototype", "TeamUpGearPrototype",
    "TransitionPrototype", "WorldEntityPrototype",
];

/// Prototype paths in EntityPrototype enum order: the entry for `enumIdx` n is at `[n - 1]`
/// (0 means none). The overlay hook reports items and killers by this index.
pub(crate) fn entity_paths(sip_path: &str) -> Result<Vec<String>, String> {
    let (pak, entries, start) = load_pak(sip_path)?;
    let blueprints = parse_blueprint_directory(&extract_pak_file(&pak, &entries, start, "Calligraphy/Blueprint.directory")?)?;
    let mut prototypes = parse_prototype_directory(&extract_pak_file(&pak, &entries, start, "Calligraphy/Prototype.directory")?)?;

    let is_entity: HashMap<u64, bool> = blueprints
        .iter()
        .map(|(id, path)| {
            let ty = extract_pak_file(&pak, &entries, start, &format!("Calligraphy/{path}"))
                .ok()
                .and_then(|d| read_fixed_string16(&d, &mut 4).ok());
            (*id, ty.is_some_and(|t| ENTITY_BLUEPRINT_TYPES.contains(&t.as_str())))
        })
        .collect();

    prototypes.sort_by_key(|p| p.prototype_id);
    Ok(prototypes
        .into_iter()
        .filter(|p| is_entity.get(&p.blueprint_id).copied().unwrap_or(false))
        .map(|p| p.path)
        .collect())
}

fn sip_path_for_server_exe(server_exe: &str) -> Result<String, String> {
    let sip_path = crate::paths::server_dir(server_exe)?
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

    Ok(sip_path_str)
}

/// The game client's English string folder: searched up from the game exe (as
/// `overlay::sip_candidates` does for its Calligraphy.sip), then in the server's own Data folder.
fn loco_dir(game_exe: &str, server_exe: &str) -> Option<std::path::PathBuf> {
    const LOCO: &str = "Data/Game/Loco/eng.all";
    std::path::Path::new(game_exe)
        .ancestors()
        .skip(1)
        .take(5)
        .map(|d| d.join(LOCO))
        .chain(crate::paths::server_dir(server_exe).ok().map(|d| d.join(LOCO)))
        .find(|p| p.is_dir())
}

/// Splits a prototype file name into words: `.../CowKingSpearLunge.prototype` → "Cow King Spear Lunge".
fn leaf_words(path: &str) -> String {
    let leaf = path.rsplit('/').next().unwrap_or(path);
    let leaf = leaf.split('.').next().unwrap_or(leaf);
    let chars: Vec<char> = leaf.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            out.push(' ');
            continue;
        }
        if i > 0 && c.is_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower) {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A prototype's display name and its source, first match wins: the server's
/// `display_names_custom.json`, the game client's strings, the bundled `display_names.json`,
/// then the file name split into words.
fn resolve_name(custom: Option<&str>, game: Option<&str>, bundled: Option<&str>, path: &str) -> (String, &'static str) {
    if let Some(n) = custom.filter(|n| crate::store::usable_name(n)) {
        return (n.to_string(), "custom");
    }
    if let Some(n) = game {
        return (n.to_string(), "game");
    }
    if let Some(n) = bundled {
        return (n.to_string(), "bundled");
    }
    (leaf_words(path), "path")
}

/// Ensure the cached prototype catalogue is loaded for the given server executable.
///
/// Rebuilds the cache when the resolved `Calligraphy.sip` path or client string folder changes.
pub(crate) fn ensure_catalogue_loaded(
    state: &CatalogueState,
    dn: &DisplayNameState,
    server_exe: &str,
    game_exe: &str,
) -> Result<(), String> {
    let sip_path_str = sip_path_for_server_exe(server_exe)?;
    let loco = loco_dir(game_exe, server_exe);
    let cache_key = format!("{sip_path_str}|{}", loco.as_deref().map(|p| p.display().to_string()).unwrap_or_default());

    let mut guard = state
        .0
        .lock()
        .map_err(|_| "Catalogue state lock is poisoned".to_string())?;

    let needs_build = guard
        .as_ref()
        .map(|(cached_key, _)| cached_key != &cache_key)
        .unwrap_or(true);

    if needs_build {
        let mut catalogue = build_catalogue(&sip_path_str, loco.as_deref())?;
        let custom = crate::store::read_custom_names(&crate::paths::server_dir(server_exe)?);
        for p in &mut catalogue.prototypes {
            (p.name, p.name_source) = resolve_name(
                custom.get(&p.path).map(String::as_str),
                p.game_name.as_deref(),
                dn.bundled(&p.path),
                &p.path,
            );
            p.search_name = p.name.to_lowercase();
            p.search_path = p.path.to_lowercase();
        }
        *guard = Some((cache_key, catalogue));
    }

    Ok(())
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Display name for a runtime prototype ID (see `resolve_name`), if the catalogue has it.
pub(crate) fn name_for_id(catalogue: &PrototypeCatalogue, id: u64) -> Option<&str> {
    catalogue.by_id.get(&id).map(|&i| catalogue.prototypes[i].name.as_str())
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
/// Filtering supports the category model used by `categories.json`:
/// - `category_path`: optional prototype-path prefix filter. May contain one or
///   more prefixes separated by `|`.
/// - `is_inventory_type`: carried through from the category config for parity
///   with the source app. The current Rust picker still filters by path prefix,
///   which is sufficient for stash categories because their prototype paths live
///   under `Entity/Inventory/...` in the catalogue.
/// - `blueprint_hint`: legacy parameter retained for compatibility with older
///   frontend code. If `category_path` is omitted, this value is treated as a
///   prototype-path prefix filter rather than a blueprint-path filter.
///
/// Search text is matched case-insensitively against both:
/// - prototype path
/// - display name (see `resolve_name`)
///
/// `class_filter` keeps prototypes whose class is, or derives from, that RuntimeBinding
/// (e.g. "ItemPrototype" also matches costumes and artifacts).
///
/// Results are ranked (see `match_rank`). Returns up to 500 matches for category browsing
/// when `query` is empty, or up to 100 matches when searching.
#[tauri::command]
pub fn search_prototypes(
    app: tauri::AppHandle,
    state: tauri::State<CatalogueState>,
    dn_state: tauri::State<DisplayNameState>,
    server_exe: String,
    query: String,
    blueprint_hint: Option<String>,
    category_path: Option<String>,
    is_inventory_type: Option<bool>,
    class_filter: Option<String>,
) -> Result<Vec<PrototypeMatch>, String> {
    let effective_category = category_path
        .or(blueprint_hint)
        .unwrap_or_default();

    if query.trim().len() < 2 && effective_category.trim().is_empty() {
        return Ok(vec![]);
    }

    let game_exe = crate::config::load_config(&app).game_exe;
    ensure_catalogue_loaded(&state, &dn_state, &server_exe, &game_exe)?;

    let guard = state
        .0
        .lock()
        .map_err(|_| "Catalogue state lock is poisoned".to_string())?;
    let (_, catalogue) = guard.as_ref().unwrap();

    let category_prefixes: Vec<String> = effective_category
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/").to_lowercase())
        .collect();

    // Accepted for parity with the source app's category config (see doc
    // comment above) but not read: path-prefix filtering already covers
    // every case this would otherwise filter.
    let _ = is_inventory_type;

    Ok(search_catalogue(catalogue, &query, &category_prefixes, class_filter.as_deref()))
}

/// How well a prototype matches a lowercase query, best first: the name starts with it, a word
/// in the name starts with it, the name contains it, the path contains it. `None` = no match.
fn match_rank(name: &str, path: &str, query: &str) -> Option<u8> {
    if query.is_empty() || name.starts_with(query) {
        Some(0)
    } else if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(query)) {
        Some(1)
    } else if name.contains(query) {
        Some(2)
    } else if path.contains(query) {
        Some(3)
    } else {
        None
    }
}

/// The search behind `search_prototypes`: non-abstract prototypes under one of `category_prefixes`
/// (lowercase; empty = any) and of `class_filter`, ranked by `match_rank`; within a rank, real
/// names come before names made from the file name, then catalogue order.
fn search_catalogue(
    catalogue: &PrototypeCatalogue,
    query: &str,
    category_prefixes: &[String],
    class_filter: Option<&str>,
) -> Vec<PrototypeMatch> {
    let query = query.trim().to_lowercase();
    let class_filter = class_filter.map(str::trim).filter(|c| !c.is_empty());
    let classes_of = |p: &PrototypeRecord| catalogue.classes.get(&p.blueprint_id).map(Vec::as_slice).unwrap_or(&[]);

    let mut hits: Vec<(u8, &PrototypeRecord)> = catalogue
        .prototypes
        .iter()
        .filter(|p| !p.is_abstract)
        .filter(|p| category_prefixes.is_empty() || category_prefixes.iter().any(|c| p.search_path.starts_with(c.as_str())))
        .filter(|p| class_filter.map_or(true, |c| classes_of(p).iter().any(|b| b == c)))
        .filter_map(|p| match_rank(&p.search_name, &p.search_path, &query).map(|rank| (rank, p)))
        .collect();
    hits.sort_by_key(|&(rank, p)| (rank, p.name_source == "path"));
    hits.truncate(if query.is_empty() { 500 } else { 100 });

    hits.into_iter()
        .map(|(_, p)| PrototypeMatch {
            path: p.path.clone(),
            blueprint: catalogue.blueprints.get(&p.blueprint_id).cloned().unwrap_or_default(),
            display_name: p.name.clone(),
            leaf: p.path.rsplit('/').next().unwrap_or(&p.path).to_string(),
            class: classes_of(p).first().cloned().unwrap_or_default(),
            name_source: p.name_source.to_string(),
            prototype_id: p.prototype_id.to_string(),
            prototype_guid: p.prototype_guid.to_string(),
        })
        .collect()
}

/// Resolve a prototype file path to its runtime ID (as a decimal string).
///
/// Given a path returned by
/// `search_prototypes`, it returns the `ItemPrototypeRuntimeIdForClient`
/// value needed to construct a `GuidItem` in a catalog entry.
///
/// Returns as a String to safely transport the full u64 range across the
/// JS boundary without precision loss.
#[tauri::command]
pub fn lookup_prototype_id(
    app: tauri::AppHandle,
    state: tauri::State<CatalogueState>,
    dn_state: tauri::State<DisplayNameState>,
    server_exe: String,
    prototype_path: String,
) -> Result<String, String> {
    let game_exe = crate::config::load_config(&app).game_exe;
    ensure_catalogue_loaded(&state, &dn_state, &server_exe, &game_exe)?;

    let guard = state
        .0
        .lock()
        .map_err(|_| "Catalogue state lock is poisoned".to_string())?;
    let (_, catalogue) = guard.as_ref().unwrap();

    id_for_path(catalogue, &prototype_path)
        .map(|id| id.to_string())
        .ok_or_else(|| format!("Prototype '{}' not found in catalogue", prototype_path))
}

#[cfg(test)]
#[path = "../../tests/rust/test_calligraphy.rs"]
mod tests;
