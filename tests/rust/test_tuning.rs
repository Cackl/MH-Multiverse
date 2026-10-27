use super::*;
use crate::test_helpers::temp_server;
use std::fs;

fn setup() -> (tempfile::TempDir, String) {
    temp_server("LiveTuning", "Data/Game/LiveTuning")
}

fn find<'a>(files: &'a [TuningFileInfo], relative_path: &str) -> &'a TuningFileInfo {
    files
        .iter()
        .find(|f| f.relative_path == relative_path)
        .unwrap_or_else(|| panic!("no entry for {relative_path} in {files:?}"))
}

#[test]
fn scan_finds_root_files() {
    let (_tmp, server_exe) = setup();
    let files = scan_tuning_files(server_exe, false).unwrap();
    let f = find(&files, "LiveTuningDataBugFixes.json");
    assert!(f.enabled);
    assert!(f.toggleable);
    assert!(f.event_id.is_none());
}

#[test]
fn scan_finds_off_files() {
    let (_tmp, server_exe) = setup();
    let files = scan_tuning_files(server_exe, false).unwrap();
    let f = find(&files, "LiveTuningDataTest.json");
    assert!(!f.enabled);
    assert!(f.toggleable);
}

#[test]
fn scan_finds_subdirectory_files() {
    let (_tmp, server_exe) = setup();
    let files = scan_tuning_files(server_exe, false).unwrap();
    let f = find(&files, "Events/Weekly/LiveTuningDataArmorIncursion.json");
    assert!(!f.toggleable);
    assert!(f.enabled);
    assert_eq!(f.event_id.as_deref(), Some("ArmorIncursion"));
}

#[test]
fn scan_skips_reserved_files() {
    let (_tmp, server_exe) = setup();
    let files = scan_tuning_files(server_exe, false).unwrap();
    // Fixture dir has 6 files on disk (4 tuning files + Events.json + EventSchedule.json);
    // only the 4 tuning files should come back.
    assert_eq!(files.len(), 4);
    assert!(files.iter().all(|f| f.canonical_name != "Events.json"));
    assert!(files.iter().all(|f| f.canonical_name != "EventSchedule.json"));
}

#[test]
fn scan_sorts_toggleable_first() {
    let (_tmp, server_exe) = setup();
    let files = scan_tuning_files(server_exe, false).unwrap();
    let last_toggleable = files.iter().rposition(|f| f.toggleable);
    let first_non_toggleable = files.iter().position(|f| !f.toggleable);
    if let (Some(last_t), Some(first_n)) = (last_toggleable, first_non_toggleable) {
        assert!(last_t < first_n, "toggleable files must sort before non-toggleable ones");
    }
}

#[test]
fn scan_deduplicates() {
    // Both the active and OFF_ variant of the same canonical file exist on disk
    // at once (a corrupted/unexpected state) — scan must still report exactly
    // one entry for it, not two.
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let dir = live_tuning_dir(&server_exe).unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("LiveTuningDataDupe.json"), "[]").unwrap();
    fs::write(dir.join("OFF_LiveTuningDataDupe.json"), "[]").unwrap();

    let files = scan_tuning_files(server_exe, false).unwrap();
    assert_eq!(files.iter().filter(|f| f.relative_path == "LiveTuningDataDupe.json").count(), 1);
}

#[test]
fn scan_auto_enables_event_owned_off_file() {
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();

    let files = scan_tuning_files(server_exe, true).unwrap();
    let f = find(&files, "LiveTuningDataCosmicChaos.json");
    assert!(f.enabled);
    assert!(f.was_auto_enabled);
    assert!(!f.toggleable); // still event-owned

    assert!(dir.join("LiveTuningDataCosmicChaos.json").exists());
    assert!(!dir.join("OFF_LiveTuningDataCosmicChaos.json").exists());
}

#[test]
fn scan_leaves_event_owned_off_file_when_apply_auto_enable_false() {
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();

    let files = scan_tuning_files(server_exe, false).unwrap();
    let f = find(&files, "LiveTuningDataCosmicChaos.json");
    assert!(!f.enabled);
    assert!(!f.was_auto_enabled);

    // Read-only scan must not have touched the file on disk.
    assert!(dir.join("OFF_LiveTuningDataCosmicChaos.json").exists());
    assert!(!dir.join("LiveTuningDataCosmicChaos.json").exists());
}

#[test]
fn read_normalises_null_prototype() {
    let (_tmp, server_exe) = setup();
    let entries = read_tuning_file(server_exe, "LiveTuningDataBugFixes.json".to_string()).unwrap();
    let e = entries.iter().find(|e| e.setting == "eGTV_XPGain").unwrap();
    assert_eq!(e.prototype, "");
}

#[test]
fn read_strips_bom() {
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let dir = live_tuning_dir(&server_exe).unwrap();
    fs::create_dir_all(&dir).unwrap();
    let mut bytes = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
    bytes.extend_from_slice(br#"[{"Prototype": null, "Setting": "eGTV_XPGain", "Value": 1.0}]"#);
    fs::write(dir.join("LiveTuningDataBom.json"), bytes).unwrap();

    let entries = read_tuning_file(server_exe, "LiveTuningDataBom.json".to_string()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].setting, "eGTV_XPGain");
}

#[test]
fn write_round_trip() {
    let (_tmp, server_exe) = setup();
    let path = "LiveTuningDataBugFixes.json".to_string();
    let original = read_tuning_file(server_exe.clone(), path.clone()).unwrap();
    write_tuning_file(server_exe.clone(), path.clone(), original.clone()).unwrap();
    let reread = read_tuning_file(server_exe, path).unwrap();

    assert_eq!(original.len(), reread.len());
    for (a, b) in original.iter().zip(reread.iter()) {
        assert_eq!(a.prototype, b.prototype);
        assert_eq!(a.setting, b.setting);
        assert_eq!(a.value, b.value);
    }
}

#[test]
fn write_preserves_disabled_state() {
    // write_tuning_file must write to whichever path (active or OFF_) exists
    // on disk, not force the file to become active.
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();
    let path = "LiveTuningDataTest.json".to_string(); // currently OFF_ on disk

    write_tuning_file(server_exe, path, vec![TuningEntry {
        prototype: String::new(),
        setting: "eGTV_XPGain".to_string(),
        value: 9.0,
    }]).unwrap();

    assert!(dir.join("OFF_LiveTuningDataTest.json").exists());
    assert!(!dir.join("LiveTuningDataTest.json").exists());
}

#[test]
fn toggle_enable() {
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();
    toggle_tuning_file(server_exe, "LiveTuningDataTest.json".to_string(), true).unwrap();
    assert!(dir.join("LiveTuningDataTest.json").exists());
    assert!(!dir.join("OFF_LiveTuningDataTest.json").exists());
}

#[test]
fn toggle_disable() {
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();
    toggle_tuning_file(server_exe, "LiveTuningDataBugFixes.json".to_string(), false).unwrap();
    assert!(dir.join("OFF_LiveTuningDataBugFixes.json").exists());
    assert!(!dir.join("LiveTuningDataBugFixes.json").exists());
}

#[test]
fn toggle_rejects_subdirectory_file() {
    let (_tmp, server_exe) = setup();
    let err = toggle_tuning_file(
        server_exe,
        "Events/Weekly/LiveTuningDataArmorIncursion.json".to_string(),
        false,
    )
    .unwrap_err();
    assert!(err.contains("subdirectory"));
}

#[test]
fn toggle_rejects_event_owned_file() {
    // LiveTuningDataCosmicChaos.json is event-owned even though it's root-level.
    let (_tmp, server_exe) = setup();
    let err = toggle_tuning_file(server_exe, "LiveTuningDataCosmicChaos.json".to_string(), true).unwrap_err();
    assert!(err.contains("event-owned"));
}

#[test]
fn create_rejects_subdirectory() {
    let (_tmp, server_exe) = setup();
    let err = create_tuning_file(server_exe, "sub/file.json".to_string(), vec![]).unwrap_err();
    assert!(err.contains("root-level"));
}

#[test]
fn create_rejects_duplicate() {
    let (_tmp, server_exe) = setup();
    let err = create_tuning_file(server_exe, "LiveTuningDataBugFixes.json".to_string(), vec![]).unwrap_err();
    assert!(err.contains("already exists"));
}

#[test]
fn create_rejects_invalid_filename() {
    let (_tmp, server_exe) = setup();
    let err = create_tuning_file(server_exe, "NotATuningFile.json".to_string(), vec![]).unwrap_err();
    assert!(err.contains("Invalid filename"));
}

#[test]
fn create_writes_new_file() {
    let (_tmp, server_exe) = setup();
    let dir = live_tuning_dir(&server_exe).unwrap();
    create_tuning_file(
        server_exe,
        "LiveTuningDataNew.json".to_string(),
        vec![TuningEntry { prototype: String::new(), setting: "eGTV_XPGain".to_string(), value: 1.0 }],
    )
    .unwrap();
    assert!(dir.join("LiveTuningDataNew.json").exists());
}

#[test]
fn write_keeps_backup_of_previous_contents() {
    let (_tmp, server_exe) = setup();
    let name = "LiveTuningDataBugFixes.json".to_string();
    let file = live_tuning_dir(&server_exe).unwrap().join(&name);
    let before = fs::read_to_string(&file).unwrap();

    write_tuning_file(server_exe, name, vec![]).unwrap();

    assert_eq!(fs::read_to_string(file.with_extension("json.bak")).unwrap(), before);
    assert_eq!(fs::read_to_string(&file).unwrap().trim(), "[]");
}
