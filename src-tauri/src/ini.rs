use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

// ── Types ─────────────────────────────────────────────────────────────────────

/// A flat map of section -> key -> value, representing one INI file.
type IniData = HashMap<String, HashMap<String, String>>;

/// The merged config sent to the frontend: defaults + overrides, plus which
/// keys have been overridden so the UI can show them as modified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigState {
    /// Merged values (override wins over default where present)
    pub values: IniData,
    /// Keys that exist in ConfigOverride.ini (section -> set of keys)
    pub overridden: HashMap<String, Vec<String>>,
}

// ── INI parser ────────────────────────────────────────────────────────────────

fn parse_ini(content: &str) -> IniData {
    let mut data: IniData = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            data.entry(current_section.clone()).or_default();
        } else if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().to_string();
            let value = line[eq + 1..].trim().to_string();
            if !current_section.is_empty() {
                data.entry(current_section.clone())
                    .or_default()
                    .insert(key, value);
            }
        }
    }
    data
}

fn serialize_ini(data: &IniData) -> String {
    let mut out = String::new();
    // Sort sections for stable output
    let mut sections: Vec<&String> = data.keys().collect();
    sections.sort();
    for section in sections {
        let keys = &data[section];
        if keys.is_empty() {
            continue;
        }
        out.push_str(&format!("[{}]\n", section));
        let mut sorted_keys: Vec<&String> = keys.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            out.push_str(&format!("{}={}\n", key, keys[key]));
        }
        out.push('\n');
    }
    out
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn server_dir(server_exe: &str) -> Option<PathBuf> {
    Path::new(server_exe).parent().map(|p| p.to_path_buf())
}

fn config_ini_path(server_exe: &str) -> Option<PathBuf> {
    server_dir(server_exe).map(|d| d.join("Config.ini"))
}

fn override_ini_path(server_exe: &str) -> Option<PathBuf> {
    server_dir(server_exe).map(|d| d.join("ConfigOverride.ini"))
}

// ── Public commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_config(server_exe: String) -> Result<ConfigState, String> {
    let config_path = config_ini_path(&server_exe)
        .ok_or("Could not determine server directory")?;
    let override_path = override_ini_path(&server_exe)
        .ok_or("Could not determine server directory")?;

    // Read Config.ini (must exist)
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Could not read Config.ini: {e}"))?;
    let defaults = parse_ini(&config_content);

    // Read ConfigOverride.ini (may not exist yet)
    let overrides = if override_path.exists() {
        let content = std::fs::read_to_string(&override_path)
            .map_err(|e| format!("Could not read ConfigOverride.ini: {e}"))?;
        parse_ini(&content)
    } else {
        HashMap::new()
    };

    // Build overridden key index
    let mut overridden: HashMap<String, Vec<String>> = HashMap::new();
    for (section, keys) in &overrides {
        let key_list: Vec<String> = keys.keys().cloned().collect();
        if !key_list.is_empty() {
            overridden.insert(section.clone(), key_list);
        }
    }

    // Merge: start with defaults, apply overrides on top
    let mut values = defaults.clone();
    for (section, keys) in &overrides {
        let section_map = values.entry(section.clone()).or_default();
        for (key, val) in keys {
            section_map.insert(key.clone(), val.clone());
        }
    }

    Ok(ConfigState { values, overridden })
}

#[tauri::command]
pub fn write_config(server_exe: String, updates: IniData) -> Result<(), String> {
    let config_path = config_ini_path(&server_exe)
        .ok_or("Could not determine server directory")?;
    let override_path = override_ini_path(&server_exe)
        .ok_or("Could not determine server directory")?;

    // Read defaults so we can diff
    let defaults = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Could not read Config.ini: {e}"))?;
        parse_ini(&content)
    } else {
        HashMap::new()
    };

    // Read existing overrides so we don't lose keys we don't manage
    let mut existing_overrides = if override_path.exists() {
        let content = std::fs::read_to_string(&override_path)
            .map_err(|e| format!("Could not read ConfigOverride.ini: {e}"))?;
        parse_ini(&content)
    } else {
        HashMap::new()
    };

    // For each key in updates: if it matches the default, remove the override.
    // If it differs, add/update the override.
    for (section, keys) in &updates {
        for (key, new_val) in keys {
            let default_val = defaults
                .get(section)
                .and_then(|s| s.get(key))
                .map(|s| s.as_str())
                .unwrap_or("");

            let section_overrides = existing_overrides.entry(section.clone()).or_default();

            if new_val.trim() == default_val.trim() {
                // Matches default — remove override if present
                section_overrides.remove(key);
            } else {
                // Differs from default — set override
                section_overrides.insert(key.clone(), new_val.clone());
            }
        }
        // Clean up empty sections
        if let Some(s) = existing_overrides.get(section) {
            if s.is_empty() {
                existing_overrides.remove(section);
            }
        }
    }

    let content = serialize_ini(&existing_overrides);
    std::fs::write(&override_path, content)
        .map_err(|e| format!("Could not write ConfigOverride.ini: {e}"))?;

    Ok(())
}

/// Reset all overrides for a specific section (restores defaults).
#[tauri::command]
pub fn reset_config_section(server_exe: String, section: String) -> Result<(), String> {
    let override_path = override_ini_path(&server_exe)
        .ok_or("Could not determine server directory")?;

    if !override_path.exists() {
        return Ok(()); // Nothing to reset
    }

    let content = std::fs::read_to_string(&override_path)
        .map_err(|e| format!("Could not read ConfigOverride.ini: {e}"))?;
    let mut overrides = parse_ini(&content);
    overrides.remove(&section);

    let new_content = serialize_ini(&overrides);
    std::fs::write(&override_path, new_content)
        .map_err(|e| format!("Could not write ConfigOverride.ini: {e}"))?;

    Ok(())
}