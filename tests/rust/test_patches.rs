use super::*;
use crate::test_helpers::temp_server;

fn setup() -> (tempfile::TempDir, String) {
    temp_server("Patches", "Data/Game/Patches")
}

#[test]
fn scan_finds_enabled_and_disabled() {
    let (_tmp, server_exe) = setup();
    let files = scan_patch_files(server_exe).unwrap();

    let enabled = files.iter().find(|f| f.file_name == "PatchDataBugFixes.json").unwrap();
    assert!(enabled.enabled);

    let disabled = files.iter().find(|f| f.file_name == "PatchDataDisabled.json").unwrap();
    assert!(!disabled.enabled);
}

#[test]
fn load_parses_all_value_types() {
    let (_tmp, server_exe) = setup();
    let entries = load_patch_file(server_exe, "PatchDataBugFixes.json".to_string(), true).unwrap();
    assert_eq!(entries.len(), 5);

    let by_path = |p: &str| entries.iter().find(|e| e.path == p).unwrap();
    assert!(by_path("SomeField").value.is_string());
    assert!(by_path("SomeNumber").value.is_number());
    assert!(by_path("SomeFlag").value.is_boolean());
    assert!(by_path("SomeArray").value.is_array());
    assert!(by_path("SomeObject").value.is_object());
}

#[test]
fn save_round_trip() {
    let (_tmp, server_exe) = setup();
    let file_name = "PatchDataBugFixes.json".to_string();
    let original = load_patch_file(server_exe.clone(), file_name.clone(), true).unwrap();

    save_patch_file(server_exe.clone(), file_name.clone(), true, original.clone()).unwrap();
    let reloaded = load_patch_file(server_exe, file_name, true).unwrap();

    assert_eq!(original.len(), reloaded.len());
    for (a, b) in original.iter().zip(reloaded.iter()) {
        assert_eq!(a.path, b.path);
        assert_eq!(a.value, b.value);
    }
}

#[test]
fn toggle_moves_file() {
    let (_tmp, server_exe) = setup();
    let new_state = set_patch_file_enabled(
        server_exe.clone(),
        "PatchDataBugFixes.json".to_string(),
        true,
    )
    .unwrap();
    assert!(!new_state);

    let files = scan_patch_files(server_exe).unwrap();
    let f = files.iter().find(|f| f.file_name == "PatchDataBugFixes.json").unwrap();
    assert!(!f.enabled);
}

#[test]
fn create_rejects_existing() {
    let (_tmp, server_exe) = setup();
    let err = create_patch_file(server_exe, "PatchDataBugFixes.json".to_string()).unwrap_err();
    assert!(err.contains("already exists"));
}

#[test]
fn create_rejects_existing_in_off_dir() {
    let (_tmp, server_exe) = setup();
    let err = create_patch_file(server_exe, "PatchDataDisabled.json".to_string()).unwrap_err();
    assert!(err.contains("already exists"));
}

#[test]
fn create_writes_new_enabled_file() {
    let (_tmp, server_exe) = setup();
    create_patch_file(server_exe.clone(), "PatchDataNew.json".to_string()).unwrap();
    let files = scan_patch_files(server_exe).unwrap();
    let f = files.iter().find(|f| f.file_name == "PatchDataNew.json").unwrap();
    assert!(f.enabled);
}

#[test]
fn save_keeps_backup_of_previous_contents() {
    let (_tmp, server_exe) = setup();
    let name = "PatchDataBugFixes.json".to_string();
    let file = patches_dir(&server_exe).unwrap().join(&name);
    let before = std::fs::read_to_string(&file).unwrap();

    save_patch_file(server_exe, name, true, vec![]).unwrap();

    assert_eq!(std::fs::read_to_string(file.with_extension("json.bak")).unwrap(), before);
    assert_eq!(std::fs::read_to_string(&file).unwrap().trim(), "[]");
}
