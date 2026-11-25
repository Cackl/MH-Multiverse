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

/// The client writes its launch arguments, auto-login password included, to `Launch.log` and keeps
/// a `Launch-backup-*.log` per launch. Masks every `-password=` value in them. A file the game
/// still holds open is skipped and caught at game exit or the next start.
pub(crate) fn scrub_launch_logs(dir: &std::path::Path) {
    // Bytes, not str: a log with non-UTF-8 text must come back unchanged apart from the mask.
    // Masks to the end of the line, so a password with spaces or quotes is covered. The client logs
    // the command line in one line and `launch_game` passes `-password=` last.
    let re = regex::bytes::Regex::new(r"(?i)(-password=)[^\r\n]+").unwrap();
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|e| e.path()) {
        let is_launch_log = path.file_name().and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("Launch") && n.ends_with(".log"));
        if !is_launch_log { continue }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let scrubbed = re.replace_all(&bytes, &b"${1}***"[..]);
        if *scrubbed != *bytes {
            let _ = std::fs::write(&path, &scrubbed);
        }
    }
}

/// Run at startup and when `game_is_running` sees the game exit.
pub(crate) fn scrub_launch_logs_for(app: &tauri::AppHandle) {
    if let Ok(docs) = app.path().document_dir() {
        scrub_launch_logs(&docs.join(r"My Games\Marvel Heroes\MarvelGame\Logs"));
    }
}

/// Last `game_is_running` result, to spot the game exiting.
static GAME_WAS_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A process is the game when it has the linked exe's file name and runs from the linked
/// install: the linked exe itself or its `Binaries\Win32`/`Win64` sibling (Force 32-bit).
/// 1.52 and 1.53 both ship `MarvelHeroesOmega.exe` (1.48 has `MarvelHeroes2016.exe`), so the
/// name alone can't tell installs apart. Falls back to the name when the path can't be read.
fn is_game_process(process_name: &std::ffi::OsStr, process_exe: Option<&std::path::Path>, game_exe: &str) -> bool {
    use std::path::Path;
    let linked = Path::new(game_exe);
    if !linked.file_name().is_some_and(|name| name.eq_ignore_ascii_case(process_name)) {
        return false;
    }
    let Some(exe) = process_exe else { return true };
    let key = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    let binaries = |p: &Path| {
        p.parent()
            .filter(|d| d.file_name().is_some_and(|n| n.eq_ignore_ascii_case("Win64") || n.eq_ignore_ascii_case("Win32")))
            .and_then(Path::parent)
            .map(key)
    };
    key(exe) == key(linked) || binaries(exe).is_some_and(|b| Some(b) == binaries(linked))
}

#[tauri::command]
pub fn game_is_running(app: tauri::AppHandle, state: tauri::State<GameProcessState>) -> bool {
    let game_exe = load_config(&app).game_exe;
    let mut sys = match state.0.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    // remove_dead_processes must be true: this System is reused across polls
    // (see GameProcessState), so a process that has exited needs to be
    // pruned from its internal list each refresh, or the search
    // below keeps finding the stale entry forever.
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
    );
    let running = sys.processes().values().any(|p| is_game_process(p.name(), p.exe(), &game_exe));
    drop(sys);
    // The frontend polls this every few seconds app-wide, so it doubles as the game-exit hook.
    if GAME_WAS_RUNNING.swap(running, std::sync::atomic::Ordering::SeqCst) && !running {
        scrub_launch_logs_for(&app);
    }
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

/// 1.48 and 1.53 clients always launch with -robocopy -nosteam. Steam's app 226320 is the 1.52
/// build, and Steam mode on the older and test clients is untested. Provisional.
fn use_steam(config: &crate::config::AppConfig) -> bool {
    !config.launch_options.no_steam && !config.client_is_1_48_or_1_53()
}

#[tauri::command]
pub fn launch_game(app: tauri::AppHandle, server_id: String) -> Result<(), String> {
    // gameRunning is polled every few seconds, so a quick second click can get here.
    if app.state::<LaunchedGame>().get().is_some_and(|g| g.is_running()) {
        return Err("The game is already running.".into());
    }
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
    let steam = use_steam(&config);

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
    if steam {
        args.push("-steam".into());
        args.push("-nosolidstate".into());
    } else {
        args.push("-robocopy".into());
        args.push("-nosteam".into());
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
    if steam {
        cmd.env("SteamAppId", "226320").env("SteamGameId", "226320");
    }
    let child = cmd
        // Detach - MH Multiverse doesn't own the game process
        .spawn()
        .map_err(|e| format!("Failed to launch game: {e}"))?;

    let game = app.state::<LaunchedGame>().set(child);
    if crate::overlay::supported(&config) {
        let sips = crate::overlay::sip_candidates(&config.game_exe, &config.server_exe);
        crate::overlay::on_game_spawned(&app, config.overlay.clone(), sips, game);
    }

    Ok(())
}

#[cfg(test)]
#[path = "../../tests/rust/test_launcher.rs"]
mod tests;
