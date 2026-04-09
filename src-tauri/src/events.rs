use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::tuning::live_tuning_dir;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDefinition {
    /// Map key from Events.json
    pub id: String,
    pub display_name: String,
    /// Path relative to the LiveTuning/ directory root
    pub file_path: String,
    pub daily_gift: Option<String>,
    pub instanced_missions: Option<Vec<String>>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub name: String,
    pub is_enabled: bool,
    /// "AlwaysOn" | "WeeklyRotation" | "DayOfWeek" | "SpecialDate" | "SpecialDateLunar"
    pub rule_type: String,
    /// For WeeklyRotation and DayOfWeek
    pub start_day_of_week: Option<String>,
    /// For SpecialDate and SpecialDateLunar
    pub start_month: Option<u32>,
    pub start_day: Option<u32>,
    pub duration_days: Option<u32>,
    pub events: Vec<String>,
}

/// Return type for load_events — carries which file was read.
#[derive(Debug, Serialize)]
pub struct EventsData {
    pub definitions: Vec<EventDefinition>,
    pub using_override: bool,
}

/// Return type for load_event_schedule — carries which file was read.
#[derive(Debug, Serialize)]
pub struct ScheduleData {
    pub rules: Vec<ScheduleRule>,
    pub using_override: bool,
}

// ── Raw serde types ───────────────────────────────────────────────────────────

/// Matches one value in the Events.json object (the key is the event ID).
#[derive(Debug, Deserialize)]
struct RawEventDefIn {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "FilePath")]
    file_path: String,
    #[serde(rename = "DailyGift")]
    daily_gift: Option<String>,
    #[serde(rename = "InstancedMissions")]
    instanced_missions: Option<Vec<String>>,
    #[serde(rename = "IsHidden")]
    is_hidden: Option<bool>,
}

/// Written back to Events.json — optional fields are omitted when None.
#[derive(Debug, Serialize)]
struct RawEventDefOut {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "FilePath")]
    file_path: String,
    #[serde(rename = "DailyGift", skip_serializing_if = "Option::is_none")]
    daily_gift: Option<String>,
    #[serde(rename = "InstancedMissions", skip_serializing_if = "Option::is_none")]
    instanced_missions: Option<Vec<String>>,
    #[serde(rename = "IsHidden", skip_serializing_if = "Option::is_none")]
    is_hidden: Option<bool>,
}

/// Matches one entry in the EventSchedule.json array.
#[derive(Debug, Deserialize, Serialize)]
struct RawScheduleRule {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "IsEnabled")]
    is_enabled: bool,
    #[serde(rename = "Type")]
    rule_type: String,
    #[serde(rename = "StartDayOfWeek", skip_serializing_if = "Option::is_none")]
    start_day_of_week: Option<String>,
    #[serde(rename = "StartMonth", skip_serializing_if = "Option::is_none")]
    start_month: Option<u32>,
    #[serde(rename = "StartDay", skip_serializing_if = "Option::is_none")]
    start_day: Option<u32>,
    #[serde(rename = "DurationDays", skip_serializing_if = "Option::is_none")]
    duration_days: Option<u32>,
    #[serde(rename = "Events")]
    events: Vec<String>,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn events_file_paths(
    dir: &Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    (dir.join("EventsOverride.json"), dir.join("Events.json"))
}

fn schedule_file_paths(
    dir: &Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        dir.join("EventScheduleOverride.json"),
        dir.join("EventSchedule.json"),
    )
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

fn read_events_from_path(path: &Path) -> Result<Vec<EventDefinition>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    let raw: HashMap<String, RawEventDefIn> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))?;

    Ok(raw
        .into_iter()
        .map(|(id, def)| EventDefinition {
            id,
            display_name: def.display_name,
            file_path: def.file_path,
            daily_gift: def.daily_gift,
            instanced_missions: def.instanced_missions,
            is_hidden: def.is_hidden,
        })
        .collect())
}

fn read_schedule_from_path(path: &Path) -> Result<Vec<ScheduleRule>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;

    let raw: Vec<RawScheduleRule> = serde_json::from_str(&contents)
        .map_err(|e| format!("JSON parse error in {}: {e}", path.display()))?;

    Ok(raw
        .into_iter()
        .map(|r| ScheduleRule {
            name: r.name,
            is_enabled: r.is_enabled,
            rule_type: r.rule_type,
            start_day_of_week: r.start_day_of_week,
            start_month: r.start_month,
            start_day: r.start_day,
            duration_days: r.duration_days,
            events: r.events,
        })
        .collect())
}

fn write_events_to_path(path: &Path, definitions: &[EventDefinition]) -> Result<(), String> {
    // Events.json is an object keyed by event ID. HashMap does not guarantee
    // key order; the override file is valid regardless of ordering.
    let map: HashMap<&str, RawEventDefOut> = definitions
        .iter()
        .map(|def| {
            (
                def.id.as_str(),
                RawEventDefOut {
                    display_name: def.display_name.clone(),
                    file_path: def.file_path.clone(),
                    daily_gift: def.daily_gift.clone(),
                    instanced_missions: def.instanced_missions.clone(),
                    is_hidden: def.is_hidden,
                },
            )
        })
        .collect();

    let contents = serde_json::to_string_pretty(&map)
        .map_err(|e| format!("JSON serialisation error: {e}"))?;

    std::fs::write(path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;

    Ok(())
}

fn write_schedule_to_path(path: &Path, rules: &[ScheduleRule]) -> Result<(), String> {
    let raw: Vec<RawScheduleRule> = rules
        .iter()
        .map(|r| RawScheduleRule {
            name: r.name.clone(),
            is_enabled: r.is_enabled,
            rule_type: r.rule_type.clone(),
            start_day_of_week: r.start_day_of_week.clone(),
            start_month: r.start_month,
            start_day: r.start_day,
            duration_days: r.duration_days,
            events: r.events.clone(),
        })
        .collect();

    let contents = serde_json::to_string_pretty(&raw)
        .map_err(|e| format!("JSON serialisation error: {e}"))?;

    std::fs::write(path, contents)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn load_events(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if override_path.exists() {
        let definitions = read_events_from_path(&override_path)?;
        Ok(EventsData {
            definitions,
            using_override: true,
        })
    } else if default_path.exists() {
        let definitions = read_events_from_path(&default_path)?;
        Ok(EventsData {
            definitions,
            using_override: false,
        })
    } else {
        Ok(EventsData {
            definitions: vec![],
            using_override: false,
        })
    }
}

#[tauri::command]
pub fn load_event_schedule(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if override_path.exists() {
        let rules = read_schedule_from_path(&override_path)?;
        Ok(ScheduleData {
            rules,
            using_override: true,
        })
    } else if default_path.exists() {
        let rules = read_schedule_from_path(&default_path)?;
        Ok(ScheduleData {
            rules,
            using_override: false,
        })
    } else {
        Ok(ScheduleData {
            rules: vec![],
            using_override: false,
        })
    }
}

#[tauri::command]
pub fn save_events_override(
    server_exe: String,
    definitions: Vec<EventDefinition>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;
    write_events_to_path(&dir.join("EventsOverride.json"), &definitions)
}

#[tauri::command]
pub fn save_schedule_override(
    server_exe: String,
    rules: Vec<ScheduleRule>,
) -> Result<(), String> {
    let dir = live_tuning_dir(&server_exe)?;
    write_schedule_to_path(&dir.join("EventScheduleOverride.json"), &rules)
}

#[tauri::command]
pub fn reset_events_override(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if !default_path.exists() {
        return Err("Events.json not found — cannot reset override".to_string());
    }

    std::fs::copy(&default_path, &override_path)
        .map_err(|e| format!("Cannot copy Events.json to override: {e}"))?;

    let definitions = read_events_from_path(&override_path)?;
    Ok(EventsData {
        definitions,
        using_override: true,
    })
}

#[tauri::command]
pub fn reset_schedule_override(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if !default_path.exists() {
        return Err("EventSchedule.json not found — cannot reset override".to_string());
    }

    std::fs::copy(&default_path, &override_path)
        .map_err(|e| format!("Cannot copy EventSchedule.json to override: {e}"))?;

    let rules = read_schedule_from_path(&override_path)?;
    Ok(ScheduleData {
        rules,
        using_override: true,
    })
}

#[tauri::command]
pub fn merge_events_override(server_exe: String) -> Result<EventsData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = events_file_paths(&dir);

    if !default_path.exists() {
        return Err("Events.json not found — cannot merge".to_string());
    }

    let defaults = read_events_from_path(&default_path)?;

    let mut current = if override_path.exists() {
        read_events_from_path(&override_path)?
    } else {
        vec![]
    };

    let existing_ids: HashSet<String> = current.iter().map(|d| d.id.clone()).collect();

    for def in defaults {
        if !existing_ids.contains(&def.id) {
            current.push(def);
        }
    }

    write_events_to_path(&override_path, &current)?;
    Ok(EventsData {
        definitions: current,
        using_override: true,
    })
}

#[tauri::command]
pub fn merge_schedule_override(server_exe: String) -> Result<ScheduleData, String> {
    let dir = live_tuning_dir(&server_exe)?;
    let (override_path, default_path) = schedule_file_paths(&dir);

    if !default_path.exists() {
        return Err("EventSchedule.json not found — cannot merge".to_string());
    }

    let defaults = read_schedule_from_path(&default_path)?;

    let mut current = if override_path.exists() {
        read_schedule_from_path(&override_path)?
    } else {
        vec![]
    };

    let existing_names: HashSet<String> = current.iter().map(|r| r.name.clone()).collect();

    for rule in defaults {
        if !existing_names.contains(&rule.name) {
            current.push(rule);
        }
    }

    write_schedule_to_path(&override_path, &current)?;
    Ok(ScheduleData {
        rules: current,
        using_override: true,
    })
}