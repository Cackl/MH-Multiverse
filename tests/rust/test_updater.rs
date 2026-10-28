use super::*;
use crate::test_helpers::empty_server;
use std::fs;
use std::path::Path;

#[test]
fn create_backup_copies_targets() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();
    fs::create_dir_all(dir.join("Data/Game/LiveTuning")).unwrap();
    fs::write(dir.join("Config.ini"), "[A]\nX=1\n").unwrap();
    fs::write(dir.join("Data/Game/LiveTuning/LiveTuningDataBugFixes.json"), "[]").unwrap();

    let manifest = create_backup(
        server_exe,
        vec!["Config.ini".to_string(), "Data/Game/LiveTuning".to_string()],
        "pre-update".to_string(),
    )
    .unwrap();

    assert_eq!(manifest.targets, vec!["Config.ini".to_string(), "Data/Game/LiveTuning".to_string()]);
    let backup_dir = dir.join("Backups").join(&manifest.id);
    assert!(backup_dir.join("Config.ini").exists());
    assert!(backup_dir.join("Data/Game/LiveTuning/LiveTuningDataBugFixes.json").exists());
}

#[test]
fn create_backup_writes_manifest() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();
    fs::write(dir.join("Config.ini"), "[A]\nX=1\n").unwrap();

    let manifest = create_backup(server_exe, vec!["Config.ini".to_string()], "pre-update".to_string()).unwrap();

    let manifest_path = dir.join("Backups").join(&manifest.id).join("manifest.json");
    let reread: BackupManifest = serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(reread.id, manifest.id);
    assert_eq!(reread.label, "pre-update");
}

#[test]
fn create_backup_skips_blacklisted() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();
    fs::create_dir_all(dir.join("Data/Game")).unwrap();
    fs::write(dir.join("Data/Game/Calligraphy.sip"), "dummy-pak-bytes").unwrap();

    let manifest = create_backup(
        server_exe,
        vec!["Data/Game/Calligraphy.sip".to_string()],
        "pre-update".to_string(),
    )
    .unwrap();

    // The target is recorded as attempted (the source file existed) even
    // though copy_path silently no-ops for a blacklisted file.
    assert_eq!(manifest.targets, vec!["Data/Game/Calligraphy.sip".to_string()]);
    let backup_dir = dir.join("Backups").join(&manifest.id);
    assert!(!backup_dir.join("Data/Game/Calligraphy.sip").exists());
}

#[test]
fn restore_backup_overwrites_files() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();
    fs::write(dir.join("Config.ini"), "original").unwrap();

    let manifest = create_backup(server_exe.clone(), vec!["Config.ini".to_string()], "pre-update".to_string()).unwrap();
    fs::write(dir.join("Config.ini"), "modified").unwrap();

    restore_backup(server_exe, manifest.id).unwrap();
    assert_eq!(fs::read_to_string(dir.join("Config.ini")).unwrap(), "original");
}

#[test]
fn restore_backup_missing_errors() {
    let (_tmp, server_exe) = empty_server();
    let err = restore_backup(server_exe, "nonexistent".to_string()).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn list_backups_sorted_newest_first() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();

    for (id, created_at) in [("20260101-000000", "2026-01-01T00:00:00+00:00"), ("20260102-000000", "2026-01-02T00:00:00+00:00")] {
        let backup_dir = dir.join("Backups").join(id);
        fs::create_dir_all(&backup_dir).unwrap();
        let manifest = BackupManifest {
            id: id.to_string(),
            created_at: created_at.to_string(),
            label: "test".to_string(),
            targets: vec![],
            size_bytes: 0,
        };
        fs::write(backup_dir.join("manifest.json"), serde_json::to_string(&manifest).unwrap()).unwrap();
    }

    let backups = list_backups(server_exe).unwrap();
    assert_eq!(backups.len(), 2);
    assert_eq!(backups[0].id, "20260102-000000"); // newest first
    assert_eq!(backups[1].id, "20260101-000000");
}

#[test]
fn delete_backup_removes_directory() {
    let (_tmp, server_exe) = empty_server();
    let dir = crate::paths::server_dir(&server_exe).unwrap();
    fs::write(dir.join("Config.ini"), "content").unwrap();
    let manifest = create_backup(server_exe.clone(), vec!["Config.ini".to_string()], "pre-update".to_string()).unwrap();

    let backup_dir = dir.join("Backups").join(&manifest.id);
    assert!(backup_dir.exists());

    delete_backup(server_exe, manifest.id).unwrap();
    assert!(!backup_dir.exists());
}

#[test]
fn delete_backup_missing_errors() {
    let (_tmp, server_exe) = empty_server();
    let err = delete_backup(server_exe, "nonexistent".to_string()).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn detect_source_dir_wrapper() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("MHServerEmu-1.0")).unwrap();
    fs::write(tmp.path().join("MHServerEmu-1.0/MHServerEmu.exe"), "").unwrap();

    let result = detect_source_dir(tmp.path()).unwrap();
    assert_eq!(result, tmp.path().join("MHServerEmu-1.0"));
}

#[test]
fn detect_source_dir_flat() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("MHServerEmu.exe"), "").unwrap();

    let result = detect_source_dir(tmp.path()).unwrap();
    assert_eq!(result, tmp.path().to_path_buf());
}

#[test]
fn copy_path_skips_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("does-not-exist.txt");
    let dst = tmp.path().join("dst.txt");
    copy_path(&src, &dst).unwrap();
    assert!(!dst.exists());
}

#[test]
fn is_blacklisted_matches() {
    assert!(is_blacklisted(Path::new("Calligraphy.sip")));
    assert!(is_blacklisted(Path::new("mu_cdata.sip")));
    assert!(is_blacklisted(Path::new("/some/dir/Calligraphy.sip")));
    assert!(!is_blacklisted(Path::new("Config.ini")));
}

#[test]
fn get_backups_dir_creates_directory() {
    let (_tmp, server_exe) = empty_server();
    let path = get_backups_dir(server_exe).unwrap();
    assert!(Path::new(&path).is_dir());
}

#[test]
fn candidate_dates_are_today_then_yesterday_in_utc() {
    use chrono::TimeZone;
    // (y, m, d, h, min, expected). Times either side of the old 07:15 cutoff must agree.
    let cases = [
        (2026, 10, 28, 7, 55, ["20261028", "20261027"]),
        (2026, 10, 28, 7, 14, ["20261028", "20261027"]),
        (2026, 10, 28, 23, 59, ["20261028", "20261027"]),
        (2026, 3, 1, 0, 30, ["20260301", "20260228"]),
        (2026, 1, 1, 0, 30, ["20260101", "20251231"]),
    ];
    for (y, mo, d, h, mi, expected) in cases {
        let now = Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
        assert_eq!(candidate_dates(now), expected, "at {now}");
    }
}
