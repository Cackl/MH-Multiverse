// ── paths.rs ──────────────────────────────────────────────────────────────
//
// Shared path-derivation helper. Previously reimplemented independently in
// ini.rs, store.rs, updater.rs, tuning.rs, patches.rs, calligraphy.rs, and
// accounts.rs.

use std::path::{Path, PathBuf};

/// Returns the directory containing `server_exe`.
pub(crate) fn server_dir(server_exe: &str) -> Result<PathBuf, String> {
    Path::new(server_exe)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())
}
