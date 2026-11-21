//! Every launcher writer must hand back what it read, after the values have made the trip
//! to the webview and back. The fixtures copy the shapes found in the v52 stock data
//! (MHServerEmu 1.0.2) with made-up names: 64-bit ids at the top level and nested inside
//! structs, repeated `Properties` keys (community cooldown patches), UTF-8 byte-order marks
//! (3 patch files and every event tuning file), `x.0` floats, optional event fields, null
//! schedule slots, several store languages and non-ASCII text.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Number, Value};
use std::fs;
use std::path::{Path, PathBuf};

const BOM: &str = "\u{FEFF}";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const PATCHES: &str = r#"[
    { "Enabled": true, "Prototype": "Powers/Player/Alpha/StrikeOne.prototype", "Path": "Keywords[]", "Description": "Id", "ValueType": "PrototypeId", "Value": 17910267881806762587 },
    { "Enabled": true, "Prototype": "Powers/Player/Alpha/StrikeOne.prototype", "Path": "SummonEntityContexts[0].SummonEntity", "Description": "Data ref", "ValueType": "PrototypeDataRef", "Value": 16683599409148793757 },
    { "Enabled": false, "Prototype": "Entity/Items/Gamma/Thing.prototype", "Path": "Guid", "Description": "Guid", "ValueType": "PrototypeGuid", "Value": 9316722085805298222 },
    { "Enabled": true, "Prototype": "Entity/Items/Gamma/Thing.prototype", "Path": "DisplayName", "Description": "Locale", "ValueType": "LocaleStringId", "Value": 12350805995292858098 },
    { "Enabled": true, "Prototype": "Entity/Items/Gamma/Thing.prototype", "Path": "Keywords", "Description": "Id list", "ValueType": "PrototypeId[]", "Value": [ 8001975789840962157, 1637531228337083315 ] },
    { "Enabled": true, "Prototype": "Entity/Items/Gamma/Thing.prototype", "Path": "Behaviors", "Description": "Data ref list", "ValueType": "PrototypeDataRef[]", "Value": [ 18202449423888946833 ] },
    { "Enabled": true, "Prototype": "Powers/Player/Alpha/Summon.prototype", "Path": "SummonMaxSimultaneous", "Description": "Formula", "ValueType": "Prototype", "Value": { "ParentDataRef": 17436925256515063008, "Value": 8, "Op": "LoadInt" } },
    { "Enabled": true, "Prototype": "Loot/Tables/Delta/Specials.prototype", "Path": "Choices[0].Choices[]", "Description": "Struct list", "ValueType": "Prototype[]", "Value": [
        { "ParentDataRef": 1071992662862269383, "Item": 5460518059085536758, "Weight": 50 },
        { "ParentDataRef": 1071992662862269383, "Item": 9964627584974134775, "Weight": 50, "NumMin": 1, "NumMax": 3 }
    ] },
    { "Enabled": true, "Prototype": "Entity/Characters/Avatars/Shipping/Epsilon.prototype", "Path": "Properties", "Description": "Repeated keys", "ValueType": "Properties", "Value": {
        "CooldownModifierPctGlobal": -0.25,
        "CooldownModifierPctForPower": [ 1076091916814587724, -0.5 ],
        "CooldownModifierPctForPower": [ 13794733944514418082, -0.5 ],
        "LootTablePrototype": [ 17910267881806762587, 0, 9316722085805298222, 16576396162899383486 ]
    } },
    { "Enabled": true, "Prototype": "Metagame/Zeta/Mode.prototype", "Path": "StatePickIntervalMS", "Description": "", "ValueType": "Integer", "Value": 60000 },
    { "Enabled": true, "Prototype": "Metagame/Zeta/Mode.prototype", "Path": "Scale", "Description": "", "ValueType": "Float", "Value": 1.0 },
    { "Enabled": true, "Prototype": "Metagame/Zeta/Mode.prototype", "Path": "IsLive", "Description": "", "ValueType": "Boolean", "Value": false },
    { "Enabled": true, "Prototype": "Metagame/Zeta/Mode.prototype", "Path": "PowerCategory", "Description": "", "ValueType": "Enum", "Value": "ComboEffect" },
    { "Enabled": true, "Prototype": "Metagame/Zeta/Mode.prototype", "Path": "Offset", "Description": "", "ValueType": "Vector3", "Value": [ 1.5, 0.0, -2.0 ] }
]"#;

const TUNING: &str = r#"[
    { "Prototype": "", "Setting": "eGTV_XPGain", "Value": 2.0 },
    { "Prototype": "", "Setting": "eGTV_CosmicPrestigeXPPct", "Value": 0.08 },
    { "Prototype": "Entity/Characters/Mobs/Eta/Brute.prototype", "Setting": "eWETV_MobHealth", "Value": 15.0 },
    { "Prototype": "Loot/Tables/Eta/Holiday.prototype", "Setting": "eLTTV_Enabled", "Value": 2 },
    { "Prototype": "Missions/Eta/Daily.prototype", "Setting": "eMTV_Enabled", "Value": 0.0 }
]"#;

const EVENTS: &str = r#"{
    "Plain": { "DisplayName": "Plain Event", "FilePath": "Events/Weekly/LiveTuningDataPlain.json" },
    "Gift": { "DisplayName": "Gift Event", "FilePath": "Events/Holiday/LiveTuningDataGift.json", "DailyGift": "Entity/Items/Theta/Present.prototype" },
    "Missions": { "DisplayName": "Missions Event", "FilePath": "Events/Weekly/LiveTuningDataMissions.json", "InstancedMissions": [ "Missions/Theta/One.prototype", "Missions/Theta/Two.prototype" ] },
    "Hidden": { "DisplayName": "Hidden Event", "FilePath": "Events/Monday/LiveTuningDataHidden.json", "IsHidden": true },
    "Shown": { "DisplayName": "Shown Event", "FilePath": "Events/Monday/LiveTuningDataShown.json", "IsHidden": false }
}"#;

const SCHEDULE: &str = r#"[
    { "Name": "Permanent", "IsEnabled": true, "Type": "AlwaysOn", "Events": [ "Plain" ] },
    { "Name": "Rotation", "IsEnabled": true, "Type": "WeeklyRotation", "StartDayOfWeek": "Friday", "Events": [ "Gift", null, "Missions" ] },
    { "Name": "Mondays", "IsEnabled": false, "Type": "DayOfWeek", "StartDayOfWeek": "Monday", "Events": [ "Hidden" ] },
    { "Name": "Holiday", "IsEnabled": true, "Type": "SpecialDate", "StartMonth": 12, "StartDay": 18, "DurationDays": 14, "Events": [ "Gift", "Shown" ] },
    { "Name": "Lunar", "IsEnabled": true, "Type": "SpecialDateLunar", "StartMonth": 1, "StartDay": 1, "DurationDays": 7, "Events": [ "Shown" ] }
]"#;

const CATALOG: &str = r#"[
    {
        "SkuId": 1001,
        "GuidItems": [ { "PrototypeGuid": 14275532478838512418, "ItemPrototypeRuntimeIdForClient": 9839468574926585837, "Quantity": 1 } ],
        "AdditionalGuidItems": [],
        "LocalizedEntries": [
            { "LanguageId": "en_us", "Description": "A hero.", "Title": "Iota Hero", "ReleaseDate": "", "ItemPrice": 1000 },
            { "LanguageId": "fr_fr", "Description": "Un héros — très fort.", "Title": "Héros Iota", "ReleaseDate": "", "ItemPrice": 1000 }
        ],
        "InfoUrls": [],
        "ContentData": [],
        "Type": { "Name": "Hero", "Order": 1 },
        "TypeModifiers": []
    },
    {
        "SkuId": 18446744073709551000,
        "GuidItems": [
            { "PrototypeGuid": 11153389812004402173, "ItemPrototypeRuntimeIdForClient": 16683599409148793757, "Quantity": 1 },
            { "PrototypeGuid": 9007199254740993, "ItemPrototypeRuntimeIdForClient": 9007199254740995, "Quantity": 5 }
        ],
        "AdditionalGuidItems": [ { "PrototypeGuid": 17436925256515063008, "ItemPrototypeRuntimeIdForClient": 1071992662862269383, "Quantity": 1 } ],
        "LocalizedEntries": [ { "LanguageId": "en_us", "Description": "Bundle ™", "Title": "Kappa Bundle", "ReleaseDate": "2016-06-01", "ItemPrice": 2500 } ],
        "InfoUrls": [ { "LanguageId": "en_us", "Url": "http://localhost/bundles/kappa.html", "ImageData": "" } ],
        "ContentData": [ { "LanguageId": "en_us", "Url": "http://localhost/bundles/kappa.png", "ImageData": "" } ],
        "Type": { "Name": "Bundle", "Order": 3 },
        "TypeModifiers": [ { "Name": "Giftable", "Order": 0 }, { "Name": "NoDisplay", "Order": 0 }, { "Name": "NoDisplayStore", "Order": 0 } ]
    }
]"#;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// What the webview does to a command's result before handing it back: every number becomes
/// a JS double, and integral doubles serialise without a fraction.
fn through_webview<T: Serialize, U: DeserializeOwned>(value: &T) -> U {
    fn js(v: Value) -> Value {
        match v {
            Value::Number(n) => {
                let f = n.as_f64().unwrap();
                if f.fract() == 0.0 && f.abs() < 1e21 {
                    if f < 0.0 { Value::from(f as i64) } else { Value::from(f as u64) }
                } else {
                    Value::Number(Number::from_f64(f).unwrap())
                }
            }
            Value::Array(a) => Value::Array(a.into_iter().map(js).collect()),
            Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, js(v))).collect()),
            other => other,
        }
    }
    serde_json::from_value(js(serde_json::to_value(value).unwrap())).unwrap()
}

/// Parsed JSON in which `2`, `2.0` and `"2.0"`-style writer differences compare equal, while
/// integers too big for a double (ids) must match exactly.
fn normalised(text: &str) -> Value {
    fn norm(v: Value) -> Value {
        match v {
            Value::Number(n) => match n.as_u64() {
                Some(u) if u > (1u64 << 53) => Value::Number(n),
                _ => Value::from(n.as_f64().unwrap()),
            },
            Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
            Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
            other => other,
        }
    }
    norm(serde_json::from_str(text.trim_start_matches(BOM)).unwrap())
}

/// Every object key in the text, counted, so a dropped repeated key shows up even though
/// `Value` itself keeps only one of them.
fn key_counts(text: &str) -> std::collections::BTreeMap<String, usize> {
    let mut counts = std::collections::BTreeMap::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let tail = after[end + 1..].trim_start();
        if tail.starts_with(':') {
            *counts.entry(after[..end].to_string()).or_default() += 1;
        }
        rest = &after[end + 1..];
    }
    counts
}

fn assert_same(label: &str, original: &str, written: &str) {
    assert_eq!(normalised(written), normalised(original), "{label}: content changed");
    assert_eq!(key_counts(written), key_counts(original), "{label}: keys changed");
}

fn server_with(files: &[(&str, String)]) -> (tempfile::TempDir, String, PathBuf) {
    let (tmp, server_exe) = crate::test_helpers::empty_server();
    let game = tmp.path().join("Data").join("Game");
    for (rel, text) in files {
        let path = game.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    (tmp, server_exe, game)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn patch_files_round_trip_through_the_editor() {
    for (name, text) in [
        ("PatchDataPlain.json", PATCHES.to_string()),
        ("PatchDataBom.json", format!("{BOM}{PATCHES}")),
    ] {
        let (_tmp, server_exe, game) = server_with(&[(&format!("Patches/{name}"), text.clone())]);

        let loaded = crate::patches::load_patch_file(server_exe.clone(), name.into(), true).unwrap();
        crate::patches::save_patch_file(server_exe, name.into(), true, through_webview(&loaded)).unwrap();

        assert_same(name, &text, &read(&game.join("Patches").join(name)));
    }
}

#[test]
fn tuning_files_round_trip_through_the_editor() {
    for (rel, text) in [
        ("LiveTuningDataUserRates.json", TUNING.to_string()),
        ("Events/Weekly/LiveTuningDataBom.json", format!("{BOM}{TUNING}")),
    ] {
        let (_tmp, server_exe, game) = server_with(&[(&format!("LiveTuning/{rel}"), text.clone())]);

        let loaded = crate::tuning::read_tuning_file(server_exe.clone(), rel.into()).unwrap();
        crate::tuning::write_tuning_file(server_exe, rel.into(), through_webview(&loaded)).unwrap();

        assert_same(rel, &text, &read(&game.join("LiveTuning").join(rel)));
    }
}

#[test]
fn events_and_schedule_round_trip_into_the_overrides() {
    let (_tmp, server_exe, game) = server_with(&[
        ("LiveTuning/Events.json", EVENTS.to_string()),
        ("LiveTuning/EventSchedule.json", SCHEDULE.to_string()),
    ]);
    let dir = game.join("LiveTuning");

    let events = crate::events::load_events(server_exe.clone()).unwrap();
    crate::events::save_events_override(server_exe.clone(), through_webview(&events.definitions)).unwrap();
    let schedule = crate::events::load_event_schedule(server_exe.clone()).unwrap();
    crate::events::save_schedule_override(server_exe, through_webview(&schedule.rules)).unwrap();

    assert_same("Events.json", EVENTS, &read(&dir.join("EventsOverride.json")));
    // A null slot (an empty week) is written as "", which the server treats the same.
    assert_same("EventSchedule.json", &SCHEDULE.replace("null", r#""""#), &read(&dir.join("EventScheduleOverride.json")));
}

#[test]
fn catalog_entries_round_trip_into_the_modified_file() {
    let (_tmp, server_exe, game) = server_with(&[("MTXStore/Catalog.json", CATALOG.to_string())]);

    let loaded = crate::store::load_catalog_entries(server_exe.clone()).unwrap();
    assert_eq!(loaded.len(), 2);
    for meta in &loaded {
        let entry: crate::store::CatalogEntry = through_webview(&meta.entry);
        crate::store::save_catalog_entry(server_exe.clone(), entry, meta.source_file.clone()).unwrap();
    }

    assert_same("Catalog.json", CATALOG, &read(&game.join("MTXStore").join("CatalogMODIFIED.json")));
}
