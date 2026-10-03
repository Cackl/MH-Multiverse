//! Self-update for MH Multiverse itself (the server updater lives in `updater.rs`).
//!
//! Every install must pass all of the following before the running exe is touched:
//! the SHA-256 digest GitHub publishes for the asset, a minisign signature from
//! `PUBKEY`, a trusted comment naming the expected file, and a version check
//! (the msi file name / exe version resource must equal the release tag). The
//! version check stops an old genuine release being replayed under a newer tag.

use base64::Engine;
use futures_util::StreamExt;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

const REPO: &str = "Cackl/MH-Multiverse";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Base64 public key printed by `npx tauri signer generate`. While empty, checks
/// still work but installs are refused.
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDlEMTdGMzY2RjE4QzNBRjMKUldUek9venhadk1YbmZHVzRMc0RhT25ZRnErNjN4Z2FxM3lWT0x1dkRTRGFwUDFwRHpaeEcxcU8K";

const PORTABLE_ASSET: &str = "mh-multiverse.exe";
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;
const MAX_SIG_BYTES: u64 = 16 * 1024;
const ALLOWED_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseNote {
    pub version: String,
    pub body: String,
    pub published_at: Option<String>,
    pub html_url: String,
    pub breaking: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateInfo {
    pub current: String,
    pub latest: String,
    pub up_to_date: bool,
    pub html_url: String,
    /// Every release newer than the running one, newest first.
    pub notes: Vec<ReleaseNote>,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    stage: &'static str,
    pct: f32,
}

fn emit_progress(app: &AppHandle, stage: &'static str, pct: f32) {
    let _ = app.emit("app-update-progress", ProgressPayload { stage, pct });
}

// ── Release selection ─────────────────────────────────────────────────────────

/// `v1.2.3` / `1.2.3` → Version. Pre-release and build-tagged versions are rejected.
fn parse_tag(tag: &str) -> Option<Version> {
    let v = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
    (v.pre.is_empty() && v.build.is_empty()).then_some(v)
}

/// A release is flagged breaking by a `## Breaking Changes` heading in its notes.
fn is_breaking(body: &str) -> bool {
    body.lines()
        .any(|l| l.trim().to_ascii_lowercase().starts_with("## breaking changes"))
}

/// Published, non-prerelease releases newer than `current`, newest first.
fn newer_releases(releases: Vec<GhRelease>, current: &Version) -> Vec<(Version, GhRelease)> {
    let mut newer: Vec<_> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| parse_tag(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, _)| v > current)
        .collect();
    newer.sort_by(|a, b| b.0.cmp(&a.0));
    newer
}

fn build_info(releases: Vec<GhRelease>, current: &Version) -> AppUpdateInfo {
    let newer = newer_releases(releases, current);
    let (latest, html_url) = match newer.first() {
        Some((v, r)) => (v.to_string(), r.html_url.clone()),
        None => (current.to_string(), format!("https://github.com/{REPO}/releases")),
    };
    AppUpdateInfo {
        current: current.to_string(),
        up_to_date: newer.is_empty(),
        latest,
        html_url,
        notes: newer
            .into_iter()
            .map(|(v, r)| {
                let body = r.body.unwrap_or_default();
                ReleaseNote {
                    version: v.to_string(),
                    breaking: is_breaking(&body),
                    body,
                    published_at: r.published_at,
                    html_url: r.html_url,
                }
            })
            .collect(),
    }
}

// ── HTTP ──────────────────────────────────────────────────────────────────────

fn host_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("mh-multiverse/{APP_VERSION}"))
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 {
                attempt.error("too many redirects")
            } else if host_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirected to an untrusted host")
            }
        }))
        .build()
        .map_err(|e| format!("Cannot create HTTP client: {e}"))
}

async fn get_json<T: serde::de::DeserializeOwned>(client: &reqwest::Client, url: &str) -> Result<T, String> {
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Cannot reach GitHub: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| format!("Cannot read GitHub response: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Unexpected GitHub response: {e}"))
}

/// Streams `asset` to `dest`, enforcing the advertised size, and returns the SHA-256.
async fn download(
    app: &AppHandle,
    client: &reqwest::Client,
    asset: &GhAsset,
    dest: &Path,
) -> Result<[u8; 32], String> {
    let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| format!("Bad asset URL: {e}"))?;
    if !host_allowed(&url) {
        return Err("Asset URL is not on GitHub".into());
    }
    if asset.size == 0 || asset.size > MAX_DOWNLOAD_BYTES {
        return Err(format!("Unexpected asset size: {} bytes", asset.size));
    }
    let resp = client.get(url).send().await.map_err(|e| format!("Download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }

    let mut file = tokio::fs::File::create(dest).await.map_err(|e| {
        format!("Cannot write to {}: {e}", dest.parent().unwrap_or(dest).display())
    })?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrupted: {e}"))?;
        received += chunk.len() as u64;
        if received > asset.size {
            return Err("Download is larger than GitHub advertised".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| format!("Cannot write download: {e}"))?;
        emit_progress(app, "downloading", received as f32 / asset.size as f32 * 100.0);
    }
    file.flush().await.map_err(|e| format!("Cannot write download: {e}"))?;
    if received != asset.size {
        return Err("Download is incomplete".into());
    }
    Ok(hasher.finalize().into())
}

async fn download_small(client: &reqwest::Client, asset: &GhAsset) -> Result<String, String> {
    let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| format!("Bad asset URL: {e}"))?;
    if !host_allowed(&url) || asset.size > MAX_SIG_BYTES {
        return Err("Signature asset rejected".into());
    }
    let resp = client.get(url).send().await.map_err(|e| format!("Download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Signature download failed with HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("Signature download failed: {e}"))?;
    if bytes.len() as u64 > MAX_SIG_BYTES {
        return Err("Signature asset rejected".into());
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "Signature is not text".into())
}

// ── Verification ──────────────────────────────────────────────────────────────

fn verify_digest(expected: Option<&str>, actual: &[u8; 32]) -> Result<(), String> {
    let expected = expected
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or("GitHub did not publish a SHA-256 digest for this file")?;
    let actual: String = actual.iter().map(|b| format!("{b:02x}")).collect();
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err("Downloaded file does not match GitHub's SHA-256 digest".into());
    }
    Ok(())
}

fn b64_text(s: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|_| "Invalid base64".to_string())?;
    String::from_utf8(bytes).map_err(|_| "Invalid UTF-8".to_string())
}

/// Checks a Tauri-format (base64-wrapped minisign) signature over `data`, and that
/// its trusted comment names `asset_name`. GitHub turns spaces in uploaded file
/// names into dots, so the signed name is compared the same way.
fn verify_signature(data: &[u8], sig_b64: &str, pubkey_b64: &str, asset_name: &str) -> Result<(), String> {
    let pk = minisign_verify::PublicKey::decode(&b64_text(pubkey_b64)?)
        .map_err(|e| format!("Invalid update public key: {e}"))?;
    let sig = minisign_verify::Signature::decode(&b64_text(sig_b64)?)
        .map_err(|e| format!("Invalid signature file: {e}"))?;
    pk.verify(data, &sig, false)
        .map_err(|_| "Signature check failed: this file was not signed by the MH Multiverse release key")?;
    let signed_name = sig
        .trusted_comment()
        .split('\t')
        .find_map(|p| p.strip_prefix("file:"))
        .ok_or("Signature does not name a file")?;
    if signed_name.replace(' ', ".") != asset_name {
        return Err(format!("Signature is for '{signed_name}', not '{asset_name}'"));
    }
    Ok(())
}

/// FileVersion from the exe's version resource (major, minor, patch).
fn pe_file_version(path: &Path) -> Result<Version, String> {
    use windows::core::{w, HSTRING};
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };
    let err = || "Cannot read the downloaded exe's version".to_string();
    let p = HSTRING::from(path.as_os_str());
    unsafe {
        let size = GetFileVersionInfoSizeW(&p, None);
        if size == 0 {
            return Err(err());
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(&p, None, size, buf.as_mut_ptr().cast()).map_err(|_| err())?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(buf.as_ptr().cast(), w!("\\"), &mut ptr, &mut len).as_bool()
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return Err(err());
        }
        let info = &*(ptr as *const VS_FIXEDFILEINFO);
        Ok(Version::new(
            (info.dwFileVersionMS >> 16) as u64,
            (info.dwFileVersionMS & 0xffff) as u64,
            (info.dwFileVersionLS >> 16) as u64,
        ))
    }
}

// ── Install ───────────────────────────────────────────────────────────────────

/// Tauri's MSI installs per-machine into Program Files; anything else is the portable exe.
fn is_msi_install(exe: &Path) -> bool {
    let exe = exe.to_string_lossy().to_lowercase();
    ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .any(|dir| exe.starts_with(&format!("{}\\", dir.to_lowercase().trim_end_matches('\\'))))
}

fn with_suffix(exe: &Path, suffix: &str) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    exe.with_file_name(name)
}

/// Swaps the running exe for `new`. Windows allows renaming a running exe, so the
/// old one moves aside to `.old` (deleted on next start) and is restored on failure.
fn swap_exe(exe: &Path, new: &Path) -> Result<(), String> {
    let old = with_suffix(exe, ".old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("Cannot move the current exe aside: {e}"))?;
    if let Err(e) = std::fs::rename(new, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(format!("Cannot put the new exe in place: {e}"));
    }
    Ok(())
}

/// Removes leftovers from a previous self-update. Runs on a background thread at
/// startup; retries because the previous process may still be exiting.
pub fn cleanup_leftovers() {
    let Ok(exe) = std::env::current_exe() else { return };
    let old = with_suffix(&exe, ".old");
    let _ = std::fs::remove_file(with_suffix(&exe, ".new"));
    for _ in 0..20 {
        if !old.exists() || std::fs::remove_file(&old).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("MH.Multiverse_") && name.ends_with("_x64_en-US.msi") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

async fn install_inner(app: &AppHandle, target: &Version, exe: &Path, dest: &Path, msi: bool) -> Result<(), String> {
    let client = client()?;
    let release: GhRelease =
        get_json(&client, &format!("https://api.github.com/repos/{REPO}/releases/tags/v{target}")).await?;
    if release.draft || release.prerelease || parse_tag(&release.tag_name).as_ref() != Some(target) {
        return Err(format!("v{target} is not a published release"));
    }

    let asset_name = if msi { format!("MH.Multiverse_{target}_x64_en-US.msi") } else { PORTABLE_ASSET.to_string() };
    let find = |name: &str| release.assets.iter().find(|a| a.name == name).cloned();
    let asset = find(&asset_name).ok_or_else(|| format!("Release v{target} has no {asset_name}"))?;
    let sig_asset = find(&format!("{asset_name}.sig"))
        .ok_or_else(|| format!("Release v{target} is not signed ({asset_name}.sig is missing)"))?;

    let sig = download_small(&client, &sig_asset).await?;
    emit_progress(app, "downloading", 0.0);
    let hash = download(app, &client, &asset, dest).await?;

    emit_progress(app, "verifying", 0.0);
    verify_digest(asset.digest.as_deref(), &hash)?;
    let data = tokio::fs::read(dest).await.map_err(|e| format!("Cannot read download: {e}"))?;
    verify_signature(&data, &sig, PUBKEY, &asset_name)?;
    drop(data);
    if !msi {
        let v = pe_file_version(dest)?;
        if &v != target {
            return Err(format!("Downloaded exe reports version {v}, expected {target}"));
        }
    }

    emit_progress(app, "installing", 100.0);
    if msi {
        std::process::Command::new("msiexec")
            .arg("/i")
            .arg(dest)
            .args(["/passive", "AUTOLAUNCHAPP=True"])
            .spawn()
            .map_err(|e| format!("Cannot start the installer: {e}"))?;
    } else {
        swap_exe(exe, dest)?;
        if let Err(e) = std::process::Command::new(exe).spawn() {
            // Update is in place; only the restart failed.
            return Err(format!("Updated to v{target}, but could not restart ({e}). Please reopen MH Multiverse."));
        }
    }
    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn check_app_update() -> Result<AppUpdateInfo, String> {
    let current = Version::parse(APP_VERSION).map_err(|e| e.to_string())?;
    let releases: Vec<GhRelease> =
        get_json(&client()?, &format!("https://api.github.com/repos/{REPO}/releases?per_page=30")).await?;
    Ok(build_info(releases, &current))
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle, version: String) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Updates can't be installed from a dev build".into());
    }
    if PUBKEY.is_empty() {
        return Err("This build has no update signing key, so it can't install updates".into());
    }
    if crate::server::server_process_is_running(&app.state::<crate::server::ServerState>()) {
        return Err("Stop the server before updating".into());
    }
    let current = Version::parse(APP_VERSION).map_err(|e| e.to_string())?;
    let target = parse_tag(&version).ok_or("Invalid version")?;
    if target <= current {
        return Err(format!("v{target} is not newer than v{current}"));
    }

    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate the running exe: {e}"))?;
    let msi = is_msi_install(&exe);
    let dest = if msi {
        std::env::temp_dir().join(format!("MH.Multiverse_{target}_x64_en-US.msi"))
    } else {
        with_suffix(&exe, ".new")
    };

    match install_inner(&app, &target, &exe, &dest, msi).await {
        Ok(()) => {
            crate::shutdown(&app);
            Ok(())
        }
        Err(e) => {
            // The msi is still needed by msiexec on success, so it's only removed here.
            let _ = std::fs::remove_file(&dest);
            Err(e)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Throwaway keypair and signatures made with `tauri signer` over the bytes b"fixture".
    const TEST_PUB: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDUzMEI1QTM2MjEzOEUxQzcKUldUSDRUZ2hObG9MVTRkbVJKUEVTbTJPTjVzRmxaWkhnMWRRaHNOeFNOY1FDYjhQQyttRkFJRUQK";
    const OTHER_PUB: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDRCRkNFMzQwRTkzMjFFM0IKUldRN0hqTHBRT1A4U3c5cExtNVpiTE9xT1lIM1JOMklubjhCOXJnL0t1K0g0VThzYzdrc0VBMHkK";
    const EXE_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUSDRUZ2hObG9MVXk1SkI1eHF1Z1JmWFF2b3dhQmYvY1NhbXRVL25Sc3I1MTk1MFRTQkw5Q1c3SmNET0RoR1NPRlFBWG1wdkZhcitIUnorMjJuREpMV0lQclpnekNLUUEwPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkwMjM3ODk3CWZpbGU6bWgtbXVsdGl2ZXJzZS5leGUKQ2pneHNRU2FuQUVyNGZTMFBEd0VYWlI2WHZweXZ1RHp0ZGd5KzdnRCtlcUNDM2JtNGt3R0ZhcDRxeTZ4ZHJRRmF3MTRXL0ZXWGZvU0NNSndFcm1kQkE9PQo=";
    const MSI_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUSDRUZ2hObG9MVXpsMGdpclY5YlJ6T3lPbjY3dEtJS0ROcU9MdE5VUVU5NGtxcUE1VlIxRU00L3hRT3laSHc5cEFWRjlwVWxVdGEzZ1JvSlBmWVFTSlZJMkdvWGl0UUFJPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkwMjM3ODk4CWZpbGU6TUggTXVsdGl2ZXJzZV8xLjQuMF94NjRfZW4tVVMubXNpCkVOb2tkakJhTGkwRmU3Z0Y1ejc1Q1BNV09oTjdjZzRnQ3hPTy8zWlN3UWZ0cDlYaTdvTkxzZ21Na1NXN1BMSnFtbTJNOGpaZGxkWFJtZWxoR1hOM0R3PT0K";
    const MSI_140: &str = "MH.Multiverse_1.4.0_x64_en-US.msi";

    fn rel(tag: &str, draft: bool, pre: bool, body: &str) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            draft,
            prerelease: pre,
            body: Some(body.into()),
            html_url: format!("https://github.com/{REPO}/releases/tag/{tag}"),
            published_at: None,
            assets: vec![],
        }
    }

    #[test]
    fn picks_highest_published_release_and_skips_junk() {
        let releases = vec![
            rel("v1.3.4", false, false, ""),
            rel("v2.0.0", true, false, ""),       // draft
            rel("v1.9.0", false, true, ""),       // prerelease
            rel("v1.6.0-beta", false, false, ""), // semver prerelease
            rel("nightly", false, false, ""),
            rel("v1.5.0", false, false, ""),
            rel("v1.3.3", false, false, ""),
            rel("v1.4.0", false, false, ""),
        ];
        let info = build_info(releases, &Version::new(1, 3, 3));
        assert!(!info.up_to_date);
        assert_eq!(info.latest, "1.5.0");
        let versions: Vec<_> = info.notes.iter().map(|n| n.version.as_str()).collect();
        assert_eq!(versions, ["1.5.0", "1.4.0", "1.3.4"]);
    }

    #[test]
    fn up_to_date_when_nothing_newer() {
        let info = build_info(vec![rel("v1.3.3", false, false, ""), rel("v1.2.0", false, false, "")], &Version::new(1, 3, 3));
        assert!(info.up_to_date);
        assert_eq!(info.latest, "1.3.3");
        assert!(info.notes.is_empty());
    }

    #[test]
    fn breaking_flag_is_reported_on_skipped_versions() {
        let releases = vec![
            rel("v1.5.0", false, false, "# 1.5.0\n## What's Changed\n- stuff"),
            rel("v1.4.0", false, false, "# 1.4.0\r\n## Breaking Changes\r\n- config moved"),
        ];
        let info = build_info(releases, &Version::new(1, 3, 3));
        assert_eq!(info.notes.iter().filter(|n| n.breaking).map(|n| n.version.as_str()).collect::<Vec<_>>(), ["1.4.0"]);
        assert!(!is_breaking("### Breaking Changes is a subheading, not the flag"));
    }

    #[test]
    fn digest_must_be_present_and_match() {
        let hash: [u8; 32] = Sha256::digest(b"fixture").into();
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        assert!(verify_digest(Some(&format!("sha256:{hex}")), &hash).is_ok());
        assert!(verify_digest(Some(&format!("sha256:{}", "0".repeat(64))), &hash).is_err());
        assert!(verify_digest(None, &hash).is_err());
        assert!(verify_digest(Some(&hex), &hash).is_err());
    }

    #[test]
    fn signature_accepts_genuine_file() {
        assert!(verify_signature(b"fixture", EXE_SIG, TEST_PUB, PORTABLE_ASSET).is_ok());
        // Signed as "MH Multiverse_…", uploaded to GitHub as "MH.Multiverse_…".
        assert!(verify_signature(b"fixture", MSI_SIG, TEST_PUB, MSI_140).is_ok());
    }

    #[test]
    fn signature_rejects_tampering_wrong_key_and_replay() {
        assert!(verify_signature(b"fixturf", EXE_SIG, TEST_PUB, PORTABLE_ASSET).is_err());
        assert!(verify_signature(b"fixture", EXE_SIG, OTHER_PUB, PORTABLE_ASSET).is_err());
        // Genuine 1.4.0 msi signature replayed as a 1.5.0 release.
        assert!(verify_signature(b"fixture", MSI_SIG, TEST_PUB, "MH.Multiverse_1.5.0_x64_en-US.msi").is_err());
        // Exe signature presented for the msi.
        assert!(verify_signature(b"fixture", EXE_SIG, TEST_PUB, MSI_140).is_err());
        // Edited trusted comment (file name swapped) breaks the global signature.
        let text = b64_text(MSI_SIG).unwrap().replace("1.4.0", "1.5.0");
        let forged = base64::engine::general_purpose::STANDARD.encode(text);
        assert!(verify_signature(b"fixture", &forged, TEST_PUB, "MH.Multiverse_1.5.0_x64_en-US.msi").is_err());
    }

    #[test]
    fn redirects_only_to_github_over_https() {
        let ok = |u: &str| host_allowed(&reqwest::Url::parse(u).unwrap());
        assert!(ok("https://release-assets.githubusercontent.com/x"));
        assert!(!ok("http://github.com/x"));
        assert!(!ok("https://github.com.evil.example/x"));
    }

    #[test]
    fn parses_tags() {
        assert_eq!(parse_tag("v1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(parse_tag("1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(parse_tag("v1.2"), None);
        assert_eq!(parse_tag("v1.2.3-rc1"), None);
    }
}
