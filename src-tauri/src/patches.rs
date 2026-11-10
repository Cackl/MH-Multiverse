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
                // Non-numeric text is left as-is so the server reports it, not us.
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

    files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
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

    let disk: Vec<DiskEntry> = entries
        .into_iter()
        .map(to_disk)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Cannot save {file_name}: {e}"))?;

    let text = serde_json::to_string_pretty(&disk)
        .map_err(|e| format!("Cannot serialise entries: {e}"))?;

    crate::store::write_with_backup(&path, &text)
        .map_err(|e| format!("Cannot write {file_name}: {e}"))
}

/// Create a new empty patch file in Patches/ (enabled by default).
/// Returns an error if the name already exists in either location.
#[tauri::command]
pub fn create_patch_file(server_exe: String, file_name: String) -> Result<(), String> {
    let patches = patches_dir(&server_exe)?;
    fs::create_dir_all(&patches)
        .map_err(|e| format!("Cannot create Patches directory: {e}"))?;

    if file_path(&patches, &file_name, true).exists()
        || file_path(&patches, &file_name, false).exists()
    {
        return Err(format!("{file_name} already exists"));
    }

    fs::write(patches.join(&file_name), "[]")
        .map_err(|e| format!("Cannot create {file_name}: {e}"))
}

/// Move a patch file between Patches/ and Patches/Off/ to toggle its enabled state.
/// Returns the new enabled state.
#[tauri::command]
pub fn set_patch_file_enabled(
    server_exe: String,
    file_name: String,
    currently_enabled: bool,
) -> Result<bool, String> {
    let patches = patches_dir(&server_exe)?;
    let off = off_dir(&patches);

    let src = file_path(&patches, &file_name, currently_enabled);
    let new_enabled = !currently_enabled;
    let dst_dir = if new_enabled { patches.clone() } else { off.clone() };

    fs::create_dir_all(&dst_dir)
        .map_err(|e| format!("Cannot create target directory: {e}"))?;

    fs::rename(&src, dst_dir.join(&file_name))
        .map_err(|e| format!("Cannot move {file_name}: {e}"))?;

    Ok(new_enabled)
}

/// Return the absolute path to the Patches directory (for open-in-explorer).
#[tauri::command]
pub fn get_patches_dir(server_exe: String) -> Result<String, String> {
    patches_dir(&server_exe).map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
#[path = "../../tests/rust/test_patches.rs"]
mod tests;
