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
    let content = "[B]\nY=2\nX=1\n\n[A]\nZ=1\n";
    let data = parse_ini(content);
    let out = serialize_ini(&data);
    assert!(out.find("[A]").unwrap() < out.find("[B]").unwrap());
    assert!(out.find("X=1").unwrap() < out.find("Y=2").unwrap());
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
