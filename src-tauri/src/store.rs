use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::calligraphy::CatalogueState;

// ── Embedded display names ────────────────────────────────────────────────────

static DISPLAY_NAMES_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/display_names.json"));

// ── Display name state ────────────────────────────────────────────────────────

/// Holds the embedded prototype path → display name map plus an optional
/// per-server-directory override map loaded from `display_names_custom.json`.
///
/// The embedded map is immutable after construction. The custom map is loaded
/// lazily on first use and refreshed if the server directory changes.
pub struct DisplayNameState {
    embedded: HashMap<String, String>,
    custom: Mutex<Option<(String, HashMap<String, String>)>>,
}

impl DisplayNameState {
    pub fn new() -> Self {
        let embedded: HashMap<String, String> =
            serde_json::from_str(DISPLAY_NAMES_JSON).unwrap_or_default();
        Self {
            embedded,
            custom: Mutex::new(None),
        }
    }

    /// Resolve a prototype path to a display name.
    ///
    /// Resolution order:
    /// 1. `display_names_custom.json` in `server_dir` (if present and non-empty)
    /// 2. Embedded `display_names.json`
    /// 3. The prototype path itself (verbatim fallback)
    pub(crate) fn lookup(&self, server_dir: &str, path: &str) -> String {
        if let Ok(mut guard) = self.custom.lock() {
            let needs_load = guard
                .as_ref()
                .map(|(cached_dir, _)| cached_dir != server_dir)
                .unwrap_or(true);

            if needs_load {
                let custom_path = Path::new(server_dir).join("display_names_custom.json");
                let map: HashMap<String, String> = fs::read_to_string(&custom_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                *guard = Some((server_dir.to_string(), map));
            }

            if let Some((_, ref custom_map)) = *guard {
                if let Some(name) = custom_map.get(path) {
                    if name != "N/A" && !name.is_empty() {
                        return name.clone();
                    }
                }
            }
        }

        self.embedded
            .get(path)
            .filter(|n| n.as_str() != "N/A" && !n.is_empty())
            .cloned()
            .unwrap_or_else(|| path.to_string())
    }
}

// ── Disk-only catalog types ───────────────────────────────────────────────────
//
// These types map directly to the on-disk JSON format. `ItemPrototypeRuntimeIdForClient`
// is a raw u64 which serialises as a JSON number. They are private to this module;
// the public "view" types below are used for all Tauri command boundaries.

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct GuidItemDisk {
    prototype_guid: i32,
    item_prototype_runtime_id_for_client: u64,
    quantity: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CatalogEntryDisk {
    sku_id: u64,
    guid_items: Vec<GuidItemDisk>,
    additional_guid_items: Vec<GuidItemDisk>,
    localized_entries: Vec<LocalizedEntry>,
    info_urls: Vec<UrlEntry>,
    content_data: Vec<UrlEntry>,
    #[serde(rename = "Type")]
    item_type: NamedItem,
    type_modifiers: Vec<NamedItem>,
}

// ── Public (frontend-facing) catalog types ────────────────────────────────────
//
// `ItemPrototypeRuntimeIdForClient` is represented as a String here to prevent
// JavaScript from silently losing precision on u64 values above Number.MAX_SAFE_INTEGER
// (9,007,199,254,740,991). Values in the catalog such as 16313827754993193555 would
// be corrupted if passed as a JSON number to the JS runtime.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GuidItem {
    pub prototype_guid: i32,
    /// Stored as a decimal string to preserve full u64 precision across the JS boundary.
    pub item_prototype_runtime_id_for_client: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LocalizedEntry {
    #[serde(default = "default_lang")]
    pub language_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub release_date: String,
    pub item_price: i32,
}

fn default_lang() -> String {
    "en_us".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UrlEntry {
    #[serde(default = "default_lang")]
    pub language_id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub image_data: String,
}

/// Shared shape for `Type` and each entry in `TypeModifiers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NamedItem {
    pub name: String,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogEntry {
    pub sku_id: u64,
    pub guid_items: Vec<GuidItem>,
    pub additional_guid_items: Vec<GuidItem>,
    pub localized_entries: Vec<LocalizedEntry>,
    pub info_urls: Vec<UrlEntry>,
    pub content_data: Vec<UrlEntry>,
    #[serde(rename = "Type")]
    pub item_type: NamedItem,
    pub type_modifiers: Vec<NamedItem>,
}

/// `CatalogEntry` with file provenance, returned by `load_catalog_entries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntryWithMeta {
    #[serde(flatten)]
    pub entry: CatalogEntry,
    /// Base catalog filename (no directory), e.g. `"CatalogBundle.json"`.
    pub source_file: String,
    /// `true` when this entry's effective version came from the `*MODIFIED.json` sibling.
    pub from_modified: bool,
}

// ── Disk ↔ view conversions ───────────────────────────────────────────────────

fn disk_to_view(d: CatalogEntryDisk) -> CatalogEntry {
    CatalogEntry {
        sku_id: d.sku_id,
        guid_items: d.guid_items.into_iter().map(guid_disk_to_view).collect(),
        additional_guid_items: d
            .additional_guid_items
            .into_iter()
            .map(guid_disk_to_view)
            .collect(),
        localized_entries: d.localized_entries,
        info_urls: d.info_urls,
        content_data: d.content_data,
        item_type: d.item_type,
        type_modifiers: d.type_modifiers,
    }
}

fn guid_disk_to_view(g: GuidItemDisk) -> GuidItem {
    GuidItem {
        prototype_guid: g.prototype_guid,
        item_prototype_runtime_id_for_client: g.item_prototype_runtime_id_for_client.to_string(),
        quantity: g.quantity,
    }
}

fn view_to_disk(v: CatalogEntry) -> Result<CatalogEntryDisk, String> {
    let guid_items = v
        .guid_items
        .into_iter()
        .map(guid_view_to_disk)
        .collect::<Result<Vec<_>, _>>()?;
    let additional_guid_items = v
        .additional_guid_items
        .into_iter()
        .map(guid_view_to_disk)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogEntryDisk {
        sku_id: v.sku_id,
        guid_items,
        additional_guid_items,
        localized_entries: v.localized_entries,
        info_urls: v.info_urls,
        content_data: v.content_data,
        item_type: v.item_type,
        type_modifiers: v.type_modifiers,
    })
}

fn guid_view_to_disk(g: GuidItem) -> Result<GuidItemDisk, String> {
    let id = g
        .item_prototype_runtime_id_for_client
        .parse::<u64>()
        .map_err(|e| {
            format!(
                "Invalid prototype ID '{}': {e}",
                g.item_prototype_runtime_id_for_client
            )
        })?;
    Ok(GuidItemDisk {
        prototype_guid: g.prototype_guid,
        item_prototype_runtime_id_for_client: id,
        quantity: g.quantity,
    })
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn server_dir_of(server_exe: &str) -> Result<PathBuf, String> {
    Path::new(server_exe)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Cannot determine server directory from exe path".to_string())
}

fn mtxstore_dir(server_exe: &str) -> Result<PathBuf, String> {
    Ok(server_dir_of(server_exe)?
        .join("Data")
        .join("Game")
        .join("MTXStore"))
}

/// Returns sorted paths of base `Catalog*.json` files, excluding `*MODIFIED*` variants.
fn base_catalog_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|e| format!("Cannot read MTXStore directory: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("Catalog")
            && name.ends_with(".json")
            && !name.to_ascii_uppercase().contains("MODIFIED")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Derives the `*MODIFIED.json` path for a given base catalog path.
fn modified_path_for(base: &Path) -> PathBuf {
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    base.with_file_name(format!("{stem}MODIFIED.json"))
}

/// Read and deserialise disk catalog entries from `path`, returning an empty
/// vec on any error (missing file, invalid JSON, wrong schema) rather than
/// propagating — callers decide whether absence is an error.
fn load_disk_entries(path: &Path) -> Vec<CatalogEntryDisk> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ── Display name resolution (internal) ───────────────────────────────────────

/// Resolve a prototype runtime ID (as a decimal string) to a display name,
/// using the catalogue and display name state. Falls back gracefully at each step.
fn name_for_id(
    dn: &DisplayNameState,
    cat: &CatalogueState,
    server_dir: &str,
    id_str: &str,
) -> String {
    let id: u64 = match id_str.parse() {
        Ok(v) => v,
        Err(_) => return id_str.to_string(),
    };

    // The MutexGuard `g` must stay live until after path_for_id returns its &str,
    // so we clone to an owned String inside the closure before `g` is dropped.
    let maybe_path: Option<String> = cat
        .0
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .and_then(|(_, c)| crate::calligraphy::path_for_id(c, id))
                .map(|s| s.to_string())
        });

    match maybe_path {
        Some(path) => dn.lookup(server_dir, &path),
        None => id_str.to_string(),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Return the absolute path to `{server_dir}/Data/Game/MTXStore`.
#[tauri::command]
pub fn get_mtxstore_dir(server_exe: String) -> Result<String, String> {
    Ok(mtxstore_dir(&server_exe)?.to_string_lossy().to_string())
}

/// Return sorted base `Catalog*.json` filenames (no directory, no MODIFIED variants)
/// present in the MTXStore directory.
#[tauri::command]
pub fn list_catalog_files(server_exe: String) -> Result<Vec<String>, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    base_catalog_files(&dir).map(|paths| {
        paths
            .into_iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect()
    })
}

/// Load all catalog entries from the MTXStore directory.
///
/// For each base `Catalog*.json` the corresponding `*MODIFIED.json` sibling is also
/// read. MODIFIED entries override base entries with the same `SkuId`. Each returned
/// entry carries `source_file` (the base filename) and `from_modified` provenance so
/// the frontend can route save/delete operations correctly.
#[tauri::command]
pub fn load_catalog_entries(server_exe: String) -> Result<Vec<CatalogEntryWithMeta>, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(vec![]);
    }

    let base_files = base_catalog_files(&dir)?;
    let mut result: Vec<CatalogEntryWithMeta> = Vec::new();
    let mut sku_index: HashMap<u64, usize> = HashMap::new();

    let mut upsert = |entry: CatalogEntryDisk, base_name: &str, from_modified: bool| {
        let sku = entry.sku_id;
        let meta = CatalogEntryWithMeta {
            entry: disk_to_view(entry),
            source_file: base_name.to_string(),
            from_modified,
        };
        if let Some(&idx) = sku_index.get(&sku) {
            result[idx] = meta;
        } else {
            sku_index.insert(sku, result.len());
            result.push(meta);
        }
    };

    for base_path in &base_files {
        let base_name = base_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        for entry in load_disk_entries(base_path) {
            upsert(entry, &base_name, false);
        }

        let modified = modified_path_for(base_path);
        if modified.exists() {
            for entry in load_disk_entries(&modified) {
                upsert(entry, &base_name, true);
            }
        }
    }

    Ok(result)
}

/// Upsert a catalog entry into `{target_file_stem}MODIFIED.json`.
///
/// `target_file` is the base filename (e.g. `"CatalogBundle.json"`). Saves always go
/// to the MODIFIED counterpart; the base file is never written by this command.
/// A `.bak` snapshot of the existing MODIFIED file is written before any mutation.
#[tauri::command]
pub fn save_catalog_entry(
    server_exe: String,
    entry: CatalogEntry,
    target_file: String,
) -> Result<(), String> {
    let dir = mtxstore_dir(&server_exe)?;
    let base_path = dir.join(&target_file);
    let modified_path = modified_path_for(&base_path);

    // Backup before touching
    if modified_path.exists() {
        let bak = modified_path.with_extension("json.bak");
        fs::copy(&modified_path, &bak).map_err(|e| format!("Backup failed: {e}"))?;
    }

    let mut disk_entries = load_disk_entries(&modified_path);
    let sku = entry.sku_id;
    let disk_entry = view_to_disk(entry)?;

    if let Some(existing) = disk_entries.iter_mut().find(|e| e.sku_id == sku) {
        *existing = disk_entry;
    } else {
        disk_entries.push(disk_entry);
    }

    let json = serde_json::to_string_pretty(&disk_entries)
        .map_err(|e| format!("Serialisation failed: {e}"))?;
    fs::write(&modified_path, json).map_err(|e| format!("Write failed: {e}"))?;

    Ok(())
}

/// Delete a catalog entry by SKU.
///
/// `source_file` is the base filename (e.g. `"CatalogBundle.json"`).
/// When `from_modified` is `true` the MODIFIED file is targeted; otherwise the base
/// file. A `.bak` snapshot of the target file is written before deletion.
#[tauri::command]
pub fn delete_catalog_entry(
    server_exe: String,
    sku_id: u64,
    source_file: String,
    from_modified: bool,
) -> Result<(), String> {
    let dir = mtxstore_dir(&server_exe)?;
    let base_path = dir.join(&source_file);

    let target_path = if from_modified {
        modified_path_for(&base_path)
    } else {
        base_path
    };

    if !target_path.exists() {
        return Err(format!(
            "{} not found",
            target_path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    let bak = target_path.with_extension("json.bak");
    fs::copy(&target_path, &bak).map_err(|e| format!("Backup failed: {e}"))?;

    let json =
        fs::read_to_string(&target_path).map_err(|e| format!("Read failed: {e}"))?;
    let mut disk_entries: Vec<CatalogEntryDisk> =
        serde_json::from_str(&json).map_err(|e| format!("Parse failed: {e}"))?;

    let before = disk_entries.len();
    disk_entries.retain(|e| e.sku_id != sku_id);

    if disk_entries.len() == before {
        return Err(format!("SKU {sku_id} not found in target file"));
    }

    let json = serde_json::to_string_pretty(&disk_entries)
        .map_err(|e| format!("Serialisation failed: {e}"))?;
    fs::write(&target_path, json).map_err(|e| format!("Write failed: {e}"))?;

    Ok(())
}

/// Scan all JSON files in the MTXStore directory (base and MODIFIED) and return the
/// next available SKU ID: `max_found + 1`, floored at 1001.
#[tauri::command]
pub fn get_next_sku_id(server_exe: String) -> Result<u64, String> {
    let dir = mtxstore_dir(&server_exe)?;
    if !dir.exists() {
        return Ok(1001);
    }

    let mut max_sku: u64 = 1000;

    for entry in
        fs::read_dir(&dir).map_err(|e| format!("Cannot read MTXStore directory: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        for e in load_disk_entries(&entry.path()) {
            if e.sku_id > max_sku {
                max_sku = e.sku_id;
            }
        }
    }

    Ok(max_sku + 1)
}

/// Resolve a prototype runtime ID (decimal string) to a human-readable display name.
///
/// The ID is a string parameter to safely transport the full u64 range from TypeScript.
/// Falls back through: custom override → embedded map → prototype path → raw ID.
#[tauri::command]
pub fn resolve_display_name(
    dn_state: tauri::State<DisplayNameState>,
    cat_state: tauri::State<CatalogueState>,
    server_exe: String,
    prototype_runtime_id: String,
) -> String {
    let server_dir = server_dir_of(&server_exe)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    name_for_id(&dn_state, &cat_state, &server_dir, &prototype_runtime_id)
}

/// Generate the HTML page for a bundle catalog entry and write it to disk.
///
/// Produces:
/// - `{output_dir}/html/{title_slug}_en_bundle.html`
/// - `{output_dir}/css/store.css` (only if the file does not already exist,
///   so user CSS edits are preserved across regenerations)
///
/// Returns the absolute path of the generated HTML file.
#[tauri::command]
pub fn generate_bundle_html(
    dn_state: tauri::State<DisplayNameState>,
    cat_state: tauri::State<CatalogueState>,
    server_exe: String,
    entry: CatalogEntry,
    output_dir: String,
) -> Result<String, String> {
    let server_dir = server_dir_of(&server_exe)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let loc = entry
        .localized_entries
        .iter()
        .find(|e| e.language_id == "en_us")
        .or_else(|| entry.localized_entries.first())
        .ok_or_else(|| "Entry has no localized entries".to_string())?;

    let title = &loc.title;
    let price = loc.item_price;
    let sku_hex = format!("0x{:X}", entry.sku_id);

    let mut items_html = String::new();
    for item in &entry.guid_items {
        let name = name_for_id(
            &dn_state,
            &cat_state,
            &server_dir,
            &item.item_prototype_runtime_id_for_client,
        );
        if item.quantity == 1 {
            items_html.push_str(&format!("                <li>{name}</li>\n"));
        } else {
            // × = multiplication sign (U+00D7), distinct from letter x
            items_html.push_str(&format!(
                "                <li>{name} \u{d7}{}</li>\n",
                item.quantity
            ));
        }
    }

    let html = HTML_TEMPLATE
        .replace("{title}", title)
        .replace("{items}", &items_html)
        .replace("{price}", &price.to_string())
        .replace("{sku_hex}", &sku_hex);

    // Default to {server_dir}/Data/Web/Bundles if no output dir was specified
    let resolved_dir = if output_dir.is_empty() {
        server_dir_of(&server_exe)?
            .join("Data")
            .join("Web")
            .join("Bundles")
            .to_string_lossy()
            .to_string()
    } else {
        output_dir
    };

    let html_dir = Path::new(&resolved_dir).join("html");
    let css_dir = Path::new(&resolved_dir).join("css");
    fs::create_dir_all(&html_dir).map_err(|e| format!("Cannot create html directory: {e}"))?;
    fs::create_dir_all(&css_dir).map_err(|e| format!("Cannot create css directory: {e}"))?;

    // Only write CSS if absent — preserves user customisation made after first generation
    let css_path = css_dir.join("store.css");
    if !css_path.exists() {
        fs::write(&css_path, STORE_CSS)
            .map_err(|e| format!("Cannot write store.css: {e}"))?;
    }

    let slug = title.to_lowercase().replace(' ', "_");
    let html_path = html_dir.join(format!("{slug}_en_bundle.html"));
    fs::write(&html_path, &html).map_err(|e| format!("Cannot write HTML: {e}"))?;

    Ok(html_path.to_string_lossy().to_string())
}

// ── CSS template ──────────────────────────────────────────────────────────────

static STORE_CSS: &str = r#":root {
    --store-bg:        #0a0a0f;
    --store-surface:   #10101a;
    --store-border:    #1e3a5f;
    --store-accent:    #4d9fff;
    --store-text:      #d0d0e0;
    --store-text-dim:  #7080a0;
    --store-price:     #f0c040;
    --store-btn-bg:    #1a3a6a;
    --store-btn-hover: #2a5aa0;
    --store-font:      'Rajdhani', 'Trebuchet MS', sans-serif;
}

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

html {
    background-color: var(--store-bg);
    color: var(--store-text);
    font-family: var(--store-font);
    font-size: 17px;
    line-height: 1.6;
}

.store-page {
    max-width: 680px;
    margin: 2rem auto;
    padding: 0 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
}

.store-header {
    border-bottom: 1px solid var(--store-border);
    padding-bottom: 1rem;
}

.bundle-title {
    font-size: 2.2rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--store-accent);
    text-transform: uppercase;
}

.bundle-contents {
    background: var(--store-surface);
    border: 1px solid var(--store-border);
    border-radius: 4px;
    padding: 1.25rem 1.5rem;
}

.bundle-contents h2 {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--store-text-dim);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-bottom: 0.85rem;
}

.item-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
}

.item-list li {
    padding-left: 1.1rem;
    position: relative;
}

.item-list li::before {
    content: '—';
    position: absolute;
    left: 0;
    color: var(--store-accent);
}

.store-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.5rem;
    background: var(--store-surface);
    border: 1px solid var(--store-border);
    border-radius: 4px;
}

.bundle-price {
    font-size: 2rem;
    font-weight: 700;
    color: var(--store-price);
    letter-spacing: 0.02em;
}

.btn-purchase {
    padding: 0.55rem 1.6rem;
    font-family: var(--store-font);
    font-size: 0.95rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: var(--store-btn-bg);
    color: var(--store-text);
    border: 1px solid var(--store-accent);
    border-radius: 3px;
    cursor: pointer;
    transition: background 0.15s ease;
}

.btn-purchase:hover { background: var(--store-btn-hover); }
"#;

// ── HTML template ─────────────────────────────────────────────────────────────

static HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Rajdhani:wght@500;700&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="css/store.css">
</head>
<body>
    <main class="store-page">
        <header class="store-header">
            <h1 class="bundle-title">{title}</h1>
        </header>
        <section class="bundle-contents">
            <h2>Included Items</h2>
            <ul class="item-list">
{items}
            </ul>
        </section>
        <footer class="store-footer">
            <span class="bundle-price">{price} G</span>
            <button class="btn-purchase" onclick="myApi.BuyBundleFromJS('{sku_hex}')">Purchase</button>
        </footer>
    </main>
</body>
</html>
"#;