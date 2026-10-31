use super::*;
use crate::test_helpers::{empty_server, fixture_path, seed_account_db};
use rusqlite::Connection;
use std::fs;

// import_account itself takes `tauri::State<ServerState>`, which can't be
// constructed outside a running Tauri app — tauri::State's inner field is
// private with no public constructor. So these tests call the same
// do_add_with_overrides/do_replace functions import_account delegates to,
// directly against a real seeded Account.db, bypassing only the
// server-running guard (which has no DB-logic risk of its own).

fn seeded_db() -> (tempfile::TempDir, String, Connection) {
    let (tmp, server_exe) = empty_server();
    let path = db_path(&server_exe).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    seed_account_db(&path);
    let conn = open_db(&server_exe).unwrap();
    (tmp, server_exe, conn)
}

fn no_overrides() -> ImportOverrides {
    ImportOverrides { email: None, player_name: None, new_password: None }
}

fn sample_data() -> ImportJson {
    read_and_parse(fixture_path("import/sample_account_export.json").to_str().unwrap()).unwrap()
}

#[test]
fn parse_import_json_summary() {
    let path = fixture_path("import/sample_account_export.json").to_string_lossy().to_string();
    let summary = parse_import_json(path).unwrap();
    assert_eq!(summary.player_name, "ImportedPlayer");
    assert_eq!(summary.email, "imported@test.local");
    assert_eq!(summary.avatar_count, 1);
    assert_eq!(summary.team_up_count, 1);
    assert_eq!(summary.item_count, 2);
    assert_eq!(summary.controlled_entity_count, 1);
    assert_eq!(summary.user_level, 0);
    assert_eq!(summary.flags, 0);
}

#[test]
fn scan_download_backups_matches_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let download = tmp.path().join("Download");
    fs::create_dir_all(&download).unwrap();
    fs::write(download.join("0x1A2B3C_PlayerOne_2026-01-15_10.30.00.json"), "{}").unwrap();
    fs::write(download.join("not_a_match.txt"), "").unwrap();
    fs::write(download.join("randomfile.json"), "{}").unwrap();

    let game_exe = tmp.path().join("MarvelHeroesOmega.exe").to_string_lossy().to_string();
    let backups = scan_download_backups(game_exe).unwrap();

    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].player_name, "PlayerOne");
    assert_eq!(backups[0].account_id_hex, "0x1A2B3C");
    assert_eq!(backups[0].modified.as_deref(), Some("2026-01-15T10:30:00Z"));
}

#[test]
fn scan_download_backups_returns_nothing_when_there_is_nothing_to_scan() {
    assert_eq!(scan_download_backups(String::new()).unwrap().len(), 0, "blank game exe");

    let tmp = tempfile::tempdir().unwrap();
    let game_exe = tmp.path().join("MarvelHeroesOmega.exe").to_string_lossy().to_string();
    assert_eq!(scan_download_backups(game_exe.clone()).unwrap().len(), 0, "missing Download dir");

    fs::create_dir_all(tmp.path().join("Download")).unwrap();
    assert_eq!(scan_download_backups(game_exe).unwrap().len(), 0, "empty Download dir");
}

#[test]
fn list_accounts_for_import_returns_seeded_rows() {
    let (_tmp, server_exe, _conn) = seeded_db();
    let accounts = list_accounts_for_import(server_exe).unwrap();
    assert_eq!(accounts.len(), 3);
    assert!(accounts.iter().any(|a| a.email == "admin@test.local" && a.id == "1"));
}

#[test]
fn blocked_player_names_reads_the_blacklist() {
    let (_tmp, server_exe) = empty_server();
    assert!(blocked_player_names(server_exe.clone()).unwrap().is_empty(), "no file");

    let data = db_path(&server_exe).unwrap().parent().unwrap().to_path_buf();
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("PlayerNameBlacklist.txt"), "\u{FEFF}Admin\r\n\r\n  Moderator  \nGM\n").unwrap();
    assert_eq!(blocked_player_names(server_exe).unwrap(), ["admin", "moderator", "gm"]);
}

#[test]
fn account_exists_table() {
    let (_tmp, server_exe, _conn) = seeded_db();
    let cases = [
        ("admin@test.local", true),
        ("  admin@test.local  ", true),
        ("ADMIN@TEST.LOCAL", true),
        ("nobody@test.local", false),
        ("", false),
    ];
    for (email, expected) in cases {
        assert_eq!(account_exists(server_exe.clone(), email.into()).unwrap(), expected, "{email:?}");
    }
}

#[test]
fn account_exists_errors_until_the_database_exists() {
    let (_tmp, server_exe) = empty_server();
    fs::create_dir_all(db_path(&server_exe).unwrap().parent().unwrap()).unwrap();
    assert!(account_exists(server_exe, "a@b.c".into()).is_err());
}

#[test]
fn account_password_matches_table() {
    let (_tmp, server_exe, conn) = seeded_db();
    let overrides = ImportOverrides {
        email: Some("pw@test.local".into()),
        player_name: Some("PwPlayer".into()),
        new_password: Some("s3cret".into()),
    };
    do_add_with_overrides(&conn, &overrides, &sample_data()).unwrap();

    let cases = [
        ("pw@test.local", "s3cret", true),
        ("PW@test.local", "s3cret", true),
        (" pw@test.local ", "s3cret", true),
        ("pw@test.local", "S3cret", false),
        ("pw@test.local", "", false),
        ("nobody@test.local", "s3cret", false),
        // Seeded rows hold a one-byte placeholder hash; they must not match, and must not error.
        ("admin@test.local", "s3cret", false),
    ];
    for (email, password, expected) in cases {
        let got = account_password_matches(server_exe.clone(), email.into(), password.into()).unwrap();
        assert_eq!(got, expected, "{email:?} / {password:?}");
    }
}

#[test]
fn import_add_creates_account() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_add_with_overrides(&conn, &no_overrides(), &data).unwrap();

    let (email, name): (String, String) = conn
        .query_row("SELECT Email, PlayerName FROM Account WHERE Email = ?1", ["imported@test.local"], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(email, "imported@test.local");
    assert_eq!(name, "ImportedPlayer");
}

#[test]
fn import_add_remaps_ids() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_add_with_overrides(&conn, &no_overrides(), &data).unwrap();

    // None of the original import file's DbGuids (555-560) should appear
    // anywhere in the database — every entity got a freshly allocated id.
    for table in ["Player", "Avatar", "TeamUp", "Item", "ControlledEntity"] {
        let count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE DbGuid IN (555,556,557,558,559,560)"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "found an unmapped original DbGuid in {table}");
    }
}

#[test]
fn import_add_rewrites_containers() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_add_with_overrides(&conn, &no_overrides(), &data).unwrap();

    let new_account_id: i64 = conn
        .query_row("SELECT Id FROM Account WHERE Email = ?1", ["imported@test.local"], |r| r.get(0))
        .unwrap();
    let new_avatar_id: i64 = conn
        .query_row("SELECT DbGuid FROM Avatar WHERE ContainerDbGuid = ?1", [new_account_id], |r| r.get(0))
        .unwrap();

    // Original Items: one contained by the avatar (556), one directly by the player (555).
    let container_ids: Vec<i64> = conn
        .prepare("SELECT ContainerDbGuid FROM Item WHERE ContainerDbGuid IN (?1, ?2) ORDER BY ContainerDbGuid")
        .unwrap()
        .query_map([new_avatar_id, new_account_id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(container_ids, vec![new_account_id.min(new_avatar_id), new_account_id.max(new_avatar_id)]);
}

#[test]
fn import_add_conflict_email() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let mut data = sample_data();
    data.email = "admin@test.local".to_string(); // already used by seeded account 1
    let err = do_add_with_overrides(&conn, &no_overrides(), &data).unwrap_err();
    assert!(err.starts_with("EMAIL_CONFLICT:"));
}

#[test]
fn import_add_conflict_name() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let mut data = sample_data();
    data.player_name = "AdminPlayer".to_string(); // already used by seeded account 1
    let err = do_add_with_overrides(&conn, &no_overrides(), &data).unwrap_err();
    assert!(err.starts_with("NAME_CONFLICT:"));
}

#[test]
fn import_add_override_email() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    let overrides = ImportOverrides {
        email: Some("custom@test.local".to_string()),
        player_name: None,
        new_password: None,
    };
    do_add_with_overrides(&conn, &overrides, &data).unwrap();

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM Account WHERE Email = ?1", ["custom@test.local"], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let orig_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM Account WHERE Email = ?1", ["imported@test.local"], |r| r.get(0))
        .unwrap();
    assert_eq!(orig_count, 0);
}

#[test]
fn import_replace_preserves_identity() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_replace(&conn, 1, &data).unwrap();

    let (email, name, flags, user_level): (String, String, i32, i32) = conn
        .query_row("SELECT Email, PlayerName, Flags, UserLevel FROM Account WHERE Id = 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .unwrap();
    assert_eq!(email, "admin@test.local");
    assert_eq!(name, "AdminPlayer");
    assert_eq!(flags, 0);
    assert_eq!(user_level, 2);
}

#[test]
fn import_replace_deletes_old_data() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_replace(&conn, 1, &data).unwrap();

    // Old avatar (101) and its items (201, 202) must be gone, not orphaned.
    let old_avatar: i64 = conn.query_row("SELECT COUNT(*) FROM Avatar WHERE DbGuid = 101", [], |r| r.get(0)).unwrap();
    assert_eq!(old_avatar, 0);
    let orphaned_items: i64 = conn
        .query_row("SELECT COUNT(*) FROM Item WHERE ContainerDbGuid = 101", [], |r| r.get(0))
        .unwrap();
    assert_eq!(orphaned_items, 0);

    // New data from the import file replaced it: 1 avatar, 2 items.
    let new_avatar_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM Avatar WHERE ContainerDbGuid = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(new_avatar_count, 1);
}

#[test]
fn import_replace_cascade_with_fk_off() {
    // Regression test for M-8: with PRAGMA foreign_keys=OFF, ON DELETE CASCADE
    // never fires, so every child table must be cleared explicitly by
    // do_replace's delete statements. If any of them were dropped, this
    // leaves an orphan row rather than erroring — check directly.
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    do_replace(&conn, 1, &data).unwrap();

    let orphan_items: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM Item WHERE ContainerDbGuid NOT IN (SELECT DbGuid FROM Player)
                                             AND ContainerDbGuid NOT IN (SELECT DbGuid FROM Avatar)
                                             AND ContainerDbGuid NOT IN (SELECT DbGuid FROM TeamUp)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphan_items, 0);
}

#[test]
fn import_replace_target_not_found() {
    let (_tmp, _server_exe, conn) = seeded_db();
    let data = sample_data();
    let err = do_replace(&conn, 9999, &data).unwrap_err();
    assert_eq!(err, "Target account not found.");
}

// restore_json_account's running-server guard needs tauri::State, so these call
// restore_json_save, which it delegates to.

/// An empty server with Data/ and the sample export saved as a backup whose Id
/// is the built-in JSON account's (above JS's safe integer range).
fn json_server() -> (tempfile::TempDir, String, String, serde_json::Value) {
    let (tmp, server_exe) = empty_server();
    fs::create_dir_all(tmp.path().join("Data")).unwrap();
    let mut backup: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path("import/sample_account_export.json")).unwrap()).unwrap();
    backup["Id"] = 0x2000000000000001u64.into();
    let backup_path = tmp.path().join("backup.json").to_string_lossy().to_string();
    fs::write(&backup_path, backup.to_string()).unwrap();
    (tmp, server_exe, backup_path, backup)
}

fn multiverse_copies(tmp: &tempfile::TempDir) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(tmp.path().join("Data"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains(".multiverse-"))
        .collect();
    names.sort();
    names
}

#[test]
fn restore_json_writes_the_backup_as_admin_and_sets_the_player_name() {
    let (tmp, server_exe, backup_path, mut backup) = json_server();
    assert_eq!(restore_json_save(&server_exe, &backup_path).unwrap(), None, "no previous save to copy");

    let raw = fs::read_to_string(tmp.path().join("Data/DefaultAccount.json")).unwrap();
    assert!(raw.contains("2305843009213693953"), "Id must stay exact: {raw}");
    backup["UserLevel"] = 2.into();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&raw).unwrap(), backup);
    assert_eq!(crate::ini::read_merged_value(&server_exe, "JsonDBManager", "PlayerName", ""), "ImportedPlayer");
}

#[test]
fn restore_json_honours_the_filename_override() {
    let (tmp, server_exe, backup_path, _) = json_server();
    fs::write(tmp.path().join("ConfigOverride.ini"), "[JsonDBManager]\nFileName=Mine.json\n").unwrap();
    restore_json_save(&server_exe, &backup_path).unwrap();
    assert!(tmp.path().join("Data/Mine.json").exists());
    assert!(!tmp.path().join("Data/DefaultAccount.json").exists());
}

#[test]
fn restore_json_copies_the_previous_save_and_keeps_max_backup_number_copies() {
    // (older copies already there, MaxBackupNumber, copies left afterwards)
    let cases = [(0, None, 1), (6, None, 5), (3, Some("2"), 2), (2, Some("0"), 1), (2, Some("x"), 3)];
    for (older, max, left) in cases {
        let (tmp, server_exe, backup_path, _) = json_server();
        let data = tmp.path().join("Data");
        fs::write(data.join("DefaultAccount.json"), "old save").unwrap();
        for i in 0..older {
            fs::write(data.join(format!("DefaultAccount.json.multiverse-20200101-00000{i}.bak")), "").unwrap();
        }
        fs::write(data.join("DefaultAccount.json.bak0"), "server's own").unwrap();
        if let Some(m) = max {
            fs::write(tmp.path().join("ConfigOverride.ini"), format!("[JsonDBManager]\nMaxBackupNumber={m}\n")).unwrap();
        }

        let copy = restore_json_save(&server_exe, &backup_path).unwrap().unwrap();
        let copies = multiverse_copies(&tmp);
        assert_eq!(copies.len(), left, "{older} older, max {max:?}: {copies:?}");
        assert_eq!(copies.last(), Some(&copy), "the new copy must be kept");
        assert_eq!(fs::read_to_string(data.join(&copy)).unwrap(), "old save");
        assert!(data.join("DefaultAccount.json.bak0").exists(), "server backups are not ours to prune");
    }
}

#[test]
fn restore_json_rejects_filenames_outside_data() {
    for bad in ["", "..", "sub/Save.json", "../Save.json", "C:/Save.json", "/Save.json", r"\\host\share\Save.json"] {
        let (tmp, server_exe, backup_path, _) = json_server();
        fs::write(tmp.path().join("ConfigOverride.ini"), format!("[JsonDBManager]\nFileName={bad}\n")).unwrap();
        let err = restore_json_save(&server_exe, &backup_path).unwrap_err();
        assert!(err.contains("FileName"), "{bad:?} -> {err}");
    }
}

#[test]
fn restore_json_rejects_bad_backups_without_touching_the_save() {
    let mut control_name: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture_path("import/sample_account_export.json")).unwrap()).unwrap();
    control_name["PlayerName"] = "Name\n[PlayerManager]".into();
    for bad in ["not json".to_string(), "{}".to_string(), r#"{"PlayerName":"x"}"#.to_string(), control_name.to_string()] {
        let (tmp, server_exe, backup_path, _) = json_server();
        fs::write(&backup_path, &bad).unwrap();
        let save = tmp.path().join("Data/DefaultAccount.json");
        fs::write(&save, "old save").unwrap();

        assert!(restore_json_save(&server_exe, &backup_path).is_err(), "{bad}");
        assert_eq!(fs::read_to_string(&save).unwrap(), "old save");
        assert!(multiverse_copies(&tmp).is_empty(), "{bad}");
        assert!(!tmp.path().join("ConfigOverride.ini").exists(), "{bad}");
    }
}

#[test]
fn hash_password_is_salted() {
    // Same password twice: a fresh random salt each time, so neither the salt
    // nor the resulting hash may repeat.
    let (hash_a, salt_a) = hash_password("same-password");
    let (hash_b, salt_b) = hash_password("same-password");
    assert_ne!(salt_a, salt_b);
    assert_ne!(hash_a, hash_b);
}

#[test]
fn open_db_missing_file_errors_without_creating_it() {
    let (_tmp, server_exe) = empty_server();
    let path = db_path(&server_exe).unwrap();
    // Data/ ships with MHServerEmu; only the DB is absent before first start.
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let err = open_db(&server_exe).unwrap_err();
    assert!(err.contains("Account.db not found."), "unexpected error: {err}");
    assert!(!path.exists(), "open_db must not create Account.db");
}

#[test]
fn db_path_defaults_to_account_db() {
    let (_tmp, server_exe) = empty_server();
    let p = db_path(&server_exe).unwrap();
    assert!(p.ends_with("Data/Account.db"), "{p:?}");
}

#[test]
fn db_path_honours_sqlite_filename_override() {
    let (tmp, server_exe) = empty_server();
    fs::write(
        tmp.path().join("ConfigOverride.ini"),
        "[SQLiteDBManager]\nFileName=Custom.db\n",
    )
    .unwrap();
    let p = db_path(&server_exe).unwrap();
    assert!(p.ends_with("Data/Custom.db"), "{p:?}");
}

fn db_path_with_filename(filename: &str) -> Result<PathBuf, String> {
    let (tmp, server_exe) = empty_server();
    fs::write(
        tmp.path().join("ConfigOverride.ini"),
        format!("[SQLiteDBManager]\nFileName={filename}\n"),
    )
    .unwrap();
    db_path(&server_exe)
}

#[test]
fn db_path_rejects_filenames_that_escape_data_dir() {
    for bad in [
        "",
        "..",
        "../Other.db",
        "sub/../../Other.db",
        "C:/Other.db",
        "C:Other.db",
        "/Other.db",
        r"\\host\share\Other.db",
    ] {
        let err = db_path_with_filename(bad).unwrap_err();
        assert!(err.contains("FileName"), "{bad:?} -> {err}");
    }
}

#[test]
fn db_path_allows_plain_and_subfolder_filenames() {
    assert!(db_path_with_filename("Other.db").unwrap().ends_with("Data/Other.db"));
    assert!(db_path_with_filename("Backup/Other.db").unwrap().ends_with("Data/Backup/Other.db"));
}

#[test]
fn open_db_missing_file_error_names_the_configured_file() {
    let (tmp, server_exe) = empty_server();
    fs::write(
        tmp.path().join("ConfigOverride.ini"),
        "[SQLiteDBManager]\nFileName=Custom.db\n",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("Data")).unwrap();
    let err = open_db(&server_exe).unwrap_err();
    assert!(err.contains("Custom.db not found."), "unexpected error: {err}");
}
