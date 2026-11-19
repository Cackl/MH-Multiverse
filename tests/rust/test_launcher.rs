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
