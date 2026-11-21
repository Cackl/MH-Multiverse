use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

use crate::overlay::OverlayConfig;
use crate::server;

const KEYRING_SERVICE: &str = "mh-multiverse";
const KEYRING_USER: &str = "encryption-key";
const CONFIG_FILENAME: &str = "multiverse.json";

fn default_true() -> bool { true }

fn default_text_size() -> f64 { 1.0 }

fn default_shutdown_broadcast() -> String {
    "Server is shutting down in {minutes} minute(s).".to_string()
}

fn default_backup_targets() -> Vec<String> {
    vec![
        "Config.ini".to_string(),
        "ConfigOverride.ini".to_string(),
        "Data/Game/LiveTuning".to_string(),
        "Data/Account.db".to_string(),
    ]
}

fn default_console_presets() -> Vec<String> {
    vec![
        "!commands".to_string(),
        "!account userlevel".to_string(),
        "!server status".to_string(),
        "!server broadcast".to_string(),
        "!server shutdown".to_string(),
        "!server reloadcatalog".to_string(),
        "!server reloadlivetuning".to_string()
    ]
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    #[serde(default)]
    pub delay_minutes: u32,
    #[serde(default = "default_shutdown_broadcast")]
    pub broadcast_message: String,
    /// Closing the app asks first when the server is running. Off: it stops the server and closes.
    #[serde(default)]
    pub ask_before_close: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            delay_minutes: 0,
            broadcast_message: default_shutdown_broadcast(),
            ask_before_close: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchOptions {
    #[serde(default = "default_true")]
    pub auto_login: bool,
    #[serde(default)]
    pub patched_client: bool,
    #[serde(default)]
    pub skip_startup_movies: bool,
    #[serde(default)]
    pub skip_motion_comics: bool,
    #[serde(default)]
    pub no_sound: bool,
    #[serde(default)]
    pub enable_client_log: bool,
    #[serde(default = "default_true")]
    pub no_steam: bool,
    #[serde(default)]
    pub force_32bit: bool,
    #[serde(default)]
    pub custom_resolution: bool,
    #[serde(default)]
    pub resolution_width: u32,
    #[serde(default)]
    pub resolution_height: u32,
    #[serde(default)]
    pub extra_args: String,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            auto_login: true,
            patched_client: false,
            skip_startup_movies: false,
            skip_motion_comics: false,
            no_sound: false,
            enable_client_log: false,
            no_steam: true,
            force_32bit: false,
            custom_resolution: false,
            resolution_width: 0,
            resolution_height: 0,
            extra_args: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub email: String,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default)]
    pub use_https: bool,
    #[serde(default)]
    pub password_enc: String,
    #[serde(default)]
    pub password_nonce: String,
    /// The linked version this profile plays on. Empty follows the active version.
    #[serde(default)]
    pub linked_id: String,
    /// Local profiles: Launch starts the server first when it isn't running.
    #[serde(default)]
    pub start_server: bool,
}

/// A client and server pair the user can switch to. The active one is copied into
/// `AppConfig::game_exe`, `server_exe` and `launch_options.no_steam`, which everything else reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkedInstall {
    pub id: String,
    pub name: String,
    /// "1.48", "1.52" or "1.53". Detected, or picked by the user when detection fails.
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub game_exe: String,
    #[serde(default)]
    pub server_exe: String,
    /// The server is a local build of a fork: no nightly updates, shown as experimental.
    #[serde(default)]
    pub custom: bool,
    #[serde(default = "default_true")]
    pub no_steam: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub game_exe: String,
    pub server_exe: String,
    #[serde(default)]
    pub linked: Vec<LinkedInstall>,
    #[serde(default)]
    pub active_linked_id: String,
    pub active_server_id: String,
    pub servers: Vec<Server>,
    #[serde(default)]
    pub theme: String,
    /// Whole-window zoom factor set in Settings › Display (1.0 = 100%).
    #[serde(default = "default_text_size")]
    pub text_size: f64,
    #[serde(default)]
    pub launch_options: LaunchOptions,
    #[serde(default)]
    pub shutdown: ShutdownConfig,
    /// Backup target paths selected in the Ops panel
    #[serde(default = "default_backup_targets")]
    pub backup_targets: Vec<String>,
    #[serde(default = "default_console_presets")]
    pub console_presets: Vec<String>,
    #[serde(default)]
    pub overlay: OverlayConfig,
    /// Data tab view: true = Advanced (the full editors), false = Simple. A config written
    /// before the field existed keeps the editors its owner already knows; a fresh install
    /// (`AppConfig::default()`) starts in Simple.
    #[serde(default = "default_true")]
    pub advanced_data: bool,
    /// Starting the server also starts the bundled Apache. On by default, also for configs
    /// from before the field, since a local login needs Apache for SiteConfig.
    #[serde(default = "default_true")]
    pub apache_auto_start: bool,
    /// The app version that last loaded this file. A different one makes a .bak first.
    #[serde(default)]
    pub app_version: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_exe: String::new(),
            server_exe: String::new(),
            linked: Vec::new(),
            active_linked_id: String::new(),
            active_server_id: String::new(),
            servers: Vec::new(),
            theme: String::new(),
            text_size: 1.0,
            launch_options: LaunchOptions::default(),
            shutdown: ShutdownConfig::default(),
            console_presets: default_console_presets(),
            backup_targets: default_backup_targets(),
            overlay: OverlayConfig::default(),
            advanced_data: false,
            apache_auto_start: true,
            app_version: String::new(),
        }
    }
}

impl AppConfig {
    /// The active linked version's game version, or "" when none is linked or it isn't known.
    pub fn active_version(&self) -> &str {
        self.active_linked().map_or("", |l| l.version.as_str())
    }

    pub fn active_linked(&self) -> Option<&LinkedInstall> {
        self.linked.iter().find(|l| l.id == self.active_linked_id)
    }

    /// The active client is 1.48 or 1.53: no Steam mode and no overlay, which are built for 1.52.
    pub fn client_is_1_48_or_1_53(&self) -> bool {
        matches!(self.active_version(), "1.48" | "1.53")
    }
}

// ── Encryption key management ─────────────────────────────────────────────────

/// Retrieve the encryption key from the OS keychain, creating it if absent.
fn get_or_create_key() -> Result<Vec<u8>, String> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keychain access failed: {e}"))?;

    match entry.get_password() {
        Ok(encoded) => {
            let bytes = B64.decode(&encoded)
                .map_err(|e| format!("Key decode failed: {e}"))?;
            if bytes.len() != 32 {
                return Err("Stored key has wrong length".into());
            }
            Ok(bytes)
        }
        Err(_) => {
            // Generate a new 256-bit key
            let key = Aes256Gcm::generate_key(OsRng);
            let encoded = B64.encode(key.as_slice());
            entry.set_password(&encoded)
                .map_err(|e| format!("Failed to store key: {e}"))?;
            Ok(key.to_vec())
        }
    }
}

fn cipher() -> Result<Aes256Gcm, String> {
    let key_bytes = get_or_create_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    Ok(Aes256Gcm::new(key))
}

// ── Encrypt / decrypt helpers ─────────────────────────────────────────────────

pub fn encrypt_password(plaintext: &str) -> Result<(String, String), String> {
    if plaintext.is_empty() {
        return Ok((String::new(), String::new()));
    }
    let cipher = cipher()?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;
    Ok((B64.encode(&ciphertext), B64.encode(nonce.as_slice())))
}

pub fn decrypt_password(enc: &str, nonce_b64: &str) -> Result<String, String> {
    if enc.is_empty() {
        return Ok(String::new());
    }
    let cipher = cipher()?;
    let ciphertext = B64.decode(enc)
        .map_err(|e| format!("Ciphertext decode failed: {e}"))?;
    let nonce_bytes = B64.decode(nonce_b64)
        .map_err(|e| format!("Nonce decode failed: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("Decryption failed: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 error: {e}"))
}

// ── Config file I/O ───────────────────────────────────────────────────────────

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create app data dir: {e}"))?;
    Ok(dir.join(CONFIG_FILENAME))
}

pub fn load_config(app: &tauri::AppHandle) -> AppConfig {
    let path = match config_path(app) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    let Some(mut config) = read_config_file(&path, APP_VERSION) else {
        // Stamped so the next load doesn't back these defaults up as another version's file.
        return AppConfig { app_version: APP_VERSION.into(), ..AppConfig::default() };
    };
    let migrated = migrate_linked(&mut config, detect_version);
    let stamped = config.app_version != APP_VERSION;
    config.app_version = APP_VERSION.into();
    if migrated || stamped {
        let _ = save_config(app, &config);
    }
    config
}

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Set while multiverse.json exists but couldn't be read or moved aside, so saves would replace it.
static CONFIG_UNREADABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A message for the user about the last load, shown once by the frontend.
static CONFIG_NOTICE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
const CONFIG_UNREADABLE_MSG: &str = "multiverse.json can't be read. Fix or remove it, then restart the app";

/// Reads the config file. None means use defaults: there is no file, or it couldn't be read, in
/// which case it is moved aside (or saves are blocked if that fails) and a notice is left.
fn read_config_file(path: &std::path::Path, current: &str) -> Option<AppConfig> {
    use std::sync::atomic::Ordering;
    let parsed = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            CONFIG_UNREADABLE.store(false, Ordering::SeqCst);
            return None;
        }
        Err(_) => None,
        Ok(contents) => serde_json::from_str::<AppConfig>(contents.trim_start_matches('\u{FEFF}'))
            .ok()
            .map(|c| (c, contents)),
    };
    let Some((config, contents)) = parsed else {
        let notice = match crate::store::move_aside(path) {
            Ok(kept) => {
                CONFIG_UNREADABLE.store(false, Ordering::SeqCst);
                format!(
                    "multiverse.json couldn't be read, so the app started with default settings. It was kept as {}.",
                    kept.display()
                )
            }
            Err(_) => {
                CONFIG_UNREADABLE.store(true, Ordering::SeqCst);
                CONFIG_UNREADABLE_MSG.into()
            }
        };
        if let Ok(mut n) = CONFIG_NOTICE.lock() {
            *n = Some(notice);
        }
        return None;
    };
    CONFIG_UNREADABLE.store(false, Ordering::SeqCst);
    backup_on_upgrade(path, &contents, &config, current);
    Some(config)
}

/// Copies the config file to multiverse.json.bak when another app version last loaded it, which is
/// before this one changes its shape. Returns true when it made one.
fn backup_on_upgrade(path: &std::path::Path, contents: &str, parsed: &AppConfig, current: &str) -> bool {
    if parsed.app_version == current {
        return false;
    }
    std::fs::write(path.with_extension("json.bak"), contents).is_ok()
}

#[tauri::command]
pub fn take_config_notice() -> Option<String> {
    CONFIG_NOTICE.lock().ok()?.take()
}

// ── Linked installs ───────────────────────────────────────────────────────────

fn detect_version(game_exe: &str, server_exe: &str) -> String {
    crate::versions::client_version(game_exe)
        .or_else(|| crate::versions::server_version(server_exe))
        .unwrap_or_default()
        .to_string()
}

/// A config from before linked installs becomes one entry, so the user's setup is kept.
/// Returns true when it changed the config.
fn migrate_linked(config: &mut AppConfig, detect: impl Fn(&str, &str) -> String) -> bool {
    if !config.linked.is_empty() || (config.game_exe.is_empty() && config.server_exe.is_empty()) {
        return false;
    }
    let version = detect(&config.game_exe, &config.server_exe);
    let install = LinkedInstall {
        id: "default".into(),
        name: if version.is_empty() { "Default".into() } else { version.clone() },
        version,
        game_exe: config.game_exe.clone(),
        server_exe: config.server_exe.clone(),
        custom: false,
        no_steam: config.launch_options.no_steam,
    };
    config.active_linked_id = install.id.clone();
    config.linked.push(install);
    true
}

/// Copies the active entry's paths and Steam setting to where the rest of the app reads them.
fn activate_linked_in(config: &mut AppConfig, id: &str) -> Result<(), String> {
    let install = config.linked.iter().find(|l| l.id == id).ok_or("No such linked version")?.clone();
    config.active_linked_id = install.id;
    config.game_exe = install.game_exe;
    config.server_exe = install.server_exe;
    config.launch_options.no_steam = install.no_steam;
    Ok(())
}

/// Settings edits the active copy, so the active entry follows it.
/// New exes get their version detected again. A name that was only the old version follows it.
fn sync_active_linked(config: &mut AppConfig, detect: impl Fn(&str, &str) -> String) {
    let (game_exe, server_exe, no_steam) =
        (config.game_exe.clone(), config.server_exe.clone(), config.launch_options.no_steam);
    if let Some(active) = config.linked.iter_mut().find(|l| l.id == config.active_linked_id) {
        if active.game_exe != game_exe || active.server_exe != server_exe {
            let version = detect(&game_exe, &server_exe);
            if !version.is_empty() && active.name == active.version {
                active.name = version.clone();
            }
            active.version = version;
        }
        active.game_exe = game_exe;
        active.server_exe = server_exe;
        active.no_steam = no_steam;
    }
}

/// Adds or replaces an entry. True when it is the active one and its exes changed,
/// which is a switch and needs the server and the game stopped. With none active, it becomes the active one.
fn upsert_linked_in(config: &mut AppConfig, install: LinkedInstall) -> bool {
    let is_active = install.id == config.active_linked_id || config.active_linked_id.is_empty();
    let switches = is_active && (install.game_exe != config.game_exe || install.server_exe != config.server_exe);
    let id = install.id.clone();
    match config.linked.iter_mut().find(|l| l.id == install.id) {
        Some(existing) => *existing = install,
        None => config.linked.push(install),
    }
    if is_active {
        let _ = activate_linked_in(config, &id);
    }
    switches
}

fn delete_linked_in(config: &mut AppConfig, id: &str) -> Result<(), String> {
    if id == config.active_linked_id {
        return Err("Switch to another version before removing this one".into());
    }
    config.linked.retain(|l| l.id != id);
    for server in config.servers.iter_mut().filter(|s| s.linked_id == id) {
        server.linked_id.clear();
    }
    Ok(())
}

/// For changes the server reads: its exe and database path.
fn ensure_server_stopped(app: &tauri::AppHandle) -> Result<(), String> {
    if crate::updater::update_running() {
        return Err("Wait for the update to finish".into());
    }
    if server::server_process_is_running(&app.state::<server::ServerState>()) {
        return Err("Stop the server first".into());
    }
    Ok(())
}

/// For a version switch, which also changes the game exe.
fn ensure_stopped(app: &tauri::AppHandle) -> Result<(), String> {
    if crate::updater::update_running() {
        return Err("Wait for the update to finish".into());
    }
    if server::server_process_is_running(&app.state::<server::ServerState>())
        || crate::launcher::game_is_running(app.clone(), app.state())
    {
        return Err("Stop the server and the game first".into());
    }
    Ok(())
}

#[tauri::command]
pub fn activate_linked(app: tauri::AppHandle, id: String) -> Result<AppConfig, String> {
    let mut config = load_config(&app);
    if id == config.active_linked_id {
        return Ok(config);
    }
    ensure_stopped(&app)?;
    activate_linked_in(&mut config, &id)?;
    save_config(&app, &config)?;
    app.state::<server::DbPath>().set_from_server_exe(&config.server_exe);
    Ok(config)
}

#[tauri::command]
pub fn upsert_linked(app: tauri::AppHandle, install: LinkedInstall) -> Result<AppConfig, String> {
    let mut config = load_config(&app);
    let switches = upsert_linked_in(&mut config, install);
    if switches {
        ensure_stopped(&app)?;
    }
    save_config(&app, &config)?;
    if switches {
        app.state::<server::DbPath>().set_from_server_exe(&config.server_exe);
    }
    Ok(config)
}

#[tauri::command]
pub fn delete_linked(app: tauri::AppHandle, id: String) -> Result<AppConfig, String> {
    let mut config = load_config(&app);
    delete_linked_in(&mut config, &id)?;
    save_config(&app, &config)?;
    Ok(config)
}

pub fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    if CONFIG_UNREADABLE.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(CONFIG_UNREADABLE_MSG.into());
    }
    let path = config_path(app)?;
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Serialisation failed: {e}"))?;
    crate::store::write_atomic(&path, &contents)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> AppConfig {
    load_config(&app)
}

#[tauri::command]
pub fn cmd_save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    save_config(&app, &config)
}

/// Add or update a server. Encrypts the password before storing.
/// `password` is the plaintext supplied from the frontend (empty = unchanged).
#[tauri::command]
pub fn upsert_server(
    app: tauri::AppHandle,
    server: Server,
    password: String,
) -> Result<AppConfig, String> {
    let mut config = load_config(&app);

    let (enc, nonce) = if password.is_empty() {
        // No new password supplied — preserve existing if server already exists
        config
            .servers
            .iter()
            .find(|s| s.id == server.id)
            .map(|s| (s.password_enc.clone(), s.password_nonce.clone()))
            .unwrap_or_default()
    } else {
        encrypt_password(&password)?
    };

    let updated = Server {
        password_enc: enc,
        password_nonce: nonce,
        ..server
    };

    if let Some(existing) = config.servers.iter_mut().find(|s| s.id == updated.id) {
        *existing = updated;
    } else {
        config.servers.push(updated);
    }

    save_config(&app, &config)?;
    Ok(config)
}

#[tauri::command]
pub fn delete_server(app: tauri::AppHandle, server_id: String) -> Result<AppConfig, String> {
    let mut config = load_config(&app);
    config.servers.retain(|s| s.id != server_id);
    if config.active_server_id == server_id {
        config.active_server_id = config.servers.first().map(|s| s.id.clone()).unwrap_or_default();
    }
    save_config(&app, &config)?;
    Ok(config)
}

#[tauri::command]
pub fn set_active_server(app: tauri::AppHandle, server_id: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.active_server_id = server_id;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_game_exe(app: tauri::AppHandle, path: String) -> Result<AppConfig, String> {
    let mut config = load_config(&app);
    config.game_exe = path;
    sync_active_linked(&mut config, detect_version);
    save_config(&app, &config)?;
    Ok(config)
}

#[tauri::command]
pub fn set_server_exe(app: tauri::AppHandle, path: String) -> Result<AppConfig, String> {
    ensure_server_stopped(&app)?;
    let mut config = load_config(&app);
    config.server_exe = path.clone();
    sync_active_linked(&mut config, detect_version);
    save_config(&app, &config)?;
    app.state::<server::DbPath>().set_from_server_exe(&path);
    Ok(config)
}

#[tauri::command]
pub fn set_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.theme = theme;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_text_size(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if !valid_text_size(size) {
        return Err(format!("Text size {size} is out of range"));
    }
    let mut config = load_config(&app);
    config.text_size = size;
    save_config(&app, &config)
}

/// A zoom the window can still be used at. NaN and infinity fail the range check too.
fn valid_text_size(size: f64) -> bool {
    (0.5..=2.0).contains(&size)
}

#[tauri::command]
pub fn set_launch_options(app: tauri::AppHandle, options: LaunchOptions) -> Result<(), String> {
    let mut config = load_config(&app);
    config.launch_options = options;
    sync_active_linked(&mut config, detect_version);
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_shutdown_config(app: tauri::AppHandle, shutdown: ShutdownConfig) -> Result<(), String> {
    let mut config = load_config(&app);
    config.shutdown = shutdown;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_backup_targets(app: tauri::AppHandle, targets: Vec<String>) -> Result<(), String> {
    let mut config = load_config(&app);
    config.backup_targets = targets;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_apache_auto_start(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    let mut config = load_config(&app);
    config.apache_auto_start = on;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_advanced_data(app: tauri::AppHandle, advanced: bool) -> Result<(), String> {
    let mut config = load_config(&app);
    config.advanced_data = advanced;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_console_presets(app: tauri::AppHandle, presets: Vec<String>) -> Result<(), String> {
    let mut config = load_config(&app);
    config.console_presets = presets;
    save_config(&app, &config)
}

#[cfg(test)]
#[path = "../../tests/rust/test_config.rs"]
mod tests;