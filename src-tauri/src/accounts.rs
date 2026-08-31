// ── accounts.rs ────────────────────────────────────────────────────────────
//
// Account import: parse a !account download JSON export, remap all IDs to
// avoid collisions, and write the data into Account.db.
//
// NOTE: `import_account` calls `crate::server::server_process_is_running`.
// Add the following function to server.rs (see CHANGES.md for context):
//
//   pub fn server_process_is_running(state: &crate::server::ServerState) -> bool {
//       state.0.lock()
//           .map(|mut proc| {
//               proc.child
//                   .as_mut()
//                   .map(|child: &mut std::process::Child| child.try_wait().ok().flatten().is_none())
//                   .unwrap_or(false)
//           })
//           .unwrap_or(false)
//   }
//
// If you prefer not to add this function, remove the server-running guard
// in import_account and rely solely on the frontend's $serverRunning check.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use regex::Regex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use tauri::State;

use crate::server::ServerState;

// ── JSON import types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ImportJson {
    id: i64,
    email: String,
    player_name: String,
    password_hash: String,
    salt: String,
    user_level: u8,
    flags: i32,
    player: ImportPlayer,
    avatars: Vec<ImportEntity>,
    team_ups: Vec<ImportEntity>,
    items: Vec<ImportEntity>,
    controlled_entities: Vec<ImportEntity>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ImportPlayer {
    db_guid: i64,
    archive_data: String,
    start_target: i64,
    #[serde(rename = "AOIVolume")]
    aoi_volume: i32,
    gazillionite_balance: i64,
    last_logout_time: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ImportEntity {
    db_guid: i64,
    container_db_guid: i64,
    inventory_proto_guid: i64,
    slot: u32,
    entity_proto_guid: i64,
    archive_data: String,
}

// ── Public types ───────────────────────────────────────────────────────────

/// Summary returned to the frontend after parsing the import file.
#[derive(Serialize, Clone)]
pub struct ImportSummary {
    pub player_name: String,
    pub email: String,
    pub avatar_count: usize,
    pub team_up_count: usize,
    pub item_count: usize,
    pub controlled_entity_count: usize,
    pub user_level: u8,
    pub flags: i32,
}

/// A single account entry, returned by the replace picker.
#[derive(Serialize)]
pub struct AccountEntry {
    pub id: i64,
    pub player_name: String,
    pub email: String,
}

/// A single backup file discovered in the game's Download/ folder.
#[derive(Serialize)]
pub struct BackupFileEntry {
    pub path: String,
    pub file_name: String,
    pub player_name: String,
    pub account_id_hex: String,
    /// ISO 8601 UTC timestamp parsed from the filename. Always Some in
    /// practice since the regex match already requires the date group;
    /// kept optional so a future looser pattern can't panic here.
    pub modified: Option<String>,
}

/// Frontend-supplied overrides applied on top of the imported JSON values.
#[derive(Deserialize)]
pub struct ImportOverrides {
    pub email: Option<String>,
    pub player_name: Option<String>,
    /// If Some and non-empty the password is re-hashed; otherwise the
    /// original PasswordHash / Salt blobs from the JSON are kept as-is.
    pub new_password: Option<String>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn db_path(server_exe: &str) -> PathBuf {
    Path::new(server_exe)
        .parent()
        .unwrap_or(Path::new("."))
        .join("Data")
        .join("Account.db")
}

fn open_db(server_exe: &str) -> Result<Connection, String> {
    let conn = Connection::open(db_path(server_exe))
        .map_err(|e| format!("Failed to open database: {e}"))?;
    // The Item table has three FOREIGN KEY constraints on ContainerDbGuid
    // (Player, Avatar, TeamUp). SQLite checks all of them independently,
    // so a single ContainerDbGuid value can never satisfy all three at once.
    // MHServerEmu does not enable FK enforcement; we match that behaviour.
    // Referential integrity is maintained by the ID remap logic in this module.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(|e| format!("Failed to configure database: {e}"))?;
    Ok(conn)
}

fn read_and_parse(json_path: &str) -> Result<ImportJson, String> {
    let raw = fs::read_to_string(json_path)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    // Strip UTF-8 BOM if present.
    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    serde_json::from_str(content).map_err(|e| format!("Failed to parse JSON: {e}"))
}

/// PBKDF2-HMAC-SHA512, 210 000 iterations, 64-byte key and salt.
/// Matches CryptographyHelper.HashPassword in MHServerEmu.
fn hash_password(password: &str) -> (Vec<u8>, Vec<u8>) {
    let mut salt = vec![0u8; 64];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut hash = vec![0u8; 64];
    pbkdf2_hmac::<Sha512>(password.as_bytes(), &salt, 210_000, &mut hash);
    (hash, salt)
}

/// Returns the highest ID currently stored across all entity tables so that
/// freshly generated IDs cannot collide with existing rows.
fn max_existing_id(conn: &Connection) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "SELECT COALESCE(MAX(id), 0) FROM (
            SELECT Id      AS id FROM Account
            UNION ALL SELECT DbGuid AS id FROM Player
            UNION ALL SELECT DbGuid AS id FROM Avatar
            UNION ALL SELECT DbGuid AS id FROM TeamUp
            UNION ALL SELECT DbGuid AS id FROM Item
            UNION ALL SELECT DbGuid AS id FROM ControlledEntity
        )",
        [],
        |row| row.get::<_, i64>(0),
    )
}

// ── Tauri commands ─────────────────────────────────────────────────────────

/// Parse the import file and return a summary. Does not touch the database.
#[tauri::command]
pub fn parse_import_json(json_path: String) -> Result<ImportSummary, String> {
    let data = read_and_parse(&json_path)?;
    Ok(ImportSummary {
        player_name: data.player_name,
        email: data.email,
        avatar_count: data.avatars.len(),
        team_up_count: data.team_ups.len(),
        item_count: data.items.len(),
        controlled_entity_count: data.controlled_entities.len(),
        user_level: data.user_level,
        flags: data.flags,
    })
}

/// Scan `<game_exe_dir>/Download` for account export files written by the
/// in-game `!account download` command. Filenames follow the pattern
/// `0x{IdHex}_{PlayerName}_{yyyy-MM-dd_HH.mm.ss}.json` (see FileHelper on
/// the MHServerEmu side — time components are dot-separated, not
/// underscore-separated). Non-matching files are silently skipped rather
/// than surfaced as errors. Returns an empty list if game_exe is unset or
/// the Download folder doesn't exist — the frontend falls back to the
/// upload button in either case.
#[tauri::command]
pub fn scan_download_backups(game_exe: String) -> Result<Vec<BackupFileEntry>, String> {
    if game_exe.trim().is_empty() {
        return Ok(Vec::new());
    }

    let dir = match Path::new(&game_exe).parent() {
        Some(p) => p.join("Download"),
        None => return Ok(Vec::new()),
    };

    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let re = Regex::new(r"^(0x[0-9A-Fa-f]+)_(.+)_(\d{4}-\d{2}-\d{2}_\d{2}\.\d{2}\.\d{2})\.json$")
        .map_err(|e| format!("Invalid pattern: {e}"))?;

    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read Download folder: {e}"))?;

    let mut backups: Vec<BackupFileEntry> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(caps) = re.captures(file_name) else {
            continue;
        };

        let date_raw = &caps[3];
        let modified = date_raw
            .split_once('_')
            .map(|(date, time)| format!("{date}T{}Z", time.replace('.', ":")));

        backups.push(BackupFileEntry {
            path: path.to_string_lossy().to_string(),
            file_name: file_name.to_string(),
            player_name: caps[2].to_string(),
            account_id_hex: caps[1].to_string(),
            modified,
        });
    }

    // Newest first.
    backups.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(backups)
}

/// Return every account in the database, ordered by player name.
/// Used for client-side email/player-name conflict checking during Add.
#[tauri::command]
pub fn list_accounts_for_import(server_exe: String) -> Result<Vec<AccountEntry>, String> {
    let conn = open_db(&server_exe)?;
    let mut stmt = conn
        .prepare(
            "SELECT Id, PlayerName, Email \
             FROM Account \
             ORDER BY PlayerName COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AccountEntry {
                id: row.get(0)?,
                player_name: row.get(1)?,
                email: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

/// Perform the account import.
///
/// `mode` is either `"add"` (create a new account) or `"replace"` (overwrite
/// the player data of an existing account identified by `target_id`).
///
/// Conflict errors are returned with a prefix that the frontend can parse:
///   `EMAIL_CONFLICT:...`
///   `NAME_CONFLICT:...`
#[tauri::command]
pub fn import_account(
    server_exe: String,
    json_path: String,
    mode: String,
    target_id: Option<i64>,
    overrides: ImportOverrides,
    server_state: State<'_, ServerState>,
) -> Result<(), String> {
    // Guard: importing while the server is running risks DB lock contention.
    // See the note at the top of this file for the required server.rs addition.
    if crate::server::server_process_is_running(&server_state) {
        return Err("Stop the server before importing an account.".into());
    }

    let data = read_and_parse(&json_path)?;

    // Resolve final values — overrides take precedence over JSON.
    let final_email = overrides
        .email
        .as_deref()
        .unwrap_or(&data.email)
        .trim()
        .to_owned();

    let final_name = overrides
        .player_name
        .as_deref()
        .unwrap_or(&data.player_name)
        .trim()
        .to_owned();

    if final_email.is_empty() {
        return Err("Email cannot be empty.".into());
    }
    if final_name.is_empty() {
        return Err("Player name cannot be empty.".into());
    }

    // Determine the password hash/salt to write.
    let (final_hash, final_salt) = match overrides
        .new_password
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(pw) => hash_password(pw),
        None => {
            let h = BASE64
                .decode(&data.password_hash)
                .map_err(|e| format!("Invalid PasswordHash in import file: {e}"))?;
            let s = BASE64
                .decode(&data.salt)
                .map_err(|e| format!("Invalid Salt in import file: {e}"))?;
            (h, s)
        }
    };

    let mut conn = open_db(&server_exe)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let result = match mode.as_str() {
        "add" => do_add(&tx, &final_email, &final_name, &final_hash, &final_salt, &data),
        "replace" => {
            let target = target_id
                .ok_or_else(|| "A target account must be selected for Replace mode.".to_owned())?;
            do_replace(&tx, target, &final_email, &final_name, &final_hash, &final_salt, &data)
        }
        other => Err(format!("Unknown import mode: {other}")),
    };

    if result.is_ok() {
        tx.commit().map_err(|e| e.to_string())?;
    }
    // On Err the transaction is dropped, triggering an automatic rollback.
    result
}

// ── Import modes ───────────────────────────────────────────────────────────

fn do_add(
    conn: &Connection,
    email: &str,
    player_name: &str,
    password_hash: &[u8],
    salt: &[u8],
    data: &ImportJson,
) -> Result<(), String> {
    // Uniqueness checks.
    if count(
        conn,
        "SELECT COUNT(*) FROM Account WHERE Email = ?1 COLLATE NOCASE",
        email,
    )? > 0
    {
        return Err(format!("EMAIL_CONFLICT:Email '{email}' is already registered."));
    }
    if count(
        conn,
        "SELECT COUNT(*) FROM Account WHERE PlayerName = ?1 COLLATE NOCASE",
        player_name,
    )? > 0
    {
        return Err(format!(
            "NAME_CONFLICT:Player name '{player_name}' is already in use."
        ));
    }

    let base = max_existing_id(conn).map_err(|e| e.to_string())?;
    let new_account_id = base + 1;

    conn.execute(
        "INSERT INTO Account (Id, Email, PlayerName, PasswordHash, Salt, UserLevel, Flags) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            new_account_id,
            email,
            player_name,
            password_hash,
            salt,
            data.user_level as i32,
            data.flags
        ],
    )
    .map_err(|e| format!("Failed to insert account: {e}"))?;

    // Entity IDs begin one above the new account ID.
    insert_player_data(conn, new_account_id, new_account_id + 1, data)
}

fn do_replace(
    conn: &Connection,
    target_id: i64,
    email: &str,
    player_name: &str,
    password_hash: &[u8],
    salt: &[u8],
    data: &ImportJson,
) -> Result<(), String> {
    // Target must exist.
    if count(
        conn,
        "SELECT COUNT(*) FROM Account WHERE Id = ?1",
        target_id,
    )? == 0
    {
        return Err("Target account not found.".into());
    }

    // Uniqueness checks, excluding the target account itself.
    let email_taken: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM Account \
             WHERE Email = ?1 COLLATE NOCASE AND Id != ?2",
            params![email, target_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if email_taken > 0 {
        return Err(format!(
            "EMAIL_CONFLICT:Email '{email}' is already in use by another account."
        ));
    }

    let name_taken: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM Account \
             WHERE PlayerName = ?1 COLLATE NOCASE AND Id != ?2",
            params![player_name, target_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if name_taken > 0 {
        return Err(format!(
            "NAME_CONFLICT:Player name '{player_name}' is already in use by another account."
        ));
    }

    // Update the account record.
    conn.execute(
        "UPDATE Account \
         SET Email = ?1, PlayerName = ?2, PasswordHash = ?3, Salt = ?4, \
             UserLevel = ?5, Flags = ?6 \
         WHERE Id = ?7",
        params![
            email,
            player_name,
            password_hash,
            salt,
            data.user_level as i32,
            data.flags,
            target_id
        ],
    )
    .map_err(|e| format!("Failed to update account: {e}"))?;

    // Delete existing player data. The ON DELETE CASCADE on Player.DbGuid
    // propagates automatically to Avatar, TeamUp, Item, and ControlledEntity.
    conn.execute(
        "DELETE FROM Player WHERE DbGuid = ?1",
        params![target_id],
    )
    .map_err(|e| format!("Failed to remove existing player data: {e}"))?;

    // Recompute max AFTER the cascade delete so new IDs don't collide with
    // any other account's entities that remain in the database.
    let base = max_existing_id(conn).map_err(|e| e.to_string())?;
    insert_player_data(conn, target_id, base + 1, data)
}

// ── Entity insertion ───────────────────────────────────────────────────────

/// Insert a Player row and all entity collections, assigning fresh IDs to
/// every entity and rewriting ContainerDbGuid references accordingly.
fn insert_player_data(
    conn: &Connection,
    account_id: i64,
    first_entity_id: i64,
    data: &ImportJson,
) -> Result<(), String> {
    let player_archive = BASE64
        .decode(&data.player.archive_data)
        .map_err(|e| format!("Invalid Player.ArchiveData: {e}"))?;

    conn.execute(
        "INSERT INTO Player \
         (DbGuid, ArchiveData, StartTarget, AOIVolume, GazillioniteBalance, LastLogoutTime) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            account_id,
            player_archive,
            data.player.start_target,
            data.player.aoi_volume,
            data.player.gazillionite_balance,
            data.player.last_logout_time
        ],
    )
    .map_err(|e| format!("Failed to insert player: {e}"))?;

    // Map every old DbGuid to a new one.  The player/account ID stays as
    // account_id; all entity rows get sequential IDs from first_entity_id.
    let mut remap: HashMap<i64, i64> = HashMap::new();
    remap.insert(data.player.db_guid, account_id);

    let mut next = first_entity_id;

    let mut new_avatars: Vec<(i64, &ImportEntity)> = Vec::new();
    for e in &data.avatars {
        remap.insert(e.db_guid, next);
        new_avatars.push((next, e));
        next += 1;
    }

    let mut new_team_ups: Vec<(i64, &ImportEntity)> = Vec::new();
    for e in &data.team_ups {
        remap.insert(e.db_guid, next);
        new_team_ups.push((next, e));
        next += 1;
    }

    let mut new_items: Vec<(i64, &ImportEntity)> = Vec::new();
    for e in &data.items {
        remap.insert(e.db_guid, next);
        new_items.push((next, e));
        next += 1;
    }

    let mut new_controlled: Vec<(i64, &ImportEntity)> = Vec::new();
    for e in &data.controlled_entities {
        remap.insert(e.db_guid, next);
        new_controlled.push((next, e));
        next += 1;
    }

    // Items can reference a Player, Avatar, or TeamUp as their container.
    // The remap covers all three, so a single lookup resolves correctly
    // regardless of which table the original ContainerDbGuid belonged to.
    insert_entities(conn, &new_avatars, &remap, account_id, "Avatar")?;
    insert_entities(conn, &new_team_ups, &remap, account_id, "TeamUp")?;
    insert_entities(conn, &new_items, &remap, account_id, "Item")?;
    insert_entities(conn, &new_controlled, &remap, account_id, "ControlledEntity")?;

    Ok(())
}

fn insert_entities(
    conn: &Connection,
    entities: &[(i64, &ImportEntity)],
    remap: &HashMap<i64, i64>,
    fallback_container: i64,
    table: &str,
) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO {table} \
         (DbGuid, ContainerDbGuid, InventoryProtoGuid, Slot, EntityProtoGuid, ArchiveData) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    );

    for (new_guid, e) in entities {
        let container = remap
            .get(&e.container_db_guid)
            .copied()
            .unwrap_or(fallback_container);

        let archive = BASE64
            .decode(&e.archive_data)
            .map_err(|err| format!("Invalid {table}.ArchiveData: {err}"))?;

        conn.execute(
            &sql,
            params![
                new_guid,
                container,
                e.inventory_proto_guid,
                e.slot,
                e.entity_proto_guid,
                archive
            ],
        )
        .map_err(|err| format!("Failed to insert {table} row: {err}"))?;
    }

    Ok(())
}

// ── Misc ───────────────────────────────────────────────────────────────────

fn count<P: rusqlite::types::ToSql>(
    conn: &Connection,
    sql: &str,
    param: P,
) -> Result<i64, String> {
    conn.query_row(sql, [param], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())
}