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
    assert_eq!(got.fields[1..], ["Bodysuit", "Bodysuit10", "885", "Cosmic"]);
    assert!(parse_line(br#"{"event":"loot_hidden","enumIdx":1,"rarity":46816}"#, &names).unwrap().text.ends_with("Epic"));
    // unknown index / rarity fall back to placeholders and the raw number
    let got = parse_line(br#"{"event":"loot_hidden","enumIdx":7,"rarity":5}"#, &names).unwrap();
    assert_eq!(got.text, "--:--:-- | ? | ? | 7 | 5");
    assert!(parse_line(br#"{"event":"region_entered"}"#, &names).is_none()); // not shown for now
    assert!(parse_line(br#"{"x":1}"#, &names).is_none());
    assert!(parse_line(b"not json", &names).is_none());
    let c = clean("a\x1b[31mred\u{7}\n");
    assert!(!c.chars().any(|ch| ch.is_control()));
    assert_eq!(clean(&"x".repeat(2000)).chars().count(), 512);
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
