//! ui/map_panel.rs
//! The map panel: rebuilds the map image only when something on it changes, and shows it
//! in the terminal with ratatui-image.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;

use super::format::coord;
use super::radar::{draw_radar, radar_px, scale};
use super::{faint, panel, Theme};
use crate::app::{App, Focus};
use crate::map::{MapView, Pos};
use crate::tiles::{compose, Icon, Look, Mark, Tiles};

/// What the map image needs between frames: it is rebuilt only when something on it changes.
pub struct MapState {
    picker: Picker,
    tiles: Tiles,
    shown: Option<Shown>,
    image: Option<StatefulProtocol>,
    /// The map's size in pixels at the last drawing.
    size_px: Option<(u32, u32)>,
}

#[derive(PartialEq)]
pub(super) struct Shown {
    view: MapView,
    w: u32,
    h: u32,
    marks: Vec<(Pos, Mark)>,
    look: Look,
}

impl MapState {
    /// `picker` says how the terminal shows images; `tiles` is the tile cache.
    pub fn new(picker: Picker, tiles: Tiles) -> MapState {
        MapState {
            picker,
            tiles,
            shown: None,
            image: None,
            size_px: None,
        }
    }

    /// The map's size in pixels at the last drawing; None before the first one.
    pub fn size_px(&self) -> Option<(u32, u32)> {
        self.size_px
    }
}

/// The map panel: title, OpenStreetMap credit and the map image.
pub(super) fn draw_map(frame: &mut Frame, app: &App, area: Rect, map: &mut MapState, theme: Theme) {
    let conv = app.current_conv();
    // The selected node's name and distance are in the node card.
    let (title, look) = match theme {
        Theme::Normal => ("Map".to_string(), Look::Plain),
        Theme::Hacker => (
            format!(
                "RADAR :: {} :: Z{}",
                coord(app.view.lat, app.view.lon),
                app.view.zoom
            ),
            Look::Night,
        ),
    };
    // No images on the Linux console: a radar of letters, with a scale instead of the tiles' credit.
    let bottom = if app.ascii {
        format!(" {} ", scale(app))
    } else {
        " (c) OpenStreetMap contributors ".to_string()
    };
    let block = panel(theme, app.ascii, app.focus == Focus::Map, &title)
        .title_bottom(Line::styled(bottom, faint(theme)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.ascii {
        map.size_px = Some(radar_px(inner));
        draw_radar(frame, app, inner, theme);
        return;
    }

    let font = map.picker.font_size();
    let (w, h) = (
        u32::from(inner.width) * u32::from(font.width),
        u32::from(inner.height) * u32::from(font.height),
    );
    map.size_px = Some((w, h));
    let now = chrono::Utc::now().timestamp();
    let mut marks: Vec<(Pos, Mark)> = app
        .nodes
        .iter()
        .filter_map(|n| {
            let age_days =
                (n.heard != 0).then(|| ((now - i64::from(n.heard)).max(0) / 86_400) as u32);
            let icon = Icon {
                kind: n.kind,
                name: n.name.clone(),
                age_days,
            };
            n.pos.map(|p| {
                (
                    p,
                    if n.prefix == conv {
                        Mark::Selected(icon)
                    } else {
                        Mark::Node(icon)
                    },
                )
            })
        })
        .collect();
    marks.extend(app.self_pos.map(|p| (p, Mark::Me)));
    let shown = Shown {
        view: app.view,
        w,
        h,
        marks,
        look,
    };
    if map.tiles.poll() || map.shown.as_ref() != Some(&shown) {
        let img = compose(&shown.view, w, h, &mut map.tiles, &shown.marks, look);
        map.image = Some(
            map.picker
                .new_resize_protocol(DynamicImage::ImageRgba8(img)),
        );
        map.shown = Some(shown);
    }
    if let Some(image) = &mut map.image {
        frame.render_stateful_widget(StatefulImage::default(), inner, image);
    }
}
