use super::*;

#[test]
fn copies_only_the_missing_files_and_keeps_the_rest() {
    let client = tempfile::tempdir().unwrap();
    let server = tempfile::tempdir().unwrap();
    let to = server.path().join("Data").join("Game");
    std::fs::write(client.path().join("Calligraphy.sip"), "client sip").unwrap();
    std::fs::write(client.path().join("mu_cdata.sip"), "client cdata").unwrap();
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join("Calligraphy.sip"), "server sip").unwrap();

    assert_eq!(missing_in(&to), vec!["mu_cdata.sip"]);
    assert_eq!(copy_missing(client.path(), &to).unwrap(), vec!["mu_cdata.sip"]);
    assert_eq!(std::fs::read_to_string(to.join("Calligraphy.sip")).unwrap(), "server sip");
    assert_eq!(std::fs::read_to_string(to.join("mu_cdata.sip")).unwrap(), "client cdata");
    assert!(!to.join("mu_cdata.sip.part").exists());

    // Nothing left to copy.
    assert!(missing_in(&to).is_empty());
    assert!(copy_missing(client.path(), &to).unwrap().is_empty());
}

#[test]
fn creates_the_server_folder_and_finds_the_client_data_above_the_exe() {
    let client = tempfile::tempdir().unwrap();
    let data = client.path().join("Data").join("Game");
    let exe = client.path().join("UnrealEngine3").join("Binaries").join("Win64").join("MarvelHeroes2016.exe");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    for f in FILES {
        std::fs::write(data.join(f), f).unwrap();
    }
    assert_eq!(client_game_dir(exe.to_str().unwrap()), Some(data.clone()));

    let server = tempfile::tempdir().unwrap();
    let to = server.path().join("Data").join("Game");
    assert_eq!(copy_missing(&data, &to).unwrap(), FILES.to_vec());
    assert!(missing_in(&to).is_empty());
}
