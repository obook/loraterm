//! ui/format.rs
//! Small text formats shared by the panels: coordinates, distances, ages, routes,
//! durations, battery gauge.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use chrono::TimeZone;
use unicode_width::UnicodeWidthChar;

pub(super) use crate::config::LOW_BATTERY;
use crate::radio::NodeKind;

/// "44.8267N 0.5571W".
pub fn coord(lat: f64, lon: f64) -> String {
    let ns = if lat < 0.0 { 'S' } else { 'N' };
    let ew = if lon < 0.0 { 'W' } else { 'E' };
    format!("{:.4}{ns} {:.4}{ew}", lat.abs(), lon.abs())
}

/// Short form of a contact detail: node type in caps, distance without the space.
pub(super) fn short(detail: &str) -> String {
    match detail {
        "repeater" => "RPT".into(),
        "room" => "ROOM".into(),
        "sensor" => "SNSR".into(),
        other => other.replace(' ', ""),
    }
}

/// `s` cut and padded to exactly `width` terminal cells (an emoji takes two).
pub(super) fn cells(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out + &" ".repeat(width - used)
}

/// First 6 hex digits of a contact's key, the node ID shown in the list.
pub(super) fn node_id(conv: &str) -> &str {
    if conv.len() == 12 && conv.bytes().all(|b| b.is_ascii_hexdigit()) {
        &conv[..6]
    } else {
        ""
    }
}

/// Battery as eight cells: "▮▮▮▮▮▮▮▯", or "[#######-]" on the Linux console.
pub fn gauge(pct: u8, ascii: bool) -> String {
    let full = (usize::from(pct.min(100)) * 8 + 50) / 100;
    if ascii {
        format!("[{}{}]", "#".repeat(full), "-".repeat(8 - full))
    } else {
        format!("{}{}", "▮".repeat(full), "▯".repeat(8 - full))
    }
}

/// Text the Linux console can show: its fonts cover ASCII and the accented Latin letters
/// (Latin-1 and Latin Extended-A), so anything else, like emoji, becomes "?".
pub fn ascii_safe(text: &str, ascii: bool) -> std::borrow::Cow<'_, str> {
    if !ascii {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut safe = String::new();
    for c in text.chars() {
        if c.is_ascii() || ('\u{a0}'..='\u{17f}').contains(&c) {
            safe.push(c);
        } else {
            safe.push('?');
        }
    }
    std::borrow::Cow::Owned(safe)
}

/// A turning bar, one step every 150 ms.
pub(super) fn spinner() -> char {
    let tick = chrono::Local::now().timestamp_millis() / 150;
    ['|', '/', '-', '\\'][tick.rem_euclid(4) as usize]
}

/// "5 MIN AGO" for a number of seconds.
pub fn age(secs: i64) -> String {
    if secs < 60 {
        "JUST NOW".into()
    } else if secs < 3600 {
        format!("{} MIN AGO", secs / 60)
    } else if secs < 86_400 {
        format!("{} H AGO", secs / 3600)
    } else {
        format!("{} D AGO", secs / 86_400)
    }
}

/// "FLOOD" when no route is known, "DIRECT" or "2 HOPS (3f > 12)".
pub fn route(path_len: i8, path: &[u8]) -> String {
    match path_len {
        ..0 => "FLOOD".into(),
        0 => "DIRECT".into(),
        n => {
            // shortcut: one byte per hop; firmwares with longer path hashes show them byte by byte.
            let hops: Vec<String> = path
                .iter()
                .take(n as usize)
                .map(|b| format!("{b:02x}"))
                .collect();
            format!("{n} HOPS ({})", hops.join(" > "))
        }
    }
}

/// "3 D 4 H", "5 H 12 MIN" or "12 MIN".
pub fn uptime(secs: u32) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    match (d, h) {
        (0, 0) => format!("{m} MIN"),
        (0, _) => format!("{h} H {m} MIN"),
        _ => format!("{d} D {h} H"),
    }
}

/// Node type in capitals, for the card.
pub(super) fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Chat => "PERSON",
        NodeKind::Repeater => "REPEATER",
        NodeKind::Room => "ROOM",
        NodeKind::Sensor => "SENSOR",
    }
}

/// "22:39": local time of a Unix timestamp.
pub(super) fn hhmm(ts: i64) -> String {
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// "250" or "62.5" kHz.
pub(super) fn khz(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        (hz / 1000).to_string()
    } else {
        format!("{:.1}", f64::from(hz) / 1000.0)
    }
}
