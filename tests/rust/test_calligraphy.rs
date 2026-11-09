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
    build_pak(&[
        ("Calligraphy/Blueprint.directory", build_blueprint_directory()),
        ("Calligraphy/Prototype.directory", build_prototype_directory()),
    ])
}

fn build_pak(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let compressed: Vec<Vec<u8>> = files.iter().map(|(_, raw)| lz4_store(raw)).collect();

    let mut out = Vec::new();
    out.extend_from_slice(&1196441931u32.to_le_bytes()); // PAK_SIGNATURE
    out.extend_from_slice(&1u32.to_le_bytes()); // PAK_VERSION
    out.extend_from_slice(&(files.len() as i32).to_le_bytes());

    let mut offset: i32 = 0;
    for ((path, raw), c) in files.iter().zip(&compressed) {
        out.extend_from_slice(&0u64.to_le_bytes()); // hash, unused
        out.extend(fixed_string32(path));
        out.extend_from_slice(&0i32.to_le_bytes()); // mod_time, unused
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&(c.len() as i32).to_le_bytes());
        out.extend_from_slice(&(raw.len() as i32).to_le_bytes());
        offset += c.len() as i32;
    }

    for c in compressed {
        out.extend(c);
    }
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
    ensure_catalogue_loaded(&state, &DisplayNameState::new(), server_exe, "").unwrap();
    state
}

#[test]
fn build_catalogue_populates_indices() {
    let (_tmp, server_exe) = setup_with_sip();
    let state = load(&server_exe);
    let guard = state.0.lock().unwrap();
    let (_, cat) = guard.as_ref().unwrap();
    assert_eq!(cat.by_id.len(), 3);
    assert_eq!(cat.blueprints.len(), 2);
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
    let err = ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe, "").unwrap_err();
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
    assert!(ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe, "").is_err());
}

#[test]
fn sip_not_found_errors() {
    let (_tmp, server_exe) = empty_server(); // Data/Game/Calligraphy.sip never created
    let state = CatalogueState(Mutex::new(None));
    let err = ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe, "").unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn ensure_catalogue_loaded_rebuilds_on_path_change() {
    let (_tmp1, server_exe1) = setup_with_sip();
    let state = CatalogueState(Mutex::new(None));
    ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe1, "").unwrap();

    let (_tmp2, server_exe2) = setup_with_sip();
    ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe2, "").unwrap();

    let cached_key = state.0.lock().unwrap().as_ref().unwrap().0.clone();
    assert!(cached_key.starts_with(&sip_path_for_server_exe(&server_exe2).unwrap()), "{cached_key}");
}

// ── Game names: skip-scanner fixtures ────────────────────────────────────────
//
// Hand-built bytes in the Calligraphy layout (MHServerEmu docs/GameData/Calligraphy.md).
// A wrong skip shifts every later read, so each fixture puts the interesting value after
// the thing being skipped and checks the scan ends exactly at the last byte.

const F_DISPLAY_NAME: u64 = 0xD1;
const F_NAME: u64 = 0xD2;

fn simple(field: u64, base_type: u8, value: &[u8]) -> Vec<u8> {
    let mut out = field.to_le_bytes().to_vec();
    out.push(base_type);
    out.extend_from_slice(value);
    out
}

fn list(field: u64, base_type: u8, values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = field.to_le_bytes().to_vec();
    out.push(base_type);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for v in values {
        out.extend_from_slice(v);
    }
    out
}

/// Prototype body: header (parent always written) + one field group.
fn proto_body(parent: u64, simples: &[Vec<u8>], lists: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0x03]; // ReferenceExists | InstanceDataExists
    out.extend_from_slice(&parent.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // one group
    out.extend_from_slice(&42u64.to_le_bytes()); // declaring blueprint
    out.push(0); // copy number
    out.extend_from_slice(&(simples.len() as u16).to_le_bytes());
    simples.iter().for_each(|s| out.extend_from_slice(s));
    out.extend_from_slice(&(lists.len() as u16).to_le_bytes());
    lists.iter().for_each(|l| out.extend_from_slice(l));
    out
}

fn proto_file(body: Vec<u8>) -> Vec<u8> {
    let mut out = b"PTP\x0b".to_vec();
    out.extend(body);
    out
}

fn name_fields() -> HashMap<u64, usize> {
    HashMap::from([(F_DISPLAY_NAME, 0), (F_NAME, 1)])
}

#[test]
fn scan_reads_parent_and_top_level_name_past_every_value_kind() {
    // An embedded struct whose own DisplayName must not count as the prototype's.
    let nested = proto_body(0, &[simple(F_DISPLAY_NAME, b'S', &99u64.to_le_bytes())], &[]);
    let file = proto_file(proto_body(
        7,
        &[
            simple(0xA0, b'L', &5i64.to_le_bytes()),
            simple(0xA1, b'R', &nested),
            simple(F_NAME, b'S', &77u64.to_le_bytes()),
        ],
        &[
            list(0xA2, b'P', &[1u64.to_le_bytes().to_vec(), 2u64.to_le_bytes().to_vec()]),
            list(0xA3, b'R', &[nested.clone()]),
        ],
    ));

    let scan = scan_prototype_file(&file, &name_fields()).unwrap();
    assert_eq!(scan.parent, 7);
    assert_eq!(scan.name, Some((1, 77)));
}

#[test]
fn scan_prefers_display_name_over_name() {
    let file = proto_file(proto_body(
        0,
        &[simple(F_NAME, b'S', &5u64.to_le_bytes()), simple(F_DISPLAY_NAME, b'S', &6u64.to_le_bytes())],
        &[],
    ));
    assert_eq!(scan_prototype_file(&file, &name_fields()).unwrap().name, Some((0, 6)));
}

#[test]
fn scan_rejects_short_long_and_unknown_data() {
    let good = proto_file(proto_body(0, &[simple(0xA0, b'D', &1f64.to_le_bytes())], &[]));
    let cases: [(&str, Vec<u8>); 3] = [
        ("truncated", good[..good.len() - 1].to_vec()),
        ("trailing byte", [good.clone(), vec![0]].concat()),
        ("unknown base type", proto_file(proto_body(0, &[simple(0xA0, b'Z', &[0; 8])], &[]))),
    ];
    for (label, bytes) in cases {
        assert!(scan_prototype_file(&bytes, &name_fields()).is_err(), "{label}");
    }
}

fn blueprint_file(binding: &str, parents: &[u64], members: &[(u64, &str, u8, bool)]) -> Vec<u8> {
    let mut out = b"BPT\x0b".to_vec();
    out.extend(fixed_string16(binding));
    out.extend_from_slice(&0u64.to_le_bytes()); // default prototype id
    out.extend_from_slice(&(parents.len() as u16).to_le_bytes());
    for p in parents {
        out.extend_from_slice(&p.to_le_bytes());
        out.push(1);
    }
    out.extend_from_slice(&1u16.to_le_bytes()); // one contributing blueprint
    out.extend_from_slice(&3u64.to_le_bytes());
    out.push(1);
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    for (id, name, base_type, list) in members {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend(fixed_string16(name));
        out.push(*base_type);
        out.push(if *list { b'L' } else { b'S' });
        if matches!(base_type, b'A' | b'C' | b'P' | b'R') {
            out.extend_from_slice(&0xFEu64.to_le_bytes()); // subtype
        }
    }
    out
}

#[test]
fn blueprint_gives_binding_parents_and_only_string_name_fields() {
    let file = blueprint_file(
        "AvatarPrototype",
        &[9],
        &[
            (0xB1, "Powers", b'P', true), // has a subtype: a wrong skip misreads what follows
            (F_DISPLAY_NAME, "DisplayName", b'S', false),
            (0xB2, "Name", b'L', false), // a Name that isn't a string id
            (F_NAME, "Name", b'S', false),
        ],
    );
    let mut fields = HashMap::new();
    let (binding, parents) = parse_blueprint_file(&file, &mut fields).unwrap();
    assert_eq!(binding, "AvatarPrototype");
    assert_eq!(parents, [9]);
    assert_eq!(fields, name_fields());
}

/// `.string` file with (id, default text, extra variant texts).
fn string_file(entries: &[(u64, &str, &[&str])]) -> Vec<u8> {
    let entry_len: usize = entries.iter().map(|(_, _, v)| 16 + v.len() * 14).sum();
    let mut text_offset = 4 + 2 + entry_len;
    let mut table = Vec::new();
    let mut texts = Vec::new();
    let mut push_text = |s: &str, table: &mut Vec<u8>| {
        table.extend_from_slice(&(text_offset as u32).to_le_bytes());
        texts.extend_from_slice(s.as_bytes());
        texts.push(0);
        text_offset += s.len() + 1;
    };
    for (id, text, variants) in entries {
        table.extend_from_slice(&id.to_le_bytes());
        table.extend_from_slice(&(variants.len() as u16 + 1).to_le_bytes());
        table.extend_from_slice(&0u16.to_le_bytes());
        push_text(text, &mut table);
        for v in *variants {
            table.extend_from_slice(&0u64.to_le_bytes());
            table.extend_from_slice(&0u16.to_le_bytes());
            push_text(v, &mut table);
        }
    }
    let mut out = b"STR\x0b".to_vec();
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend(table);
    out.extend(texts);
    out
}

#[test]
fn string_file_maps_ids_to_default_text_past_variants() {
    let file = string_file(&[(10, "Doctor Doom", &["Doktor Doom", "Docteur Fatalis"]), (11, "Ghost Rider", &[])]);
    let mut strings = HashMap::new();
    parse_string_file(&file, &mut strings).unwrap();
    assert_eq!(strings, HashMap::from([(10, "Doctor Doom".to_string()), (11, "Ghost Rider".to_string())]));
}

#[test]
fn clean_game_name_cases() {
    let cases = [
        ("Ghost Rider", Some("Ghost Rider")),
        ("#EnhancedKeywordColor#ENHANCED#/EnhancedKeywordColor# Shuri Costume", Some("ENHANCED Shuri Costume")),
        ("#orange#Double Legendary Mystic Scroll#/orange#", Some("Double Legendary Mystic Scroll")),
        ("$RegionName$", None),
        ("   ", None),
        ("#orange##/orange#", None),
    ];
    for (input, want) in cases {
        assert_eq!(clean_game_name(input).as_deref(), want, "{input:?}");
    }
}

#[test]
fn game_name_is_inherited_until_a_prototype_sets_one() {
    let scan = |parent, name| PrototypeScan { parent, name };
    let scans = HashMap::from([
        (1, scan(2, None)),          // child, no name of its own
        (2, scan(3, Some((0, 50)))), // parent: named
        (3, scan(0, Some((0, 51)))), // grandparent: a different name, never reached from 1
        (4, scan(2, Some((0, 0)))),  // explicitly unnamed: does not inherit
        (5, scan(999, None)),        // parent not in the archive
    ]);
    let strings = HashMap::from([(50, "Hero".to_string()), (51, "Base".to_string())]);
    let cases = [(1, Some("Hero")), (2, Some("Hero")), (3, Some("Base")), (4, None), (5, None)];
    for (id, want) in cases {
        assert_eq!(inherited_game_name(id, &scans, &strings).as_deref(), want, "prototype {id}");
    }
}

#[test]
fn build_catalogue_names_prototypes_from_client_strings() {
    let tmp = tempfile::tempdir().unwrap();
    let sip = tmp.path().join("Calligraphy.sip");
    let loco = tmp.path().join("eng.all");
    std::fs::create_dir(&loco).unwrap();
    std::fs::write(
        loco.join("eng.all_FFFFFFFFFFFFFFFF.string"),
        string_file(&[(7001, "#orange#Black Widow#/orange#", &[])]),
    )
    .unwrap();

    let avatar_members = [(F_DISPLAY_NAME, "DisplayName", b'S', false)];
    let ref_only = |parent: u64| [b"PTP\x0b".to_vec(), vec![0x01], parent.to_le_bytes().to_vec()].concat();
    std::fs::write(
        &sip,
        build_pak(&[
            ("Calligraphy/Blueprint.directory", build_blueprint_directory()),
            ("Calligraphy/Prototype.directory", build_prototype_directory()),
            ("Calligraphy/Entity/Avatar.blueprint", blueprint_file("AvatarPrototype", &[2], &avatar_members)),
            ("Calligraphy/Entity/Items/Consumables.blueprint", blueprint_file("ItemPrototype", &[], &[])),
            // AvatarBase (abstract) sets the name; BlackWidow only points at it.
            (
                "Calligraphy/Entity/Avatar/Prototypes/AvatarBase.prototype",
                proto_file(proto_body(1, &[simple(F_DISPLAY_NAME, b'S', &7001u64.to_le_bytes())], &[])),
            ),
            ("Calligraphy/Entity/Avatar/Prototypes/BlackWidow.prototype", ref_only(1002)),
            ("Calligraphy/Entity/Items/Consumables/Prototypes/HealthPotion.prototype", ref_only(2)),
        ]),
    )
    .unwrap();

    let cat = build_catalogue(sip.to_str().unwrap(), Some(&loco)).unwrap();
    let name = |leaf: &str| cat.prototypes.iter().find(|p| p.path.ends_with(leaf)).unwrap().game_name.clone();
    assert_eq!(name("BlackWidow.prototype").as_deref(), Some("Black Widow"));
    assert_eq!(name("HealthPotion.prototype"), None);
    assert_eq!(cat.classes[&1], ["AvatarPrototype", "ItemPrototype"]);

    // Without client strings nothing is named, and the build still succeeds.
    let cat = build_catalogue(sip.to_str().unwrap(), None).unwrap();
    assert!(cat.prototypes.iter().all(|p| p.game_name.is_none()));
}

// ── Display name order and client lookup ─────────────────────────────────────

#[test]
fn leaf_words_cases() {
    let cases = [
        ("Entity/Mobs/CowKingSpearLunge.prototype", "Cow King Spear Lunge"),
        ("Powers/Pet045GiantAntPower.prototype", "Pet045 Giant Ant Power"),
        ("Powers/MrFantasticConeRapidPunchHSEffec.prototype", "Mr Fantastic Cone Rapid Punch HS Effec"),
        ("Regions/DR_Victory_Room.prototype", "DR Victory Room"),
        ("NoFolder", "No Folder"),
    ];
    for (path, want) in cases {
        assert_eq!(leaf_words(path), want, "{path}");
    }
}

#[test]
fn resolve_name_order() {
    let path = "Entity/Items/HealthPotion.prototype";
    let cases = [
        ((Some("Mine"), Some("Game"), Some("Bundled")), ("Mine", "custom")),
        ((Some("#N/A"), Some("Game"), Some("Bundled")), ("Game", "game")),
        ((Some(" "), None, Some("Bundled")), ("Bundled", "bundled")),
        ((None, None, None), ("Health Potion", "path")),
    ];
    for ((custom, game, bundled), (name, source)) in cases {
        assert_eq!(resolve_name(custom, game, bundled, path), (name.to_string(), source), "{custom:?} {game:?} {bundled:?}");
    }
}

#[test]
fn loco_dir_prefers_the_client_then_the_server() {
    let tmp = tempfile::tempdir().unwrap();
    let game_exe = tmp.path().join("Marvel Heroes/UnrealEngine3/Binaries/Win64/MarvelHeroesOmega.exe");
    let client_loco = tmp.path().join("Marvel Heroes/Data/Game/Loco/eng.all");
    let server_exe = tmp.path().join("Server/MHServerEmu/MHServerEmu.exe");
    let server_loco = crate::paths::server_dir(server_exe.to_str().unwrap()).unwrap().join("Data/Game/Loco/eng.all");
    let (game, server) = (game_exe.to_str().unwrap(), server_exe.to_str().unwrap());

    assert_eq!(loco_dir(game, server), None);
    std::fs::create_dir_all(&server_loco).unwrap();
    assert_eq!(loco_dir(game, server), Some(server_loco.clone()));
    assert_eq!(loco_dir("", server), Some(server_loco));
    std::fs::create_dir_all(&client_loco).unwrap();
    assert_eq!(loco_dir(game, server), Some(client_loco));
}

#[test]
fn loaded_catalogue_uses_custom_names_and_rebuilds_when_the_client_appears() {
    let (_tmp, server_exe) = setup_with_sip();
    let server_dir = crate::paths::server_dir(&server_exe).unwrap();
    std::fs::write(
        server_dir.join("display_names_custom.json"),
        r#"{"Entity/Avatar/Prototypes/BlackWidow.prototype": "My Widow"}"#,
    )
    .unwrap();
    let dn = DisplayNameState::new();
    let state = CatalogueState(Mutex::new(None));
    let name = |state: &CatalogueState, id: u64| {
        let guard = state.0.lock().unwrap();
        name_for_id(&guard.as_ref().unwrap().1, id).map(str::to_string)
    };

    ensure_catalogue_loaded(&state, &dn, &server_exe, "").unwrap();
    assert_eq!(name(&state, 1001).as_deref(), Some("My Widow"));
    assert_eq!(name(&state, 1003).as_deref(), Some("Health Potion"));
    assert_eq!(name(&state, 4242), None);

    // A client string folder showing up later changes the cache key, so the catalogue is rebuilt.
    let before = state.0.lock().unwrap().as_ref().unwrap().0.clone();
    std::fs::create_dir_all(server_dir.join("Data/Game/Loco/eng.all")).unwrap();
    ensure_catalogue_loaded(&state, &dn, &server_exe, "").unwrap();
    assert_ne!(state.0.lock().unwrap().as_ref().unwrap().0, before);
}

/// Opt-in check against real game data (not run by default):
///   MHM_SERVER_EXE=<MHServerEmu.exe> MHM_GAME_EXE=<MarvelHeroesOmega.exe> \
///   cargo test --lib real_game_data -- --ignored --nocapture
#[test]
#[ignore = "needs MHM_SERVER_EXE and MHM_GAME_EXE pointing at a real install"]
fn real_game_data_names() {
    let (Ok(server_exe), Ok(game_exe)) = (std::env::var("MHM_SERVER_EXE"), std::env::var("MHM_GAME_EXE")) else {
        panic!("set MHM_SERVER_EXE and MHM_GAME_EXE");
    };
    let state = CatalogueState(Mutex::new(None));
    let start = std::time::Instant::now();
    ensure_catalogue_loaded(&state, &DisplayNameState::new(), &server_exe, &game_exe).unwrap();
    println!("catalogue + names built in {:?}", start.elapsed());

    let guard = state.0.lock().unwrap();
    let cat = &guard.as_ref().unwrap().1;
    let mut by_source: HashMap<&str, usize> = HashMap::new();
    for p in cat.prototypes.iter().filter(|p| !p.is_abstract) {
        *by_source.entry(p.name_source).or_default() += 1;
    }
    println!("non-abstract prototypes by name source: {by_source:?}");
    assert!(by_source.get("game").copied().unwrap_or(0) > 10_000, "client strings not found or not used");

    for (query, class) in [("doom", None), ("wolverine", Some("CostumePrototype")), ("xp", Some("ItemPrototype"))] {
        let start = std::time::Instant::now();
        let hits = search_catalogue(cat, query, &[], class);
        let top: Vec<String> = hits.iter().take(5).map(|m| format!("{} [{}]", m.display_name, m.class)).collect();
        println!("search {query:?} {class:?}: {} hits in {:?}; top: {top:?}", hits.len(), start.elapsed());
    }
}

// ── Search ranking and class filter ──────────────────────────────────────────

#[test]
fn match_rank_cases() {
    let cases = [
        ("doom bot", "entity/x.prototype", "doom", Some(0)),
        ("doctor doom", "entity/x.prototype", "doom", Some(1)),
        ("s.h.i.e.l.d. agent", "entity/x.prototype", "agent", Some(1)),
        ("kadoomer", "entity/x.prototype", "doom", Some(2)),
        ("x", "entity/doom/x.prototype", "doom", Some(3)),
        ("x", "entity/x.prototype", "doom", None),
        ("anything", "entity/x.prototype", "", Some(0)),
    ];
    for (name, path, query, want) in cases {
        assert_eq!(match_rank(name, path, query), want, "{name:?} / {query:?}");
    }
}

fn record(id: u64, blueprint_id: u64, path: &str, name: &str, is_abstract: bool) -> PrototypeRecord {
    let name_source = if name.starts_with("Doom Fight") { "path" } else { "game" };
    PrototypeRecord {
        prototype_id: id,
        prototype_guid: id + 1000,
        blueprint_id,
        path: path.to_string(),
        is_abstract,
        game_name: None,
        name: name.to_string(),
        name_source,
        search_name: name.to_lowercase(),
        search_path: path.to_lowercase(),
    }
}

fn search_fixture() -> PrototypeCatalogue {
    PrototypeCatalogue {
        blueprints: HashMap::new(),
        classes: HashMap::from([
            (1, vec!["CostumePrototype".to_string(), "ItemPrototype".to_string()]),
            (2, vec!["AgentPrototype".to_string()]),
        ]),
        // Deliberately listed worst match first, so ranking has to reorder them.
        prototypes: vec![
            record(1, 2, "Entity/Doom/Henchman.prototype", "Henchman", false),
            record(6, 2, "Missions/DoomFightPhase2.prototype", "Doom Fight Phase2", false), // name from the file name
            record(2, 2, "Entity/Mobs/Kadoomer.prototype", "Kadoomer", false),
            record(3, 1, "Entity/Items/Costumes/DoctorDoom.prototype", "Doctor Doom", false),
            record(4, 2, "Entity/Mobs/DoomBot.prototype", "Doom Bot", false),
            record(5, 2, "Entity/Mobs/DoomBase.prototype", "Doom Base", true),
        ],
        by_id: HashMap::new(),
    }
}

// The Store editor writes this id straight into ItemPrototypeRuntimeIdForClient.
#[test]
fn search_results_carry_the_runtime_id() {
    let cat = search_fixture();
    let hits = search_catalogue(&cat, "Doom Bot", &[], None);
    assert_eq!(hits[0].prototype_id, "4");
}

#[test]
fn search_ranks_name_matches_above_path_matches_and_skips_abstract() {
    let cat = search_fixture();
    let names: Vec<String> = search_catalogue(&cat, " DOOM ", &[], None).into_iter().map(|m| m.display_name).collect();
    assert_eq!(names, ["Doom Bot", "Doom Fight Phase2", "Doctor Doom", "Kadoomer", "Henchman"]);
}

#[test]
fn search_filters_by_class_ancestry_and_category_prefix() {
    let cat = search_fixture();

    let items = search_catalogue(&cat, "doom", &[], Some("ItemPrototype"));
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].display_name.as_str(), items[0].class.as_str()), ("Doctor Doom", "CostumePrototype"));
    assert_eq!(items[0].name_source, "game");

    let mobs: Vec<String> = search_catalogue(&cat, "", &["entity/mobs/".to_string()], None).into_iter().map(|m| m.display_name).collect();
    assert_eq!(mobs, ["Kadoomer", "Doom Bot"]);

    assert!(search_catalogue(&cat, "doom", &[], Some("AvatarPrototype")).is_empty());
}
