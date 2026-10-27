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
