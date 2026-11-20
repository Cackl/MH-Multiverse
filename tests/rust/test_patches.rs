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
    assert!(!new_state.enabled);
    assert_eq!(new_state.kept_as, None);

    let files = scan_patch_files(server_exe).unwrap();
    let f = files.iter().find(|f| f.file_name == "PatchDataBugFixes.json").unwrap();
    assert!(!f.enabled);
}

#[test]
fn toggle_with_both_copies_keeps_the_switched_one_and_backs_up_the_other() {
    for currently_enabled in [true, false] {
        let (_tmp, server_exe) = setup();
        let patches = patches_dir(&server_exe).unwrap();
        let (on, off) = (patches.join("PatchDataBugFixes.json"), off_dir(&patches).join("PatchDataBugFixes.json"));
        let on_text = fs::read_to_string(&on).unwrap();
        fs::write(&off, "[\"off copy\"]").unwrap();

        let switch = set_patch_file_enabled(server_exe, "PatchDataBugFixes.json".to_string(), currently_enabled).unwrap();
        let now = switch.enabled;

        let read = |p: &Path| fs::read_to_string(p).ok();
        let (kept, gone, kept_text, replaced_text) = if currently_enabled {
            (&off, &on, on_text.as_str(), "[\"off copy\"]")
        } else {
            (&on, &off, "[\"off copy\"]", on_text.as_str())
        };
        assert_eq!(now, !currently_enabled);
        assert_eq!(read(kept).as_deref(), Some(kept_text), "was enabled={currently_enabled}");
        let bak = crate::test_helpers::moved_aside(kept).expect("the replaced copy is moved aside");
        assert_eq!(read(&bak).as_deref(), Some(replaced_text), "was enabled={currently_enabled}");
        let folder = if currently_enabled { "Off/" } else { "" };
        assert_eq!(switch.kept_as, Some(format!("{folder}{}", crate::store::file_name_of(&bak))));
        assert!(!gone.exists(), "was enabled={currently_enabled}");
    }
}

#[test]
fn toggle_of_a_missing_file_leaves_the_other_copy_alone() {
    let (_tmp, server_exe) = setup();
    let off = off_dir(&patches_dir(&server_exe).unwrap()).join("PatchDataDisabled.json");
    let off_text = fs::read_to_string(&off).unwrap();

    // Asked to switch off a copy that isn't in Patches/: nothing may be moved aside.
    assert!(set_patch_file_enabled(server_exe, "PatchDataDisabled.json".to_string(), true).is_err());
    assert_eq!(fs::read_to_string(&off).unwrap(), off_text);
    assert!(crate::test_helpers::moved_aside(&off).is_none());
}

#[test]
fn create_rejects_existing() {
    let (_tmp, server_exe) = setup();
    let err = create_patch_file(server_exe, "BugFixes".to_string()).unwrap_err();
    assert!(err.contains("already exists"));
}

#[test]
fn create_rejects_existing_in_off_dir() {
    let (_tmp, server_exe) = setup();
    let err = create_patch_file(server_exe, "Disabled".to_string()).unwrap_err();
    assert!(err.contains("already exists"));
}

#[test]
fn create_rejects_names_that_could_leave_the_folder() {
    let (_tmp, server_exe) = setup();
    for bad in ["", "..\\Evil", "../Evil", "Sub/Name", "A.json", "Has Space"] {
        let err = create_patch_file(server_exe.clone(), bad.to_string()).unwrap_err();
        assert!(err.contains("letters, numbers"), "{bad:?} gave {err}");
    }
    assert!(create_patch_file(server_exe, "My_Tweaks-2".to_string()).is_ok());
}

#[test]
fn create_writes_new_enabled_file() {
    let (_tmp, server_exe) = setup();
    let name = create_patch_file(server_exe.clone(), "New".to_string()).unwrap();
    assert_eq!(name, "PatchDataNew.json");
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

#[test]
fn value_error_refuses_only_what_the_server_cannot_read() {
    let ok = |t: &str, v: &str| value_error(t, &serde_json::from_str(v).unwrap());
    for (t, v) in [
        ("String", r#""x""#), ("String", "null"), ("Enum", r#""A""#), ("Boolean", "false"),
        ("Float", "1.5"), ("Float", "3"), ("Float", "-3.4028235e38"),
        ("Integer", "-2147483648"), ("Integer", "2147483647"),
        ("PrototypeId", "18446744073709551615"), ("PrototypeGuid", "0"), ("LocaleStringId", "7"),
        ("PrototypeId[]", "[]"), ("PrototypeDataRef[]", "[1, 18446744073709551615]"),
        ("Vector3", "[1, 2.5, -3]"),
        ("Prototype", r#"{"ParentDataRef": 10449030269642610036, "Anything": "goes"}"#),
        ("Prototype[]", r#"[{"ParentDataRef": 1}]"#), ("Prototype[]", "[]"),
        ("Properties", r#"{"CooldownModifierPctGlobal": -0.25, "LootTablePrototype": [1, 2]}"#),
        ("Properties", r#"{"5": 1, " AIDefeated , Invalid ": 1}"#),
        ("SomethingNew", "null"),
    ] {
        assert_eq!(ok(t, v), None, "{t} {v}");
    }
    for (t, v) in [
        ("String", "5"), ("Enum", "true"), ("Boolean", r#""true""#), ("Boolean", "null"),
        ("Float", "null"), ("Float", r#""1""#), ("Float", "3.5e38"),
        ("Integer", "1.0"), ("Integer", "2147483648"), ("Integer", "-2147483649"), ("Integer", "null"),
        ("PrototypeId", r#""Powers/Foo.prototype""#), ("PrototypeId", "-1"), ("PrototypeId", "1.5"),
        ("PrototypeDataRef", "18446744073709551616"), ("PrototypeGuid", "null"),
        ("PrototypeId[]", r#"[1, "abc"]"#), ("PrototypeDataRef[]", "1"),
        ("Vector3", "[1, 2]"), ("Vector3", "[1, 2, 3, 4]"), ("Vector3", r#"[1, 2, "3"]"#), ("Vector3", "[1, 2, 1e39]"),
        ("Prototype", "null"), ("Prototype", r#"{"Value": 1}"#), ("Prototype", r#"{"ParentDataRef": "1"}"#),
        ("Prototype[]", r#"{"ParentDataRef": 1}"#), ("Prototype[]", r#"[{"ParentDataRef": 1}, {}]"#),
        ("Properties", "null"), ("Properties", "[]"), ("Properties", r#"{"CooldownModifierPctGlobl": 1}"#),
    ] {
        assert!(ok(t, v).is_some(), "{t} {v} should be refused");
    }
    assert_eq!(ok("Properties", r#"{"Typo": 1}"#).as_deref(), Some("Unknown property Typo."));
}

#[test]
fn save_refuses_a_value_the_server_cannot_read_even_when_disabled() {
    let (_tmp, server_exe) = setup();
    let file = patches_dir(&server_exe).unwrap().join("PatchDataComplex.json");
    let before = std::fs::read_to_string(&file).unwrap();
    let mut entries = load_complex(&server_exe);
    entries.push(PatchEntry {
        enabled: false,
        prototype: "Powers/Foo.prototype".into(),
        path: "Keywords[]".into(),
        description: String::new(),
        value_type: "PrototypeId".into(),
        value: JsonValue::String("Powers/Keywords/Bar.prototype".into()),
    });

    let err = save_patch_file(server_exe, "PatchDataComplex.json".to_string(), true, entries).unwrap_err();

    assert!(err.contains("Entry 5 (Powers/Foo.prototype): PrototypeId needs a numeric id."), "{err}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn id_text_is_checked_after_it_becomes_a_number() {
    let entry = |value: JsonValue| PatchEntry {
        enabled: true, prototype: "P".into(), path: "X".into(), description: "Named".into(),
        value_type: "PrototypeId[]".into(), value,
    };
    assert!(to_disk_all(vec![entry(serde_json::json!(["18446744073709551615", " 5 "]))]).is_ok());
    let err = to_disk_all(vec![entry(serde_json::json!(["5", "abc"]))]).err().unwrap();
    assert_eq!(err, "Entry 1 (Named): PrototypeId[] needs a list of numeric ids.");
}

#[test]
fn every_fixture_file_saves() {
    let (_tmp, server_exe) = setup();
    for f in scan_patch_files(server_exe.clone()).unwrap() {
        let entries = load_patch_file(server_exe.clone(), f.file_name.clone(), f.enabled).unwrap();
        assert!(to_disk_all(entries).is_ok(), "{}", f.file_name);
    }
}

#[test]
fn load_accepts_a_byte_order_mark() {
    let (_tmp, server_exe) = setup();
    let file = patches_dir(&server_exe).unwrap().join("PatchDataBom.json");
    let body = r#"[{ "Enabled": true, "Prototype": "A.prototype", "Path": "X", "Description": "", "ValueType": "Integer", "Value": 1 }]"#;
    std::fs::write(&file, format!("\u{FEFF}{body}")).unwrap();

    let entries = load_patch_file(server_exe, "PatchDataBom.json".to_string(), true).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, 1);
}

#[test]
fn scan_lists_files_in_server_load_order() {
    let (_tmp, server_exe) = setup();
    fs::write(patches_dir(&server_exe).unwrap().join("PatchData_X.json"), "[]").unwrap();
    let names: Vec<String> = scan_patch_files(server_exe).unwrap().into_iter().map(|f| f.file_name).collect();
    let at = |n: &str| names.iter().position(|f| f == n).unwrap();
    // The server's culture sort puts _ before letters. An ordinal sort would not.
    assert!(at("PatchData_X.json") < at("PatchDataBugFixes.json"), "{names:?}");
}
