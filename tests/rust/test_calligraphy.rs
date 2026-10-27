use super::*;
use crate::test_helpers::empty_server;
use std::sync::Mutex;

// No real Calligraphy.sip fixture is used here (the real file is ~80MB and
// isn't worth requiring a manual copy step for). Instead these tests build a
// synthetic .sip in the exact binary layout load_pak/build_catalogue expect,
// which lets every code path (including the truncation/bad-signature error
// paths) run deterministically. The trade-off: this can't catch a real
// MHServerEmu file format differing from what the parser assumes — only a
// real fixture would catch that.

fn calligraphy_header(version: u8) -> Vec<u8> {
    vec![b'C', b'a', b'l', version]
}

fn fixed_string16(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = (bytes.len() as u16).to_le_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

fn fixed_string32(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = (bytes.len() as i32).to_le_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

/// Encodes `data` as a valid but maximally boring LZ4 block: one "last
/// sequence" token (literal-length only, no match) followed by the raw
/// bytes. The LZ4 block spec allows a block containing nothing but literals.
fn lz4_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let len = data.len();
    if len < 15 {
        out.push((len as u8) << 4);
    } else {
        out.push(0xF0);
        let mut remaining = len - 15;
        while remaining >= 255 {
            out.push(0xFF);
            remaining -= 255;
        }
        out.push(remaining as u8);
    }
    out.extend_from_slice(data);
    out
}

fn build_blueprint_directory() -> Vec<u8> {
    let mut out = calligraphy_header(11);
    let records: [(u64, u64, &str); 2] = [
        (1, 9001, "Entity/Avatar.blueprint"),
        (2, 9002, "Entity/Items/Consumables.blueprint"),
    ];
    out.extend_from_slice(&(records.len() as i32).to_le_bytes());
    for (id, guid, path) in records {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&guid.to_le_bytes());
        out.push(0); // flags, unused
        out.extend(fixed_string16(path));
    }
    out
}

/// (prototype_id, prototype_guid, blueprint_id, flags, path).
/// flags bit 0 = abstract.
const PROTOTYPES: [(u64, u64, u64, u8, &str); 3] = [
    (1001, 5001, 1, 0x00, "Entity/Avatar/Prototypes/BlackWidow.prototype"),
    (1002, 5002, 1, 0x01, "Entity/Avatar/Prototypes/AvatarBase.prototype"),
    (1003, 5003, 2, 0x00, "Entity/Items/Consumables/Prototypes/HealthPotion.prototype"),
];

fn build_prototype_directory() -> Vec<u8> {
    let mut out = calligraphy_header(11);
    out.extend_from_slice(&(PROTOTYPES.len() as i32).to_le_bytes());
    for (pid, guid, bpid, flags, path) in PROTOTYPES {
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&guid.to_le_bytes());
        out.extend_from_slice(&bpid.to_le_bytes());
        out.push(flags);
        out.extend(fixed_string16(path));
    }
    out
}

fn build_sip_bytes() -> Vec<u8> {
    let blueprint_raw = build_blueprint_directory();
    let prototype_raw = build_prototype_directory();
    let blueprint_compressed = lz4_store(&blueprint_raw);
    let prototype_compressed = lz4_store(&prototype_raw);

    let entries = [
        ("Calligraphy/Blueprint.directory", blueprint_compressed.len(), blueprint_raw.len()),
        ("Calligraphy/Prototype.directory", prototype_compressed.len(), prototype_raw.len()),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(&1196441931u32.to_le_bytes()); // PAK_SIGNATURE
    out.extend_from_slice(&1u32.to_le_bytes()); // PAK_VERSION
    out.extend_from_slice(&(entries.len() as i32).to_le_bytes());

    let mut offset: i32 = 0;
    for (path, csize, usize_) in &entries {
        out.extend_from_slice(&0u64.to_le_bytes()); // hash, unused
        out.extend(fixed_string32(path));
        out.extend_from_slice(&0i32.to_le_bytes()); // mod_time, unused
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&(*csize as i32).to_le_bytes());
        out.extend_from_slice(&(*usize_ as i32).to_le_bytes());
        offset += *csize as i32;
    }

    out.extend(blueprint_compressed);
    out.extend(prototype_compressed);
    out
}

fn write_sip(server_exe: &str, bytes: &[u8]) {
    let dir = crate::paths::server_dir(server_exe).unwrap().join("Data").join("Game");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Calligraphy.sip"), bytes).unwrap();
}

fn setup_with_sip() -> (tempfile::TempDir, String) {
    let (tmp, server_exe) = empty_server();
    write_sip(&server_exe, &build_sip_bytes());
    (tmp, server_exe)
}

fn load(server_exe: &str) -> CatalogueState {
    let state = CatalogueState(Mutex::new(None));
    ensure_catalogue_loaded(&state, server_exe).unwrap();
    state
}

#[test]
fn build_catalogue_populates_indices() {
    let (_tmp, server_exe) = setup_with_sip();
    let state = load(&server_exe);
    let guard = state.0.lock().unwrap();
    let (_, cat) = guard.as_ref().unwrap();
    assert_eq!(cat.by_id.len(), 3);
    assert_eq!(cat.by_path.len(), 3);
    assert_eq!(cat.blueprints.len(), 2);
}

#[test]
fn id_for_path_round_trip() {
    let (_tmp, server_exe) = setup_with_sip();
    let state = load(&server_exe);
    let guard = state.0.lock().unwrap();
    let (_, cat) = guard.as_ref().unwrap();

    let path = "Entity/Avatar/Prototypes/BlackWidow.prototype";
    let id = id_for_path(cat, path).unwrap();
    assert_eq!(id, 1001);
    assert_eq!(path_for_id(cat, id), Some(path));
}

#[test]
fn parsed_abstract_flag_matches_bit0() {
    let (_tmp, server_exe) = setup_with_sip();
    let state = load(&server_exe);
    let guard = state.0.lock().unwrap();
    let (_, cat) = guard.as_ref().unwrap();

    let abstract_count = cat.prototypes.iter().filter(|p| p.is_abstract).count();
    assert_eq!(abstract_count, 1);
    assert!(cat.prototypes.iter().any(|p| p.path.ends_with("AvatarBase.prototype") && p.is_abstract));
    assert!(cat.prototypes.iter().any(|p| p.path.ends_with("BlackWidow.prototype") && !p.is_abstract));
}

#[test]
fn load_pak_invalid_signature() {
    let (_tmp, server_exe) = empty_server();
    let mut bytes = build_sip_bytes();
    bytes[0] = 0; // corrupt the signature
    write_sip(&server_exe, &bytes);

    let state = CatalogueState(Mutex::new(None));
    let err = ensure_catalogue_loaded(&state, &server_exe).unwrap_err();
    assert!(err.contains("signature"), "unexpected error: {err}");
}

#[test]
fn load_pak_truncated() {
    // Regression test for H-3: a truncated/corrupt file must return an
    // Err from the bounds-checked readers, never panic on an out-of-range slice.
    let (_tmp, server_exe) = empty_server();
    let mut bytes = build_sip_bytes();
    bytes.truncate(bytes.len() / 2);
    write_sip(&server_exe, &bytes);

    let state = CatalogueState(Mutex::new(None));
    assert!(ensure_catalogue_loaded(&state, &server_exe).is_err());
}

#[test]
fn sip_not_found_errors() {
    let (_tmp, server_exe) = empty_server(); // Data/Game/Calligraphy.sip never created
    let state = CatalogueState(Mutex::new(None));
    let err = ensure_catalogue_loaded(&state, &server_exe).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn ensure_catalogue_loaded_rebuilds_on_path_change() {
    let (_tmp1, server_exe1) = setup_with_sip();
    let state = CatalogueState(Mutex::new(None));
    ensure_catalogue_loaded(&state, &server_exe1).unwrap();

    let (_tmp2, server_exe2) = setup_with_sip();
    ensure_catalogue_loaded(&state, &server_exe2).unwrap();

    let cached_path = state.0.lock().unwrap().as_ref().unwrap().0.clone();
    assert_eq!(cached_path, sip_path_for_server_exe(&server_exe2).unwrap());
}
