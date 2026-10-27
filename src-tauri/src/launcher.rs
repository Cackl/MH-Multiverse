use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tauri::Manager;
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

/// A game process this app launched. Holding the `Child` keeps its PID from being reused.
#[derive(Clone)]
pub struct Game {
    pub pid: u32,
    child: Arc<Mutex<Child>>,
}

impl Game {
    pub fn is_running(&self) -> bool {
        self.child.lock().is_ok_and(|mut c| matches!(c.try_wait(), Ok(None)))
    }
}

/// The game this app last launched, if any. Unlike `GameProcessState`, which finds the game by
/// name, this knows only our own launch, so it can't mistake another instance for it.
#[derive(Default)]
pub struct LaunchedGame(Mutex<Option<Game>>);

impl LaunchedGame {
    fn set(&self, child: Child) -> Game {
        let game = Game { pid: child.id(), child: Arc::new(Mutex::new(child)) };
        if let Ok(mut g) = self.0.lock() {
            *g = Some(game.clone());
        }
        game
    }

    pub fn get(&self) -> Option<Game> {
        self.0.lock().ok().and_then(|g| g.clone())
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

/// Same rule as the Overlay launcher: the client ships `Binaries\Win64` and
/// `Binaries\Win32` side by side; use the sibling exe in the other folder when
/// 32-bit is forced. Falls back to the chosen exe if the layout doesn't match.
fn resolve_exe(exe: &str, force_32bit: bool) -> String {
    if !force_32bit {
        return exe.to_string();
    }
    let path = std::path::Path::new(exe);
    let win64 = path.parent().filter(|p| {
        p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("Win64"))
    });
    if let (Some(dir), Some(name)) = (win64, path.file_name()) {
        if let Some(bin) = dir.parent() {
            let alt = bin.join("Win32").join(name);
            if alt.is_file() {
                return alt.to_string_lossy().into_owned();
            }
        }
    }
    exe.to_string()
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

    // Always exactly one mode: -robocopy, -steam and -solidstate are mutually
    // exclusive and the client exits if the set is wrong.
    if opts.no_steam {
        args.push("-robocopy".into());
        args.push("-nosteam".into());
    } else {
        args.push("-steam".into());
        args.push("-nosolidstate".into());
    }
    if opts.skip_startup_movies  { args.push("-nostartupmovies".into()); }
    if opts.skip_motion_comics   { args.push("-skipmotioncomics".into()); }
    if opts.no_sound    { args.push("-nosound".into()); }
    if opts.enable_client_log    { args.push("-log".into()); }

    if opts.custom_resolution && opts.resolution_width > 0 && opts.resolution_height > 0 {
        args.push(format!("-ResX={}", opts.resolution_width));
        args.push(format!("-ResY={}", opts.resolution_height));
    }

    // ponytail: split on whitespace, so a single argument can't contain spaces; add quote parsing if needed
    args.extend(opts.extra_args.split_whitespace().map(String::from));

    if !server.email.is_empty() && opts.auto_login {
        let password = decrypt_password(&server.password_enc, &server.password_nonce)?;
        args.push(format!("-emailaddress={}", server.email));
        args.push(format!("-password={}", password));
    }

    let exe = resolve_exe(&exe, opts.force_32bit);
    let mut cmd = Command::new(&exe);
    cmd.args(&args);
    // Without these the Steam build asks Steam to restart it using Steam's own
    // launch entry, which discards our arguments.
    if !opts.no_steam {
        cmd.env("SteamAppId", "226320").env("SteamGameId", "226320");
    }
    let child = cmd
        // Detach - MH Multiverse doesn't own the game process
        .spawn()
        .map_err(|e| format!("Failed to launch game: {e}"))?;

    let sips = crate::overlay::sip_candidates(&config.game_exe, &config.server_exe);
    let game = app.state::<LaunchedGame>().set(child);
    crate::overlay::on_game_spawned(&app, config.overlay.clone(), sips, game);

    Ok(())
}

#[cfg(test)]
#[path = "../../tests/rust/test_launcher.rs"]
mod tests;
