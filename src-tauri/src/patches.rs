use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Number, Value as JsonValue};
use std::fs;
use std::path::{Path, PathBuf};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PatchEntry {
    pub enabled: bool,
    pub prototype: String,
    pub path: String,
    #[serde(default)]
    pub description: String,
    pub value_type: String,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFileInfo {
    /// Bare filename, e.g. "PatchDataBugFixes.json"
    pub file_name: String,
    /// true  → lives in Patches/
    /// false → lives in Patches/Off/
    pub enabled: bool,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn patches_dir(server_exe: &str) -> Result<PathBuf, String> {
    Ok(crate::paths::server_dir(server_exe)?
        .join("Data")
        .join("Game")
        .join("Patches"))
}

fn off_dir(patches: &Path) -> PathBuf {
    patches.join("Off")
}

fn file_path(patches: &Path, file_name: &str, enabled: bool) -> PathBuf {
    if enabled {
        patches.join(file_name)
    } else {
        off_dir(patches).join(file_name)
    }
}

// ── 64-bit ID handling ────────────────────────────────────────────────────────
//
// Prototype IDs/GUIDs are u64 and routinely exceed 2^53, which the webview's
// JSON round-trip (Tauri IPC) silently rounds. They cross to the frontend as
// strings and are turned back into bare integers (what the server's patch
// loader expects) when written to disk.

const ID_TYPES: &[&str] = &[
    "PrototypeId", "PrototypeDataRef", "PrototypeGuid", "LocaleStringId",
    "PrototypeId[]", "PrototypeDataRef[]",
];

fn convert_ids(entry: &mut PatchEntry, to_strings: bool) {
    if !ID_TYPES.contains(&entry.value_type.as_str()) {
        return;
    }
    fn convert(v: &mut JsonValue, to_strings: bool) {
        match v {
            JsonValue::Array(items) => items.iter_mut().for_each(|i| convert(i, to_strings)),
            JsonValue::Number(n) if to_strings => *v = JsonValue::String(n.to_string()),
            JsonValue::String(s) if !to_strings => {
                // Non-numeric text stays text, and value_error refuses it.
                if let Ok(n) = s.trim().parse::<u64>() {
                    *v = JsonValue::Number(Number::from(n));
                }
            }
            _ => {}
        }
    }
    convert(&mut entry.value, to_strings);
}

// Struct values (`Prototype`, `Prototype[]`) and `Properties` nest 64-bit ids
// at any depth, and `Properties` repeats keys (one `CooldownModifierPctForPower`
// per power). `JsonValue` keeps only the last of a repeated key and the webview
// rounds the ids, so these values cross to the frontend as their original JSON
// text and are written back verbatim.
const RAW_TYPES: &[&str] = &["Prototype", "Prototype[]", "Properties"];

/// On-disk shape: the value stays as unparsed JSON text.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct DiskEntry {
    enabled: bool,
    prototype: String,
    path: String,
    #[serde(default)]
    description: String,
    value_type: String,
    value: Box<RawValue>,
}

fn from_disk(d: DiskEntry) -> Result<PatchEntry, String> {
    let value = if RAW_TYPES.contains(&d.value_type.as_str()) {
        JsonValue::String(d.value.get().to_string())
    } else {
        serde_json::from_str(d.value.get()).map_err(|e| format!("{}: {e}", d.path))?
    };
    let mut entry = PatchEntry {
        enabled: d.enabled,
        prototype: d.prototype,
        path: d.path,
        description: d.description,
        value_type: d.value_type,
        value,
    };
    convert_ids(&mut entry, true);
    Ok(entry)
}

fn to_disk(mut entry: PatchEntry) -> Result<DiskEntry, String> {
    convert_ids(&mut entry, false);
    let value = match &entry.value {
        JsonValue::String(text) if RAW_TYPES.contains(&entry.value_type.as_str()) => {
            RawValue::from_string(text.trim().to_string())
                .map_err(|e| format!("{}: Value is not valid JSON: {e}", entry.path))?
        }
        v => serde_json::value::to_raw_value(v).map_err(|e| format!("{}: {e}", entry.path))?,
    };
    Ok(DiskEntry {
        enabled: entry.enabled,
        prototype: entry.prototype,
        path: entry.path,
        description: entry.description,
        value_type: entry.value_type,
        value,
    })
}

// ── Values the server can't read ──────────────────────────────────────────────
//
// The server reads Value before Enabled and drops the whole file when Value doesn't
// parse, even for a disabled entry (PatchEntryConverter.GetValueBase in MHServerEmu).
// Only those cases are refused here. Values that load but do nothing are left alone.

const FLOAT_MAX: f64 = 3.4028235e38;

fn is_float(v: &JsonValue) -> bool {
    v.as_f64().is_some_and(|f| f.abs() <= FLOAT_MAX)
}

fn is_proto(v: &JsonValue) -> bool {
    v.get("ParentDataRef").is_some_and(|r| r.as_u64().is_some())
}

/// Every PropertyEnum name, for Properties keys (the server's Enum.Parse throws on others).
fn is_property(key: &str) -> bool {
    // Enum.Parse also takes numbers and comma-joined names, and trims spaces.
    key.split(',').map(str::trim).all(|k| {
        k.parse::<i64>().is_ok()
            || include_str!("property_names.txt").lines().any(|n| n == k)
    })
}

/// Why the server would fail to read this Value, if it would.
fn value_error(value_type: &str, v: &JsonValue) -> Option<String> {
    let ok = match value_type {
        "String" | "Enum" => v.is_string() || v.is_null(),
        "Boolean" => v.is_boolean(),
        "Float" => is_float(v),
        "Integer" => v.as_i64().is_some_and(|n| i32::try_from(n).is_ok()),
        "PrototypeGuid" | "PrototypeId" | "PrototypeDataRef" | "LocaleStringId" => v.as_u64().is_some(),
        "PrototypeId[]" | "PrototypeDataRef[]" => {
            v.as_array().is_some_and(|a| a.iter().all(|x| x.as_u64().is_some()))
        }
        "Vector3" => v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(is_float)),
        "Prototype" => is_proto(v),
        "Prototype[]" => v.as_array().is_some_and(|a| a.iter().all(is_proto)),
        "Properties" => {
            let Some(obj) = v.as_object() else { return Some("Properties needs an object.".into()) };
            return obj.keys().find(|k| !is_property(k)).map(|k| format!("Unknown property {k}."));
        }
        // ponytail: unknown ValueTypes pass (test fixtures use made-up ones). Refuse them too if users hit it.
        _ => return None,
    };
    if ok {
        return None;
    }
    Some(match value_type {
        "String" | "Enum" => format!("{value_type} needs text in quotes."),
        "Boolean" => "Boolean needs true or false.".into(),
        "Float" => "Float needs a number no bigger than 3.4e38.".into(),
        "Integer" => "Integer needs a whole number from -2147483648 to 2147483647.".into(),
        "PrototypeId[]" | "PrototypeDataRef[]" => format!("{value_type} needs a list of numeric ids."),
        "Vector3" => "Vector3 needs a list of 3 numbers.".into(),
        "Prototype" => "Prototype needs an object with a numeric ParentDataRef.".into(),
        "Prototype[]" => "Prototype[] needs a list of objects, each with a numeric ParentDataRef.".into(),
        _ => format!("{value_type} needs a numeric id."),
    })
}

/// The entries as written to disk, or why the server would drop the file.
fn to_disk_all(entries: Vec<PatchEntry>) -> Result<Vec<DiskEntry>, String> {
    entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            let label = if entry.description.trim().is_empty() { entry.prototype.clone() } else { entry.description.clone() };
            let disk = to_disk(entry)?;
            let value: JsonValue = serde_json::from_str(disk.value.get()).map_err(|e| e.to_string())?;
            match value_error(&disk.value_type, &value) {
                Some(why) => Err(format!("Entry {} ({label}): {why}", i + 1)),
                None => Ok(disk),
            }
        })
        .collect()
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Scan the Patches directory and return all PatchData*.json files.
/// Files in Patches/ are enabled; files in Patches/Off/ are disabled.
#[tauri::command]
pub fn scan_patch_files(server_exe: String) -> Result<Vec<PatchFileInfo>, String> {
    let patches = patches_dir(&server_exe)?;

    if !patches.exists() {
        return Ok(vec![]);
    }

    let mut files: Vec<PatchFileInfo> = Vec::new();

    collect_patch_files(&patches, true, &mut files)?;

    let off = off_dir(&patches);
    if off.exists() {
        collect_patch_files(&off, false, &mut files)?;
    }

    // The server's load order (culture sort, see tuning.rs). Off/ files sit among them by name.
    files.sort_by_cached_key(|f| crate::tuning::server_sort_key(&f.file_name));
    Ok(files)
}

fn collect_patch_files(
    dir: &Path,
    enabled: bool,
    out: &mut Vec<PatchFileInfo>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Cannot read directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("PatchData") && name_str.ends_with(".json") {
            out.push(PatchFileInfo {
                file_name: name_str.to_string(),
                enabled,
            });
        }
    }

    Ok(())
}

/// Load and parse the entries from a patch file.
#[tauri::command]
pub fn load_patch_file(
    server_exe: String,
    file_name: String,
    enabled: bool,
) -> Result<Vec<PatchEntry>, String> {
    let patches = patches_dir(&server_exe)?;
    let path = file_path(&patches, &file_name, enabled);

    let text = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {file_name}: {e}"))?;

    // Some stock files (e.g. Off/PatchDataCowpocalypse.json) start with a UTF-8 BOM.
    let disk: Vec<DiskEntry> = serde_json::from_str(text.trim_start_matches('\u{FEFF}'))
        .map_err(|e| format!("Cannot parse {file_name}: {e}"))?;
    disk.into_iter()
        .map(from_disk)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Cannot parse {file_name}: {e}"))
}

/// Write entries back to a patch file, pretty-printed.
#[tauri::command]
pub fn save_patch_file(
    server_exe: String,
    file_name: String,
    enabled: bool,
    entries: Vec<PatchEntry>,
) -> Result<(), String> {
    let patches = patches_dir(&server_exe)?;
    let path = file_path(&patches, &file_name, enabled);

    let disk = to_disk_all(entries).map_err(|e| format!("Cannot save {file_name}: {e}"))?;

    let text = serde_json::to_string_pretty(&disk)
        .map_err(|e| format!("Cannot serialise entries: {e}"))?;

    crate::store::write_with_backup(&path, &text)
        .map_err(|e| format!("Cannot write {file_name}: {e}"))
}

/// Create `PatchData{suffix}.json`, empty and switched on, and return its name. The suffix is
/// letters, numbers, `_` or `-`, and the name must not exist yet in Patches/ or Patches/Off/.
#[tauri::command]
pub fn create_patch_file(server_exe: String, suffix: String) -> Result<String, String> {
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Use letters, numbers, _ or - for the name".into());
    }
    let file_name = format!("PatchData{suffix}.json");
    let patches = patches_dir(&server_exe)?;
    fs::create_dir_all(&patches)
        .map_err(|e| format!("Cannot create Patches directory: {e}"))?;

    if file_path(&patches, &file_name, true).exists()
        || file_path(&patches, &file_name, false).exists()
    {
        return Err(format!("{file_name} already exists"));
    }

    fs::write(patches.join(&file_name), "[]")
        .map_err(|e| format!("Cannot create {file_name}: {e}"))?;
    Ok(file_name)
}

/// The result of a Patches switch.
#[derive(Debug, Serialize)]
pub struct PatchSwitch {
    pub enabled: bool,
    /// Where the replaced copy went, relative to Patches/, when both copies existed.
    pub kept_as: Option<String>,
}

/// Move a patch file between Patches/ and Patches/Off/ to toggle its enabled state.
#[tauri::command]
pub fn set_patch_file_enabled(
    server_exe: String,
    file_name: String,
    currently_enabled: bool,
) -> Result<PatchSwitch, String> {
    let patches = patches_dir(&server_exe)?;
    let off = off_dir(&patches);

    let src = file_path(&patches, &file_name, currently_enabled);
    let new_enabled = !currently_enabled;
    let dst_dir = if new_enabled { patches.clone() } else { off.clone() };
    let dst = dst_dir.join(&file_name);

    // Both copies exist when an older server update added one back. The copy being switched
    // wins, and the one it replaces is moved aside (fs::rename would delete it).
    let kept_as = if src.exists() && dst.exists() {
        let bak = crate::store::move_aside(&dst)?;
        Some(bak.strip_prefix(&patches).unwrap_or(&bak).to_string_lossy().replace('\\', "/"))
    } else {
        None
    };

    fs::create_dir_all(&dst_dir)
        .map_err(|e| format!("Cannot create target directory: {e}"))?;

    fs::rename(&src, &dst)
        .map_err(|e| format!("Cannot move {file_name}: {e}"))?;

    Ok(PatchSwitch { enabled: new_enabled, kept_as })
}

/// Return the absolute path to the Patches directory (for open-in-explorer).
#[tauri::command]
pub fn get_patches_dir(server_exe: String) -> Result<String, String> {
    patches_dir(&server_exe).map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
#[path = "../../tests/rust/test_patches.rs"]
mod tests;
