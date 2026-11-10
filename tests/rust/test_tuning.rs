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
    let last_toggleable = files.iter().rposition(|f| f.toggleable).expect("fixture has toggleable files");
    let first_other = files.iter().position(|f| !f.toggleable).expect("fixture has non-toggleable files");
    assert!(last_toggleable < first_other, "toggleable files must sort before non-toggleable ones");
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
fn read_reports_entries_that_would_crash_the_server() {
    // The server throws on a null or missing Prototype, so these must not load as if fine.
    let cases = [
        (r#"[{"Prototype": null, "Setting": "eGTV_XPGain", "Value": 2.0}]"#, "null"),
        (r#"[{"Setting": "eGTV_XPGain", "Value": 2.0}]"#, "missing field `Prototype`"),
        (r#"[{"Prototype": "", "Value": 2.0}]"#, "missing field `Setting`"),
    ];
    for (body, expected) in cases {
        let (_tmp, server_exe) = crate::test_helpers::empty_server();
        let dir = live_tuning_dir(&server_exe).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("LiveTuningDataBad.json"), body).unwrap();

        let err = read_tuning_file(server_exe, "LiveTuningDataBad.json".to_string()).unwrap_err();
        assert!(err.contains("LiveTuningDataBad.json") && err.contains(expected), "{body}: {err}");
    }
}

#[test]
fn read_keeps_empty_prototype_for_globals() {
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
    bytes.extend_from_slice(br#"[{"Prototype": "", "Setting": "eGTV_XPGain", "Value": 1.0}]"#);
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
    create_tuning_file(
        server_exe.clone(),
        "LiveTuningDataNew.json".to_string(),
        vec![TuningEntry { prototype: String::new(), setting: "eGTV_XPGain".to_string(), value: 2.5 }],
    )
    .unwrap();
    let entries = read_tuning_file(server_exe, "LiveTuningDataNew.json".to_string()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].setting, "eGTV_XPGain");
    assert_eq!(entries[0].value, 2.5);
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

#[test]
fn files_sort_in_the_order_the_server_loads_them() {
    // Parse order seen on a throwaway MHServerEmu 1.0.2 server (.NET 8, ICU), see
    // SimpleView-LiveTuning.md §2.1.
    let observed = [
        "LiveTuningData_MultiverseRates.json",
        "LiveTuningData_TEST.json",
        "LiveTuningData.json",
        "LiveTuningDataGlobal.json",
        "LiveTuningDataMultiverseRates.json",
        "LiveTuningDataPvP.json",
        "LiveTuningDataUser1.json",
        "LiveTuningDataZZMultiverse.json",
    ];
    let mut names: Vec<String> = observed.iter().rev().map(|s| s.to_string()).collect();
    names.swap(1, 5);
    sort_like_server(&mut names);
    assert_eq!(names, observed);

    // The two the launcher relies on: owned and user files load after Global, case ignored.
    let mut ours: Vec<String> = ["LiveTuningDataUserrates.json", "LiveTuningDataMultiverse.json", "LiveTuningDataglobal.json", "LiveTuningData-x.json"]
        .iter().map(|s| s.to_string()).collect();
    sort_like_server(&mut ours);
    assert_eq!(ours, ["LiveTuningData-x.json", "LiveTuningDataglobal.json", "LiveTuningDataMultiverse.json", "LiveTuningDataUserrates.json"]);
}

#[test]
fn global_tuning_lists_what_the_server_would_load() {
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let dir = live_tuning_dir(&server_exe).unwrap();
    fs::create_dir_all(dir.join("Events/Weekly")).unwrap();
    let write = |rel: &str, body: &str| fs::write(dir.join(rel), body).unwrap();
    let g = |s: &str, v: f64| format!(r#"{{ "Prototype": "", "Setting": "{s}", "Value": {v:?} }}"#);

    write("LiveTuningDataMultiverse.json", &format!("[{}]", g("eGTV_XPGain", 2.0)));
    write("LiveTuningDataGlobal.json", &format!("[{}, {}]", g("eGTV_XPGain", 1.0), g("eGTV_LootDropRate", 1.0)));
    write("LiveTuningData_Old.json", &format!("\u{FEFF}[{}]", g("eGTV_XPGain", 3.0)));
    write("LiveTuningDataPvP.json", r#"[{ "Prototype": "Entity/Mobs/Brute.prototype", "Setting": "eWETV_MobHealth", "Value": 15.0 },
                                        { "Prototype": "Entity/Mobs/Brute.prototype", "Setting": "eGTV_XPGain", "Value": 9.0 }]"#);
    write("LiveTuningDataBroken.json", "[{ \"Prototype\": \"\", ");
    write("OFF_LiveTuningDataUserOff.json", &format!("[{}]", g("eGTV_XPGain", 5.0)));
    write("Events/Weekly/LiveTuningDataNested.json", &format!("[{}]", g("eGTV_XPGain", 6.0)));
    write("Events/Weekly/LiveTuningDataChaos.json", &format!("\u{FEFF}[{}]", g("eGTV_CosmicPrestigeXPPct", 0.08)));
    write("Events/Weekly/LiveTuningDataLoot.json", r#"[{ "Prototype": "Loot/Tables/Holiday.prototype", "Setting": "eLTTV_Enabled", "Value": 2.0 }]"#);
    write("Events/Weekly/OFF_LiveTuningDataGone.json", &format!("[{}]", g("eGTV_XPGain", 7.0)));
    write("Events.json", r#"{
        "Chaos": { "DisplayName": "Cosmic Chaos", "FilePath": "Events/Weekly/LiveTuningDataChaos.json" },
        "Loot": { "DisplayName": "Loot Only", "FilePath": "Events/Weekly/LiveTuningDataLoot.json" },
        "Gone": { "DisplayName": "Missing File", "FilePath": "Events/Weekly/LiveTuningDataGone.json" }
    }"#);

    let state = read_global_tuning(server_exe).unwrap();

    let order: Vec<&str> = state.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(order, [
        "LiveTuningData_Old.json",
        "LiveTuningDataBroken.json",
        "LiveTuningDataGlobal.json",
        "LiveTuningDataMultiverse.json",
        "LiveTuningDataPvP.json",
    ]);
    let values = |i: usize| state.files[i].values.iter().map(|v| (v.setting.as_str(), v.value)).collect::<Vec<_>>();
    assert_eq!(values(0), [("eGTV_XPGain", 3.0)]);
    assert!(state.files[1].error.is_some() && state.files[1].values.is_empty());
    assert_eq!(values(2), [("eGTV_XPGain", 1.0), ("eGTV_LootDropRate", 1.0)]);
    assert_eq!(values(3), [("eGTV_XPGain", 2.0)]);
    assert!(values(4).is_empty(), "per-prototype entries, even eGTV_ ones, are not globals");

    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].display_name, "Cosmic Chaos");
    assert_eq!(state.events[0].values, [GlobalValue { setting: "eGTV_CosmicPrestigeXPPct".into(), value: 0.08 }]);
}

#[test]
fn global_tuning_without_a_live_tuning_folder_lists_only_the_server_rates_file() {
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let state = read_global_tuning(server_exe).unwrap();
    let names: Vec<&str> = state.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(names, [SERVER_RATES_FILE]);
    assert!(state.files[0].values.is_empty() && state.events.is_empty());
}

#[test]
fn global_tuning_lists_the_server_rates_file_where_it_would_load_before_it_exists() {
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let dir = live_tuning_dir(&server_exe).unwrap();
    fs::create_dir_all(&dir).unwrap();
    for name in ["LiveTuningDataUserRates.json", "LiveTuningDataGlobal.json", "LiveTuningDataPvP.json"] {
        fs::write(dir.join(name), "[]").unwrap();
    }

    let state = read_global_tuning(server_exe).unwrap();

    let names: Vec<&str> = state.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(names, ["LiveTuningDataGlobal.json", SERVER_RATES_FILE, "LiveTuningDataPvP.json", "LiveTuningDataUserRates.json"]);
    assert!(state.files[1].values.is_empty() && state.files[1].error.is_none(), "{:?}", state.files[1]);
}

#[test]
fn global_tuning_reads_the_infinity_and_event_switches() {
    // (Config.ini, ConfigOverride.ini, infinity_system, live_events); server defaults are true.
    let cases = [
        (None, None, true, true),
        (Some("[GameOptions]\nInfinitySystemEnabled=false\n[GameData]\nEnableLiveTuningEvents=False\n"), None, false, false),
        (Some("[GameOptions]\nInfinitySystemEnabled=true\n[GameData]\nEnableLiveTuningEvents=true\n"),
         Some("[GameOptions]\nInfinitySystemEnabled=false\n"), false, true),
        (Some("[GameOptions]\nInfinitySystemEnabled=false\n"),
         Some("[GameOptions]\nInfinitySystemEnabled=True\n[GameData]\nEnableLiveTuningEvents=false\n"), true, false),
    ];
    for (config, over, infinity, events) in cases {
        let (tmp, server_exe) = crate::test_helpers::empty_server();
        if let Some(c) = config { fs::write(tmp.path().join("Config.ini"), c).unwrap(); }
        if let Some(o) = over { fs::write(tmp.path().join("ConfigOverride.ini"), o).unwrap(); }
        let state = read_global_tuning(server_exe).unwrap();
        assert_eq!((state.infinity_system, state.live_events), (infinity, events), "{config:?} / {over:?}");
    }
}

#[test]
fn scan_lists_root_files_in_load_order() {
    let (_tmp, server_exe) = crate::test_helpers::empty_server();
    let dir = live_tuning_dir(&server_exe).unwrap();
    fs::create_dir_all(&dir).unwrap();
    for name in ["LiveTuningDataUserRates.json", "LiveTuningDataGlobal.json", "LiveTuningData.json", "LiveTuningData_TEST.json"] {
        fs::write(dir.join(name), "[]").unwrap();
    }

    let files = scan_tuning_files(server_exe, false).unwrap();

    let names: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(names, ["LiveTuningData_TEST.json", "LiveTuningData.json", "LiveTuningDataGlobal.json", "LiveTuningDataUserRates.json"]);
}
