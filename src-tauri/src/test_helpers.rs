// Shared helpers for unit tests. Compiled only under `cargo test`.
//
// Every path-derivation function under test (server_dir, live_tuning_dir,
// mtxstore_dir, db_path, patches_dir, sip_path_for_server_exe) only ever calls
// `.parent()` on the `server_exe` string — none of them touch the exe file
// itself — so tests fabricate a `server_exe` path that just needs to *exist
// syntactically* under a temp dir, not point at a real executable.

use std::path::{Path, PathBuf};

/// Resolves a path under `tests/fixtures/`, relative to the workspace root
/// (one level above `src-tauri`).
pub(crate) fn fixture_path(relative: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.push("tests");
    p.push("fixtures");
    p.push(relative);
    p
}

fn copy_recursive(src: &Path, dst: &Path) {
    if src.is_dir() {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            copy_recursive(&entry.path(), &dst.join(entry.file_name()));
        }
    } else {
        std::fs::copy(src, dst).unwrap();
    }
}

/// Creates a fresh temp "server directory", copies the fixture at
/// `fixture_relative` (file or directory, under tests/fixtures/) to
/// `dest_relative` inside it, and returns (tempdir handle, fake server_exe
/// path). The tempdir handle must be kept alive for the duration of the test.
pub(crate) fn temp_server(fixture_relative: &str, dest_relative: &str) -> (tempfile::TempDir, String) {
    let (tmp, server_exe) = empty_server();
    let src = fixture_path(fixture_relative);
    let dst = tmp.path().join(dest_relative);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    copy_recursive(&src, &dst);
    (tmp, server_exe)
}

/// A fresh temp "server directory" with nothing in it yet.
pub(crate) fn empty_server() -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().unwrap();
    let server_exe = tmp.path().join("MHServerEmu.exe").to_string_lossy().to_string();
    (tmp, server_exe)
}

/// Creates a minimal Account.db at `path` with the schema accounts.rs and
/// server.rs read/write, seeded with 3 accounts (admin/banned/whitelisted).
/// Account 1 (admin) additionally has Player/Avatar/Item rows so Replace-mode
/// tests have real data to delete and recreate.
///
/// Built from raw DDL rather than shipping a binary fixture: the real
/// Account.db schema isn't defined anywhere in this crate (it's created by
/// MHServerEmu), so the columns below are exactly — and only — the ones
/// accounts.rs and server.rs actually read or write.
pub(crate) fn seed_account_db(path: &std::path::Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "
        CREATE TABLE Account (
            Id INTEGER PRIMARY KEY,
            Email TEXT NOT NULL,
            PlayerName TEXT NOT NULL,
            PasswordHash BLOB NOT NULL,
            Salt BLOB NOT NULL,
            UserLevel INTEGER NOT NULL,
            Flags INTEGER NOT NULL
        );
        CREATE TABLE Player (
            DbGuid INTEGER PRIMARY KEY,
            ArchiveData BLOB NOT NULL,
            StartTarget INTEGER NOT NULL,
            AOIVolume INTEGER NOT NULL,
            GazillioniteBalance INTEGER NOT NULL,
            LastLogoutTime INTEGER NOT NULL
        );
        CREATE TABLE Avatar (
            DbGuid INTEGER PRIMARY KEY,
            ContainerDbGuid INTEGER NOT NULL,
            InventoryProtoGuid INTEGER NOT NULL,
            Slot INTEGER NOT NULL,
            EntityProtoGuid INTEGER NOT NULL,
            ArchiveData BLOB NOT NULL
        );
        CREATE TABLE TeamUp (
            DbGuid INTEGER PRIMARY KEY,
            ContainerDbGuid INTEGER NOT NULL,
            InventoryProtoGuid INTEGER NOT NULL,
            Slot INTEGER NOT NULL,
            EntityProtoGuid INTEGER NOT NULL,
            ArchiveData BLOB NOT NULL
        );
        CREATE TABLE Item (
            DbGuid INTEGER PRIMARY KEY,
            ContainerDbGuid INTEGER NOT NULL,
            InventoryProtoGuid INTEGER NOT NULL,
            Slot INTEGER NOT NULL,
            EntityProtoGuid INTEGER NOT NULL,
            ArchiveData BLOB NOT NULL
        );
        CREATE TABLE ControlledEntity (
            DbGuid INTEGER PRIMARY KEY,
            ContainerDbGuid INTEGER NOT NULL,
            InventoryProtoGuid INTEGER NOT NULL,
            Slot INTEGER NOT NULL,
            EntityProtoGuid INTEGER NOT NULL,
            ArchiveData BLOB NOT NULL
        );

        INSERT INTO Account (Id, Email, PlayerName, PasswordHash, Salt, UserLevel, Flags)
        VALUES
            (1, 'admin@test.local', 'AdminPlayer', X'00', X'00', 2, 0),
            (2, 'user@test.local', 'RegularUser', X'00', X'00', 0, 2),
            (3, 'whitelist@test.local', 'WhitelistUser', X'00', X'00', 0, 16);

        INSERT INTO Player (DbGuid, ArchiveData, StartTarget, AOIVolume, GazillioniteBalance, LastLogoutTime)
        VALUES (1, X'AABB', 12345, 2600, 1000, 1700000000);

        INSERT INTO Avatar (DbGuid, ContainerDbGuid, InventoryProtoGuid, Slot, EntityProtoGuid, ArchiveData)
        VALUES (101, 1, 5001, 0, 9001, X'CCDD');

        INSERT INTO Item (DbGuid, ContainerDbGuid, InventoryProtoGuid, Slot, EntityProtoGuid, ArchiveData)
        VALUES
            (201, 101, 5002, 0, 9002, X'EEFF'),
            (202, 1, 5003, 1, 9003, X'1122');
        ",
    )
    .unwrap();
}
