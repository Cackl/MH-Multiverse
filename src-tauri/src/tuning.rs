use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningFileInfo {
    /// Canonical filename — always without any prefix, e.g. "LiveTuningData_CosmicChaos.json"
    pub canonical_name: String,
    /// Whether the file is currently active (no OFF_ prefix on disk)
    pub enabled: bool,
    /// False when the file has an unrecognised prefix — toggle is disallowed
    pub toggleable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningEntry {
    pub prototype: String,
    pub setting: String,
    pub value: f64,
}

// Raw serde shapes matching MHServerEmu JSON format
#[derive(Debug, Deserialize)]
struct RawEntryIn {
    #[serde(rename = "Prototype", default)]
    prototype: Option<String>,
    #[serde(rename = "Setting")]
    setting: String,
    #[serde(rename = "Value")]
    value: f64,
}

#[derive(Debug, Serialize)]
struct RawEntryOut {
    #[serde(rename = "Prototype")]
    prototype: String,
    #[serde(rename = "Setting")]
    setting: String,
    #[serde(rename = "Value")]
    value: f64,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn live_tuning_dir(server_exe: &str) -> Result<std::path::PathBuf, String> {
    let exe_path = Path::new(server_exe);
    let server_dir = exe_path
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())?;
    Ok(server_dir.join("Data").join("Game").join("LiveTuning"))
}

fn is_tuning_file(name: &str) -> bool {
    name.contains("LiveTuningData") && name.ends_with(".json")
}

/// Returns the canonical name (no prefix) and toggleability for a discovered filename.
fn classify_filename(name: &str) -> (String, bool, bool) {
    if name.starts_with("OFF_LiveTuningData") {
        // Recognised inactive state
        (name[4..].to_string(), false, true)
    } else if name.starts_with("LiveTuningData") {
        // Active, no prefix
        (name.to_string(), true, true)
    } else {
        // Unknown prefix — show but disallow toggle
        (name.to_string(), true, false)
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn scan_tuning_files(server_exe: String) -> Result<Vec<TuningFileInfo>, String> {
    let dir = live_tuning_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let read_dir = std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot read LiveTuning directory: {e}"))?;

    let mut files: Vec<TuningFileInfo> = Vec::new();

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Directory read error: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();

        if !is_tuning_file(&name) {
            continue;
        }

        let (canonical_name, enabled, toggleable) = classify_filename(&name);

        // Deduplicate: if both LiveTuningData_X.json and OFF_LiveTuningData_X.json somehow
        // exist, the active version wins. In practice this shouldn't happen.
        if files.iter().any(|f| f.canonical_name == canonical_name) {
            continue;
        }

        files.push(TuningFileInfo { canonical_name, enabled, toggleable });
    }

    // Sort: toggleable (known) before unknown prefix, then alphabetical within each group
    files.sort_by(|a, b| {
        b.toggleable.cmp(&a.toggleable).then(a.canonical_name.cmp(&b.canonical_name))
    });

    Ok(files)
}

#[tauri::command]
pub fn read_tuning_file(
    server_exe: String,
    canonical_name: String,
) -> Result<Vec<TuningEntry>, String> {
    let dir = live_tuning_dir(&server_exe)?;

    let active_path = dir.join(&canonical_name);
    let inactive_path = dir.join(format!("OFF_{}", canonical_name));

    let path = if active_path.exists() {
        active_path
    } else if inactive_path.exists() {
        inactive_path
    } else {
        return Err(format!("File not found: {canonical_name}"));
    };

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {canonical_name}: {e}"))?;

    let raw: Vec<RawEntryIn> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {canonical_name}: {e}"))?;

    Ok(raw
        .into_iter()
        .map(|r| TuningEntry {
            prototype: r.prototype.unwrap_or_default(),
            setting: r.setting,
            value: r.value,
        })
        .collect())
}

#[tauri::command]
pub fn write_tuning_file(
    server_exe: String,
    canonical_name: String,
    entries: Vec<TuningEntry>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;

    // Preserve current active/inactive state — write to whichever path exists
    let active_path = dir.join(&canonical_name);
    let inactive_path = dir.join(format!("OFF_{}", canonical_name));

    let path = if active_path.exists() {
        active_path
    } else if inactive_path.exists() {
        inactive_path
    } else {
        return Err(format!("File not found: {canonical_name}"));
    };

    let raw: Vec<RawEntryOut> = entries
        .into_iter()
        .map(|e| RawEntryOut {
            prototype: e.prototype,
            setting: e.setting,
            value: e.value,
        })
        .collect();

    let contents = serde_json::to_string_pretty(&raw)
        .map_err(|e| format!("JSON serialisation error: {e}"))?;

    std::fs::write(&path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;

    Ok(())
}

#[tauri::command]
pub fn toggle_tuning_file(
    server_exe: String,
    canonical_name: String,
    enabled: bool,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;

    let active_path = dir.join(&canonical_name);
    let inactive_path = dir.join(format!("OFF_{}", canonical_name));

    if enabled {
        if inactive_path.exists() {
            std::fs::rename(&inactive_path, &active_path)
                .map_err(|e| format!("Cannot enable {canonical_name}: {e}"))?;
        } else if !active_path.exists() {
            return Err(format!("Cannot enable {canonical_name}: file not found"));
        }
        // Already active — no-op
    } else {
        if active_path.exists() {
            std::fs::rename(&active_path, &inactive_path)
                .map_err(|e| format!("Cannot disable {canonical_name}: {e}"))?;
        } else if !inactive_path.exists() {
            return Err(format!("Cannot disable {canonical_name}: file not found"));
        }
        // Already inactive — no-op
    }

    Ok(())
}