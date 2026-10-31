//! ui/radar.rs
//! The map without images, for the Linux console: my radio is a "+" at its place, each
//! located node is its initial, and the selected node also shows its name. The view, zoom
//! and pan are the map's (app.view), as if each character cell were 10 x 20 pixels.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::Frame;

use super::format::ascii_safe;
use super::{faint, strong, text, Theme};
use crate::app::App;
use crate::geo::{format_distance, Pos};
use crate::tiles::{initials, ring_px};

/// Virtual pixels of one character cell, so the radar can use the map's pixel math. A
/// cell is twice as tall as wide; at this size a panel of 34 x 16 cells covers the area the
/// first view is fitted to (config::FIT_PX).
const CELL_PX: (u32, u32) = (24, 48);

/// "1 cell = 500 m": the width of one character cell at the current zoom.
pub(super) fn scale(app: &App) -> String {
    let px_per_km = ring_px(1.0, app.view.lat, app.view.zoom);
    format!(
        "1 cell = {}",
        format_distance(f64::from(CELL_PX.0) / px_per_km)
    )
}

/// Size in virtual pixels of a radar drawn in `area`.
pub(super) fn radar_px(area: Rect) -> (u32, u32) {
    (
        u32::from(area.width) * CELL_PX.0,
        u32::from(area.height) * CELL_PX.1,
    )
}

/// Character cell of `pos` inside `area`, if it falls there.
fn cell_of(app: &App, area: Rect, pos: Pos) -> Option<(u16, u16)> {
    let width_px = u32::from(area.width) * CELL_PX.0;
    let height_px = u32::from(area.height) * CELL_PX.1;
    let (x, y) = app.view.screen_px(pos, width_px, height_px);
    let column = (x / f64::from(CELL_PX.0)).floor();
    let row = (y / f64::from(CELL_PX.1)).floor();
    let inside = column >= 0.0
        && row >= 0.0
        && column < f64::from(area.width)
        && row < f64::from(area.height);
    if inside {
        Some((area.x + column as u16, area.y + row as u16))
    } else {
        None
    }
}

/// Draw the radar in `area` (the inside of the map panel).
pub(super) fn draw_radar(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }
    let buffer = frame.buffer_mut();
    let middle_x = area.x + area.width / 2;
    let middle_y = area.y + area.height / 2;
    let right = area.x + area.width - 1;
    let bottom = area.y + area.height - 1;
    for (x, y, letter) in [(middle_x, area.y, "N"), (middle_x, bottom, "S")] {
        buffer.set_string(x, y, letter, faint(theme));
    }
    for (x, y, letter) in [(area.x, middle_y, "W"), (right, middle_y, "E")] {
        buffer.set_string(x, y, letter, faint(theme));
    }

    let selected = app.current_conv();
    for node in &app.nodes {
        let Some(pos) = node.pos else { continue };
        let Some((x, y)) = cell_of(app, area, pos) else {
            continue;
        };
        let initial = initials(&node.name)
            .chars()
            .next()
            .unwrap_or('*')
            .to_string();
        if node.prefix == selected {
            let style = strong(theme).add_modifier(Modifier::REVERSED);
            buffer.set_string(x, y, initial, style);
            // The name, cut at the panel's right edge.
            let room = usize::from(right.saturating_sub(x));
            let name: String = ascii_safe(&node.name, true).chars().take(room).collect();
            buffer.set_string(x + 1, y, format!(" {name}"), strong(theme));
        } else {
            buffer.set_string(x, y, initial, text(theme));
        }
    }
    if let Some((x, y)) = app.self_pos.and_then(|pos| cell_of(app, area, pos)) {
        buffer.set_string(x, y, "+", strong(theme));
    }
}
