use super::*;
use crate::test_helpers::{empty_server, fixture_path};
use std::fs;

fn copy_config_fixtures(dir: &std::path::Path, with_override: bool) {
    fs::copy(fixture_path("Config.ini"), dir.join("Config.ini")).unwrap();
    if with_override {
        fs::copy(fixture_path("ConfigOverride.ini"), dir.join("ConfigOverride.ini")).unwrap();
    }
}

#[test]
fn parse_ini_basic() {
    let content = "[Section]\nKey=Value\nOther = Spaced\n";
    let data = parse_ini(content);
    assert_eq!(data["Section"]["Key"], "Value");
    assert_eq!(data["Section"]["Other"], "Spaced");
}

#[test]
fn parse_ini_comments_ignored() {
    let content = "[Section]\n;Comment=Ignored\nKey=Value\n";
    let data = parse_ini(content);
    assert_eq!(data["Section"].len(), 1);
    assert_eq!(data["Section"]["Key"], "Value");
}

#[test]
fn parse_ini_no_section() {
    // Keys before any [Section] header have current_section == "" and are
    // dropped by the `if !current_section.is_empty()` guard in parse_ini.
    let content = "Orphan=Value\n[Section]\nKey2=Value2\n";
    let data = parse_ini(content);
    assert!(!data.contains_key(""));
    assert_eq!(data.len(), 1);
    assert_eq!(data["Section"]["Key2"], "Value2");
}

#[test]
fn serialize_ini_round_trip() {
    let content = "[A]\nX=1\nY=2\n\n[B]\nZ=3\n";
    let data = parse_ini(content);
    let reparsed = parse_ini(&serialize_ini(&data));
    assert_eq!(data, reparsed);
}

#[test]
fn serialize_ini_sorted() {
    // IniData is a HashMap with a random iteration order per run, so use enough
    // sections and keys that sorted output cannot come out by chance.
    let mut content = String::new();
    for name in ["H", "G", "F", "E", "D", "C", "B", "A"] {
        content.push_str(&format!("[{name}]
Z=1
Y=2
X=3
W=4

"));
    }
    let out = serialize_ini(&parse_ini(&content));

    let sections: Vec<&str> = out.lines().filter(|l| l.starts_with('[')).collect();
    let mut sorted = sections.clone();
    sorted.sort();
    assert_eq!(sections, sorted);

    let first_keys: Vec<&str> = out.lines().skip(1).take(4).map(|l| l.split('=').next().unwrap()).collect();
    assert_eq!(first_keys, ["W", "X", "Y", "Z"]);
}

#[test]
fn merge_override_wins() {
    let (_tmp, server_exe) = empty_server();
    let dir = std::path::Path::new(&server_exe).parent().unwrap();
    copy_config_fixtures(dir, true);

    let state = read_config(server_exe).unwrap();
    assert_eq!(state.values["WebFrontend"]["Port"], "9090");
    assert_eq!(state.values["Frontend"]["Port"], "8888");
    assert_eq!(state.overridden["WebFrontend"], vec!["Port".to_string()]);
}

#[test]
fn write_config_removes_default_match() {
    let (_tmp, server_exe) = empty_server();
    let dir = std::path::Path::new(&server_exe).parent().unwrap().to_path_buf();
    copy_config_fixtures(&dir, true);

    let mut updates: IniData = HashMap::new();
    updates.insert(
        "WebFrontend".to_string(),
        HashMap::from([("Port".to_string(), "8080".to_string())]), // matches Config.ini default
    );
    write_config(server_exe, updates).unwrap();

    let content = fs::read_to_string(dir.join("ConfigOverride.ini")).unwrap();
    assert!(!content.contains("Port=8080"));
    assert!(!content.contains("[WebFrontend]"));
}

#[test]
fn write_config_adds_non_default() {
    let (_tmp, server_exe) = empty_server();
    let dir = std::path::Path::new(&server_exe).parent().unwrap().to_path_buf();
    copy_config_fixtures(&dir, false);

    let mut updates: IniData = HashMap::new();
    updates.insert(
        "Frontend".to_string(),
        HashMap::from([("Port".to_string(), "9999".to_string())]),
    );
    write_config(server_exe, updates).unwrap();

    let content = fs::read_to_string(dir.join("ConfigOverride.ini")).unwrap();
    assert!(content.contains("Port=9999"));
}

#[test]
fn reset_section_removes_from_override() {
    let (_tmp, server_exe) = empty_server();
    let dir = std::path::Path::new(&server_exe).parent().unwrap().to_path_buf();
    copy_config_fixtures(&dir, true);

    reset_config_section(server_exe, "WebFrontend".to_string()).unwrap();

    let content = fs::read_to_string(dir.join("ConfigOverride.ini")).unwrap();
    assert!(!content.contains("WebFrontend"));
}

#[test]
fn read_merged_value_fallback() {
    let (_tmp, server_exe) = empty_server();
    // No Config.ini at all — both files fail to read.
    let v = read_merged_value(&server_exe, "Nope", "Nope", "default_val");
    assert_eq!(v, "default_val");
}

#[test]
fn read_merged_value_override_wins() {
    let (_tmp, server_exe) = empty_server();
    let dir = std::path::Path::new(&server_exe).parent().unwrap().to_path_buf();
    copy_config_fixtures(&dir, true);

    assert_eq!(read_merged_value(&server_exe, "WebFrontend", "Port", "0"), "9090");
    assert_eq!(read_merged_value(&server_exe, "Frontend", "Port", "0"), "8888");
}

#[test]
fn uses_json_persistence_table() {
    let json = "[PlayerManager]\nUseJsonDBManager = true\n";
    let sqlite = "[PlayerManager]\nUseJsonDBManager = false\n";
    // (Config.ini, ConfigOverride.ini, expected)
    let cases: [(Option<&str>, Option<&str>, bool); 8] = [
        (None, None, false),
        (Some(""), None, false),
        (Some(json), None, true),
        (Some("[PlayerManager]\nUseJsonDBManager = True\n"), None, true),
        (Some(json), Some(sqlite), false),
        (Some(sqlite), Some(json), true),
        (Some("[Other]\nUseJsonDBManager = true\n"), None, false),
        (Some(json), Some("[PlayerManager]\nEnablePersistence = true\n"), true),
    ];
    for (i, (config, over, want)) in cases.iter().enumerate() {
        let (tmp, server_exe) = empty_server();
        if let Some(c) = config {
            fs::write(tmp.path().join("Config.ini"), c).unwrap();
        }
        if let Some(o) = over {
            fs::write(tmp.path().join("ConfigOverride.ini"), o).unwrap();
        }
        assert_eq!(uses_json_persistence(server_exe), *want, "case {i}");
    }
}
