//! Tells which game version (1.48, 1.52 or 1.53) a client or server exe is for.
//! A fork reads as its base version, so Custom stays a tag the user sets.

use std::path::Path;

/// (game version string, short name, game build number). Server builds embed the string, clients
/// carry it as their file version, and saves carry the build number (MHServerEmu's GameBuildNumber).
const VERSIONS: [(&str, &str, u32); 3] = [
    ("1.48.0.1712", "1.48", 391562),
    ("1.52.0.1700", "1.52", 479899),
    ("1.53.0.203", "1.53", 493640),
];

/// The one game version string `MHServerEmu.dll` holds as UTF-16. None when it holds none,
/// or more than one, since then it can't be told apart.
fn server_version_in(dll: &[u8]) -> Option<&'static str> {
    let mut found = VERSIONS.iter().filter(|(full, _, _)| {
        let needle: Vec<u8> = full.encode_utf16().flat_map(u16::to_le_bytes).collect();
        dll.windows(needle.len()).any(|w| w == needle)
    });
    let (_, short, _) = found.next()?;
    found.next().is_none().then_some(*short)
}

/// The game build number a server of this version writes into its saves.
pub fn game_build(short: &str) -> Option<u32> {
    VERSIONS.iter().find(|(_, s, _)| *s == short).map(|(_, _, build)| *build)
}

/// "1.48" for 391562, or the bare number for a build we don't know.
pub fn build_label(build: u32) -> String {
    VERSIONS
        .iter()
        .find(|(_, _, b)| *b == build)
        .map_or_else(|| format!("game build {build}"), |(_, short, _)| short.to_string())
}

/// Clients write their version with commas ("1,52,0,1700").
fn client_version_from(product_version: &str) -> Option<&'static str> {
    let normalized: String = product_version
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    VERSIONS.iter().find(|(full, _, _)| *full == normalized).map(|(_, short, _)| *short)
}

pub fn server_version(server_exe: &str) -> Option<&'static str> {
    let dll = Path::new(server_exe).parent()?.join("MHServerEmu.dll");
    server_version_in(&std::fs::read(dll).ok()?)
}

pub fn client_version(game_exe: &str) -> Option<&'static str> {
    client_version_from(&crate::updater::product_version(Path::new(game_exe))?)
}

#[tauri::command]
pub fn detect_server_version(server_exe: String) -> Option<&'static str> {
    server_version(&server_exe)
}

#[tauri::command]
pub fn detect_client_version(game_exe: String) -> Option<&'static str> {
    client_version(&game_exe)
}

#[cfg(test)]
#[path = "../../tests/rust/test_versions.rs"]
mod tests;
