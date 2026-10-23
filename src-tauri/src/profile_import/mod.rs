//! Import server profiles from other community launchers. Files are recognised by shape,
//! not name: Bifrost's `Bifrost.ServerList.json` (array), `Bifrost.LaunchConfig.json`
//! (supplies the single global login) and the MHO Overlay launcher's `launcher-settings.json`
//! (`servers` array with per-server logins). Passwords never leave the backend: the preview
//! only says whether a login exists, and the import re-reads the files.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{encrypt_password, load_config, save_config, Server};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_SERVERS: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub host: String,
    pub use_https: bool,
    pub email: String,
    pub has_password: bool,
    pub source: &'static str,
    pub duplicate: bool,
    #[serde(skip)]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct Preview {
    pub profiles: Vec<Candidate>,
    pub rejected: Vec<String>,
}

/// Case-insensitive key lookup (Bifrost is PascalCase, the overlay launcher camelCase).
fn get<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    o.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

fn get_str(o: &Map<String, Value>, key: &str) -> String {
    get(o, key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn get_bool(o: &Map<String, Value>, key: &str) -> bool {
    get(o, key).and_then(Value::as_bool).unwrap_or(false)
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// `host[:port]/SiteConfig.xml` (optionally with a scheme) -> (host, use_https).
/// The value ends up in a command-line argument, so anything but a plain host is refused.
fn parse_site_url(raw: &str) -> Result<(String, bool), String> {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    let (rest, https) = if lower.starts_with("https://") {
        (&raw[8..], true)
    } else if lower.starts_with("http://") {
        (&raw[7..], false)
    } else {
        (raw, false)
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if !path.is_empty() && !path.eq_ignore_ascii_case("SiteConfig.xml") {
        return Err("unsupported SiteConfig path".into());
    }
    let ok = !host.is_empty()
        && host.len() <= 255
        && host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:_[]".contains(c));
    if ok { Ok((host.to_string(), https)) } else { Err("invalid host".into()) }
}

/// Credentials are only usable if they carry no control characters and are a sane length.
fn valid_login(email: &str, password: &str) -> bool {
    let ok = |s: &str| s.len() <= 256 && !s.chars().any(char::is_control);
    !email.is_empty() && !password.is_empty() && ok(email) && ok(password)
}

fn candidate(
    o: &Map<String, Value>,
    name_key: &str,
    url_key: &str,
    source: &'static str,
    rejected: &mut Vec<String>,
) -> Option<Candidate> {
    let name = clean_text(&get_str(o, name_key), 100);
    let url = get_str(o, url_key);
    if name.is_empty() || url.is_empty() {
        rejected.push(format!("{source}: entry with a blank name or URL"));
        return None;
    }
    match parse_site_url(&url) {
        Ok((host, use_https)) => Some(Candidate {
            name, host, use_https, source,
            email: String::new(), has_password: false, duplicate: false, password: String::new(),
        }),
        Err(why) => {
            rejected.push(format!("{source}: \"{name}\" skipped ({why})"));
            None
        }
    }
}

fn set_login(c: &mut Candidate, email: &str, password: &str) {
    if valid_login(email, password) {
        c.email = email.to_string();
        c.password = password.to_string();
        c.has_password = true;
    }
}

/// Turn the uploaded files (name, parsed JSON) into candidates. Files are classified by shape.
pub fn parse_files(files: &[(String, Value)]) -> (Vec<Candidate>, Vec<String>) {
    let mut out: Vec<Candidate> = Vec::new();
    let mut rejected = Vec::new();
    // Bifrost's selected-server index refers to the original list, so keep a slot per entry.
    let mut bifrost_slots: Vec<Option<usize>> = Vec::new();
    let mut launch_cfg: Option<&Map<String, Value>> = None;

    for (fname, v) in files {
        match v {
            Value::Array(items) => {
                for item in items.iter().take(MAX_SERVERS) {
                    let c = match item.as_object() {
                        Some(o) => candidate(o, "Name", "SiteConfigUrl", "Bifrost", &mut rejected),
                        None => { rejected.push("Bifrost: entry is not an object".into()); None }
                    };
                    bifrost_slots.push(c.map(|c| { out.push(c); out.len() - 1 }));
                }
            }
            Value::Object(o) if get(o, "servers").is_some_and(Value::is_array) => {
                for item in get(o, "servers").and_then(Value::as_array).unwrap().iter().take(MAX_SERVERS) {
                    let Some(so) = item.as_object() else { continue };
                    let Some(mut c) = candidate(so, "name", "siteConfigUrl", "MHO Overlay", &mut rejected) else { continue };
                    if get_bool(so, "enableAutoLogin") {
                        set_login(&mut c, &get_str(so, "autoLoginEmailAddress"), &get_str(so, "autoLoginPassword"));
                    }
                    out.push(c);
                }
            }
            Value::Object(o) if get(o, "serverindex").is_some() && get(o, "enableautologin").is_some() => {
                launch_cfg = Some(o);
            }
            _ => rejected.push(format!("{fname}: not a recognised Bifrost or MHO Overlay file")),
        }
    }

    // Bifrost's login is global: attach it to the server it has selected, only if auto-login is on.
    if let Some(o) = launch_cfg.filter(|o| get_bool(o, "EnableAutoLogin")) {
        let idx = get(o, "ServerIndex").and_then(Value::as_u64).map(|i| i as usize);
        if let Some(c) = idx.and_then(|i| bifrost_slots.get(i).copied().flatten()).map(|i| &mut out[i]) {
            set_login(c, &get_str(o, "AutoLoginEmailAddress"), &get_str(o, "AutoLoginPassword"));
        }
    }
    (out, rejected)
}

/// What makes an imported profile a duplicate of one already known. Matches are skipped.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchBy {
    Host,
    /// So several accounts on one server all import. A profile with no email on either side
    /// matches any on that host (an earlier import without logins stores no email).
    #[default]
    HostEmail,
    HostName,
    /// Import everything.
    Nothing,
}

/// Flag candidates matching an existing profile (host, email, name) or an earlier one in the same
/// batch, case-insensitively, under the chosen strategy.
pub fn mark_duplicates(cands: &mut [Candidate], existing: &[(String, String, String)], by: MatchBy) {
    let lc = |(h, e, n): &(String, String, String)| (h.to_lowercase(), e.to_lowercase(), n.to_lowercase());
    let mut seen: Vec<_> = existing.iter().map(lc).collect();
    for c in cands {
        let k = lc(&(c.host.clone(), c.email.clone(), c.name.clone()));
        c.duplicate = seen.iter().any(|s| match by {
            MatchBy::Host => s.0 == k.0,
            MatchBy::HostEmail => s.0 == k.0 && (k.1.is_empty() || s.1.is_empty() || s.1 == k.1),
            MatchBy::HostName => s.0 == k.0 && s.2 == k.2,
            MatchBy::Nothing => false,
        });
        seen.push(k);
    }
}

fn read_files(paths: &[String]) -> Result<Vec<(String, Value)>, String> {
    paths.iter().map(|p| {
        let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let meta = std::fs::metadata(p).map_err(|e| format!("{name}: {e}"))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!("{name}: file is too large"));
        }
        let text = std::fs::read_to_string(p).map_err(|e| format!("{name}: {e}"))?;
        let v = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| format!("{name}: not valid JSON"))?;
        Ok((name, v))
    }).collect()
}

fn collect(
    app: &tauri::AppHandle,
    paths: &[String],
    include_credentials: bool,
    match_by: MatchBy,
) -> Result<(Vec<Candidate>, Vec<String>), String> {
    let (mut cands, rejected) = parse_files(&read_files(paths)?);
    let existing: Vec<_> = load_config(app).servers.iter()
        .filter(|s| !s.is_local)
        .map(|s| (s.host.clone(), s.email.clone(), s.name.clone()))
        .collect();
    // Match on the email shown in the preview whether or not it will be imported.
    mark_duplicates(&mut cands, &existing, match_by);
    if !include_credentials {
        for c in &mut cands {
            c.password.clear();
        }
    }
    Ok((cands, rejected))
}

#[tauri::command]
pub fn preview_profile_import(app: tauri::AppHandle, paths: Vec<String>, include_credentials: bool, match_by: MatchBy) -> Result<Preview, String> {
    let (profiles, rejected) = collect(&app, &paths, include_credentials, match_by)?;
    Ok(Preview { profiles, rejected })
}

/// Bifrost keeps its files in its own working directory: the client root or, typically, a
/// `Bifrost*` folder (often versioned) inside it. The game exe sits a few levels below the root,
/// so walk up from it; if several Bifrost folders match, take the last by name (newest version).
fn find_bifrost_dir(game_exe: &std::path::Path) -> Option<std::path::PathBuf> {
    for dir in game_exe.ancestors().skip(1).take(5) {
        let mut hits: Vec<std::path::PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten()
            .filter(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("bifrost"))
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        hits.sort();
        hits.push(dir.to_path_buf());
        if let Some(d) = hits.into_iter().rev().find(|d| d.join("Bifrost.ServerList.json").is_file()) {
            return Some(d);
        }
    }
    None
}

/// Look for known source files: Bifrost's near the game exe, the overlay launcher's in
/// LocalAppData. Returns the ones that exist, ready to pass to `preview_profile_import`.
#[tauri::command]
pub fn detect_profile_sources(app: tauri::AppHandle) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(dir) = find_bifrost_dir(std::path::Path::new(&load_config(&app).game_exe)) {
        found.extend(["Bifrost.ServerList.json", "Bifrost.LaunchConfig.json"].map(|f| dir.join(f)));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        found.push(std::path::Path::new(&local).join("MarvelHeroes.Injection.Launcher").join("launcher-settings.json"));
    }
    found.into_iter().filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).collect()
}

/// Add the profiles at positions `include` in the parsed list (the preview's row order), which
/// is how the user's per-row overrides of the match rule arrive. Logins are stored only when
/// `include_credentials` is set. Returns the number added.
#[tauri::command]
pub fn import_profiles(
    app: tauri::AppHandle,
    paths: Vec<String>,
    include_credentials: bool,
    include: Vec<usize>,
) -> Result<usize, String> {
    let (cands, _) = collect(&app, &paths, include_credentials, MatchBy::Nothing)?;
    let mut config = load_config(&app);
    let mut added = 0;
    for (_, c) in cands.into_iter().enumerate().filter(|(i, _)| include.contains(i)) {
        let with_login = include_credentials && c.has_password;
        let (password_enc, password_nonce) = if with_login { encrypt_password(&c.password)? } else { Default::default() };
        let id: String = rand::random::<[u8; 16]>().iter().map(|b| format!("{b:02x}")).collect();
        config.servers.push(Server {
            id,
            name: c.name,
            host: c.host,
            email: if with_login { c.email } else { String::new() },
            is_local: false,
            use_https: c.use_https,
            password_enc,
            password_nonce,
        });
        added += 1;
    }
    if added > 0 {
        save_config(&app, &config)?;
    }
    Ok(added)
}

#[cfg(test)]
mod tests;
