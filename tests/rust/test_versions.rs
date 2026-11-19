use super::*;

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// A fake dll: some bytes, the strings, some more bytes.
fn dll_with(strings: &[Vec<u8>]) -> Vec<u8> {
    let mut dll = vec![0x4D, 0x5A, 0x90, 0x00, 0x03];
    for s in strings {
        dll.extend_from_slice(s);
        dll.extend_from_slice(&[0, 0, 0x41, 0]);
    }
    dll
}

#[test]
fn server_version_reads_the_embedded_utf16_string() {
    let cases = [
        (dll_with(&[utf16("1.48.0.1712")]), Some("1.48")),
        (dll_with(&[utf16("1.52.0.1700")]), Some("1.52")),
        (dll_with(&[utf16("1.53.0.203")]), Some("1.53")),
        // ASCII is not how the dll stores it.
        (dll_with(&[b"1.52.0.1700".to_vec()]), None),
        // PS4 and anything else is out of scope.
        (dll_with(&[utf16("1.4.0.26")]), None),
        // Two versions in one dll: can't tell.
        (dll_with(&[utf16("1.48.0.1712"), utf16("1.53.0.203")]), None),
        (Vec::new(), None),
    ];
    for (dll, want) in cases {
        assert_eq!(server_version_in(&dll), want);
    }
}

#[test]
fn client_version_reads_the_comma_file_version() {
    let cases = [
        ("1,48,0,1712", Some("1.48")),
        ("1,52,0,1700", Some("1.52")),
        ("1, 53, 0, 203", Some("1.53")),
        ("1.52.0.1700", Some("1.52")),
        ("1,52,0,1699", None),
        ("1,0,3,0", None),
        ("", None),
    ];
    for (version, want) in cases {
        assert_eq!(client_version_from(version), want, "{version}");
    }
}

#[test]
fn server_version_is_none_without_a_dll() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("MHServerEmu.exe");
    assert_eq!(server_version(exe.to_str().unwrap()), None);
    std::fs::write(dir.path().join("MHServerEmu.dll"), dll_with(&[utf16("1.53.0.203")])).unwrap();
    assert_eq!(server_version(exe.to_str().unwrap()), Some("1.53"));
}
