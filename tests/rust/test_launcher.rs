use super::*;

#[test]
fn normalize_host_strips_scheme() {
    assert_eq!(normalize_host_str("https://example.com"), "example.com");
}

#[test]
fn normalize_host_strips_path() {
    assert_eq!(normalize_host_str("example.com/foo/bar"), "example.com");
}

#[test]
fn normalize_host_preserves_port() {
    assert_eq!(normalize_host_str("example.com:8080"), "example.com:8080");
}

#[test]
fn normalize_host_bare() {
    assert_eq!(normalize_host_str("example.com"), "example.com");
}

#[test]
fn normalize_host_with_everything() {
    assert_eq!(normalize_host_str("http://host:9090/path"), "host:9090");
}

#[test]
fn normalize_host_trims_whitespace() {
    assert_eq!(normalize_host_str("  example.com  "), "example.com");
}
