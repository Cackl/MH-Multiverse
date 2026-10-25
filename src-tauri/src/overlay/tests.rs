use super::*;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mhm-overlay-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(name: &str) -> PathBuf {
    let d = tmp(name);
    std::fs::write(d.join(DLL_NAME), b"dll").unwrap();
    std::fs::write(d.join("MhClientHook.terminals.bin"), b"bin").unwrap();
    std::fs::create_dir_all(d.join("MhClientHook.ui-fonts")).unwrap();
    std::fs::write(d.join("MhClientHook.ui-fonts/a.ttf"), b"font").unwrap();
    // mutable / unrelated files must be ignored
    std::fs::write(d.join("MhClientHook.dps-enabled.flag"), b"1").unwrap();
    std::fs::write(d.join("MhClientHook.dps-panel.json"), b"{}").unwrap();
    std::fs::write(d.join("MhLauncher.exe"), b"exe").unwrap();
    d
}

fn record(d: &Path) -> TrustRecord {
    let files = scan_set(d).unwrap();
    TrustRecord { set_hash: set_hash(&files), files }
}

#[test]
fn hash_is_deterministic_and_ignores_mutable_files() {
    let d = fixture("hash");
    let a = record(&d);
    std::fs::write(d.join("MhClientHook.dps-enabled.flag"), b"0").unwrap();
    std::fs::write(d.join("MhClientHook.dps-panel.json"), b"{\"x\":1}").unwrap();
    std::fs::write(d.join("MhLauncher.exe"), b"other").unwrap();
    assert_eq!(a, record(&d));
    assert_eq!(a.files.len(), 3);
}

#[test]
fn diff_reports_changed_missing_added() {
    let d = fixture("diff");
    let t = record(&d);
    std::fs::write(d.join(DLL_NAME), b"tampered").unwrap();
    std::fs::remove_file(d.join("MhClientHook.terminals.bin")).unwrap();
    std::fs::write(d.join("MhClientHook.enum-names.bin"), b"new").unwrap();
    let mut got = diff(&t.files, &scan_set(&d).unwrap());
    got.sort();
    assert_eq!(got, [
        "added: MhClientHook.enum-names.bin",
        "changed: MhClientHook.dll",
        "missing: MhClientHook.terminals.bin",
    ]);
}

#[test]
fn unexpected_entries_in_code_folders_are_flagged() {
    let d = fixture("unhashable");
    let t = record(&d);
    // a subfolder (stand-in for a symlink) appears after trust
    std::fs::create_dir_all(d.join("MhClientHook.ui-fonts/sub")).unwrap();
    let now = scan_set(&d).unwrap();
    assert_eq!(now["MhClientHook.ui-fonts/sub"], UNHASHABLE);
    assert_eq!(diff(&t.files, &now), ["added: MhClientHook.ui-fonts/sub"]);
}

#[test]
fn status_states() {
    let d = fixture("status");
    let mut cfg = OverlayConfig::default();
    assert_eq!(status_of(&cfg).state, "not_linked");
    cfg.dir = d.to_string_lossy().into();
    assert_eq!(status_of(&cfg).state, "untrusted");
    cfg.trusted = Some(record(&d));
    assert_eq!(status_of(&cfg).state, "trusted");
    std::fs::write(d.join(DLL_NAME), b"x").unwrap();
    assert_eq!(status_of(&cfg).state, "changed");
    std::fs::remove_file(d.join(DLL_NAME)).unwrap();
    assert_eq!(status_of(&cfg).state, "missing");
}

#[test]
fn flag_writer_is_allowlisted_and_writes_one_byte() {
    let d = tmp("flag");
    write_flag(&d, "MhClientHook.dps-enabled.flag", true).unwrap();
    assert_eq!(std::fs::read(d.join("MhClientHook.dps-enabled.flag")).unwrap(), b"1");
    write_flag(&d, "MhClientHook.dps-enabled.flag", false).unwrap();
    assert_eq!(std::fs::read(d.join("MhClientHook.dps-enabled.flag")).unwrap(), b"0");
    assert!(write_flag(&d, "../evil", true).is_err());
}

#[test]
fn flags_read_back_including_bom_and_absent() {
    let d = tmp("flags-read");
    let f = read_flags(&d);
    assert!(f["dps"] && f["terminals"] && !f["detector_log"]); // absent = per-flag default
    write_flag(&d, "MhClientHook.dps-enabled.flag", true).unwrap();
    std::fs::write(d.join("MhClientHook.terminals-enabled.flag"), b"\xEF\xBB\xBF1").unwrap();
    let f = read_flags(&d);
    assert!(f["dps"] && f["terminals"] && !f["detector_log"]);
}

#[test]
fn pipe_splitter_handles_chunks_and_overlong_lines() {
    use super::pipe::LineSplitter;
    let mut s = LineSplitter::default();
    assert!(s.feed(b"{\"event\":\"reg").is_empty());
    assert_eq!(s.feed(b"ion_exited\"}\n\nnext\n"), vec![b"{\"event\":\"region_exited\"}".to_vec(), b"next".to_vec()]);
    let mut long = vec![b'a'; 9000];
    long.extend_from_slice(b"\nok\n");
    assert_eq!(s.feed(&long), vec![b"ok".to_vec()]);
}


#[test]
fn pipe_parser_formats_loot_and_ignores_the_rest() {
    use super::pipe::{clean, parse_line};
    let mut names = std::collections::HashMap::new();
    names.insert(885u64, "Bodysuit10".to_string());
    let line = br#"{"event":"loot_hidden","enumIdx":885,"entityId":"1","rarity":43721,"reason":"x","atUtc":"2026-10-21T11:01:04.159Z"}"#;
    let got = parse_line(line, &names).unwrap();
    assert_eq!(got.event, "loot_hidden");
    assert!(got.text.ends_with(" | Bodysuit | Bodysuit10 | 885 | Cosmic"), "{}", got.text);
    assert_eq!(got.fields[1..], ["Bodysuit", "Bodysuit10", "885", "Cosmic", "x"]);
    assert!(parse_line(br#"{"event":"loot_hidden","enumIdx":1,"rarity":46816}"#, &names).unwrap().text.ends_with("Epic"));
    // unknown index / rarity fall back to placeholders and the raw number
    let got = parse_line(br#"{"event":"loot_hidden","enumIdx":7,"rarity":5}"#, &names).unwrap();
    assert_eq!(got.text, "--:--:-- | ? | ? | 7 | 5");
    assert!(parse_line(br#"{"event":"region_exited","regionId":"1"}"#, &names).is_none()); // not shown
    assert!(parse_line(br#"{"x":1}"#, &names).is_none());
    assert!(parse_line(b"not json", &names).is_none());
    let c = clean("a\x1b[31mred\u{7}\n");
    assert!(!c.chars().any(|ch| ch.is_control()));
    assert_eq!(clean(&"x".repeat(2000)).chars().count(), 512);
}

#[test]
fn pipe_parser_shows_terminal_difficulty_mission_and_loot_reason() {
    use super::pipe::parse_line;
    let names = std::collections::HashMap::new();
    let p = |s: &str| parse_line(s.as_bytes(), &names);
    assert!(p(r#"{"event":"region_entered","regionId":"1"}"#).is_none()); // hub: no terminal
    assert_eq!(p(r#"{"event":"region_entered","terminal":"Doom","tier":"Heroic"}"#).unwrap().text, "Terminal  Doom  Heroic");
    assert!(p(r#"{"event":"tier_resolved","regionId":"1"}"#).is_none());
    assert_eq!(p(r#"{"event":"tier_resolved","tier":"Cosmic"}"#).unwrap().text, "Difficulty: Cosmic");
    assert!(p(r#"{"event":"mission_state","from":1,"to":2}"#).is_none());
    assert_eq!(p(r#"{"event":"mission_state","to":4}"#).unwrap().text, "✓ Mission completed");
    assert_eq!(p(r#"{"event":"mission_state","to":4,"terminal":"Doom"}"#).unwrap().text, "✓ Mission completed — Doom");
    let loot = |reason: &str| p(&format!(r#"{{"event":"loot_hidden","enumIdx":1,"reason":"{reason}"}}"#)).unwrap().fields[5].clone();
    assert_eq!((loot("-rarity"), loot("matrix"), loot("static-list"), loot("")), ("rarity".into(), "".into(), "".into(), "".into()));
}

#[test]
fn pipe_parser_names_and_categorises_from_prototype_paths() {
    use super::pipe::parse_line;
    let mut names = std::collections::HashMap::new();
    for (i, p) in [
        (1u64, "Entity/Items/Rings/Prototypes/Ring01.prototype"),
        (2, "Entity/Items/Armor/Prototypes/X23/o2Bodysuit/Bodysuit10.prototype"),
        (3, "Entity/Items/Misc/Thing.prototype"),
        (4, "Entity/Characters/Mobs/Ultron.prototype"),
    ] {
        names.insert(i, p.to_string());
    }
    let f = |i: u32| parse_line(format!(r#"{{"event":"loot_hidden","enumIdx":{i}}}"#).as_bytes(), &names).unwrap().fields;
    assert_eq!((f(1)[1].as_str(), f(1)[2].as_str()), ("Ring", "Ring01"));
    assert_eq!((f(2)[1].as_str(), f(2)[2].as_str()), ("Armor", "Bodysuit10"));
    assert_eq!(f(3)[1], "Item");
    let d = parse_line(br#"{"event":"player_death","killerEnumIdx":4}"#, &names).unwrap();
    assert_eq!(d.fields[1], "☠ Killed by Ultron");
}

#[test]
fn pipe_parser_builds_death_card() {
    use super::pipe::parse_line;
    let mut names = std::collections::HashMap::new();
    names.insert(5u64, "Ultron".to_string());
    names.insert(6u64, "Sentinel".to_string());
    let line = br#"{"event":"player_death","killerEnumIdx":5,"dmgType":"Energy","dmgValue":1234567,"recent":[
        {"idx":6,"value":100,"hpLost":0,"msAgo":2500,"hp":0,"hpMax":0,"type":"Physical","power":"Punch"},
        {"idx":5,"value":200,"hpLost":400,"msAgo":300,"hp":50,"hpMax":200,"type":"Energy","power":"Beam"},
        {"idx":9,"value":0,"msAgo":1}]}"#;
    let d = parse_line(line, &names).unwrap();
    assert_eq!(d.fields[1], "☠ Killed by Ultron");
    assert_eq!(d.fields[2], "1,234,567 Energy · Beam"); // power from the first hit with hpMax
    assert_eq!(d.hits.len(), 2); // value == 0 skipped
    assert_eq!((d.hits[0].time.as_str(), d.hits[0].health.as_str(), d.hits[0].damage.as_str()), ("−2.5s", "", "~100"));
    assert_eq!((d.hits[1].health.as_str(), d.hits[1].damage.as_str(), d.hits[1].attacker.as_str()), ("50 HP (25%)", "400", "Ultron"));
    assert!(d.hits[1].note.contains("x2.00"));
    let u = parse_line(br#"{"event":"player_death","killerEnumIdx":0}"#, &names).unwrap();
    assert_eq!((u.fields[1].as_str(), u.fields[2].as_str()), ("You were killed", "killer unknown"));
    let u = parse_line(br#"{"event":"player_death","killerEnumIdx":77}"#, &names).unwrap();
    assert_eq!(u.fields[2], "enumIdx 77");
}

#[test]
fn enum_names_load_and_tolerate_bad_files() {
    use super::pipe::load_names;
    let d = tmp("names");
    assert!(load_names(&d).is_empty()); // missing
    let mut b = b"MHNE".to_vec();
    b.extend_from_slice(&[1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
    for (i, n) in [(2u32, "Cyberware07"), (3, "Cape07")] {
        b.extend_from_slice(&i.to_le_bytes());
        b.extend_from_slice(&(n.len() as u16).to_le_bytes());
        b.extend_from_slice(n.as_bytes());
    }
    b.extend_from_slice(&[9, 0, 0, 0, 200, 0, b'x']); // truncated record
    std::fs::write(d.join("MhClientHook.enum-names.bin"), &b).unwrap();
    let n = load_names(&d);
    assert_eq!((n.len(), n[&2].as_str(), n[&3].as_str()), (2, "Cyberware07", "Cape07"));
    std::fs::write(d.join("MhClientHook.enum-names.bin"), b"garbage").unwrap();
    assert!(load_names(&d).is_empty());
}
