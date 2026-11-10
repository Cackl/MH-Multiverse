use super::*;

// These hit the real OS keychain (keyring crate, service "mh-multiverse") to
// get or create the encryption key — the same entry the running app uses.
// Not mockable without a bigger abstraction than 3 tests justify; run locally,
// not in a keychain-less CI environment.

#[test]
fn encrypt_decrypt_round_trip() {
    let (enc, nonce) = encrypt_password("s3cret-password").unwrap();
    assert!(!enc.is_empty());
    assert!(!nonce.is_empty());
    let plain = decrypt_password(&enc, &nonce).unwrap();
    assert_eq!(plain, "s3cret-password");
}

#[test]
fn empty_password_stays_empty() {
    assert_eq!(encrypt_password("").unwrap(), (String::new(), String::new()));
    assert_eq!(decrypt_password("", "").unwrap(), "");
}

// multiverse.json as 1.6.1 wrote it: every field of that release, none added since.
const CONFIG_1_6_1: &str = r#"{
  "game_exe": "C:\\Games\\MHO\\UnrealEngine3\\Binaries\\Win64\\MarvelHeroesOmega.exe",
  "server_exe": "C:\\Servers\\MHServerEmu\\MHServerEmu.exe",
  "active_server_id": "local",
  "servers": [
    { "id": "local", "name": "Local", "host": "localhost", "email": "test@test.com",
      "is_local": true, "use_https": false, "password_enc": "", "password_nonce": "" }
  ],
  "theme": "phoenix",
  "launch_options": { "auto_login": true, "patched_client": false, "skip_startup_movies": true,
    "skip_motion_comics": false, "no_sound": false, "enable_client_log": false, "no_steam": true,
    "force_32bit": false, "custom_resolution": false, "resolution_width": 0, "resolution_height": 0,
    "extra_args": "" },
  "shutdown": { "delay_minutes": 5, "broadcast_message": "Server restarting" },
  "tuning_tags": { "LiveTuningData_Rates.json": "rates" },
  "tuning_favourites": [ "LiveTuningData_Rates.json" ],
  "backup_targets": [ "Config.ini", "Data/Account.db" ],
  "store_html_output_dir": "",
  "console_presets": [ "!server status" ],
  "overlay": { "dir": "", "trusted": null, "auto_inject": false, "omega_gear": false }
}"#;

#[test]
fn config_from_1_6_1_loads_and_keeps_the_advanced_editors() {
    let config: AppConfig = serde_json::from_str(CONFIG_1_6_1).unwrap();
    assert_eq!(config.servers.len(), 1);
    assert_eq!(config.theme, "phoenix");
    assert!(config.advanced_data, "an upgraded config must stay in Advanced");
}

#[test]
fn fresh_install_starts_simple_and_the_choice_survives_a_save() {
    let fresh = AppConfig::default();
    assert!(!fresh.advanced_data);

    for advanced in [false, true] {
        let saved = serde_json::to_string(&AppConfig { advanced_data: advanced, ..AppConfig::default() }).unwrap();
        let loaded: AppConfig = serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded.advanced_data, advanced);
    }
}
