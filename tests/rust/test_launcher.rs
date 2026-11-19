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
        assert_eq!(is_game_process(process.as_ref(), game_exe), want, "{process} vs {game_exe:?}");
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
