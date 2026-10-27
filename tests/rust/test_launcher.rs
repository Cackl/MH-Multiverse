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
