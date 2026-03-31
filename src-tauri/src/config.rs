use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::Manager;

const KEYRING_SERVICE: &str = "mh-manifold";
const KEYRING_USER: &str = "encryption-key";
const CONFIG_FILENAME: &str = "manifold.json";

fn default_true() -> bool { true }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBackupOptions {
    #[serde(default = "default_true")]
    pub config_ini: bool,
    #[serde(default = "default_true")]
    pub live_tuning: bool,
    #[serde(default = "default_true")]
    pub billing_store: bool,
}

impl Default for UpdateBackupOptions {
    fn default() -> Self {
        Self {
            config_ini: true,
            live_tuning: true,
            billing_store: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    #[serde(default)]
    pub delay_minutes: u32,
    #[serde(default = "default_shutdown_broadcast")]
    pub broadcast_message: String,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            delay_minutes: 0,
            broadcast_message: default_shutdown_broadcast(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchOptions {
    #[serde(default = "default_true")]
    pub auto_login: bool,
    #[serde(default)]
    pub skip_startup_movies: bool,
    #[serde(default)]
    pub skip_motion_comics: bool,
    #[serde(default)]
    pub no_sound: bool,
    #[serde(default)]
    pub enable_client_log: bool,
    #[serde(default = "default_true")]
    pub robocopy: bool,
    #[serde(default = "default_true")]
    pub no_steam: bool,
    #[serde(default)]
    pub custom_resolution: bool,
    #[serde(default)]
    pub resolution_width: u32,
    #[serde(default)]
    pub resolution_height: u32,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            auto_login: true,
            skip_startup_movies: false,
            skip_motion_comics: false,
            no_sound: false,
            enable_client_log: false,
            robocopy: true,
            no_steam: true,
            custom_resolution: false,
            resolution_width: 0,
            resolution_height: 0,
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
    pub password_enc: String,
    #[serde(default)]
    pub password_nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub game_exe: String,
    pub server_exe: String,
    pub active_server_id: String,
    pub servers: Vec<Server>,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub launch_options: LaunchOptions,
    #[serde(default)]
    pub shutdown: ShutdownConfig,
    /// Tag assignments keyed by canonical filename, e.g. "LiveTuningData_CosmicChaos.json" -> "event"
    #[serde(default)]
    pub tuning_tags: HashMap<String, String>,
    /// Canonical filenames pinned to the top of the Tuning Files grid
    #[serde(default)]
    pub tuning_favourites: Vec<String>,
    /// Backup target paths selected in the Ops panel
    #[serde(default = "default_backup_targets")]
    pub backup_targets: Vec<String>,
    /// Auto-backup options for the nightly updater
    #[serde(default)]
    pub update_backup_options: UpdateBackupOptions,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_exe: String::new(),
            server_exe: String::new(),
            active_server_id: String::new(),
            servers: Vec::new(),
            theme: String::new(),
            launch_options: LaunchOptions::default(),
            shutdown: ShutdownConfig::default(),
            tuning_tags: HashMap::new(),
            tuning_favourites: Vec::new(),
            backup_targets: default_backup_targets(),
            update_backup_options: UpdateBackupOptions::default(),
        }
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
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AppConfig::default(),
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let contents = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Serialisation failed: {e}"))?;
    std::fs::write(&path, contents)
        .map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
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
pub fn set_game_exe(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.game_exe = path;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_server_exe(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.server_exe = path;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_theme(app: tauri::AppHandle, theme: String) -> Result<(), String> {
    let mut config = load_config(&app);
    config.theme = theme;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_launch_options(app: tauri::AppHandle, options: LaunchOptions) -> Result<(), String> {
    let mut config = load_config(&app);
    config.launch_options = options;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_shutdown_config(app: tauri::AppHandle, shutdown: ShutdownConfig) -> Result<(), String> {
    let mut config = load_config(&app);
    config.shutdown = shutdown;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_tuning_tags(app: tauri::AppHandle, tags: HashMap<String, String>) -> Result<(), String> {
    let mut config = load_config(&app);
    config.tuning_tags = tags;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_tuning_favourites(app: tauri::AppHandle, favourites: Vec<String>) -> Result<(), String> {
    let mut config = load_config(&app);
    config.tuning_favourites = favourites;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_backup_targets(app: tauri::AppHandle, targets: Vec<String>) -> Result<(), String> {
    let mut config = load_config(&app);
    config.backup_targets = targets;
    save_config(&app, &config)
}

#[tauri::command]
pub fn set_update_backup_options(app: tauri::AppHandle, options: UpdateBackupOptions) -> Result<(), String> {
    let mut config = load_config(&app);
    config.update_backup_options = options;
    save_config(&app, &config)
}