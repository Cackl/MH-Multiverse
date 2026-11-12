use super::*;
use crate::test_helpers::temp_server;
use std::fs;

fn setup() -> (tempfile::TempDir, String) {
    temp_server("LiveTuning", "Data/Game/LiveTuning")
}

fn dir_for(server_exe: &str) -> std::path::PathBuf {
    crate::tuning::live_tuning_dir(server_exe).unwrap()
}

#[test]
fn load_falls_back_to_default() {
    let (_tmp, server_exe) = setup();
    let data = load_events(server_exe).unwrap();
    assert!(!data.using_override);
    assert_eq!(data.definitions.len(), 2);
    assert!(data.definitions.iter().any(|d| d.id == "ArmorIncursion"));
}

#[test]
fn load_prefers_override() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    fs::write(
        dir.join("EventsOverride.json"),
        r#"{"ArmorIncursion":{"DisplayName":"Overridden Name","FilePath":"Events/Weekly/LiveTuningDataArmorIncursion.json","DailyGift":null,"InstancedMissions":null,"IsHidden":false}}"#,
    )
    .unwrap();

    let data = load_events(server_exe).unwrap();
    assert!(data.using_override);
    assert_eq!(data.definitions.len(), 1);
    assert_eq!(data.definitions[0].display_name, "Overridden Name");
}

#[test]
fn save_override_writes_to_correct_file() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    let original_default = fs::read_to_string(dir.join("Events.json")).unwrap();

    save_events_override(
        server_exe,
        vec![EventDefinition {
            id: "NewEvent".to_string(),
            display_name: "New Event".to_string(),
            file_path: "LiveTuningDataNewEvent.json".to_string(),
            daily_gift: None,
            instanced_missions: None,
            is_hidden: None,
        }],
    )
    .unwrap();

    assert!(dir.join("EventsOverride.json").exists());
    assert_eq!(fs::read_to_string(dir.join("Events.json")).unwrap(), original_default);
}

#[test]
fn reset_copies_default_to_override() {
    let (_tmp, server_exe) = setup();
    let data = reset_events_override(server_exe).unwrap();
    assert!(data.using_override);
    assert_eq!(data.definitions.len(), 2);
}

#[test]
fn merge_adds_missing_events() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    fs::write(
        dir.join("EventsOverride.json"),
        r#"{"ArmorIncursion":{"DisplayName":"Customised","FilePath":"Events/Weekly/LiveTuningDataArmorIncursion.json","DailyGift":null,"InstancedMissions":null,"IsHidden":false}}"#,
    )
    .unwrap();

    let data = merge_events_override(server_exe).unwrap();
    assert_eq!(data.definitions.len(), 2);
    assert!(data.definitions.iter().any(|d| d.id == "CosmicChaos"));
}

#[test]
fn merge_does_not_overwrite_existing() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    fs::write(
        dir.join("EventsOverride.json"),
        r#"{"ArmorIncursion":{"DisplayName":"Customised","FilePath":"Events/Weekly/LiveTuningDataArmorIncursion.json","DailyGift":null,"InstancedMissions":null,"IsHidden":false}}"#,
    )
    .unwrap();

    let data = merge_events_override(server_exe).unwrap();
    let armor = data.definitions.iter().find(|d| d.id == "ArmorIncursion").unwrap();
    assert_eq!(armor.display_name, "Customised");
}

#[test]
fn schedule_round_trip() {
    let (_tmp, server_exe) = setup();
    let original = load_event_schedule(server_exe.clone()).unwrap().rules;
    save_schedule_override(server_exe.clone(), original.clone()).unwrap();
    let reloaded = load_event_schedule(server_exe).unwrap();

    assert!(reloaded.using_override);
    assert_eq!(original.len(), reloaded.rules.len());
    for (a, b) in original.iter().zip(reloaded.rules.iter()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.rule_type, b.rule_type);
        assert_eq!(a.duration_days, b.duration_days);
        assert_eq!(a.events, b.events);
    }
}

#[test]
fn null_event_slot_loads_as_empty_and_is_saved_as_empty_string() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    fs::write(
        dir.join("EventScheduleOverride.json"),
        r#"[{"Name":"Rot","IsEnabled":true,"Type":"WeeklyRotation","StartDayOfWeek":"Monday","Events":["ArmorIncursion",null,"CosmicChaos"]}]"#,
    )
    .unwrap();

    let rules = load_event_schedule(server_exe.clone()).unwrap().rules;
    assert_eq!(rules[0].events, ["ArmorIncursion", "", "CosmicChaos"]);

    save_schedule_override(server_exe, rules).unwrap();
    let written = fs::read_to_string(dir.join("EventScheduleOverride.json")).unwrap();
    assert!(!written.contains("null"), "{written}");
    assert!(written.contains("\"\""), "{written}");
}

#[test]
fn schedule_merge_by_name() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    // Override only has one of the two default rules, with IsEnabled flipped.
    fs::write(
        dir.join("EventScheduleOverride.json"),
        r#"[{"Name":"ArmorIncursionAlwaysOn","IsEnabled":false,"Type":"AlwaysOn","Events":["ArmorIncursion"]}]"#,
    )
    .unwrap();

    let data = merge_schedule_override(server_exe).unwrap();
    assert_eq!(data.rules.len(), 2);

    let armor = data.rules.iter().find(|r| r.name == "ArmorIncursionAlwaysOn").unwrap();
    assert!(!armor.is_enabled); // existing override entry not overwritten by merge

    assert!(data.rules.iter().any(|r| r.name == "CosmicChaosWeekly"));
}

#[test]
fn load_strips_bom() {
    let (_tmp, server_exe) = setup();
    let dir = dir_for(&server_exe);
    for name in ["Events.json", "EventSchedule.json"] {
        let text = fs::read_to_string(dir.join(name)).unwrap();
        fs::write(dir.join(name), format!("\u{FEFF}{text}")).unwrap();
    }
    assert_eq!(load_events(server_exe.clone()).unwrap().definitions.len(), 2);
    assert!(!load_event_schedule(server_exe).unwrap().rules.is_empty());
}

fn custom_event(id: &str, slug: &str) -> EventDefinition {
    EventDefinition {
        id: id.into(),
        display_name: "Double XP Weekend".into(),
        file_path: format!("Events/Multiverse/LiveTuningData{slug}.json"),
        daily_gift: None,
        instanced_missions: None,
        is_hidden: None,
    }
}

fn day_rule(day: &str, events: &[&str]) -> ScheduleRule {
    ScheduleRule {
        name: format!("Multiverse: Double XP Weekend ({day})"),
        is_enabled: true,
        rule_type: "DayOfWeek".into(),
        start_day_of_week: Some(day.into()),
        start_month: None,
        start_day: None,
        duration_days: None,
        events: events.iter().map(|s| s.to_string()).collect(),
    }
}

fn xp_file(slug: &str) -> OwnedEventFile {
    OwnedEventFile {
        slug: slug.into(),
        entries: vec![crate::tuning::TuningEntry { prototype: String::new(), setting: "eGTV_XPGain".into(), value: 2.0 }],
    }
}

#[test]
fn bundle_writes_the_tuning_file_and_both_overrides() {
    let (_tmp, server_exe) = setup();
    let id = "zzMultiverse_DoubleXPWeekend";
    let mut defs = load_events(server_exe.clone()).unwrap().definitions;
    defs.push(custom_event(id, "DoubleXPWeekend"));
    let mut rules = load_event_schedule(server_exe.clone()).unwrap().rules;
    rules.push(day_rule("Saturday", &[id]));
    rules.push(day_rule("Sunday", &[id]));

    save_event_bundle(server_exe.clone(), defs, rules, vec![xp_file("DoubleXPWeekend")]).unwrap();

    let entries = crate::tuning::read_tuning_file(
        server_exe.clone(),
        "Events/Multiverse/LiveTuningDataDoubleXPWeekend.json".into(),
    )
    .unwrap();
    assert_eq!((entries[0].setting.as_str(), entries[0].value), ("eGTV_XPGain", 2.0));
    let events = load_events(server_exe.clone()).unwrap();
    assert!(events.using_override && events.definitions.iter().any(|d| d.id == id));
    let schedule = load_event_schedule(server_exe.clone()).unwrap();
    assert_eq!(schedule.rules.iter().filter(|r| r.events == [id]).count(), 2);

    // The scan ties the owned file to its event, as it does for stock event files.
    let files = crate::tuning::scan_tuning_files(server_exe, false).unwrap();
    let f = files.iter().find(|f| f.relative_path == "Events/Multiverse/LiveTuningDataDoubleXPWeekend.json").unwrap();
    assert_eq!(f.event_id.as_deref(), Some(id));
}

#[test]
fn bundle_refuses_bad_slugs_and_writes_nothing() {
    for slug in ["", "../Evil", "a/b", r"a\b", "C:", "Dé", "Has Space", &"a".repeat(61)] {
        let (_tmp, server_exe) = setup();
        let err = save_event_bundle(server_exe.clone(), vec![], vec![], vec![xp_file(slug)]).unwrap_err();
        assert!(err.contains("Invalid event file name"), "{slug}: {err}");
        assert!(!dir_for(&server_exe).join("EventsOverride.json").exists(), "{slug}");
        assert!(!dir_for(&server_exe).join("Events").join("Multiverse").exists(), "{slug}");
    }
}

#[test]
fn bundle_refuses_a_rule_naming_a_missing_event() {
    let (_tmp, server_exe) = setup();
    let defs = vec![custom_event("zzMultiverse_A", "A")];
    let err = save_event_bundle(server_exe.clone(), defs.clone(), vec![day_rule("Saturday", &["zzMultiverse_A", "Gone"])], vec![])
        .unwrap_err();
    assert!(err.contains("'Gone'"), "{err}");
    assert!(!dir_for(&server_exe).join("EventScheduleOverride.json").exists());

    // Blank slots are allowed.
    save_event_bundle(server_exe, defs, vec![day_rule("Saturday", &["zzMultiverse_A", "", " "])], vec![]).unwrap();
}

#[test]
fn stock_events_ignore_overrides() {
    let (_tmp, server_exe) = setup();
    save_event_bundle(server_exe.clone(), vec![custom_event("zzMultiverse_A", "A")], vec![], vec![]).unwrap();
    let stock = load_stock_events(server_exe).unwrap();
    assert_eq!(stock.definitions.len(), 2);
    assert!(stock.definitions.iter().all(|d| d.id != "zzMultiverse_A"));
    assert!(!stock.rules.is_empty());
}
