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
/// Only loot-filter events are shown for now. Others the hook sends (region_entered,
/// tier_resolved, mission_state, player_death, region_exited) are ignored; add here to show them.
const KNOWN: &[&str] = &["loot_hidden"];
const NAMES_FILE: &str = "MhClientHook.enum-names.bin";

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct PipeLine {
    /// one of KNOWN, or "_status" for our own connection notes
    pub event: String,
    pub text: String,
    /// Column values for rows that have them (loot_hidden: time, category, item, code, rarity).
    pub fields: Vec<String>,
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

/// Rarity is a separate game enum (not in enum-names.bin), so these pairs are embedded.
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

/// "Bodysuit10" -> "Bodysuit". A heuristic: the enum name minus its trailing number.
fn category(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

fn local_time(utc: Option<&str>) -> String {
    utc.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".into())
}

/// Returns a display line only for JSON with a known `event`; everything else is ignored.
/// loot_hidden renders as `time | category | item name | item code | rarity`.
pub fn parse_line(raw: &[u8], names: &HashMap<u64, String>) -> Option<PipeLine> {
    let text = String::from_utf8_lossy(raw);
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let event = v.get("event")?.as_str()?;
    if !KNOWN.contains(&event) {
        return None;
    }
    let idx = v.get("enumIdx").and_then(|x| x.as_u64());
    let name = idx.and_then(|i| names.get(&i)).map(|s| s.as_str());
    let fields: Vec<String> = [
        local_time(v.get("atUtc").and_then(|x| x.as_str())),
        name.map(category).unwrap_or("?").to_string(),
        name.unwrap_or("?").to_string(),
        idx.map(|i| i.to_string()).unwrap_or_else(|| "?".into()),
        v.get("rarity").and_then(|x| x.as_u64()).map(rarity_name).unwrap_or_else(|| "?".into()),
    ]
    .iter()
    .map(|f| clean(f))
    .collect();
    Some(PipeLine { event: event.to_string(), text: fields.join(" | "), fields })
}

fn status(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("overlay-pipe", vec![PipeLine { event: "_status".into(), text: msg.into(), fields: vec![] }]);
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

/// Connects (for up to 30 s) and forwards events until the pipe closes, which
/// happens when the game exits. Events are emitted in batches, one per read.
pub fn spawn(app: tauri::AppHandle, dir: PathBuf) {
    let _ = std::thread::Builder::new().name("overlay-pipe".into()).spawn(move || {
        let Some(mut pipe) = connect() else {
            status(&app, "Could not connect to the overlay status pipe.");
            return;
        };
        status(&app, "Connected to the overlay status pipe.");
        let names = load_names(&dir);
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
        status(&app, "Overlay status pipe closed.");
    });
}
