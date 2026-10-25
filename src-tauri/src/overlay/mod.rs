//! Third-party overlay integration. Nothing here runs overlay executables: the
//! user links a folder and trusts it (TOFU), and the DLL is injected only if
//! the folder still matches the trusted hashes at the moment of injection.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::Emitter;

use crate::config::{load_config, save_config};

mod inject;
mod pipe;
pub(crate) use pipe::sip_candidates;
#[cfg(test)]
mod tests;

const DLL_NAME: &str = "MhClientHook.dll";
/// The hook's loot-filter settings, under %LOCALAPPDATA%. User data outside the overlay folder, so never hashed.
const LOOT_CONFIG: &str = "MarvelHeroesOmega.LootFilter/loot-filter-config.json";
/// Stands in for the hash of a symlink or subfolder found inside a code folder.
const UNHASHABLE: &str = "!not-a-regular-file";

/// Code and static assets the hook loads. Everything else in the overlay
/// folder (legacy exes, updater, flags, panel layouts) is never hashed or copied.
const CODE_FILES: &[&str] = &[
    DLL_NAME,
    "MhClientHook.terminals.bin",
    "MhClientHook.enum-names.bin",
    "LootFilter/loot-filter-config.default.json",
];
const CODE_DIRS: &[&str] = &[
    "MhClientHook.dps-portraits",
    "MhClientHook.menu-icons",
    "MhClientHook.ui-fonts",
    "MhClientHook.ui-textures",
];
/// Control flags the hook reads next to the DLL.
const FLAGS: &[&str] = &[
    "MhClientHook.terminals-enabled.flag",
    "MhClientHook.dps-enabled.flag",
    "MhClientHook.detector-log.flag",
    "MhClientHook.scaleform-instrument.flag",
    "MhClientHook.skip-boot-loading-gate.flag",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TrustRecord {
    pub set_hash: String,
    /// relative path (forward slashes) -> sha256 hex
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OverlayConfig {
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub trusted: Option<TrustRecord>,
    /// Opt-in: inject after launch once trusted.
    #[serde(default)]
    pub auto_inject: bool,
    /// Opt-in: keep the overlay loot panel's `showOmega` on (Omega column and slots).
    #[serde(default)]
    pub omega_gear: bool,
}

#[derive(Debug, Serialize)]
pub struct OverlayStatus {
    /// not_linked | missing | untrusted | trusted | changed
    pub state: &'static str,
    pub dir: String,
    pub auto_inject: bool,
    pub omega_gear: bool,
    /// "changed: x" / "missing: x" / "added: x" lines when state == changed
    pub changes: Vec<String>,
}

// ── hashing ───────────────────────────────────────────────────────────────────

fn hash_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    Ok(format!("{:x}", h.finalize()))
}

fn is_plain_file(path: &Path) -> bool {
    // symlink_metadata so a link can't redirect a hashed path elsewhere
    std::fs::symlink_metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// Hash every present file of the code set under `dir`. The DLL is required.
fn scan_set(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for rel in CODE_FILES {
        let p = dir.join(rel);
        if is_plain_file(&p) {
            out.insert(rel.to_string(), hash_file(&p)?);
        }
    }
    for d in CODE_DIRS {
        let Ok(rd) = std::fs::read_dir(dir.join(d)) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let key = format!("{d}/{}", e.file_name().to_string_lossy());
            // symlinks and subfolders can't be hashed; record a marker so they surface as
            // "added" after trust, and so Trust can refuse them
            let hash = if is_plain_file(&p) { hash_file(&p)? } else { UNHASHABLE.to_string() };
            out.insert(key, hash);
        }
    }
    if !out.contains_key(DLL_NAME) {
        return Err(format!("{DLL_NAME} not found in {}", dir.display()));
    }
    Ok(out)
}

fn set_hash(files: &BTreeMap<String, String>) -> String {
    let mut h = Sha256::new();
    for (k, v) in files {
        h.update(k.as_bytes());
        h.update([0]);
        h.update(v.as_bytes());
        h.update([b'\n']);
    }
    format!("{:x}", h.finalize())
}

fn diff(trusted: &BTreeMap<String, String>, now: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in trusted {
        match now.get(k) {
            None => out.push(format!("missing: {k}")),
            Some(n) if n != v => out.push(format!("changed: {k}")),
            _ => {}
        }
    }
    for k in now.keys().filter(|k| !trusted.contains_key(*k)) {
        out.push(format!("added: {k}"));
    }
    out
}

// ── status / trust ────────────────────────────────────────────────────────────

fn status_of(cfg: &OverlayConfig) -> OverlayStatus {
    let mk = |state, changes| OverlayStatus {
        state,
        dir: cfg.dir.clone(),
        auto_inject: cfg.auto_inject,
        omega_gear: cfg.omega_gear,
        changes,
    };
    if cfg.dir.is_empty() {
        return mk("not_linked", vec![]);
    }
    let Ok(now) = scan_set(Path::new(&cfg.dir)) else { return mk("missing", vec![]) };
    match &cfg.trusted {
        None => mk("untrusted", vec![]),
        Some(t) => {
            let changes = diff(&t.files, &now);
            if changes.is_empty() { mk("trusted", vec![]) } else { mk("changed", changes) }
        }
    }
}

/// Strip the `\\?\` prefix canonicalize adds on Windows.
fn clean_path(p: PathBuf) -> String {
    p.to_string_lossy().trim_start_matches(r"\\?\").to_string()
}

// async: hashes every overlay file, which would otherwise block the main thread (and the window)
#[tauri::command(async)]
pub fn overlay_status(app: tauri::AppHandle) -> OverlayStatus {
    status_of(&load_config(&app).overlay)
}

/// Link a folder. Changing folder drops any previous trust.
#[tauri::command]
pub fn overlay_set_dir(app: tauri::AppHandle, dir: String) -> Result<OverlayStatus, String> {
    let canon = std::fs::canonicalize(&dir).map_err(|e| format!("Cannot open folder: {e}"))?;
    if !canon.join(DLL_NAME).is_file() {
        return Err(format!("{DLL_NAME} not found in that folder."));
    }
    let mut config = load_config(&app);
    config.overlay.dir = clean_path(canon);
    config.overlay.trusted = None;
    config.overlay.auto_inject = false;
    save_config(&app, &config)?;
    Ok(status_of(&config.overlay))
}

#[tauri::command]
pub fn overlay_trust(app: tauri::AppHandle) -> Result<OverlayStatus, String> {
    let mut config = load_config(&app);
    if config.overlay.dir.is_empty() {
        return Err("No overlay folder linked.".into());
    }
    let files = scan_set(Path::new(&config.overlay.dir))?;
    if let Some(k) = files.iter().find(|(_, v)| *v == UNHASHABLE).map(|(k, _)| k) {
        return Err(format!("Cannot trust the overlay: {k} is a link or folder, not a plain file."));
    }
    config.overlay.trusted = Some(TrustRecord { set_hash: set_hash(&files), files });
    save_config(&app, &config)?;
    Ok(status_of(&config.overlay))
}

#[tauri::command]
pub fn overlay_untrust(app: tauri::AppHandle) -> Result<OverlayStatus, String> {
    let mut config = load_config(&app);
    config.overlay.trusted = None;
    config.overlay.auto_inject = false;
    save_config(&app, &config)?;
    Ok(status_of(&config.overlay))
}

#[tauri::command]
pub fn overlay_set_auto_inject(app: tauri::AppHandle, enabled: bool) -> Result<OverlayStatus, String> {
    let mut config = load_config(&app);
    if enabled && status_of(&config.overlay).state != "trusted" {
        return Err("Trust the overlay before enabling injection.".into());
    }
    config.overlay.auto_inject = enabled;
    save_config(&app, &config)?;
    Ok(status_of(&config.overlay))
}

/// Saves the setting and applies it now, on or off, so a running game picks it up live.
/// While on, every injected launch re-applies it; while off, launches leave the file alone.
/// Turning it on needs a trusted overlay; turning it off is always allowed, to undo our own write.
#[tauri::command]
pub fn overlay_set_omega_gear(app: tauri::AppHandle, enabled: bool) -> Result<OverlayStatus, String> {
    let mut config = load_config(&app);
    if enabled && status_of(&config.overlay).state != "trusted" {
        return Err("Trust the overlay before enabling Omega gear.".into());
    }
    config.overlay.omega_gear = enabled;
    save_config(&app, &config)?;
    if let Some(path) = loot_config_path() {
        set_show_omega_at(&path, enabled)?;
    }
    Ok(status_of(&config.overlay))
}

// ── injection + flags ──────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct InjectPayload {
    ok: bool,
    message: String,
}

/// Called right after the game is spawned. When injection is enabled, a background
/// thread verifies the hashes (refusing if anything changed), waits for the game to
/// settle, re-verifies right before loading, and injects. Never blocks or fails the launch;
/// the outcome is emitted as an `overlay-inject` event. The thread owns the
/// `Child`, which keeps the PID from being reused while we work, and hands it to the pipe reader.
pub(crate) fn on_game_spawned(app: &tauri::AppHandle, cfg: OverlayConfig, sips: Vec<PathBuf>, mut child: std::process::Child) {
    if !cfg.auto_inject {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new().name("overlay-inject".into()).spawn(move || {
        // Once before injecting, for an existing config, and once after: on a first run the hook
        // creates the file while attaching and picks the second write up through its live reload.
        sync_show_omega(&app, &cfg);
        let result = inject_into(&cfg, &mut child);
        if result.is_ok() {
            sync_show_omega(&app, &cfg);
        }
        let payload = match result {
            Ok(()) => {
                pipe::spawn(app.clone(), PathBuf::from(&cfg.dir), sips, child);
                InjectPayload { ok: true, message: "Overlay injected.".into() }
            }
            Err(m) => InjectPayload { ok: false, message: m },
        };
        let _ = app.emit("overlay-inject", payload);
    });
}

/// Sets `showOmega` in the hook's loot-filter config, which only changes the in-game panel layout
/// (Omega rarity column and Omega Slot 1-5 rows). The hook re-reads the file when its write time
/// changes, so a write after injection applies live.
/// - A missing file is left missing: the hook seeds the user's config from the default file only when
///   it is absent, and a stub here would stop that.
/// - Anything that isn't a JSON object is left untouched and reported.
/// - An already-correct value is not rewritten, so the write time (the hook's reload trigger) stays put.
/// - Other keys are kept; serde_json may reorder them.
fn set_show_omega_at(path: &Path, on: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read loot-filter-config.json: {e}"))?;
    let mut v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("loot-filter-config.json is not valid JSON: {e}"))?;
    let obj = v.as_object_mut().ok_or("loot-filter-config.json is not a JSON object")?;
    if obj.get("showOmega") == Some(&serde_json::Value::Bool(on)) {
        return Ok(());
    }
    obj.insert("showOmega".into(), on.into());
    let out = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, out).map_err(|e| format!("Cannot write loot-filter-config.json: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Cannot write loot-filter-config.json: {e}"))
}

fn loot_config_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|local| Path::new(&local).join(LOOT_CONFIG))
}

/// Re-applies the Omega gear setting at launch when it is on and the overlay is still trusted (an
/// overlay in the "changed" state gets no help from us). Never fails the launch: a problem is only
/// noted in the log.
fn sync_show_omega(app: &tauri::AppHandle, cfg: &OverlayConfig) {
    if !cfg.omega_gear || status_of(cfg).state != "trusted" {
        return;
    }
    let Some(path) = loot_config_path() else { return };
    if let Err(e) = set_show_omega_at(&path, true) {
        pipe::status(app, &format!("Omega gear setting not applied: {e}"));
    }
}

fn inject_into(cfg: &OverlayConfig, child: &mut std::process::Child) -> Result<(), String> {
    use inject::InjectError;
    if status_of(cfg).state != "trusted" {
        return Err("The overlay files changed since they were trusted; not injecting. Re-trust it in Settings.".into());
    }
    let dir = PathBuf::from(&cfg.dir);
    // '0' on a fresh launch so the hook arms its loading gate (a stale '1' would skip it)
    write_flag(&dir, "MhClientHook.skip-boot-loading-gate.flag", false)?;
    let dll = dir.join(DLL_NAME);
    let pid = child.id();

    let _ = inject::wait_input_idle(pid);
    for attempt in 1..=5 {
        if let Ok(Some(_)) = child.try_wait() {
            return Err("The game exited before the overlay could be injected.".into());
        }
        // verify right before loading; anything changed since Trust is refused
        if status_of(cfg).state != "trusted" {
            return Err("Overlay files changed since they were trusted; not injecting.".into());
        }
        match inject::inject(pid, &dll) {
            Ok(()) => return Ok(()),
            // the remote LoadLibrary may still finish after our wait: give it a moment
            // before retrying (or giving up) and accept it if the module shows up
            Err(InjectError::Timeout) => {
                let grace = if attempt < 5 { 2 } else { 10 };
                if inject::wait_module(pid, DLL_NAME, grace) {
                    return Ok(());
                }
                if attempt == 5 {
                    return Err("Timed out waiting for the overlay to initialise.".into());
                }
            }
            Err(InjectError::Failed(m)) => return Err(m),
        }
    }
    unreachable!()
}

/// Write a control flag atomically ('1' / '0', as the hook expects).
pub(crate) fn write_flag(dir: &Path, name: &str, on: bool) -> Result<(), String> {
    if !FLAGS.contains(&name) {
        return Err(format!("Unknown flag: {name}"));
    }
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, if on { b"1" } else { b"0" }).map_err(|e| format!("Cannot write {name}: {e}"))?;
    std::fs::rename(&tmp, dir.join(name)).map_err(|e| format!("Cannot write {name}: {e}"))
}

/// User-facing toggles: key, flag file, value when the file is absent.
/// Terminals/DPS default on (the overlay's own launcher defaults them on, and the
/// injector treats an absent terminals flag as enabled); logging defaults off.
const USER_FLAGS: &[(&str, &str, bool)] = &[
    ("terminals", "MhClientHook.terminals-enabled.flag", true),
    ("dps", "MhClientHook.dps-enabled.flag", true),
    ("detector_log", "MhClientHook.detector-log.flag", false),
];

/// Missing or empty file = `default`; otherwise on only if the first byte
/// (after an optional UTF-8 BOM) is '1'.
fn read_flag(dir: &Path, name: &str, default: bool) -> bool {
    let Ok(b) = std::fs::read(dir.join(name)) else { return default };
    match b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b).first() {
        None => default,
        Some(c) => *c == b'1',
    }
}

fn read_flags(dir: &Path) -> BTreeMap<&'static str, bool> {
    USER_FLAGS.iter().map(|(k, f, d)| (*k, read_flag(dir, f, *d))).collect()
}

fn linked_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = load_config(app).overlay.dir;
    if dir.is_empty() {
        return Err("No overlay folder linked.".into());
    }
    Ok(PathBuf::from(dir))
}

#[tauri::command]
pub fn overlay_get_flags(app: tauri::AppHandle) -> Result<BTreeMap<&'static str, bool>, String> {
    Ok(read_flags(&linked_dir(&app)?))
}

#[tauri::command]
pub fn overlay_set_flag(app: tauri::AppHandle, key: String, on: bool) -> Result<BTreeMap<&'static str, bool>, String> {
    let dir = linked_dir(&app)?;
    let (_, file, _) = USER_FLAGS.iter().find(|(k, _, _)| *k == key).ok_or("Unknown flag.")?;
    write_flag(&dir, file, on)?;
    Ok(read_flags(&dir))
}
