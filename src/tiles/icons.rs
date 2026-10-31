//! tiles/icons.rs
//! Node icons of the Plain look, after map.meshcore.io (MIT, Rastislav Vysoky): a person is
//! a disc in a color derived from its name, with its initials; a repeater, room or sensor is
//! the map's pictogram (assets/node_types), colored by how long ago it was heard.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0
use image::{Rgba, RgbaImage};

use super::compose::put;
use super::font::{draw_text, glyph};
use crate::config::FRESHNESS_DAYS;
use crate::radio::NodeKind;

/// What a node's icon shows (plain look).
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub kind: NodeKind,
    pub name: String,
    /// Days since my radio last heard it; None if never.
    pub age_days: Option<u32>,
}

/// The pictograms' own background color, replaced by a freshness color.
pub const SLATE: Rgba<u8> = Rgba([102, 123, 137, 255]);
/// Heard less than FRESHNESS_DAYS[0] days ago.
pub const RECENT: Rgba<u8> = Rgba([46, 160, 67, 255]);
/// Heard less than FRESHNESS_DAYS[1] days ago.
pub const STALE: Rgba<u8> = Rgba([214, 170, 0, 255]);
/// Heard less than FRESHNESS_DAYS[2] days ago.
pub const OLD: Rgba<u8> = Rgba([207, 64, 48, 255]);
/// Not heard for longer.
pub const EXTINCT: Rgba<u8> = Rgba([140, 140, 140, 255]);
/// The initials and the pictograms' symbols.
pub const GLYPH: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// The selected node's map pin: red on the plain map, white (SELECTED) on the night map.
pub const PIN_RED: Rgba<u8> = Rgba([211, 47, 47, 255]);
/// The cross in the head of the Hacker pin.
pub const PIN_CROSS: Rgba<u8> = Rgba([40, 40, 40, 255]);
/// From the pin's tip (on the node) up to the center of its head.
pub const PIN_HEAD: i64 = 34;
/// Half the size of an icon, in pixels (icons are 28 x 28).
pub(super) const ICON_HALF: i64 = 14;

/// The pictograms of map.meshcore.io, 28 px (assets/node_types, MIT).
pub(super) fn pictogram(kind: NodeKind) -> &'static RgbaImage {
    static ICONS: std::sync::OnceLock<[RgbaImage; 3]> = std::sync::OnceLock::new();
    let icons = ICONS.get_or_init(|| {
        let load = |png: &[u8]| {
            image::load_from_memory(png)
                .map(|i| i.to_rgba8())
                .unwrap_or_else(|_| RgbaImage::new(28, 28))
        };
        [
            load(include_bytes!("../../assets/node_types/2.png")),
            load(include_bytes!("../../assets/node_types/3.png")),
            load(include_bytes!("../../assets/node_types/4.png")),
        ]
    });
    match kind {
        NodeKind::Room => &icons[1],
        NodeKind::Sensor => &icons[2],
        _ => &icons[0],
    }
}

/// FNV-1a over UTF-16 code units, as the map does in JavaScript.
pub fn fnv1a(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for code_unit in name.encode_utf16() {
        hash ^= u32::from(code_unit);
        // Multiplication modulo 2^32, as JavaScript's 32-bit arithmetic does.
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// hsl(hash % 360, 60 %, 50 %).
pub fn name_color(name: &str) -> Rgba<u8> {
    let h = f64::from(fnv1a(name) % 360) / 60.0;
    let (c, m) = (0.6, 0.2);
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f64| ((v + m) * 255.0).round() as u8;
    Rgba([to(r), to(g), to(b), 255])
}

/// One letter, or the first letters of the first and last words; emoji cannot be drawn
/// here, so they are dropped.
pub fn initials(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| c.is_ascii() || c.is_alphanumeric())
        .collect();
    let words: Vec<&str> = kept.split(' ').filter(|w| !w.is_empty()).collect();
    let picked: Vec<char> = match words.as_slice() {
        [] => vec![],
        [one] => one.chars().take(1).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    };
    picked
        .into_iter()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| glyph(*c).is_some())
        .collect()
}

/// Days since last heard -> the map's freshness color (5, 10 and 20 day thresholds).
pub fn freshness(age_days: Option<u32>) -> Rgba<u8> {
    match age_days {
        None => SLATE,
        Some(d) if d < FRESHNESS_DAYS[0] => RECENT,
        Some(d) if d < FRESHNESS_DAYS[1] => STALE,
        Some(d) if d < FRESHNESS_DAYS[2] => OLD,
        Some(_) => EXTINCT,
    }
}

/// Recolor a pictogram pixel: its slate background becomes `color`, its white glyph stays.
pub fn tint(pixel: Rgba<u8>, color: Rgba<u8>) -> Rgba<u8> {
    let whiteness = ((f32::from(pixel[0]) - 102.0) / 153.0).clamp(0.0, 1.0);
    let mix = |c: u8| (f32::from(c) * (1.0 - whiteness) + 255.0 * whiteness).round() as u8;
    Rgba([mix(color[0]), mix(color[1]), mix(color[2]), pixel[3]])
}

/// A node's icon centered on (cx, cy).
pub(super) fn draw_icon(img: &mut RgbaImage, cx: i64, cy: i64, icon: &Icon) {
    if icon.kind == NodeKind::Chat {
        let color = name_color(&icon.name);
        for dy in -ICON_HALF..=ICON_HALF {
            for dx in -ICON_HALF..=ICON_HALF {
                if dx * dx + dy * dy <= ICON_HALF * ICON_HALF {
                    put(img, cx + dx, cy + dy, color);
                }
            }
        }
        draw_text(img, cx, cy, &initials(&icon.name));
        return;
    }
    let color = freshness(icon.age_days);
    for (x, y, pixel) in pictogram(icon.kind).enumerate_pixels() {
        let (px, py) = (cx - ICON_HALF + i64::from(x), cy - ICON_HALF + i64::from(y));
        if pixel[3] == 0
            || px < 0
            || py < 0
            || px >= i64::from(img.width())
            || py >= i64::from(img.height())
        {
            continue;
        }
        let top = tint(*pixel, color);
        let alpha = f32::from(pixel[3]) / 255.0;
        let under = *img.get_pixel(px as u32, py as u32);
        let blend = |i: usize| {
            (f32::from(top[i]) * alpha + f32::from(under[i]) * (1.0 - alpha)).round() as u8
        };
        img.put_pixel(
            px as u32,
            py as u32,
            Rgba([blend(0), blend(1), blend(2), 255]),
        );
    }
}
