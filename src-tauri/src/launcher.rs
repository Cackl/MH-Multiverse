use std::process::Command;
use crate::config::{load_config, decrypt_password};
use crate::ini;
use sysinfo::System;

#[tauri::command]
pub fn game_is_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, false);
    let running = sys.processes_by_exact_name("MarvelHeroesOmega.exe".as_ref())
        .next()
        .is_some();
    running
}

/// Strips any scheme ("http://", "https://") and any path suffix ("/foo/bar")
/// from a raw host string, leaving only "host" or "host:port".
fn normalize_host(raw: &str) -> String {
    let without_scheme = match raw.find("://") {
        Some(pos) => &raw[pos + 3..],
        None => raw,
    };
    match without_scheme.find('/') {
        Some(pos) => without_scheme[..pos].trim().to_string(),
        None => without_scheme.trim().to_string(),
    }
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
        let port = ini::read_merged_value(&config.server_exe, "WebFrontend", "Port", "8080");
        format!("http://localhost:{port}/SiteConfig.xml")
    } else {
        let scheme = if server.use_https { "https" } else { "http" };
        let host = normalize_host(&server.host);
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