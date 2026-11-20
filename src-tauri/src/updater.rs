use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

// ── App Version ───────────────────────────────────────────────────────────────

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub build_date: String,
    pub download_url: String,
    pub available: bool,
    /// Build date (YYYYMMDD) stamped into the installed MHServerEmu.exe, when it has one.
    #[serde(default)]
    pub installed_build_date: Option<String>,
    /// The installed build is at least as new as `build_date`, so updating would change nothing.
    #[serde(default)]
    pub up_to_date: bool,
    /// Why no nightly is offered for this server (Custom, or its game version wasn't detected).
    /// When set, `build_date` and `download_url` are empty.
    #[serde(default)]
    pub unavailable_reason: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub id: String,
    pub created_at: String,
    pub label: String,
    pub targets: Vec<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Serialize)]
struct UpdateProgressPayload {
    stage: String,
    pct: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn server_dir_from_exe(server_exe: &str) -> Result<PathBuf, String> {
    crate::paths::server_dir(server_exe)
}

/// Build dates to try, newest first: today and yesterday (UTC). The nightly for a date appears
/// partway through that UTC day, so until then yesterday's is the newest build.
fn candidate_dates(now: DateTime<Utc>) -> [String; 2] {
    let fmt = |t: DateTime<Utc>| t.format("%Y%m%d").to_string();
    [fmt(now), fmt(now - Duration::days(1))]
}

/// Upstream builds one nightly per game version. The server's own dll says which one it is, so
/// updating never swaps a 1.48 or 1.53 server for a 1.52 build. A fork reads as its base version,
/// which is why Custom servers get no nightlies at all.
fn nightly_flavour(server_version: Option<&str>, custom: bool) -> Result<&'static str, String> {
    if custom {
        return Err("Nightly updates are off for Custom servers.".into());
    }
    match server_version {
        Some("1.48") => Ok("v48"),
        Some("1.52") => Ok("v52"),
        Some("1.53") => Ok("v53"),
        _ => Err("Couldn't tell which game version this server is for, so no nightly is offered.".into()),
    }
}

/// The flavour for this server, from its dll and the active linked version's Custom flag.
fn server_flavour(app: &AppHandle, server_exe: &str) -> Result<&'static str, String> {
    if server_exe.trim().is_empty() {
        return Err("Set the MHServerEmu executable in Settings first.".into());
    }
    let custom = crate::config::load_config(app).active_linked().is_some_and(|l| l.custom);
    nightly_flavour(crate::versions::server_version(server_exe), custom)
}

fn build_download_url(flavour: &str, build_date: &str) -> String {
    format!(
        "https://nightly.link/Crypto137/MHServerEmu/workflows/nightly-release-windows-{flavour}/master/MHServerEmu-nightly-{build_date}-Release-windows-{flavour}.zip"
    )
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("mh-multiverse/{APP_VERSION}"))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))
}

/// The newest nightly that exists, as (build date, download url).
async fn newest_build(client: &reqwest::Client, flavour: &str) -> Option<(String, String)> {
    for build_date in candidate_dates(Utc::now()) {
        let url = build_download_url(flavour, &build_date);
        let exists = client
            .get(&url)
            .header("Range", "bytes=0-0")
            .send()
            .await
            .map(|r| r.status().is_success() || r.status().as_u16() == 206)
            .unwrap_or(false);
        if exists {
            return Some((build_date, url));
        }
    }
    None
}

/// The exe's ProductVersion string, e.g. "1.0.3+build20261027131155". The numeric file version
/// drops the "+build" stamp that identifies a nightly.
pub(crate) fn product_version(path: &Path) -> Option<String> {
    use windows::core::{w, HSTRING};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
    let p = HSTRING::from(path.as_os_str());
    unsafe {
        let size = GetFileVersionInfoSizeW(&p, None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(&p, None, size, buf.as_mut_ptr().cast()).ok()?;
        // The string table is keyed by the file's first (language, codepage) pair.
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(buf.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool() || len < 4 {
            return None;
        }
        let lang = std::slice::from_raw_parts(ptr as *const u16, 2);
        let key = HSTRING::from(format!("\\StringFileInfo\\{:04x}{:04x}\\ProductVersion", lang[0], lang[1]));
        if !VerQueryValueW(buf.as_ptr().cast(), &key, &mut ptr, &mut len).as_bool() || len == 0 {
            return None;
        }
        let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        Some(String::from_utf16_lossy(text).trim_end_matches('\0').to_string())
    }
}

/// Build date (YYYYMMDD) from a ProductVersion like "1.0.3+build20261027131155".
fn build_date_from_version(version: &str) -> Option<String> {
    let date = version.split_once("+build")?.1.get(..8)?;
    date.bytes().all(|b| b.is_ascii_digit()).then(|| date.to_string())
}

/// Build date of the installed server, or None when its exe has no nightly stamp (e.g. a build
/// from source), in which case updates are never blocked.
fn installed_build_date(server_exe: &str) -> Option<String> {
    build_date_from_version(&product_version(Path::new(server_exe))?)
}

/// Both dates are YYYYMMDD, so string order is date order.
fn is_up_to_date(installed: Option<&str>, build_date: &str) -> bool {
    installed.is_some_and(|installed| installed >= build_date)
}

fn emit_progress(app: &AppHandle, stage: &str, pct: f32, message: Option<String>) {
    let _ = app.emit("update-progress", UpdateProgressPayload {
        stage: stage.to_string(),
        pct,
        message,
    });
}

// ── File utilities ────────────────────────────────────────────────────────────

// Files excluded from all backups regardless of target selection.
// These are large read-only game data files shipped with the server
// and replaced on every update — never modified by users.
const BACKUP_BLACKLIST: &[&str] = &[
    "Calligraphy.sip",
    "mu_cdata.sip",
];

fn is_blacklisted(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| BACKUP_BLACKLIST.contains(&n))
        .unwrap_or(false)
}

/// Recursively copy a file or directory. Silently skips missing sources.
fn copy_path(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if is_blacklisted(src) {
        return Ok(());
    }
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create directory {}: {e}", parent.display()))?;
        }
        std::fs::copy(src, dst)
            .map_err(|e| format!("Cannot copy {}: {e}", src.display()))?;
    } else if src.is_dir() {
        std::fs::create_dir_all(dst)
            .map_err(|e| format!("Cannot create directory {}: {e}", dst.display()))?;
        for entry in std::fs::read_dir(src)
            .map_err(|e| format!("Cannot read directory {}: {e}", src.display()))?
        {
            let entry = entry.map_err(|e| format!("Directory entry error: {e}"))?;
            copy_path(&entry.path(), &dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Copy the contents of src directory into dst directory.
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Cannot create destination directory: {e}"))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("Cannot read source directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Directory entry error: {e}"))?;
        copy_path(&entry.path(), &dst.join(entry.file_name()))?;
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    if path.is_dir() {
        return std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| dir_size(&e.path()))
                    .sum()
            })
            .unwrap_or(0);
    }
    0
}

/// If the staging directory contains exactly one subdirectory and no loose files,
/// treat that subdirectory as the top-level wrapper and return it. Otherwise return
/// staging itself.
fn detect_source_dir(staging: &Path) -> Result<PathBuf, String> {
    let entries: Vec<_> = std::fs::read_dir(staging)
        .map_err(|e| format!("Cannot read staging directory: {e}"))?
        .filter_map(|e| e.ok())
        .collect();

    let has_files = entries.iter().any(|e| e.path().is_file());
    let dirs: Vec<_> = entries.iter().filter(|e| e.path().is_dir()).collect();

    if !has_files && dirs.len() == 1 {
        Ok(dirs[0].path())
    } else {
        Ok(staging.to_path_buf())
    }
}

// ── Switched stock files ──────────────────────────────────────────────────────

/// A stock file the update shipped in the other state, kept as the user had switched it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KeptFile {
    pub file: String,
    pub enabled: bool,
    /// The name the replaced copy was moved to, in the same folder.
    pub kept_as: String,
}

/// Each `.json` file the update ships where the app can switch it: its name, where it ships, where
/// its copy switched the other way lives, and whether that copy is the switched-on one. Live
/// Tuning switches a root file off with an `OFF_` prefix, Patches and the Store with an Off folder.
fn switchable_files(source: &Path) -> Vec<(String, String, String, bool)> {
    const TUNING: &str = "Data/Game/LiveTuning";
    let folders = [
        (TUNING, TUNING, false),
        ("Data/Game/Patches", "Data/Game/Patches/Off", false),
        ("Data/Game/Patches/Off", "Data/Game/Patches", true),
        ("Data/Game/MTXStore", "Data/Game/MTXStore/Off", false),
        ("Data/Game/MTXStore/Off", "Data/Game/MTXStore", true),
    ];
    let mut out = Vec::new();
    for (dir, other_dir, other_on) in folders {
        let Ok(entries) = std::fs::read_dir(source.join(dir)) else { continue };
        for entry in entries.flatten().filter(|e| e.path().is_file()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".json") || (dir == TUNING && !name.starts_with("LiveTuningData")) {
                continue;
            }
            let other = if dir == TUNING { format!("OFF_{name}") } else { name.clone() };
            out.push((name.clone(), format!("{dir}/{name}"), format!("{other_dir}/{other}"), other_on));
        }
    }
    out
}

/// Puts back the user's on/off choice for stock files the update shipped in the other state.
/// Only a switch moves a stock file out of the place it ships in, so when both copies exist, the
/// other one is the user's choice. The update's copy replaces it and the replaced copy is moved
/// aside to `<name>.<timestamp>.bak`. Where the backup was just restored, the restored copy stays
/// and the update's copy is the one moved aside.
fn keep_switched_files(server_dir: &Path, source: &Path, restored: &[String]) -> Result<Vec<KeptFile>, String> {
    let is_restored = |rel: &str| restored.iter().any(|t| rel == t || rel.starts_with(&format!("{t}/")));
    let rename = |from: &Path, to: &Path| {
        std::fs::rename(from, to).map_err(|e| format!("Cannot move {}: {e}", from.display()))
    };
    let mut kept = Vec::new();
    for (name, shipped, other, other_on) in switchable_files(source) {
        let (shipped_path, other_path) = (server_dir.join(&shipped), server_dir.join(&other));
        if !shipped_path.is_file() || !other_path.is_file() {
            continue;
        }
        let bak = if is_restored(&other) {
            crate::store::move_aside(&shipped_path)?
        } else {
            let bak = crate::store::move_aside(&other_path)?;
            rename(&shipped_path, &other_path)?;
            bak
        };
        kept.push(KeptFile { file: name, enabled: other_on, kept_as: crate::store::file_name_of(&bak) });
    }
    Ok(kept)
}

// ── Backup internals ──────────────────────────────────────────────────────────

fn create_backup_inner(
    server_dir: &Path,
    targets: Vec<String>,
    label: &str,
) -> Result<BackupManifest, String> {
    let now = Local::now();
    let id = now.format("%Y%m%d-%H%M%S").to_string();
    let created_at = now.to_rfc3339();

    let backup_dir = server_dir.join("Backups").join(&id);
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Cannot create backup directory: {e}"))?;

    let mut backed_up = Vec::new();
    for target in &targets {
        let src = server_dir.join(target);
        if !src.exists() {
            continue;
        }
        let dst = backup_dir.join(target);
        copy_path(&src, &dst)?;
        backed_up.push(target.clone());
    }

    let size_bytes = dir_size(&backup_dir);

    let manifest = BackupManifest {
        id: id.clone(),
        created_at,
        label: label.to_string(),
        targets: backed_up,
        size_bytes,
    };

    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Cannot serialise manifest: {e}"))?;
    std::fs::write(backup_dir.join("manifest.json"), json)
        .map_err(|e| format!("Cannot write manifest: {e}"))?;

    Ok(manifest)
}

// ── Update internals ──────────────────────────────────────────────────────────

async fn download_with_progress(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("mh-multiverse/{APP_VERSION}"))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))?;

    let mut response = client.get(url).send().await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest).await
        .map_err(|e| format!("Cannot create download file: {e}"))?;

    while let Some(chunk) = response.chunk().await
        .map_err(|e| format!("Download stream error: {e}"))?
    {
        downloaded += chunk.len() as u64;

        file.write_all(&chunk).await
            .map_err(|e| format!("Cannot write download chunk: {e}"))?;

        if total_size > 0 {
            let pct = (downloaded as f32 / total_size as f32) * 100.0;
            emit_progress(app, "downloading", pct, None);
        }
    }

    file.flush().await
        .map_err(|e| format!("Cannot flush download file: {e}"))?;

    Ok(())
}

fn extract_zip_to(app: &AppHandle, zip_path: &Path, staging_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(staging_dir)
        .map_err(|e| format!("Cannot create staging directory: {e}"))?;

    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Cannot open zip file: {e}"))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Cannot read zip archive: {e}"))?;

    let total = archive.len();

    for i in 0..total {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Cannot read zip entry {i}: {e}"))?;

        let out_path = match entry.enclosed_name() {
            Some(p) => staging_dir.join(p),
            None => continue,
        };

        emit_progress(
            app,
            "extracting",
            (i + 1) as f32 / total as f32 * 100.0,
            entry.name().ok().map(|n| n.into_owned()),
        );

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Cannot create directory {}: {e}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create parent directory: {e}"))?;
            }
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Cannot create file {}: {e}", out_path.display()))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Cannot extract {}: {e}", out_path.display()))?;
        }
    }

    Ok(())
}

async fn run_update_inner(
    app: &AppHandle,
    server_exe: &str,
    server_dir: &Path,
    zip_path: &Path,
    staging_dir: &Path,
    backup_targets: Vec<String>,
) -> Result<Vec<KeptFile>, String> {
    if crate::server::server_running(app, server_exe) {
        return Err("Cannot update while the server is running. Stop the server first.".into());
    }

    // Resolve the build before backing up, so a missing build leaves nothing behind
    let flavour = server_flavour(app, server_exe)?;
    let (build_date, url) = newest_build(&client()?, flavour)
        .await
        .ok_or("No nightly build is available yet. Try again later.")?;
    if is_up_to_date(installed_build_date(server_exe).as_deref(), &build_date) {
        return Err(format!("This server already has the {build_date} build or a newer one."));
    }

    // Pre-update backup
    let backup_manifest: Option<BackupManifest> = {
        let targets = backup_targets;

        if !targets.is_empty() {
            emit_progress(app, "backing_up", 0.0, None);
            let dir = server_dir.to_path_buf();
            let manifest = tokio::task::spawn_blocking(move || {
                create_backup_inner(&dir, targets, "pre-update")
            })
            .await
            .map_err(|e| format!("Backup task failed: {e}"))??;
            Some(manifest)
        } else {
            None
        }
    };

    // Download
    emit_progress(app, "downloading", 0.0, None);
    download_with_progress(app, &url, zip_path).await?;

    // Extract zip to staging dir
    emit_progress(app, "extracting", 0.0, None);
    {
        let app_clone = app.clone();
        let zip = zip_path.to_path_buf();
        let staging = staging_dir.to_path_buf();
        tokio::task::spawn_blocking(move || extract_zip_to(&app_clone, &zip, &staging))
            .await
            .map_err(|e| format!("Extraction task failed: {e}"))??;
    }

    // Copy staged files into server dir
    emit_progress(app, "installing", 0.0, None);
    {
        let staging = staging_dir.to_path_buf();
        let dst = server_dir.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let source = detect_source_dir(&staging)?;
            copy_dir_contents(&source, &dst)
        })
        .await
        .map_err(|e| format!("Install task failed: {e}"))??;
    }

    // Restore user files that the update overwrote
    if let Some(manifest) = &backup_manifest {
        emit_progress(app, "restoring", 0.0, None);
        let backup_dir = server_dir.join("Backups").join(&manifest.id);
        for (i, target) in manifest.targets.iter().enumerate() {
            let src = backup_dir.join(target);
            let dst = server_dir.join(target);
            copy_path(&src, &dst)?;
            emit_progress(
                app,
                "restoring",
                (i + 1) as f32 / manifest.targets.len() as f32 * 100.0,
                Some(target.clone()),
            );
        }
    }

    let restored = backup_manifest.map(|m| m.targets).unwrap_or_default();
    keep_switched_files(server_dir, &detect_source_dir(staging_dir)?, &restored)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn check_update_available(app: AppHandle, server_exe: Option<String>) -> Result<UpdateInfo, String> {
    let installed_build_date = server_exe.as_deref().and_then(installed_build_date);
    let flavour = match server_flavour(&app, server_exe.as_deref().unwrap_or_default()) {
        Ok(flavour) => flavour,
        Err(reason) => {
            return Ok(UpdateInfo {
                build_date: String::new(),
                download_url: String::new(),
                available: false,
                installed_build_date,
                up_to_date: false,
                unavailable_reason: Some(reason),
            })
        }
    };
    Ok(match newest_build(&client()?, flavour).await {
        Some((build_date, download_url)) => UpdateInfo {
            up_to_date: is_up_to_date(installed_build_date.as_deref(), &build_date),
            installed_build_date,
            build_date,
            download_url,
            available: true,
            unavailable_reason: None,
        },
        None => {
            let [build_date, _] = candidate_dates(Utc::now());
            UpdateInfo {
                download_url: build_download_url(flavour, &build_date),
                build_date,
                available: false,
                installed_build_date,
                up_to_date: false,
                unavailable_reason: None,
            }
        }
    })
}

static UPDATE_RUNNING: AtomicBool = AtomicBool::new(false);

/// True while `run_update` holds the lock.
pub(crate) fn update_running() -> bool {
    UPDATE_RUNNING.load(Ordering::SeqCst)
}

/// Held while an update runs. Two runs would share `_update.zip` and the staging folder, and the
/// UI can't prevent it: leaving the tab resets its own "updating" flag while the run continues.
struct UpdateLock;

impl UpdateLock {
    fn acquire() -> Option<Self> {
        UPDATE_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        UPDATE_RUNNING.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn run_update(
    app: AppHandle,
    server_exe: String,
    backup_targets: Vec<String>,
) -> Result<Vec<KeptFile>, String> {
    // Before anything else: the cleanup below would delete the running update's files
    let _lock = UpdateLock::acquire().ok_or("An update is already running.")?;
    let server_dir = server_dir_from_exe(&server_exe)?;
    let zip_path = server_dir.join("_update.zip");
    let staging_dir = server_dir.join("_update_staging");

    let result = run_update_inner(&app, &server_exe, &server_dir, &zip_path, &staging_dir, backup_targets).await;

    // Always clean up temp files regardless of outcome
    let _ = std::fs::remove_file(&zip_path);
    let _ = std::fs::remove_dir_all(&staging_dir);

    if result.is_ok() {
        emit_progress(&app, "done", 100.0, None);
    }

    result
}

fn ensure_server_stopped(app: &AppHandle, server_exe: &str) -> Result<(), String> {
    if crate::server::server_running(app, server_exe) {
        return Err("Stop the server first".into());
    }
    Ok(())
}

#[tauri::command]
pub fn create_backup(
    app: AppHandle,
    server_exe: String,
    targets: Vec<String>,
    label: String,
) -> Result<BackupManifest, String> {
    // A copy of Account.db taken mid-write is torn.
    ensure_server_stopped(&app, &server_exe)?;
    backup(&server_exe, targets, &label)
}

fn backup(server_exe: &str, targets: Vec<String>, label: &str) -> Result<BackupManifest, String> {
    create_backup_inner(&server_dir_from_exe(server_exe)?, targets, label)
}

#[tauri::command]
pub fn list_backups(server_exe: String) -> Result<Vec<BackupManifest>, String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
    let backups_dir = server_dir.join("Backups");

    if !backups_dir.exists() {
        return Ok(vec![]);
    }

    let mut manifests: Vec<BackupManifest> = Vec::new();

    for entry in std::fs::read_dir(&backups_dir)
        .map_err(|e| format!("Cannot read Backups directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Directory entry error: {e}"))?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let contents = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Ok(m) = serde_json::from_str::<BackupManifest>(&contents) {
            manifests.push(m);
        }
    }

    // Newest first
    manifests.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(manifests)
}

#[tauri::command]
pub fn restore_backup(app: AppHandle, server_exe: String, backup_id: String) -> Result<(), String> {
    // A running server writes its players back over the restored files.
    ensure_server_stopped(&app, &server_exe)?;
    restore(&server_exe, &backup_id)
}

fn restore(server_exe: &str, backup_id: &str) -> Result<(), String> {
    let server_dir = server_dir_from_exe(server_exe)?;
    let backup_dir = server_dir.join("Backups").join(&backup_id);

    if !backup_dir.exists() {
        return Err(format!("Backup '{backup_id}' not found"));
    }

    let contents = std::fs::read_to_string(backup_dir.join("manifest.json"))
        .map_err(|e| format!("Cannot read backup manifest: {e}"))?;
    let manifest: BackupManifest = serde_json::from_str(&contents)
        .map_err(|e| format!("Cannot parse backup manifest: {e}"))?;

    for target in &manifest.targets {
        let src = backup_dir.join(target);
        let dst = server_dir.join(target);
        copy_path(&src, &dst)?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_backup(server_exe: String, backup_id: String) -> Result<(), String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
    let backup_dir = server_dir.join("Backups").join(&backup_id);

    if !backup_dir.exists() {
        return Err(format!("Backup '{backup_id}' not found"));
    }

    std::fs::remove_dir_all(&backup_dir)
        .map_err(|e| format!("Cannot delete backup: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn get_backups_dir(server_exe: String) -> Result<String, String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
    let backups_dir = server_dir.join("Backups");
    std::fs::create_dir_all(&backups_dir)
        .map_err(|e| format!("Cannot create Backups directory: {e}"))?;
    backups_dir
        .to_str()
        .ok_or_else(|| "Backups path contains invalid UTF-8".to_string())
        .map(|s| s.to_string())
}

#[cfg(test)]
#[path = "../../tests/rust/test_updater.rs"]
mod tests;