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

// Ids above 2^53 that the webview would round, and the repeated key that a
// parsed JSON object would collapse, all from PatchDataComplex.json.
const COMPLEX_IDS: &[&str] = &[
    "1076091916814587724", "13794733944514418082", "17910267881806762587",
    "9316722085805298222", "16576396162899383486", "1071992662862269383",
    "5460518059085536758", "9964627584974134775", "10449030269642610036",
    "8783203720206489305",
];

fn load_complex(server_exe: &str) -> Vec<PatchEntry> {
    load_patch_file(server_exe.to_string(), "PatchDataComplex.json".to_string(), true).unwrap()
}

#[test]
fn complex_values_load_as_their_original_text() {
    let (_tmp, server_exe) = setup();
    let entries = load_complex(&server_exe);

    for e in &entries {
        assert!(e.value.is_string(), "{} should cross as text", e.value_type);
    }
    let cooldowns = entries[0].value.as_str().unwrap();
    assert_eq!(cooldowns.matches("CooldownModifierPctForPower").count(), 2);
    assert!(cooldowns.contains("13794733944514418082"));
}

#[test]
fn complex_values_survive_a_save_unchanged() {
    let (_tmp, server_exe) = setup();
    let entries = load_complex(&server_exe);
    save_patch_file(server_exe.clone(), "PatchDataComplex.json".to_string(), true, entries).unwrap();

    let file = patches_dir(&server_exe).unwrap().join("PatchDataComplex.json");
    let text = std::fs::read_to_string(file).unwrap();
    for id in COMPLEX_IDS {
        assert!(text.contains(id), "{id} lost or rounded");
    }
    assert_eq!(text.matches("CooldownModifierPctForPower").count(), 2);
    // Still a file the server can parse.
    assert_eq!(serde_json::from_str::<Vec<JsonValue>>(&text).unwrap().len(), 4);
}

#[test]
fn complex_value_edited_as_text_is_written_as_json() {
    let (_tmp, server_exe) = setup();
    let mut entries = load_complex(&server_exe);
    entries[3].value = JsonValue::String(r#" { "ParentDataRef": 10449030269642610036, "Ability": 1 } "#.into());
    save_patch_file(server_exe.clone(), "PatchDataComplex.json".to_string(), true, entries).unwrap();

    // On disk the value is a JSON object, not a quoted string.
    let file = patches_dir(&server_exe).unwrap().join("PatchDataComplex.json");
    let text = std::fs::read_to_string(file).unwrap();
    let on_disk: Vec<JsonValue> = serde_json::from_str(&text).unwrap();
    assert_eq!(on_disk[3]["Value"]["Ability"], 1);
    assert!(text.contains("10449030269642610036"));
}

#[test]
fn save_rejects_complex_value_that_is_not_json() {
    let (_tmp, server_exe) = setup();
    let file = patches_dir(&server_exe).unwrap().join("PatchDataComplex.json");
    let before = std::fs::read_to_string(&file).unwrap();
    let mut entries = load_complex(&server_exe);
    entries[1].value = JsonValue::String("{ not json".into());

    let err = save_patch_file(server_exe, "PatchDataComplex.json".to_string(), true, entries).unwrap_err();

    assert!(err.contains("not valid JSON"), "{err}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}
