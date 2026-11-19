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
    assert_eq!(config.text_size, 1.0);
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

fn install(id: &str, game_exe: &str, server_exe: &str, no_steam: bool) -> LinkedInstall {
    LinkedInstall {
        id: id.into(), name: id.into(), version: "1.48".into(),
        game_exe: game_exe.into(), server_exe: server_exe.into(), custom: false, no_steam,
    }
}

#[test]
fn an_older_config_becomes_one_active_linked_version() {
    let mut config: AppConfig = serde_json::from_str(CONFIG_1_6_1).unwrap();
    config.launch_options.no_steam = false;
    assert!(migrate_linked(&mut config, |_, _| "1.52".into()));
    assert_eq!(config.linked, vec![LinkedInstall {
        id: "default".into(), name: "1.52".into(), version: "1.52".into(),
        game_exe: config.game_exe.clone(), server_exe: config.server_exe.clone(),
        custom: false, no_steam: false,
    }]);
    assert_eq!(config.active_linked_id, "default");
    // Only once.
    assert!(!migrate_linked(&mut config, |_, _| "1.53".into()));
    assert_eq!(config.linked.len(), 1);
}

#[test]
fn migration_names_an_undetected_version_and_skips_an_empty_config() {
    let mut config: AppConfig = serde_json::from_str(CONFIG_1_6_1).unwrap();
    assert!(migrate_linked(&mut config, |_, _| String::new()));
    assert_eq!((config.linked[0].name.as_str(), config.linked[0].version.as_str()), ("Default", ""));

    let mut fresh = AppConfig::default();
    assert!(!migrate_linked(&mut fresh, |_, _| "1.52".into()));
    assert!(fresh.linked.is_empty());
}

#[test]
fn activating_copies_the_entry_to_the_active_paths() {
    let mut config = AppConfig::default();
    config.linked = vec![install("a", "A.exe", "SA.exe", true), install("b", "B.exe", "", false)];
    activate_linked_in(&mut config, "a").unwrap();
    activate_linked_in(&mut config, "b").unwrap();
    assert_eq!(
        (config.active_linked_id.as_str(), config.game_exe.as_str(), config.server_exe.as_str(), config.launch_options.no_steam),
        ("b", "B.exe", "", false)
    );
    assert!(activate_linked_in(&mut config, "missing").is_err());
    assert_eq!(config.active_linked_id, "b");
}

#[test]
fn settings_edits_follow_into_the_active_entry_only() {
    let mut config = AppConfig::default();
    config.linked = vec![install("a", "A.exe", "SA.exe", true), install("b", "B.exe", "SB.exe", true)];
    activate_linked_in(&mut config, "a").unwrap();
    config.game_exe = "A2.exe".into();
    config.launch_options.no_steam = false;
    sync_active_linked(&mut config, |_, _| "1.48".into());
    assert_eq!(config.linked[0], install("a", "A2.exe", "SA.exe", false));
    assert_eq!(config.linked[1], install("b", "B.exe", "SB.exe", true));
}

#[test]
fn new_exes_in_settings_detect_the_version_again() {
    let mut config = AppConfig::default();
    config.linked = vec![LinkedInstall { name: "1.52".into(), version: "1.52".into(), ..install("a", "A.exe", "SA.exe", false) }];
    activate_linked_in(&mut config, "a").unwrap();

    // Launch options alone don't detect.
    config.launch_options.no_steam = true;
    sync_active_linked(&mut config, |_, _| panic!("detected without an exe change"));
    assert_eq!(config.linked[0].version, "1.52");

    // A 1.48 client: the version and the version-only name follow.
    config.game_exe = "MarvelHeroes2016.exe".into();
    sync_active_linked(&mut config, |game, _| if game == "MarvelHeroes2016.exe" { "1.48".into() } else { "1.52".into() });
    assert_eq!((config.linked[0].name.as_str(), config.linked[0].version.as_str()), ("1.48", "1.48"));

    // A name the user chose stays, and an undetected exe clears the version.
    config.linked[0].name = "Mine".into();
    config.server_exe = "Other.exe".into();
    sync_active_linked(&mut config, |_, _| String::new());
    assert_eq!((config.linked[0].name.as_str(), config.linked[0].version.as_str()), ("Mine", ""));
}

#[test]
fn the_first_entry_becomes_the_active_one() {
    let mut config = AppConfig::default();
    upsert_linked_in(&mut config, install("a", "A.exe", "SA.exe", false));
    upsert_linked_in(&mut config, install("b", "B.exe", "SB.exe", false));
    assert_eq!((config.active_linked_id.as_str(), config.game_exe.as_str()), ("a", "A.exe"));
}

#[test]
fn upsert_reports_a_switch_only_when_the_active_exes_change() {
    let mut config = AppConfig::default();
    config.linked = vec![install("a", "A.exe", "SA.exe", true)];
    activate_linked_in(&mut config, "a").unwrap();

    // Renaming the active entry, or adding another, is not a switch.
    assert!(!upsert_linked_in(&mut config, LinkedInstall { name: "Mine".into(), ..install("a", "A.exe", "SA.exe", true) }));
    assert!(!upsert_linked_in(&mut config, install("b", "B.exe", "SB.exe", true)));
    assert_eq!(config.game_exe, "A.exe");

    // New paths on the active entry are a switch, and become the active paths.
    assert!(upsert_linked_in(&mut config, install("a", "A.exe", "SA2.exe", true)));
    assert_eq!(config.server_exe, "SA2.exe");
    assert_eq!(config.linked.len(), 2);
}

#[test]
fn the_active_entry_cannot_be_removed() {
    let mut config = AppConfig::default();
    config.linked = vec![install("a", "A.exe", "", true), install("b", "B.exe", "", true)];
    activate_linked_in(&mut config, "a").unwrap();
    assert!(delete_linked_in(&mut config, "a").is_err());
    delete_linked_in(&mut config, "b").unwrap();
    assert_eq!(config.linked.len(), 1);
}

#[test]
fn a_new_app_version_or_an_unreadable_file_backs_the_config_up_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multiverse.json");
    let bak = dir.path().join("multiverse.json.bak");
    let old: AppConfig = serde_json::from_str(CONFIG_1_6_1).unwrap();

    // Same version: no backup.
    let current = AppConfig { app_version: "1.7.0".into(), ..old.clone() };
    assert!(!backup_on_upgrade(&path, "same", Some(&current), "1.7.0"));
    assert!(!bak.exists());

    // A file from before the stamp existed, or from another version.
    assert!(backup_on_upgrade(&path, CONFIG_1_6_1, Some(&old), "1.7.0"));
    assert_eq!(std::fs::read_to_string(&bak).unwrap(), CONFIG_1_6_1);

    // A file that can't be read.
    assert!(backup_on_upgrade(&path, "{ broken", None, "1.7.0"));
    assert_eq!(std::fs::read_to_string(&bak).unwrap(), "{ broken");
}

#[test]
fn text_size_accepts_usable_zooms_only() {
    for ok in [0.5, 0.9, 1.0, 1.25, 2.0] {
        assert!(valid_text_size(ok), "{ok}");
    }
    for bad in [0.0, 0.49, 2.01, -1.0, f64::NAN, f64::INFINITY] {
        assert!(!valid_text_size(bad), "{bad}");
    }
}
