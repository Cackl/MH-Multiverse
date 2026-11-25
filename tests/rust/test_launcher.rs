use super::*;

#[test]
fn normalize_host_strips_scheme_and_path_but_keeps_port() {
    for (raw, expected) in [
        ("https://example.com", "example.com"),
        ("example.com/foo/bar", "example.com"),
        ("example.com:8080", "example.com:8080"),
        ("example.com", "example.com"),
        ("http://host:9090/path", "host:9090"),
        ("  example.com  ", "example.com"),
    ] {
        assert_eq!(normalize_host_str(raw), expected, "input {raw:?}");
    }
}

#[test]
fn game_process_matches_the_linked_exe_name() {
    let omega = r"D:\Steam\Marvel Heroes\UnrealEngine3\Binaries\Win64\MarvelHeroesOmega.exe";
    let v148 = r"D:\Games\MHO-1.48\UnrealEngine3\Binaries\Win64\MarvelHeroes2016.exe";
    for (process, game_exe, want) in [
        ("MarvelHeroesOmega.exe", omega, true),
        ("MarvelHeroes2016.exe", v148, true),
        ("marvelheroes2016.EXE", v148, true),
        ("MarvelHeroesOmega.exe", v148, false),
        ("MarvelHeroes2016.exe", omega, false),
        ("MarvelHeroesOmega.exe", "", false),
    ] {
        assert_eq!(is_game_process(process.as_ref(), None, game_exe), want, "{process} vs {game_exe:?}");
    }
}

#[test]
fn game_process_must_run_from_the_linked_install() {
    let v153 = r"D:\Games\MHO-1.53\UnrealEngine3\Binaries\Win64\MarvelHeroesOmega.exe";
    let name = "MarvelHeroesOmega.exe";
    for (exe, want) in [
        (v153, true),
        (r"d:/games/mho-1.53/unrealengine3/binaries/win64/marvelheroesomega.EXE", true),
        (r"D:\Games\MHO-1.53\UnrealEngine3\Binaries\Win32\MarvelHeroesOmega.exe", true),
        // The 1.52 Steam client ships the same exe name.
        (r"D:\Steam\Marvel Heroes\UnrealEngine3\Binaries\Win64\MarvelHeroesOmega.exe", false),
        (r"D:\Steam\Marvel Heroes\UnrealEngine3\Binaries\Win32\MarvelHeroesOmega.exe", false),
        (r"D:\Games\MHO-1.53\UnrealEngine3\Binaries\Other\MarvelHeroesOmega.exe", false),
        // Unreadable path (sysinfo reports it empty): fall back to the name.
        ("", true),
    ] {
        assert_eq!(is_game_process(name.as_ref(), Some(exe.as_ref()), v153), want, "{exe}");
    }
}

#[test]
fn steam_mode_is_off_for_the_1_48_and_1_53_clients() {
    use crate::config::{AppConfig, LinkedInstall};
    let config_for = |no_steam: bool, version: &str, active: &str| {
        let mut config = AppConfig::default();
        config.launch_options.no_steam = no_steam;
        config.linked = vec![LinkedInstall {
            id: "a".into(), name: "a".into(), version: version.into(),
            game_exe: String::new(), server_exe: String::new(), custom: false, no_steam,
        }];
        config.active_linked_id = active.into();
        config
    };
    for (no_steam, version, active, want) in [
        (false, "1.52", "a", true),
        (false, "", "a", true),
        (false, "1.48", "a", false),
        (false, "1.53", "a", false),
        (true, "1.52", "a", false),
        // Nothing active: today's behaviour.
        (false, "1.48", "", true),
    ] {
        assert_eq!(use_steam(&config_for(no_steam, version, active)), want, "no_steam={no_steam} {version} active={active:?}");
    }
}

#[test]
fn scrub_launch_logs_masks_every_password_and_leaves_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let line = |pw: &str| format!("Init: Command line: -steam -emailaddress=a@b.c -password={pw}\r\n");
    let two = format!("{}Log: other\r\n{}", line("hunter2"), line("with space\""));
    std::fs::write(tmp.path().join("Launch.log"), &two).unwrap();
    // Non-UTF-8 byte must survive untouched.
    let mut backup = line("p@ss").into_bytes();
    backup.extend_from_slice(b"Log: \xE9t\xE9\r\n");
    std::fs::write(tmp.path().join("Launch-backup-2026.11.21-22.07.29.log"), &backup).unwrap();
    std::fs::write(tmp.path().join("Other.log"), line("keep")).unwrap();

    scrub_launch_logs(tmp.path());

    let read = |n: &str| std::fs::read(tmp.path().join(n)).unwrap();
    assert_eq!(read("Launch.log"), format!("{}Log: other\r\n{}", line("***"), line("***")).into_bytes());
    let mut want = line("***").into_bytes();
    want.extend_from_slice(b"Log: \xE9t\xE9\r\n");
    assert_eq!(read("Launch-backup-2026.11.21-22.07.29.log"), want);
    assert_eq!(read("Other.log"), line("keep").into_bytes());

    // Running again changes nothing.
    scrub_launch_logs(tmp.path());
    assert_eq!(read("Launch.log"), format!("{}Log: other\r\n{}", line("***"), line("***")).into_bytes());
}
