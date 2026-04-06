use chrono::{Duration, Local, Timelike, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub build_date: String,
    pub download_url: String,
    pub available: bool,
}

use crate::config::UpdateBackupOptions;

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
    Path::new(server_exe)
        .parent()
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())
        .map(|p| p.to_path_buf())
}

fn target_build_date() -> String {
    let now = Utc::now();
    let seconds_of_day = now.hour() * 3600 + now.minute() * 60 + now.second();
    let cutoff = 7 * 3600 + 15 * 60; // 07:15 UTC — when nightly build completes
    let target = if seconds_of_day < cutoff {
        now - Duration::days(1)
    } else {
        now
    };
    target.format("%Y%m%d").to_string()
}

fn build_download_url(build_date: &str) -> String {
    format!(
        "https://nightly.link/Crypto137/MHServerEmu/workflows/nightly-release-windows-x64/master/MHServerEmu-nightly-{build_date}-Release-windows-x64.zip"
    )
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
        .user_agent("mh-manifold/1.0")
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))?;

    let response = client.get(url).send().await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Download failed with HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest).await
        .map_err(|e| format!("Cannot create download file: {e}"))?;

    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Download stream error: {e}"))?;
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
            Some(entry.name().to_string()),
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
                .map_err(|e| format!("Cannot extract {}: {e}", entry.name()))?;
        }
    }

    Ok(())
}

async fn run_update_inner(
    app: &AppHandle,
    server_dir: &Path,
    zip_path: &Path,
    staging_dir: &Path,
    backup_options: UpdateBackupOptions,
) -> Result<(), String> {
    // Guard: server must not be running
    {
        let state = app.state::<crate::server::ServerState>();
        let mut proc = state.0.lock()
            .map_err(|_| "Cannot acquire server state lock".to_string())?;
        if let Some(ref mut child) = proc.child {
            match child.try_wait() {
                Ok(None) => {
                    return Err("Cannot update while the server is running. Stop the server first.".into());
                }
                _ => {}
            }
        }
    }

    // Pre-update backup
    let backup_manifest: Option<BackupManifest> = {
        let mut targets: Vec<String> = Vec::new();
        if backup_options.config_ini    { targets.push("Config.ini".into()); }
        if backup_options.live_tuning   { targets.push("Data/Game/LiveTuning".into()); }
        if backup_options.account_db { targets.push("Data/Account.db".into()); }

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
    let build_date = target_build_date();
    let url = build_download_url(&build_date);
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
    if let Some(manifest) = backup_manifest {
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

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn check_update_available() -> Result<UpdateInfo, String> {
    let build_date = target_build_date();
    let download_url = build_download_url(&build_date);

    let client = reqwest::Client::builder()
        .user_agent("mh-manifold/1.0")
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))?;

    let available = client
        .get(&download_url)
        .header("Range", "bytes=0-0")
        .send()
        .await
        .map(|r| r.status().is_success() || r.status().as_u16() == 206)
        .unwrap_or(false);

    Ok(UpdateInfo { build_date, download_url, available })
}

#[tauri::command]
pub async fn run_update(
    app: AppHandle,
    server_exe: String,
    backup_options: UpdateBackupOptions,
) -> Result<(), String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
    let zip_path = server_dir.join("_update.zip");
    let staging_dir = server_dir.join("_update_staging");

    let result = run_update_inner(&app, &server_dir, &zip_path, &staging_dir, backup_options).await;

    // Always clean up temp files regardless of outcome
    let _ = std::fs::remove_file(&zip_path);
    let _ = std::fs::remove_dir_all(&staging_dir);

    if result.is_ok() {
        emit_progress(&app, "done", 100.0, None);
    }

    result
}

#[tauri::command]
pub fn create_backup(
    server_exe: String,
    targets: Vec<String>,
    label: String,
) -> Result<BackupManifest, String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
    create_backup_inner(&server_dir, targets, &label)
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
pub fn restore_backup(server_exe: String, backup_id: String) -> Result<(), String> {
    let server_dir = server_dir_from_exe(&server_exe)?;
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