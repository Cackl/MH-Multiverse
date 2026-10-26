//! Reader for the hook's status pipe. The hook is the server and writes JSON
//! lines; any local process could squat or write that name, so every line is
//! treated as untrusted text: capped, sanitised, and only known events pass.

use serde::Serialize;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::Emitter;

const PIPE: &str = r"\\.\pipe\MhTerminalsHelper";
const MAX_LINE: usize = 8192;
const MAX_TEXT: usize = 512;
/// Events shown in the log. Others the hook sends (hello, region_exited, map_open, ...) are ignored.
const KNOWN: &[&str] = &["loot_hidden", "region_entered", "tier_resolved", "mission_state", "player_death"];
/// `mission_state.to` value for a completed mission.
const MISSION_DONE: u64 = 4;
/// The hook sends about 5 recent hits with a death.
const MAX_HITS: usize = 8;
const NAMES_FILE: &str = "MhClientHook.enum-names.bin";

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct PipeLine {
    /// one of KNOWN, or "_status" for our own connection notes
    pub event: String,
    pub text: String,
    /// Column values for rows that have them (loot_hidden: time, category, item, code, rarity, reason;
    /// region_entered: time, terminal, tier; player_death: time, heading, detail).
    pub fields: Vec<String>,
    /// Damage table for player_death.
    pub hits: Vec<Hit>,
}

/// One row of a death's recent-damage table.
#[derive(Clone, Serialize, Debug, PartialEq, Default)]
pub struct Hit {
    pub time: String,
    pub health: String,
    pub damage: String,
    pub kind: String,
    pub attacker: String,
    pub power: String,
    /// tooltip explaining the damage figure
    pub note: String,
}

/// Splits a byte stream into lines. A line longer than MAX_LINE is dropped whole.
#[derive(Default)]
pub struct LineSplitter {
    buf: Vec<u8>,
    skipping: bool,
}

impl LineSplitter {
    pub fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in data {
            if b == b'\n' {
                if !self.skipping && !self.buf.is_empty() {
                    out.push(std::mem::take(&mut self.buf));
                }
                self.buf.clear();
                self.skipping = false;
            } else if self.skipping {
            } else if self.buf.len() >= MAX_LINE {
                self.buf.clear();
                self.skipping = true;
            } else {
                self.buf.push(b);
            }
        }
        out
    }
}

/// Control characters (including ANSI escapes) become spaces; length is capped.
pub fn clean(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_TEXT).collect()
}

/// Rarity codes are 1-based positions in the game's global prototype enum (Calligraphy prototypes plus
/// the mu_cdata.sip resource prototypes, sorted by PrototypeId), so they can't be rebuilt from
/// Calligraphy.sip alone; 57781 is Entity/Items/Rarity/R6Omega.prototype in the stock data. These pairs are embedded.
/// Taken from an analysis of the overlay; 43721/46816/51581 also match the hook's debug log.
/// Unknown codes are shown as numbers.
fn rarity_name(code: u64) -> String {
    match code {
        0 => "None",
        51581 => "Common",
        8563 => "Uncommon",
        83121 => "Rare",
        46816 => "Epic",
        43721 => "Cosmic",
        42236 => "Ultimate",
        57781 => "Omega",
        16832 => "Runeword",
        30640 => "Unique",
        31605 => "Default",
        _ => return code.to_string(),
    }
    .into()
}

/// enumIdx -> name from the overlay's enum-names.bin: 16-byte header ("MHNE", version,
/// count, reserved) then records of u32 index, u16 length, name bytes. Stops quietly at
/// anything malformed; a missing or bad file just means no names.
pub fn load_names(dir: &Path) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    let Ok(d) = std::fs::read(dir.join(NAMES_FILE)) else { return out };
    if d.len() < 16 || &d[..4] != b"MHNE" {
        return out;
    }
    let mut o = 16;
    while o + 6 <= d.len() {
        let idx = u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as u64;
        let len = u16::from_le_bytes(d[o + 4..o + 6].try_into().unwrap()) as usize;
        o += 6;
        let Some(name) = d.get(o..o + len) else { break };
        out.insert(idx, String::from_utf8_lossy(name).into_owned());
        o += len;
    }
    out
}

/// Name table values are prototype paths from Calligraphy, or bare names from enum-names.bin.
fn leaf(value: &str) -> &str {
    let file = value.rsplit('/').next().unwrap_or(value);
    file.strip_suffix(".prototype").unwrap_or(file)
}

/// Path values are matched on their folder, first match wins. Bare names ("Bodysuit10" ->
/// "Bodysuit") fall back to the name minus its trailing number, a heuristic.
fn category(value: &str) -> &str {
    if !value.contains('/') {
        return value.trim_end_matches(|c: char| c.is_ascii_digit());
    }
    const BY_FOLDER: &[(&str, &str)] = &[
        ("/Insignias/", "Insignia"), ("/Medals/", "Medal"), ("/Rings/", "Ring"), ("/Artifacts/", "Artifact"),
        ("/TeamUps/", "Team-up"), ("/TeamUpGear/", "Team-up"), ("/Catalysts/", "Catalyst"),
        ("/Armor/", "Armor"), ("/Genome/", "Genome"),
    ];
    BY_FOLDER.iter().find(|(f, _)| value.contains(f)).map_or("Item", |(_, c)| c)
}

/// enumIdx -> prototype path from the first readable Calligraphy.sip, else bare names from
/// the overlay's enum-names.bin. The sip covers agents too, so it can name killers.
/// Built once per connection on the pipe thread; ~70 ms for the full client sip.
pub fn load_entity_names(sips: &[PathBuf], dir: &Path) -> HashMap<u64, String> {
    for sip in sips.iter().filter(|p| p.is_file()) {
        if let Ok(paths) = crate::calligraphy::entity_paths(&sip.to_string_lossy()) {
            return paths.into_iter().enumerate().map(|(i, p)| (i as u64 + 1, p)).collect();
        }
    }
    load_names(dir)
}

/// Calligraphy.sip candidates, best first: the client's (the hook's indices come from it),
/// then the server's copy.
pub fn sip_candidates(game_exe: &str, server_exe: &str) -> Vec<PathBuf> {
    const SIP: &str = "Data/Game/Calligraphy.sip";
    let mut out: Vec<PathBuf> =
        Path::new(game_exe).ancestors().skip(1).take(5).map(|d| d.join(SIP)).filter(|p| p.is_file()).take(1).collect();
    if let Ok(d) = crate::paths::server_dir(server_exe) {
        out.push(d.join(SIP));
    }
    out
}

fn local_time(utc: Option<&str>) -> String {
    utc.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".into())
}

/// Lenient number: a JSON number or a numeric string (the hook sends u64s as strings); anything else is 0.
fn num(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0)
}

fn signed(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

/// Cleaned hook text; empty when missing or not a string.
fn txt(v: &serde_json::Value, key: &str) -> String {
    clean(v.get(key).and_then(|x| x.as_str()).unwrap_or("")).trim().to_string()
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn hit_row(h: &serde_json::Value, names: &HashMap<u64, String>) -> Hit {
    let (value, lost, hp, hp_max) = (num(h, "value"), num(h, "hpLost"), signed(h, "hp"), signed(h, "hpMax"));
    let health = match (hp, hp_max) {
        (hp, _) if hp <= 0 => String::new(),
        (hp, max) if max <= 0 => format!("{hp} HP"),
        (hp, max) => format!("{hp} HP ({}%)", hp * 100 / max),
    };
    let note = if lost == 0 {
        "Real health cost unknown; the shown damage is an estimate.".into()
    } else if value != 0 && lost != value {
        format!("Shown {value}, lost {lost}: x{:.2} difficulty multiplier", lost as f64 / value as f64)
    } else {
        String::new()
    };
    Hit {
        time: format!("−{:.1}s", num(h, "msAgo") as f64 / 1000.0),
        health,
        damage: if lost != 0 { lost.to_string() } else { format!("~{value}") },
        kind: txt(h, "type"),
        attacker: names.get(&num(h, "idx")).map(|n| clean(leaf(n))).unwrap_or_else(|| "unknown".into()),
        power: txt(h, "power"),
        note,
    }
}

fn death_line(v: &serde_json::Value, names: &HashMap<u64, String>) -> PipeLine {
    let recent: Vec<&serde_json::Value> =
        v.get("recent").and_then(|r| r.as_array()).map(|a| a.iter().take(MAX_HITS).collect()).unwrap_or_default();
    let hits: Vec<Hit> = recent.iter().filter(|h| num(h, "value") != 0).map(|h| hit_row(h, names)).collect();
    let killer_idx = num(v, "killerEnumIdx");
    let killer = names.get(&killer_idx);
    let heading = match killer {
        Some(k) => format!("☠ Killed by {}", clean(leaf(k))),
        None => "You were killed".into(),
    };
    let mut detail = Vec::new();
    let (dmg_type, dmg) = (txt(v, "dmgType"), num(v, "dmgValue"));
    if !dmg_type.is_empty() && dmg != 0 {
        let power = recent.iter().find(|h| signed(h, "hpMax") > 0).or(recent.first()).map(|h| txt(h, "power")).unwrap_or_default();
        let base = format!("{} {dmg_type}", thousands(dmg));
        detail.push(if power.is_empty() { base } else { format!("{base} · {power}") });
    }
    if killer.is_none() {
        detail.push(if killer_idx == 0 { "killer unknown".into() } else { format!("enumIdx {killer_idx}") });
    }
    let fields = vec![local_time(v.get("atUtc").and_then(|x| x.as_str())), heading, detail.join("  ")];
    PipeLine { event: "player_death".into(), text: format!("{}  {}", fields[1], fields[2]).trim_end().to_string(), fields, hits }
}

/// Returns a display line only for JSON with a known `event`; everything else is ignored.
/// loot_hidden renders as `time | category | item name | item code | rarity` (+ reason as a 6th field).
pub fn parse_line(raw: &[u8], names: &HashMap<u64, String>) -> Option<PipeLine> {
    let text = String::from_utf8_lossy(raw);
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let event = v.get("event")?.as_str()?;
    if !KNOWN.contains(&event) {
        return None;
    }
    let time = local_time(v.get("atUtc").and_then(|x| x.as_str()));
    let line = |text: String, fields: Vec<String>| PipeLine { event: event.to_string(), text, fields, hits: vec![] };
    match event {
        "region_entered" => {
            let (terminal, tier) = (txt(&v, "terminal"), txt(&v, "tier"));
            if terminal.is_empty() {
                return None; // hubs and unmapped regions
            }
            Some(line(format!("Terminal  {terminal}  {tier}").trim_end().to_string(), vec![time, terminal, tier]))
        }
        "tier_resolved" => {
            let tier = txt(&v, "tier");
            (!tier.is_empty()).then(|| line(format!("Difficulty: {tier}"), vec![]))
        }
        "mission_state" => {
            if num(&v, "to") != MISSION_DONE {
                return None;
            }
            let terminal = txt(&v, "terminal");
            let t = if terminal.is_empty() { "✓ Mission completed".into() } else { format!("✓ Mission completed — {terminal}") };
            Some(line(t, vec![]))
        }
        "player_death" => Some(death_line(&v, names)),
        _ => {
            let idx = v.get("enumIdx").and_then(|x| x.as_u64());
            let name = idx.and_then(|i| names.get(&i)).map(|s| s.as_str());
            let reason = txt(&v, "reason");
            let reason = reason.trim_start_matches('-').trim();
            let reason = if matches!(reason, "matrix" | "static-list") { "" } else { reason };
            let fields: Vec<String> = [
                time,
                name.map(category).unwrap_or("?").to_string(),
                name.map(leaf).unwrap_or("?").to_string(),
                idx.map(|i| i.to_string()).unwrap_or_else(|| "?".into()),
                v.get("rarity").and_then(|x| x.as_u64()).map(rarity_name).unwrap_or_else(|| "?".into()),
                reason.to_string(),
            ]
            .iter()
            .map(|f| clean(f))
            .collect();
            Some(line(fields[..5].join(" | "), fields))
        }
    }
}

pub(super) fn status(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("overlay-pipe", vec![PipeLine { event: "_status".into(), text: msg.into(), fields: vec![], hits: vec![] }]);
}

fn connect() -> Option<File> {
    // the hook creates the pipe during init, which can be a little after injection returns
    for _ in 0..15 {
        if let Ok(f) = OpenOptions::new().read(true).open(PIPE) {
            return Some(f);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    None
}

/// Connects (for up to 30 s) and forwards events until the game exits. Events are emitted in
/// batches, one per read. If the pipe drops while the game is still running, reconnect after
/// 2 s (the hook should accept a new client); events sent in between are lost. `alive` is how we
/// tell the two cases apart; for a launched game it holds the `Child`, which keeps the PID from being reused.
pub fn spawn(app: tauri::AppHandle, dir: PathBuf, sips: Vec<PathBuf>, mut alive: impl FnMut() -> bool + Send + 'static) {
    let _ = std::thread::Builder::new().name("overlay-pipe".into()).spawn(move || {
        let Some(mut pipe) = connect() else {
            status(&app, "Could not connect to the overlay status pipe.");
            return;
        };
        status(&app, "Connected to the overlay status pipe.");
        let names = load_entity_names(&sips, &dir);
        loop {
            let mut splitter = LineSplitter::default();
            let mut buf = [0u8; 4096];
            loop {
                match pipe.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let lines: Vec<PipeLine> =
                            splitter.feed(&buf[..n]).iter().filter_map(|l| parse_line(l, &names)).collect();
                        if !lines.is_empty() {
                            let _ = app.emit("overlay-pipe", lines);
                        }
                    }
                }
            }
            if !alive() {
                break; // game exited (or can't be checked)
            }
            status(&app, "Overlay status pipe dropped; reconnecting.");
            std::thread::sleep(std::time::Duration::from_secs(2));
            match connect() {
                Some(p) => pipe = p,
                None => {
                    status(&app, "Could not reconnect to the overlay status pipe.");
                    return;
                }
            }
            status(&app, "Reconnected to the overlay status pipe.");
        }
        status(&app, "Overlay status pipe closed.");
    });
}
