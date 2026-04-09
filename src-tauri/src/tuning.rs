use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningFileInfo {
    /// Canonical filename — always without any prefix, e.g. "LiveTuningDataCosmicChaos.json"
    pub canonical_name: String,
    /// Whether the file is currently active (no OFF_ prefix on disk)
    pub enabled: bool,
    /// False for subdirectory files and event-owned files — toggle is disallowed
    pub toggleable: bool,
    /// Path relative to the LiveTuning/ directory root, e.g.
    ///   "LiveTuningDataCosmicChaos.json" for root files
    ///   "Events/Weekly/LiveTuningDataArmorIncursion.json" for subdirectory files
    pub relative_path: String,
    /// Which event owns this file, if any
    pub event_id: Option<String>,
    /// True if an OFF_ prefix was stripped from this file automatically during scan
    pub was_auto_enabled: bool,
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

pub(crate) fn live_tuning_dir(server_exe: &str) -> Result<std::path::PathBuf, String> {
    let exe_path = Path::new(server_exe);
    let server_dir = exe_path
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())?;
    Ok(server_dir.join("Data").join("Game").join("LiveTuning"))
}

fn is_tuning_file(name: &str) -> bool {
    name.contains("LiveTuningData") && name.ends_with(".json")
}

/// Constructs the inactive (OFF_-prefixed) path for a given relative path.
/// The OFF_ prefix is applied to the filename component only, not the directory.
fn inactive_path_for(dir: &Path, relative_path: &str) -> std::path::PathBuf {
    let rel = Path::new(relative_path);
    let filename = rel
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let off_filename = format!("OFF_{filename}");
    match rel.parent() {
        Some(parent) if parent != Path::new("") => dir.join(parent).join(off_filename),
        _ => dir.join(off_filename),
    }
}

// ── Scan helpers ──────────────────────────────────────────────────────────────

/// Loads Events.json (or EventsOverride.json if present) and returns a map of
/// FilePath -> event_id for use during file scanning.
fn load_events_file_map(live_tuning_dir: &Path) -> HashMap<String, String> {
    let override_path = live_tuning_dir.join("EventsOverride.json");
    let default_path = live_tuning_dir.join("Events.json");

    let path = if override_path.exists() {
        override_path
    } else if default_path.exists() {
        default_path
    } else {
        return HashMap::new();
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let raw: HashMap<String, serde_json::Value> = match serde_json::from_str(&contents) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    raw.into_iter()
        .filter_map(|(event_id, def)| {
            def.get("FilePath")
                .and_then(|v| v.as_str())
                .map(|fp| (fp.to_string(), event_id))
        })
        .collect()
}

/// Recursively collects all files under `dir`, returning (full_path, relative_path_from_base)
/// pairs. Separators in relative_path are normalised to '/'.
fn collect_files_recursive(
    dir: &Path,
    base: &Path,
    out: &mut Vec<(std::path::PathBuf, String)>,
) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read directory {}: {e}", dir.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Directory read error: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, base, out)?;
        } else {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("Path strip error: {e}"))?
                .to_string_lossy()
                .replace('\\', "/");
            out.push((path, relative));
        }
    }

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Files in the LiveTuning root that are managed by the event scheduler.
const RESERVED_FILENAMES: &[&str] = &[
    "Events.json",
    "EventsOverride.json",
    "EventSchedule.json",
    "EventScheduleOverride.json",
];

#[tauri::command]
pub fn scan_tuning_files(server_exe: String) -> Result<Vec<TuningFileInfo>, String> {
    let dir = live_tuning_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    // Load FilePath -> event_id map from the active events definition.
    let events_map = load_events_file_map(&dir);

    // Collect all files under LiveTuning/ recursively.
    let mut all_files: Vec<(std::path::PathBuf, String)> = Vec::new();
    collect_files_recursive(&dir, &dir, &mut all_files)?;

    let mut files: Vec<TuningFileInfo> = Vec::new();

    for (full_path, relative_path) in all_files {
        let filename = match Path::new(&relative_path).file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };

        // Skip reserved event-system files.
        if RESERVED_FILENAMES.contains(&filename.as_str()) {
            continue;
        }

        // Determine whether the file has an OFF_ prefix on disk.
        let (canonical_filename, has_off_prefix) = if filename.starts_with("OFF_") {
            (filename[4..].to_string(), true)
        } else {
            (filename.clone(), false)
        };

        // Only process files whose canonical name matches the tuning file pattern.
        if !is_tuning_file(&canonical_filename) {
            continue;
        }

        // Build canonical relative_path (OFF_ stripped from the filename component).
        let canonical_relative_path = if has_off_prefix {
            let parent = Path::new(&relative_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if parent.is_empty() {
                canonical_filename.clone()
            } else {
                format!("{parent}/{canonical_filename}")
            }
        } else {
            relative_path.clone()
        };

        // Deduplicate by canonical_relative_path.
        if files.iter().any(|f| f.relative_path == canonical_relative_path) {
            continue;
        }

        let in_subdirectory = canonical_relative_path.contains('/');
        let event_id = events_map.get(&canonical_relative_path).cloned();

        // Toggleable only for root-level files that are not event-owned.
        let toggleable = !in_subdirectory && event_id.is_none();

        let (enabled, was_auto_enabled) = if has_off_prefix && event_id.is_some() {
            // Event-owned file has an OFF_ prefix: strip it automatically.
            let canonical_full_path = full_path
                .parent()
                .ok_or_else(|| format!("Cannot determine parent for {}", full_path.display()))?
                .join(&canonical_filename);
            std::fs::rename(&full_path, &canonical_full_path)
                .map_err(|e| format!("Cannot auto-enable {canonical_relative_path}: {e}"))?;
            (true, true)
        } else {
            (!has_off_prefix, false)
        };

        files.push(TuningFileInfo {
            canonical_name: canonical_filename,
            enabled,
            toggleable,
            relative_path: canonical_relative_path,
            event_id,
            was_auto_enabled,
        });
    }

    // Toggleable (root, non-event) files first, then alphabetical by relative_path.
    files.sort_by(|a, b| {
        b.toggleable
            .cmp(&a.toggleable)
            .then(a.relative_path.cmp(&b.relative_path))
    });

    Ok(files)
}

#[tauri::command]
pub fn read_tuning_file(
    server_exe: String,
    relative_path: String,
) -> Result<Vec<TuningEntry>, String> {
    let dir = live_tuning_dir(&server_exe)?;

    let active_path = dir.join(&relative_path);
    let inactive_path = inactive_path_for(&dir, &relative_path);

    let path = if active_path.exists() {
        active_path
    } else if inactive_path.exists() {
        inactive_path
    } else {
        return Err(format!("File not found: {relative_path}"));
    };

    let mut contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {relative_path}: {e}"))?;

    // Strip UTF-8 BOM if present.
    if contents.starts_with('\u{FEFF}') {
        contents = contents.trim_start_matches('\u{FEFF}').to_string();
    }

    let raw: Vec<RawEntryIn> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {relative_path}: {e}"))?;

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
    relative_path: String,
    entries: Vec<TuningEntry>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;

    // Preserve current active/inactive state — write to whichever path exists.
    let active_path = dir.join(&relative_path);
    let inactive_path = inactive_path_for(&dir, &relative_path);

    let path = if active_path.exists() {
        active_path
    } else if inactive_path.exists() {
        inactive_path
    } else {
        return Err(format!("File not found: {relative_path}"));
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
pub fn get_live_tuning_dir(server_exe: String) -> Result<String, String> {
    let dir = live_tuning_dir(&server_exe)?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create LiveTuning directory: {e}"))?;
    }
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn create_tuning_file(
    server_exe: String,
    relative_path: String,
    entries: Vec<TuningEntry>,
) -> Result<(), String> {
    // create_tuning_file only supports root-level files.
    if relative_path.contains('/') || relative_path.contains('\\') {
        return Err(format!(
            "Invalid path '{relative_path}': create_tuning_file only supports root-level files"
        ));
    }

    if !relative_path.starts_with("LiveTuningData") || !relative_path.ends_with(".json") {
        return Err(format!(
            "Invalid filename '{relative_path}': must start with 'LiveTuningData' and end with '.json'"
        ));
    }

    let dir = live_tuning_dir(&server_exe)?;

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create LiveTuning directory: {e}"))?;

    let active_path = dir.join(&relative_path);
    let inactive_path = dir.join(format!("OFF_{relative_path}"));

    if active_path.exists() || inactive_path.exists() {
        return Err(format!("File already exists: {relative_path}"));
    }

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

    std::fs::write(&active_path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", active_path.display()))?;

    Ok(())
}

#[tauri::command]
pub fn toggle_tuning_file(
    server_exe: String,
    relative_path: String,
    enabled: bool,
) -> Result<(), String> {
    // Guard: subdirectory files cannot be toggled.
    if relative_path.contains('/') || relative_path.contains('\\') {
        return Err(format!(
            "Cannot toggle subdirectory file: {relative_path}"
        ));
    }

    // Guard: event-owned files cannot be toggled.
    let dir = live_tuning_dir(&server_exe)?;
    let events_map = load_events_file_map(&dir);
    if events_map.contains_key(&relative_path) {
        return Err(format!(
            "Cannot toggle event-owned file: {relative_path}"
        ));
    }

    let active_path = dir.join(&relative_path);
    let inactive_path = dir.join(format!("OFF_{relative_path}"));

    if enabled {
        if inactive_path.exists() {
            std::fs::rename(&inactive_path, &active_path)
                .map_err(|e| format!("Cannot enable {relative_path}: {e}"))?;
        } else if !active_path.exists() {
            return Err(format!("Cannot enable {relative_path}: file not found"));
        }
        // Already active — no-op.
    } else {
        if active_path.exists() {
            std::fs::rename(&active_path, &inactive_path)
                .map_err(|e| format!("Cannot disable {relative_path}: {e}"))?;
        } else if !inactive_path.exists() {
            return Err(format!("Cannot disable {relative_path}: file not found"));
        }
        // Already inactive — no-op.
    }

    Ok(())
}