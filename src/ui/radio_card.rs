//! ui/radio_card.rs
//! The card of my own radio: model, firmware, battery, radio settings, position and link.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use ratatui::text::Line;

use super::card::field;
use super::format::{coord, khz, uptime, LOW_BATTERY};
use super::{alert, faint, text, Theme};
use crate::app::App;
use crate::radio::{Link, MyRadio};

/// What my radio says about itself, its battery and the link.
pub(super) fn radio_lines<'a>(app: &App, radio: &MyRadio, theme: Theme) -> Vec<Line<'a>> {
    let or_dash = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
    let fw = match (&radio.version, &radio.build) {
        (Some(v), Some(b)) => format!("{v} ({b})"),
        (Some(x), None) | (None, Some(x)) => x.clone(),
        (None, None) => "-".into(),
    };
    let mut lines = vec![
        field(theme, "MODEL", or_dash(&radio.model), text(theme)),
        field(theme, "FW", fw, text(theme)),
    ];
    lines.push(match app.battery {
        Some(b) => field(
            theme,
            "BAT",
            format!("{} %  {:.2} V", b.pct, f64::from(b.mv) / 1000.0),
            if b.pct < LOW_BATTERY {
                alert(theme)
            } else {
                text(theme)
            },
        ),
        None => field(theme, "BAT", "-".into(), faint(theme)),
    });
    if let Some((used, total)) = app.battery.and_then(|b| b.used_kb.zip(b.total_kb)) {
        lines.push(field(
            theme,
            "STORE",
            format!("{used} / {total} KB"),
            text(theme),
        ));
    }
    lines.push(field(
        theme,
        "FREQ",
        format!("{:.3} MHz", f64::from(radio.freq_khz) / 1000.0),
        text(theme),
    ));
    lines.push(field(
        theme,
        "LORA",
        format!(
            "BW {} kHz SF {} CR {}",
            khz(radio.bw_hz),
            radio.sf,
            radio.cr
        ),
        text(theme),
    ));
    lines.push(field(
        theme,
        "TX",
        format!("{} dBm (max {})", radio.tx_power, radio.max_tx_power),
        text(theme),
    ));
    lines.push(match app.self_pos {
        Some(p) => field(theme, "POS", coord(p.lat, p.lon), text(theme)),
        None => field(theme, "POS", "UNKNOWN".into(), faint(theme)),
    });
    // Whether my adverts carry the position.
    lines.push(field(
        theme,
        "GPS",
        if radio.shares_position {
            "SHARED".into()
        } else {
            "HIDDEN".into()
        },
        text(theme),
    ));
    // shortcut: the maximum is not shown; meshcore-rs 0.2.0 caps the firmware's value at 255.
    lines.push(field(
        theme,
        "NODES",
        app.nodes.len().to_string(),
        text(theme),
    ));
    lines.push(match (&app.link, app.connected_since) {
        (Link::Connected(name), Some(since)) => field(
            theme,
            "LINK",
            format!("{name}  {}", uptime(since.elapsed().as_secs() as u32)),
            text(theme),
        ),
        _ => field(theme, "LINK", "DOWN".into(), alert(theme)),
    });
    lines
}
