use super::*;
use crate::test_helpers::{empty_server, temp_server};
use std::fs;

fn setup() -> (tempfile::TempDir, String) {
    temp_server("MTXStore", "Data/Game/MTXStore")
}

fn empty_mtxstore() -> (tempfile::TempDir, String) {
    let (tmp, server_exe) = empty_server();
    fs::create_dir_all(mtxstore_dir(&server_exe).unwrap()).unwrap();
    (tmp, server_exe)
}

fn sample_entry(sku: &str) -> CatalogEntry {
    CatalogEntry {
        sku_id: sku.to_string(),
        guid_items: vec![],
        additional_guid_items: vec![],
        localized_entries: vec![LocalizedEntry {
            language_id: "en_us".to_string(),
            description: String::new(),
            title: "Sample".to_string(),
            release_date: String::new(),
            item_price: 100,
        }],
        info_urls: vec![],
        content_data: vec![],
        item_type: NamedItem { name: "Boost".to_string(), order: 5 },
        type_modifiers: vec![],
    }
}

#[test]
fn load_merges_base_and_modified() {
    let (_tmp, server_exe) = setup();
    let entries = load_catalog_entries(server_exe).unwrap();
    assert_eq!(entries.len(), 3);

    let e1001 = entries.iter().find(|e| e.entry.sku_id == "1001").unwrap();
    assert!(e1001.from_modified);
    assert_eq!(e1001.entry.localized_entries[0].item_price, 450);

    let e1002 = entries.iter().find(|e| e.entry.sku_id == "1002").unwrap();
    assert!(!e1002.from_modified);

    let e1003 = entries.iter().find(|e| e.entry.sku_id == "1003").unwrap();
    assert!(e1003.from_modified);
}

fn write_entries(dir: &std::path::Path, file: &str, entries: &[(&str, i64)]) {
    let list: Vec<CatalogEntryDisk> = entries
        .iter()
        .map(|(sku, price)| {
            let mut e = sample_entry(sku);
            e.localized_entries[0].item_price = *price;
            view_to_disk(e).unwrap()
        })
        .collect();
    fs::write(dir.join(file), serde_json::to_string(&list).unwrap()).unwrap();
}

#[test]
fn load_follows_server_file_order_and_reports_stock() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    // Server order: Catalog, CatalogMODIFIED, CatalogRestoredHeroes, CatalogRestoredHeroesMODIFIED.
    write_entries(&dir, "Catalog.json", &[("1", 100), ("2", 200)]);
    write_entries(&dir, "CatalogMODIFIED.json", &[("1", 110), ("2", 210), ("9", 900)]);
    write_entries(&dir, "CatalogRestoredHeroes.json", &[("2", 250), ("3", 300)]);
    write_entries(&dir, "CatalogRestoredHeroesMODIFIED.json", &[("3", 330)]);

    let entries = load_catalog_entries(server_exe).unwrap();
    let get = |sku: &str| entries.iter().find(|e| e.entry.sku_id == sku).unwrap();
    let price = |e: &CatalogEntry| e.localized_entries[0].item_price;

    let e1 = get("1");
    assert_eq!((price(&e1.entry), e1.source_file.as_str(), e1.from_modified), (110, "Catalog.json", true));
    assert_eq!(price(e1.stock.as_ref().unwrap()), 100);

    // CatalogRestoredHeroes.json loads after CatalogMODIFIED.json, so its 250 wins.
    let e2 = get("2");
    assert_eq!((price(&e2.entry), e2.source_file.as_str(), e2.from_modified), (250, "CatalogRestoredHeroes.json", false));
    assert_eq!(price(e2.stock.as_ref().unwrap()), 250);

    let e3 = get("3");
    assert_eq!((price(&e3.entry), e3.source_file.as_str(), e3.from_modified), (330, "CatalogRestoredHeroes.json", true));
    assert_eq!(price(e3.stock.as_ref().unwrap()), 300);

    let e9 = get("9");
    assert!(e9.from_modified && e9.stock.is_none());
    assert_eq!(entries.len(), 4);
}

#[test]
fn save_creates_modified_file() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();

    save_catalog_entry(server_exe, sample_entry("2001"), "CatalogBundle.json".to_string()).unwrap();

    assert!(dir.join("CatalogBundleMODIFIED.json").exists());
    assert!(!dir.join("CatalogBundleMODIFIED.json.bak").exists());
}

#[test]
fn save_creates_bak() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();

    save_catalog_entry(server_exe.clone(), sample_entry("2001"), "CatalogBundle.json".to_string()).unwrap();
    save_catalog_entry(server_exe, sample_entry("2002"), "CatalogBundle.json".to_string()).unwrap();

    assert!(dir.join("CatalogBundleMODIFIED.json.bak").exists());
}

#[test]
fn delete_removes_from_modified() {
    let (_tmp, server_exe) = setup();
    delete_catalog_entry(server_exe.clone(), "1001".to_string(), "CatalogBundle.json".to_string(), true).unwrap();

    // 1001 is gone from MODIFIED, so the base CatalogBundle.json value resurfaces.
    let entries = load_catalog_entries(server_exe).unwrap();
    assert_eq!(entries.len(), 3);
    let e1001 = entries.iter().find(|e| e.entry.sku_id == "1001").unwrap();
    assert!(!e1001.from_modified);
    assert_eq!(e1001.entry.localized_entries[0].item_price, 500);
}

#[test]
fn delete_refuses_stock_rows_and_leaves_every_file_alone() {
    let (_tmp, server_exe) = setup();
    let dir = mtxstore_dir(&server_exe).unwrap();
    let before = |f: &str| fs::read_to_string(dir.join(f)).unwrap();
    let (base, modified) = (before("CatalogBundle.json"), before("CatalogBundleMODIFIED.json"));

    // 1002 exists only in the base file; 1001 is in both.
    for sku in ["1002", "1001"] {
        let err = delete_catalog_entry(server_exe.clone(), sku.into(), "CatalogBundle.json".into(), false).unwrap_err();
        assert!(err.contains("stock"), "{err}");
    }
    // A path is never a catalog file name, even when the file it points at exists.
    let outside = dir.parent().unwrap().join("CatalogBundleMODIFIED.json");
    fs::write(&outside, &modified).unwrap();
    delete_catalog_entry(server_exe, "1001".into(), "../CatalogBundle.json".into(), true).unwrap_err();
    assert_eq!(fs::read_to_string(&outside).unwrap(), modified);

    assert_eq!(before("CatalogBundle.json"), base);
    assert_eq!(before("CatalogBundleMODIFIED.json"), modified);
    assert!(!dir.join("CatalogBundle.json.bak").exists());
}

#[test]
fn delete_missing_sku_errors() {
    let (_tmp, server_exe) = setup();
    let err = delete_catalog_entry(server_exe, "9999".to_string(), "CatalogBundle.json".to_string(), true).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn get_next_sku_id_scans_all_files() {
    let (_tmp, server_exe) = setup();
    assert_eq!(get_next_sku_id(server_exe).unwrap(), "1004");
}

#[test]
fn switching_a_file_off_and_on_moves_it_with_its_modified_file() {
    let (_tmp, server_exe) = setup();
    let dir = mtxstore_dir(&server_exe).unwrap();
    let off = dir.join("Off");
    let read = |p: std::path::PathBuf| fs::read_to_string(p).unwrap();
    let (base, modified) = (read(dir.join("CatalogBundle.json")), read(dir.join("CatalogBundleMODIFIED.json")));

    set_catalog_file_enabled(server_exe.clone(), "CatalogBundle.json".into(), false).unwrap();
    for f in ["CatalogBundle.json", "CatalogBundleMODIFIED.json"] {
        assert!(!dir.join(f).exists() && off.join(f).exists(), "{f}");
    }
    assert!(!list_catalog_files(server_exe.clone()).unwrap().contains(&"CatalogBundle.json".to_string()));
    assert_eq!(list_off_catalog_files(server_exe.clone()).unwrap(), ["CatalogBundle.json"]);
    assert!(load_catalog_entries(server_exe.clone()).unwrap().iter().all(|e| e.source_file != "CatalogBundle.json"));
    // Its SKUs stay taken, so a new entry can't clash with them later.
    assert_eq!(get_next_sku_id(server_exe.clone()).unwrap(), "1004");

    set_catalog_file_enabled(server_exe.clone(), "CatalogBundle.json".into(), true).unwrap();
    assert_eq!(read(dir.join("CatalogBundle.json")), base);
    assert_eq!(read(dir.join("CatalogBundleMODIFIED.json")), modified);
    assert!(list_off_catalog_files(server_exe).unwrap().is_empty());
}

#[test]
fn switching_refuses_bad_names_and_clashes_and_moves_nothing() {
    let (_tmp, server_exe) = setup();
    let dir = mtxstore_dir(&server_exe).unwrap();
    for name in ["../CatalogBundle.json", "CatalogBundleMODIFIED.json", "CatalogNope.json"] {
        set_catalog_file_enabled(server_exe.clone(), name.into(), false).unwrap_err();
    }
    assert!(!dir.join("Off").exists());

    // An older copy of the MODIFIED file already waits in Off: nothing moves, nothing is overwritten.
    fs::create_dir_all(dir.join("Off")).unwrap();
    fs::write(dir.join("Off").join("CatalogBundleMODIFIED.json"), "old").unwrap();
    let err = set_catalog_file_enabled(server_exe, "CatalogBundle.json".into(), false).unwrap_err();
    assert!(err.contains("Nothing was moved"), "{err}");
    assert!(dir.join("CatalogBundle.json").exists() && dir.join("CatalogBundleMODIFIED.json").exists());
    assert_eq!(fs::read_to_string(dir.join("Off").join("CatalogBundleMODIFIED.json")).unwrap(), "old");
}

#[test]
fn create_catalog_file_makes_an_empty_list_and_refuses_bad_or_taken_names() {
    let (_tmp, server_exe) = setup();
    let dir = mtxstore_dir(&server_exe).unwrap();

    assert_eq!(create_catalog_file(server_exe.clone(), "My_Items-2".into()).unwrap(), "CatalogMy_Items-2.json");
    assert_eq!(fs::read_to_string(dir.join("CatalogMy_Items-2.json")).unwrap(), "[]");
    assert!(list_catalog_files(server_exe.clone()).unwrap().contains(&"CatalogMy_Items-2.json".to_string()));

    set_catalog_file_enabled(server_exe.clone(), "CatalogMy_Items-2.json".into(), false).unwrap();
    let bundle = fs::read_to_string(dir.join("CatalogBundle.json")).unwrap();
    // Bundle and its MODIFIED file are on, My_Items-2 is off: none can be made again.
    for suffix in ["", "../Bad", "Has Space", "a.json", "Bundle", "bundle", "BundleMODIFIED", "Modified", "My_Items-2"] {
        create_catalog_file(server_exe.clone(), suffix.into()).unwrap_err();
    }
    assert_eq!(fs::read_to_string(dir.join("CatalogBundle.json")).unwrap(), bundle);
    assert!(!dir.join("CatalogMy_Items-2.json").exists());
    assert!(!dir.join("CatalogModified.json").exists());
}

#[test]
fn apache_bundles_found_only_when_the_folder_sits_next_to_the_server() {
    let tmp = tempfile::tempdir().unwrap();
    let server_exe = tmp.path().join("MHServerEmu").join("MHServerEmu.exe").to_string_lossy().to_string();
    assert!(!apache_bundles_found(server_exe.clone()));
    fs::create_dir_all(tmp.path().join("Apache24").join("htdocs")).unwrap();
    assert!(!apache_bundles_found(server_exe.clone()));
    fs::create_dir_all(tmp.path().join("Apache24").join("htdocs").join("bundles")).unwrap();
    assert!(apache_bundles_found(server_exe));
}

#[test]
fn bundle_page_lists_bonus_items_after_the_bought_ones() {
    let item = |id: &str, quantity| GuidItem {
        prototype_guid: "0".to_string(),
        item_prototype_runtime_id_for_client: id.to_string(),
        quantity,
    };
    let mut entry = sample_entry("1");
    entry.guid_items = vec![item("1", 1), item("2", 3)];
    let name = |id: &str| format!("Item{id}");
    assert_eq!(
        bundle_items_html(&entry, name),
        "            <li>Item1</li>\n            <li>Item2 x3</li>\n"
    );
    entry.additional_guid_items = vec![item("9", 1)];
    assert_eq!(
        bundle_items_html(&entry, name),
        "            <li>Item1</li>\n            <li>Item2 x3</li>\n        </ul>\n        <p>Free with purchase:</p>\n        <ul>\n            <li>Item9</li>\n"
    );
}

#[test]
fn get_next_sku_id_floor_1001_empty_dir() {
    let (_tmp, server_exe) = empty_mtxstore();
    assert_eq!(get_next_sku_id(server_exe).unwrap(), "1001");
}

#[test]
fn get_next_sku_id_floor_1001_missing_dir() {
    let (_tmp, server_exe) = empty_server();
    assert_eq!(get_next_sku_id(server_exe).unwrap(), "1001");
}

#[test]
fn disk_to_view_preserves_u64() {
    // 16313827754993193555 exceeds Number.MAX_SAFE_INTEGER — the whole reason
    // this field is transported as a string across the JS boundary.
    let disk = GuidItemDisk {
        prototype_guid: 1,
        item_prototype_runtime_id_for_client: 16313827754993193555u64,
        quantity: 1,
    };
    let view = guid_disk_to_view(disk);
    assert_eq!(view.item_prototype_runtime_id_for_client, "16313827754993193555");

    let back = guid_view_to_disk(view).unwrap();
    assert_eq!(back.item_prototype_runtime_id_for_client, 16313827754993193555u64);
}

#[test]
fn save_thumbnail_writes_file() {
    let (_tmp, server_exe) = empty_server();
    let path = save_thumbnail(
        server_exe,
        "test_slug".to_string(),
        "1001".to_string(),
        "AAECAw==".to_string(),
        false,
    )
    .unwrap();
    assert!(std::path::Path::new(&path).exists());
    assert_eq!(fs::read(&path).unwrap(), vec![0, 1, 2, 3]);
}

#[test]
fn save_thumbnail_rejects_invalid_base64() {
    let (_tmp, server_exe) = empty_server();
    let err = save_thumbnail(
        server_exe,
        "test_slug".to_string(),
        "1001".to_string(),
        "not-valid-base64!!".to_string(),
        false,
    )
    .unwrap_err();
    assert!(err.contains("Invalid base64"));
}

#[test]
fn write_with_backup_snapshots_previous_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("X.json");

    write_with_backup(&path, "one").unwrap();
    assert!(!tmp.path().join("X.json.bak").exists(), "nothing to back up on first write");

    write_with_backup(&path, "two").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "two");
    assert_eq!(fs::read_to_string(tmp.path().join("X.json.bak")).unwrap(), "one");
    assert!(!tmp.path().join("X.json.tmp").exists(), "temp file must not be left behind");
}

#[test]
fn unparseable_modified_file_blocks_save_and_delete_and_is_left_untouched() {
    for bad in ["", "{not json", "[{\"SkuId\": 1}]", "{\"SkuId\": 1}"] {
        let (_tmp, server_exe) = empty_mtxstore();
        let dir = mtxstore_dir(&server_exe).unwrap();
        let modified = dir.join("CatalogBundleMODIFIED.json");
        let bak = dir.join("CatalogBundleMODIFIED.json.bak");
        fs::write(&modified, bad).unwrap();
        fs::write(&bak, "previous good").unwrap();

        let err = save_catalog_entry(server_exe.clone(), sample_entry("2001"), "CatalogBundle.json".to_string())
            .unwrap_err();
        assert!(err.contains("CatalogBundleMODIFIED.json"), "error should name the file: {err}");
        delete_catalog_entry(server_exe, "2001".into(), "CatalogBundle.json".into(), true).unwrap_err();

        assert_eq!(fs::read_to_string(&modified).unwrap(), bad, "input {bad:?}");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "previous good", "input {bad:?}");
    }
}

#[test]
fn corrupt_modified_file_is_reported_when_listing() {
    let (_tmp, server_exe) = setup();
    let dir = mtxstore_dir(&server_exe).unwrap();
    fs::write(dir.join("CatalogBundleMODIFIED.json"), "{oops").unwrap();
    let err = load_catalog_entries(server_exe).unwrap_err();
    assert!(err.contains("CatalogBundleMODIFIED.json"), "{err}");
}

#[test]
fn bom_file_parses_and_values_beyond_i32_round_trip() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    let modified = dir.join("CatalogBundleMODIFIED.json");
    let json = r#"[{"SkuId":7,"GuidItems":[{"PrototypeGuid":18446744073709551615,"ItemPrototypeRuntimeIdForClient":1,"Quantity":1}],
        "AdditionalGuidItems":[],"LocalizedEntries":[{"LanguageId":"en_us","Description":"","Title":"T","ReleaseDate":"","ItemPrice":5000000000}],
        "InfoUrls":[],"ContentData":[],"Type":{"Name":"Boost","Order":5},"TypeModifiers":[]}]"#;
    fs::write(&modified, format!("\u{FEFF}{json}")).unwrap();

    save_catalog_entry(server_exe, sample_entry("8"), "CatalogBundle.json".to_string()).unwrap();

    let written = fs::read_to_string(&modified).unwrap();
    assert!(written.contains("\"PrototypeGuid\": 18446744073709551615"), "{written}");
    assert!(written.contains("\"ItemPrice\": 5000000000"), "{written}");
    assert!(written.contains("\"SkuId\": 8"), "{written}");
}

fn upsert(sku: &str, price: i64, target: &str) -> StoreUpsert {
    let mut entry = sample_entry(sku);
    entry.localized_entries[0].item_price = price;
    StoreUpsert { entry, target_file: target.to_string() }
}

fn removal(sku: &str, source: &str) -> StoreRemoval {
    StoreRemoval { sku_id: sku.to_string(), source_file: source.to_string() }
}

fn skus(path: &std::path::Path) -> Vec<u64> {
    load_disk_entries(path).unwrap().iter().map(|e| e.sku_id).collect()
}

#[test]
fn apply_store_changes_writes_each_file_once_and_never_touches_base() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    write_entries(&dir, "Catalog.json", &[("1", 100), ("2", 200)]);
    write_entries(&dir, "CatalogMODIFIED.json", &[("1", 110), ("2", 210)]);
    write_entries(&dir, "CatalogLimitedCostumesMODIFIED.json", &[("5", 500)]);
    let base_before = fs::read_to_string(dir.join("Catalog.json")).unwrap();
    let limited_before = fs::read_to_string(dir.join("CatalogLimitedCostumesMODIFIED.json")).unwrap();

    apply_store_changes(
        server_exe.clone(),
        vec![upsert("1", 120, "Catalog.json"), upsert("3", 300, "Catalog.json"), upsert("6", 600, "CatalogLimitedCostumes.json")],
        vec![removal("2", "Catalog.json"), removal("7", "Catalog.json")],
        Some(serde_json::json!({ "version": 1 })),
    )
    .unwrap();

    assert_eq!(fs::read_to_string(dir.join("Catalog.json")).unwrap(), base_before);
    assert_eq!(skus(&dir.join("CatalogMODIFIED.json")), vec![1, 3]);
    assert_eq!(load_disk_entries(&dir.join("CatalogMODIFIED.json")).unwrap()[0].localized_entries[0].item_price, 120);
    assert_eq!(skus(&dir.join("CatalogLimitedCostumesMODIFIED.json")), vec![5, 6]);
    // One .bak per file, holding the file as it was before this call.
    assert_eq!(skus(&dir.join("CatalogMODIFIED.json.bak")), vec![1, 2]);
    assert_eq!(fs::read_to_string(dir.join("CatalogLimitedCostumesMODIFIED.json.bak")).unwrap(), limited_before);
    assert_eq!(load_store_state(server_exe).unwrap()["version"], 1);
}

#[test]
fn apply_store_changes_writes_nothing_when_any_file_fails_to_parse() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    write_entries(&dir, "CatalogMODIFIED.json", &[("1", 110)]);
    fs::write(dir.join("CatalogRestoredHeroesMODIFIED.json"), "{oops").unwrap();
    let good_before = fs::read_to_string(dir.join("CatalogMODIFIED.json")).unwrap();

    let err = apply_store_changes(
        server_exe.clone(),
        vec![upsert("1", 120, "Catalog.json"), upsert("3", 300, "CatalogRestoredHeroes.json")],
        vec![],
        Some(serde_json::json!({ "version": 1 })),
    )
    .unwrap_err();

    assert!(err.contains("CatalogRestoredHeroesMODIFIED.json"), "{err}");
    assert_eq!(fs::read_to_string(dir.join("CatalogMODIFIED.json")).unwrap(), good_before);
    assert_eq!(fs::read_to_string(dir.join("CatalogRestoredHeroesMODIFIED.json")).unwrap(), "{oops");
    assert!(!dir.join("CatalogMODIFIED.json.bak").exists());
    assert!(load_store_state(server_exe).unwrap().is_null());
}

#[test]
fn apply_store_changes_refuses_bad_targets_and_negative_prices() {
    for (target, price) in [("../Catalog.json", 1), ("CatalogMODIFIED.json", 1), ("Events.json", 1), ("Catalog.json", -1)] {
        let (_tmp, server_exe) = empty_mtxstore();
        let dir = mtxstore_dir(&server_exe).unwrap();
        apply_store_changes(server_exe, vec![upsert("1", price, target)], vec![], None).unwrap_err();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0, "{target} {price}");
    }
}

#[test]
fn move_aside_never_overwrites_an_earlier_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("X.json");

    fs::write(&path, "first").unwrap();
    let a = move_aside(&path).unwrap();
    fs::write(&path, "second").unwrap();
    let b = move_aside(&path).unwrap();
    assert_ne!(a, b, "two moves in the same second get different names");
    assert!(!path.exists());

    // An ordinary save keeps its own single .bak and leaves both copies alone.
    write_with_backup(&path, "third").unwrap();
    write_with_backup(&path, "fourth").unwrap();
    assert_eq!(fs::read_to_string(&a).unwrap(), "first");
    assert_eq!(fs::read_to_string(&b).unwrap(), "second");
    let name = file_name_of(&a);
    assert!(name.starts_with("X.json.") && name.ends_with(".bak"), "{name}");
}

#[test]
fn a_read_only_file_never_leaves_a_read_only_bak_or_a_tmp() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("X.json");
    let bak = tmp.path().join("X.json.bak");
    let set_readonly = |p: &Path, on: bool| {
        let mut perms = fs::metadata(p).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(on);
        fs::set_permissions(p, perms).unwrap();
    };

    // Saving over a read-only file may fail (it does on Windows), but the .bak stays writable
    // and no .tmp is left behind.
    fs::write(&path, "one").unwrap();
    set_readonly(&path, true);
    let _ = write_with_backup(&path, "two");
    assert!(!fs::metadata(&bak).unwrap().permissions().readonly());
    assert!(!tmp.path().join("X.json.tmp").exists());

    // An older read-only .bak doesn't block the next save.
    set_readonly(&path, false);
    fs::write(&path, "three").unwrap();
    set_readonly(&bak, true);
    write_with_backup(&path, "four").unwrap();
    assert_eq!(fs::read_to_string(&bak).unwrap(), "three");
    assert_eq!(fs::read_to_string(&path).unwrap(), "four");
}

#[test]
fn save_catalog_entry_refuses_a_negative_price() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    let mut e = sample_entry("1");
    e.localized_entries[0].item_price = -1;
    save_catalog_entry(server_exe, e, "Catalog.json".into()).unwrap_err();
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn catalog_files_load_with_fields_the_server_defaults() {
    let (_tmp, server_exe) = empty_mtxstore();
    let dir = mtxstore_dir(&server_exe).unwrap();
    fs::write(dir.join("Catalog.json"), r#"[{
        "SkuId": 7,
        "GuidItems": [{ "ItemPrototypeRuntimeIdForClient": 5 }],
        "AdditionalGuidItems": [],
        "LocalizedEntries": [{ "LanguageId": "en_us", "Description": "", "Title": "T", "ReleaseDate": "", "ItemPrice": 1 }],
        "InfoUrls": [{ "LanguageId": "en_us", "Url": null, "ImageData": null }],
        "ContentData": [],
        "Type": { "Name": "Boost", "Order": 1 },
        "TypeModifiers": []
    }]"#).unwrap();
    let entries = load_catalog_entries(server_exe).unwrap();
    let g = &entries[0].entry.guid_items[0];
    assert_eq!((g.prototype_guid.as_str(), g.quantity), ("0", 0));
    assert_eq!(entries[0].entry.info_urls[0].image_data, "");
}
