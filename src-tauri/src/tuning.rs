use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

// Raw serde shapes matching MHServerEmu JSON format. All three keys are required, and
// Prototype may not be null: the server looks a null Prototype up by name and the
// unhandled exception takes it down, at start-up and on `!server reloadlivetuning`.
#[derive(Debug, Deserialize)]
struct RawEntryIn {
    #[serde(rename = "Prototype")]
    prototype: String,
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
    Ok(crate::paths::server_dir(server_exe)?
        .join("Data")
        .join("Game")
        .join("LiveTuning"))
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
// `visited` holds canonical directory paths: a Windows junction pointing back at
// an ancestor would otherwise recurse forever and hang the scan.
fn collect_files_recursive(
    dir: &Path,
    base: &Path,
    out: &mut Vec<(std::path::PathBuf, String)>,
    visited: &mut HashSet<std::path::PathBuf>,
) -> Result<(), String> {
    let canonical = std::fs::canonicalize(dir)
        .map_err(|e| format!("Cannot resolve directory {}: {e}", dir.display()))?;
    if !visited.insert(canonical) {
        return Ok(());
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read directory {}: {e}", dir.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Directory read error: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_recursive(&path, base, out, visited)?;
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

/// `apply_auto_enable`: if true, an event-owned file found with an OFF_
/// prefix has that prefix stripped on disk (auto-enabling it) as part of
/// this scan; if false, the scan is read-only and such a file is reported
/// as disabled, reflecting its actual current state on disk.
#[tauri::command]
pub fn scan_tuning_files(
    server_exe: String,
    apply_auto_enable: bool,
) -> Result<Vec<TuningFileInfo>, String> {
    let dir = live_tuning_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    // Load FilePath -> event_id map from the active events definition.
    let events_map = load_events_file_map(&dir);

    // Collect all files under LiveTuning/ recursively.
    let mut all_files: Vec<(std::path::PathBuf, String)> = Vec::new();
    collect_files_recursive(&dir, &dir, &mut all_files, &mut HashSet::new())?;

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

        let (enabled, was_auto_enabled) = if has_off_prefix && event_id.is_some() && apply_auto_enable {
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

    // Toggleable (root, non-event) files first, then in the order the server loads them, so
    // the list reads top to bottom as "later files win".
    files.sort_by_cached_key(|f| (!f.toggleable, server_sort_key(&f.relative_path)));

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
            prototype: r.prototype,
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

    crate::store::write_with_backup(&path, &contents)
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

// ── Global settings across files (effective values) ──────────────────────────

/// One global (`eGTV_`, empty `Prototype`) setting as a file sets it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GlobalValue {
    pub setting: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalTuningFile {
    pub file: String,
    pub values: Vec<GlobalValue>,
    /// Set when the server would skip the whole file (unreadable or invalid JSON).
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalTuningEvent {
    pub id: String,
    pub display_name: String,
    pub values: Vec<GlobalValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalTuning {
    /// Base files in the order the server loads them; for a setting, the last file wins.
    pub files: Vec<GlobalTuningFile>,
    /// Every defined event that sets a global. While an event runs, its values replace the
    /// base value, whichever file that came from.
    pub events: Vec<GlobalTuningEvent>,
    /// `[GameOptions] InfinitySystemEnabled`: true means Infinity XP is live, false Omega XP.
    pub infinity_system: bool,
    /// `[GameData] EnableLiveTuningEvents`: false means no event overrides anything.
    pub live_events: bool,
}

fn ini_flag(server_exe: &str, section: &str, key: &str) -> bool {
    crate::ini::read_merged_value(server_exe, section, key, "true")
        .trim()
        .eq_ignore_ascii_case("true")
}

/// Sort key matching the server's `Array.Sort` of file names, which is culture-sensitive
/// (ICU) rather than ordinal: `_` < `-` < `.` < other punctuation < digits < letters, letters
/// ignoring case first. This is why `LiveTuningData_X.json` loads before
/// `LiveTuningDataGlobal.json`.
// ponytail: covers the characters that appear in tuning file names, not full ICU collation.
fn server_sort_key(name: &str) -> (Vec<(u8, u32)>, String) {
    let primary = name
        .chars()
        .map(|c| match c {
            '_' => (0, 0),
            '-' => (1, 0),
            '.' => (2, 0),
            c if c.is_ascii_digit() => (4, c as u32),
            c if c.is_alphabetic() => (5, c.to_lowercase().next().unwrap_or(c) as u32),
            c => (3, c as u32),
        })
        .collect();
    (primary, name.to_string())
}

pub(crate) fn sort_like_server(names: &mut [String]) {
    names.sort_by_cached_key(|n| server_sort_key(n));
}

fn globals_in(path: &Path) -> Result<Vec<GlobalValue>, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let raw: Vec<RawEntryIn> = serde_json::from_str(contents.trim_start_matches('\u{FEFF}'))
        .map_err(|e| e.to_string())?;
    Ok(raw
        .into_iter()
        .filter(|e| e.prototype.is_empty() && e.setting.starts_with("eGTV_"))
        .map(|e| GlobalValue { setting: e.setting, value: e.value })
        .collect())
}

/// What the server would load for global settings: the base files in its own order, and each
/// event's values.
#[tauri::command]
pub fn read_global_tuning(server_exe: String) -> Result<GlobalTuning, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let infinity_system = ini_flag(&server_exe, "GameOptions", "InfinitySystemEnabled");
    let live_events = ini_flag(&server_exe, "GameData", "EnableLiveTuningEvents");
    if !dir.exists() {
        return Ok(GlobalTuning { files: vec![], events: vec![], infinity_system, live_events });
    }

    // The server reads only top-level LiveTuningData*.json (FileHelper.GetFilesWithPrefix),
    // so OFF_ files and subfolders are not part of the base chain.
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot read {}: {e}", dir.display()))?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("LiveTuningData") && n.ends_with(".json"))
        .collect();
    sort_like_server(&mut names);

    let files = names
        .into_iter()
        .map(|file| match globals_in(&dir.join(&file)) {
            Ok(values) => GlobalTuningFile { file, values, error: None },
            Err(e) => GlobalTuningFile { file, values: vec![], error: Some(e) },
        })
        .collect();

    // An unreadable Events.json means the server runs no events, so none can override.
    let definitions = crate::events::load_events(server_exe)
        .map(|d| d.definitions)
        .unwrap_or_default();
    let mut events: Vec<GlobalTuningEvent> = definitions
        .into_iter()
        .filter_map(|d| {
            // The server skips an event whose file is missing; an OFF_ copy doesn't count.
            let values = globals_in(&dir.join(&d.file_path)).ok()?;
            (!values.is_empty()).then(|| GlobalTuningEvent { id: d.id, display_name: d.display_name, values })
        })
        .collect();
    events.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(GlobalTuning { files, events, infinity_system, live_events })
}

#[cfg(test)]
#[path = "../../tests/rust/test_tuning.rs"]
mod tests;
