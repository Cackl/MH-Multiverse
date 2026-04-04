use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
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
    Path::new(server_exe)
        .parent()
        .ok_or_else(|| "Cannot determine server directory".to_string())
        .map(|p| p.join("Data").join("Game").join("Patches"))
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

    serde_json::from_str(&text)
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

    let text = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Cannot serialise entries: {e}"))?;

    fs::write(&path, text)
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