use super::*;

#[test]
fn server_dir_returns_parent() {
    let dir = server_dir("C:/Games/MHServerEmu/MHServerEmu.exe").unwrap();
    assert_eq!(dir, std::path::PathBuf::from("C:/Games/MHServerEmu"));
}

#[test]
fn server_dir_of_bare_filename_is_empty_path() {
    // Path::parent() on a bare filename returns Some("") rather than None,
    // so this resolves rather than erroring — worth pinning down since every
    // other module's path-derivation silently builds on top of this.
    assert_eq!(server_dir("MHServerEmu.exe").unwrap(), std::path::PathBuf::from(""));
}
