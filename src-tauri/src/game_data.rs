//! The server needs the client's game data files in its own Data/Game. Copies them over
//! when they are missing, never replacing one that is there.

use std::path::{Path, PathBuf};

const FILES: [&str; 2] = ["Calligraphy.sip", "mu_cdata.sip"];

/// The files missing from a Data/Game folder.
fn missing_in(game_dir: &Path) -> Vec<&'static str> {
    FILES.into_iter().filter(|f| !game_dir.join(f).is_file()).collect()
}

/// Copies the files `to` is missing from `from`. Each one goes to a .part file first, so a
/// copy that stops halfway never looks like a finished one. Returns the files copied.
fn copy_missing(from: &Path, to: &Path) -> Result<Vec<&'static str>, String> {
    let missing = missing_in(to);
    std::fs::create_dir_all(to).map_err(|e| format!("Cannot create {}: {e}", to.display()))?;
    for f in &missing {
        let part = to.join(format!("{f}.part"));
        std::fs::copy(from.join(f), &part).map_err(|e| format!("Cannot copy {f}: {e}"))?;
        std::fs::rename(&part, to.join(f)).map_err(|e| format!("Cannot copy {f}: {e}"))?;
    }
    Ok(missing)
}

fn server_game_dir(server_exe: &str) -> Result<PathBuf, String> {
    Ok(crate::paths::server_dir(server_exe)?.join("Data").join("Game"))
}

/// The client's Data/Game: searched up from the game exe, as `overlay::sip_candidates` does.
fn client_game_dir(game_exe: &str) -> Option<PathBuf> {
    Path::new(game_exe)
        .ancestors()
        .skip(1)
        .take(5)
        .map(|d| d.join("Data").join("Game"))
        .find(|d| missing_in(d).is_empty())
}

#[tauri::command]
pub fn missing_game_data(server_exe: String) -> Result<Vec<&'static str>, String> {
    Ok(missing_in(&server_game_dir(&server_exe)?))
}

#[tauri::command]
pub async fn copy_game_data(game_exe: String, server_exe: String) -> Result<Vec<&'static str>, String> {
    let from = client_game_dir(&game_exe).ok_or("The game folder has no Calligraphy.sip and mu_cdata.sip")?;
    let to = server_game_dir(&server_exe)?;
    tokio::task::spawn_blocking(move || copy_missing(&from, &to)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
#[path = "../../tests/rust/test_game_data.rs"]
mod tests;
