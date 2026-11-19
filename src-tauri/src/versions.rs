//! Tells which game version (1.48, 1.52 or 1.53) a client or server exe is for.
//! A fork reads as its base version, so Custom stays a tag the user sets.

use std::path::Path;

/// (game version string, short name). Server builds embed the string, clients carry it as
/// their file version.
const VERSIONS: [(&str, &str); 3] = [
    ("1.48.0.1712", "1.48"),
    ("1.52.0.1700", "1.52"),
    ("1.53.0.203", "1.53"),
];

/// The one game version string `MHServerEmu.dll` holds as UTF-16. None when it holds none,
/// or more than one, since then it can't be told apart.
fn server_version_in(dll: &[u8]) -> Option<&'static str> {
    let mut found = VERSIONS.iter().filter(|(full, _)| {
        let needle: Vec<u8> = full.encode_utf16().flat_map(u16::to_le_bytes).collect();
        dll.windows(needle.len()).any(|w| w == needle)
    });
    let (_, short) = found.next()?;
    found.next().is_none().then_some(*short)
}

/// Clients write their version with commas ("1,52,0,1700").
fn client_version_from(product_version: &str) -> Option<&'static str> {
    let normalized: String = product_version
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    VERSIONS.iter().find(|(full, _)| *full == normalized).map(|(_, short)| *short)
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
