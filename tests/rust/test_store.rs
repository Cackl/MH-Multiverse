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
