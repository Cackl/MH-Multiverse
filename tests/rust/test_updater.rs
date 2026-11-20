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

#[test]
fn nightly_url_is_the_windows_build_for_that_flavour_and_date() {
    assert_eq!(
        build_download_url("v48", "20261117"),
        "https://nightly.link/Crypto137/MHServerEmu/workflows/nightly-release-windows-v48/master/MHServerEmu-nightly-20261117-Release-windows-v48.zip"
    );
}

#[test]
fn nightly_flavour_follows_the_server_version_and_is_off_for_custom_and_unknown() {
    for (version, custom, want) in [
        (Some("1.48"), false, Some("v48")),
        (Some("1.52"), false, Some("v52")),
        (Some("1.53"), false, Some("v53")),
        (Some("1.52"), true, None),
        (Some("1.48"), true, None),
        (None, false, None),
    ] {
        assert_eq!(nightly_flavour(version, custom).ok(), want, "{version:?} custom={custom}");
    }
}

#[test]
fn build_date_comes_from_the_nightly_stamp() {
    let cases = [
        ("1.0.3+build20261027131155", Some("20261027")),
        ("1.0.2+build20260901123337", Some("20260901")),
        ("1.0.2+build20260901", Some("20260901")),
        ("1.0.2", None),
        ("", None),
        ("1.0.2+build", None),
        ("1.0.2+build2026", None),
        ("1.0.2+buildabcdefgh", None),
        ("1.0.2+g1a2b3c4d", None),
    ];
    for (version, expected) in cases {
        assert_eq!(build_date_from_version(version).as_deref(), expected, "for {version:?}");
    }
}

#[test]
fn up_to_date_needs_an_installed_build_at_least_as_new() {
    let cases = [
        (Some("20261027"), "20261027", true),
        (Some("20261028"), "20261027", true),
        (Some("20261026"), "20261027", false),
        (Some("20251231"), "20260101", false),
        // no stamp (e.g. built from source): never blocks an update
        (None, "20261027", false),
    ];
    for (installed, build_date, expected) in cases {
        assert_eq!(is_up_to_date(installed, build_date), expected, "{installed:?} vs {build_date}");
    }
}

#[test]
fn only_one_update_can_run_at_a_time() {
    let first = UpdateLock::acquire().expect("lock is free");
    assert!(UpdateLock::acquire().is_none(), "a second run while the first holds the lock");
    drop(first);
    assert!(UpdateLock::acquire().is_some(), "free again once the first run ends");
}

#[test]
fn update_keeps_switched_stock_files_as_the_user_had_them() {
    const P: &str = "Data/Game/Patches";
    const LT: &str = "Data/Game/LiveTuning";
    const MTX: &str = "Data/Game/MTXStore";
    let bug_off_pair = || vec![
        (format!("{P}/PatchDataBugFixes.json"), "new"),
        (format!("{P}/Off/PatchDataBugFixes.json"), "mine"),
    ];
    let carnival_on_pair = || vec![
        (format!("{P}/PatchDataCarnival.json"), "mine"),
        (format!("{P}/Off/PatchDataCarnival.json"), "new"),
    ];
    // (case, files the update ships, server files after install and restore, restored targets,
    //  files kept, server files afterwards where None means gone)
    type Case = (&'static str, Vec<String>, Vec<(String, &'static str)>, Vec<&'static str>, Vec<(&'static str, bool)>, Vec<(String, Option<&'static str>)>);
    let cases: Vec<Case> = vec![
        (
            "switched off, the update adds an on copy",
            vec![format!("{P}/PatchDataBugFixes.json")],
            bug_off_pair(),
            vec![],
            vec![("PatchDataBugFixes.json", false)],
            vec![
                (format!("{P}/PatchDataBugFixes.json"), None),
                (format!("{P}/Off/PatchDataBugFixes.json"), Some("new")),
                (format!("{P}/Off/PatchDataBugFixes.json.bak"), Some("mine")),
            ],
        ),
        (
            "a restore of another folder changes nothing",
            vec![format!("{P}/PatchDataBugFixes.json")],
            bug_off_pair(),
            vec![MTX, LT],
            vec![("PatchDataBugFixes.json", false)],
            vec![
                (format!("{P}/PatchDataBugFixes.json"), None),
                (format!("{P}/Off/PatchDataBugFixes.json"), Some("new")),
                (format!("{P}/Off/PatchDataBugFixes.json.bak"), Some("mine")),
            ],
        ),
        (
            "switched a stock-off pack on, the update adds an off copy",
            vec![format!("{P}/Off/PatchDataCarnival.json")],
            carnival_on_pair(),
            vec![],
            vec![("PatchDataCarnival.json", true)],
            vec![
                (format!("{P}/PatchDataCarnival.json"), Some("new")),
                (format!("{P}/PatchDataCarnival.json.bak"), Some("mine")),
                (format!("{P}/Off/PatchDataCarnival.json"), None),
            ],
        ),
        (
            "Live Tuning keeps its OFF_ copy",
            vec![format!("{LT}/LiveTuningDataPvP.json")],
            vec![(format!("{LT}/LiveTuningDataPvP.json"), "new"), (format!("{LT}/OFF_LiveTuningDataPvP.json"), "mine")],
            vec![],
            vec![("LiveTuningDataPvP.json", false)],
            vec![
                (format!("{LT}/LiveTuningDataPvP.json"), None),
                (format!("{LT}/OFF_LiveTuningDataPvP.json"), Some("new")),
                (format!("{LT}/OFF_LiveTuningDataPvP.json.bak"), Some("mine")),
            ],
        ),
        (
            "a restored folder keeps the restored copy",
            vec![format!("{P}/PatchDataBugFixes.json")],
            bug_off_pair(),
            vec![P],
            vec![("PatchDataBugFixes.json", false)],
            vec![
                (format!("{P}/PatchDataBugFixes.json"), None),
                (format!("{P}/PatchDataBugFixes.json.bak"), Some("new")),
                (format!("{P}/Off/PatchDataBugFixes.json"), Some("mine")),
                (format!("{P}/Off/PatchDataBugFixes.json.bak"), None),
            ],
        ),
        (
            "a full Data restore counts as restored",
            vec![format!("{P}/Off/PatchDataCarnival.json")],
            carnival_on_pair(),
            vec!["Data"],
            vec![("PatchDataCarnival.json", true)],
            vec![
                (format!("{P}/PatchDataCarnival.json"), Some("mine")),
                (format!("{P}/Off/PatchDataCarnival.json"), None),
                (format!("{P}/Off/PatchDataCarnival.json.bak"), Some("new")),
            ],
        ),
        (
            "a store catalog leaves its MODIFIED file in Off",
            vec![format!("{MTX}/CatalogBundles.json")],
            vec![
                (format!("{MTX}/CatalogBundles.json"), "new"),
                (format!("{MTX}/Off/CatalogBundles.json"), "mine"),
                (format!("{MTX}/Off/CatalogBundlesMODIFIED.json"), "edits"),
            ],
            vec![],
            vec![("CatalogBundles.json", false)],
            vec![
                (format!("{MTX}/CatalogBundles.json"), None),
                (format!("{MTX}/Off/CatalogBundles.json"), Some("new")),
                (format!("{MTX}/Off/CatalogBundlesMODIFIED.json"), Some("edits")),
            ],
        ),
        (
            "untouched: one copy, files the update doesn't ship, files the app can't switch",
            vec![
                format!("{P}/PatchDataMissions.json"),
                format!("{LT}/Events/LiveTuningDataCosmic.json"),
                format!("{LT}/Events.json"),
            ],
            vec![
                (format!("{P}/PatchDataMissions.json"), "new"),
                (format!("{P}/PatchDataMine.json"), "mine"),
                (format!("{P}/Off/PatchDataMine.json"), "mine too"),
                (format!("{LT}/Events/LiveTuningDataCosmic.json"), "new"),
                (format!("{LT}/Events/OFF_LiveTuningDataCosmic.json"), "mine"),
                (format!("{LT}/Events.json"), "new"),
                (format!("{LT}/OFF_Events.json"), "mine"),
            ],
            vec![],
            vec![],
            vec![
                (format!("{P}/PatchDataMissions.json"), Some("new")),
                (format!("{P}/PatchDataMine.json"), Some("mine")),
                (format!("{P}/Off/PatchDataMine.json"), Some("mine too")),
                (format!("{LT}/Events/LiveTuningDataCosmic.json"), Some("new")),
                (format!("{LT}/Events/OFF_LiveTuningDataCosmic.json"), Some("mine")),
                (format!("{LT}/Events.json"), Some("new")),
                (format!("{LT}/OFF_Events.json"), Some("mine")),
            ],
        ),
    ];
    let write = |root: &Path, rel: &str, text: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    for (case, ships, server_files, restored, kept, after) in cases {
        let (source, server) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        ships.iter().for_each(|rel| write(source.path(), rel, "new"));
        server_files.iter().for_each(|(rel, text)| write(server.path(), rel, text));
        let restored: Vec<String> = restored.into_iter().map(String::from).collect();

        let got = keep_switched_files(server.path(), source.path(), &restored).unwrap();

        let got_pairs: Vec<(String, bool)> = got.iter().map(|k| (k.file.clone(), k.enabled)).collect();
        let want: Vec<(String, bool)> = kept.into_iter().map(|(file, enabled)| (file.into(), enabled)).collect();
        assert_eq!(got_pairs, want, "{case}: kept");
        // `X.json.bak` in a case stands for the timestamped copy `move_aside` made of X.json.
        for (rel, text) in after {
            let path = match rel.strip_suffix(".bak") {
                Some(orig) => crate::test_helpers::moved_aside(&server.path().join(orig)),
                None => Some(server.path().join(&rel)),
            };
            if let (Some(p), Some(_)) = (&path, text) {
                if rel.ends_with(".bak") {
                    assert!(got.iter().any(|k| k.kept_as == crate::store::file_name_of(p)), "{case}: kept_as names {rel}");
                }
            }
            assert_eq!(path.and_then(|p| fs::read_to_string(p).ok()).as_deref(), text, "{case}: {rel}");
        }
    }
}

#[test]
fn missing_or_unversioned_exe_has_no_build_date() {
    let (_tmp, server_exe) = empty_server();
    assert_eq!(installed_build_date(&server_exe), None);
    assert_eq!(installed_build_date("C:/definitely/not/here/MHServerEmu.exe"), None);
}
