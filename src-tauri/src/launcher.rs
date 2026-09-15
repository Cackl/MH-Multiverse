use std::process::Command;
use std::sync::Mutex;
use crate::config::{load_config, decrypt_password};
use sysinfo::System;

/// Reused across `game_is_running` polls (called every few seconds while
/// LaunchPanel is mounted) instead of constructing a fresh `System` and
/// doing a full process refresh on every call.
pub struct GameProcessState(pub Mutex<System>);

impl GameProcessState {
    pub fn new() -> Self {
        Self(Mutex::new(System::new()))
    }
}

#[tauri::command]
pub fn game_is_running(state: tauri::State<GameProcessState>) -> bool {
    let mut sys = match state.0.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    // remove_dead_processes must be true: this System is reused across polls
    // (see GameProcessState), so a process that has exited needs to be
    // pruned from its internal list each refresh, or processes_by_exact_name
    // below keeps finding the stale entry forever.
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let running = sys
        .processes_by_exact_name("MarvelHeroesOmega.exe".as_ref())
        .next()
        .is_some();
    running
}

/// Strips any scheme ("http://", "https://") and any path suffix ("/foo/bar")
/// from a raw host string, leaving only "host" or "host:port".
fn normalize_host_str(raw: &str) -> String {
    let without_scheme = match raw.find("://") {
        Some(pos) => &raw[pos + 3..],
        None => raw,
    };
    match without_scheme.find('/') {
        Some(pos) => without_scheme[..pos].trim().to_string(),
        None => without_scheme.trim().to_string(),
    }
}

/// Exposes `normalize_host_str` to the frontend, so dashboard/home URLs in
/// LaunchPanel.svelte use the exact same logic that builds the actual game
/// launch URL below, instead of maintaining an independent TS copy that can
/// drift out of sync.
#[tauri::command]
pub fn normalize_host(raw: String) -> String {
    normalize_host_str(&raw)
}

#[tauri::command]
pub fn launch_game(app: tauri::AppHandle, server_id: String) -> Result<(), String> {
    let config = load_config(&app);

    let exe = config.game_exe.trim().to_string();
    if exe.is_empty() {
        return Err("Game executable path is not set.".into());
    }
    if !std::path::Path::new(&exe).is_file() {
        return Err(format!("Executable not found: {exe}"));
    }

    let server = config
        .servers
        .iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| format!("Server not found: {server_id}"))?;

    let opts = &config.launch_options;

    let siteconfig_url = if server.is_local {
        if config.launch_options.patched_client {
            "http://localhost/Dashboard/SiteConfig.xml".to_string()
        } else {
            "http://localhost/SiteConfig.xml".to_string()
        }
    } else {
        let scheme = if server.use_https { "https" } else { "http" };
        let host = normalize_host_str(&server.host);
        format!("{scheme}://{host}/SiteConfig.xml")
    };

    let mut args: Vec<String> = vec![
        format!("-siteconfigurl={siteconfig_url}"),
    ];

    if opts.no_steam    { args.push("-nosteam".into()); }
    if opts.robocopy    { args.push("-robocopy".into()); }
    if opts.skip_startup_movies  { args.push("-nostartupmovies".into()); }
    if opts.skip_motion_comics   { args.push("-skipmotioncomics".into()); }
    if opts.no_sound    { args.push("-nosound".into()); }
    if opts.enable_client_log    { args.push("-log".into()); }

    if opts.custom_resolution && opts.resolution_width > 0 && opts.resolution_height > 0 {
        args.push(format!("-ResX={}", opts.resolution_width));
        args.push(format!("-ResY={}", opts.resolution_height));
    }

    if !server.email.is_empty() && opts.auto_login {
        let password = decrypt_password(&server.password_enc, &server.password_nonce)?;
        args.push(format!("-emailaddress={}", server.email));
        args.push(format!("-password={}", password));
    }

    Command::new(&exe)
        .args(&args)
        // Detach - MH Multiverse doesn't own the game process
        .spawn()
        .map_err(|e| format!("Failed to launch game: {e}"))?;

    Ok(())
}
