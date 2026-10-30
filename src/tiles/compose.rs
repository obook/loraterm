//! tiles/compose.rs
//! The map image: tiles, then marks for the nodes, my radio and the selected node (a pin).
//! The Night look (Hacker theme) recolors the tiles and adds a grid, range rings, a
//! crosshair and scanlines.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use image::{imageops, Rgba, RgbaImage};

use super::icons::{draw_icon, Icon, PIN_CROSS, PIN_HEAD, PIN_RED};
use super::Tiles;
use crate::config::RINGS_KM;
use crate::map::{MapView, Pos};

/// How the map is drawn: OSM colors, or night vision with radar overlays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    Plain,
    Night,
}

// Plain palette.
/// Where a tile is missing (Normal look).
pub(super) const PLAIN_BACKGROUND: Rgba<u8> = Rgba([40, 40, 40, 255]);
/// My radio's triangle (Normal look).
pub(super) const PLAIN_ME: Rgba<u8> = Rgba([255, 140, 0, 255]);

// Night-vision palette.
/// Where a tile is missing (Hacker look).
pub const BACKGROUND: Rgba<u8> = Rgba([0, 8, 2, 255]);
/// A node's cross (Hacker look).
pub const NODE: Rgba<u8> = Rgba([170, 255, 170, 255]);
/// The selected node's pin (Hacker look).
pub const SELECTED: Rgba<u8> = Rgba([255, 255, 255, 255]);
/// My radio's triangle (Hacker look).
pub const ME: Rgba<u8> = Rgba([255, 50, 50, 255]);
/// Range rings around my radio.
pub const RING: Rgba<u8> = Rgba([150, 255, 180, 255]);
/// The dotted grid.
pub(super) const GRID: Rgba<u8> = Rgba([0, 70, 25, 255]);
/// The crosshair at the center of the map.
pub(super) const CROSSHAIR: Rgba<u8> = Rgba([0, 200, 80, 255]);
/// Outline that keeps marks readable on any tile.
pub(super) const OUTLINE: Rgba<u8> = Rgba([0, 0, 0, 255]);
/// Grid spacing, in pixels; lines fall between the center and the edges.
pub(super) const GRID_PX: i64 = 100;

/// OSM colors to night vision: luminance inverted, boosted, in green. Light land turns dark,
/// dark labels turn bright.
pub fn night(c: Rgba<u8>) -> Rgba<u8> {
    let lum = 0.299 * f64::from(c[0]) + 0.587 * f64::from(c[1]) + 0.114 * f64::from(c[2]);
    let v = ((255.0 - lum) * 1.8).min(255.0);
    Rgba([0, v as u8, (v / 4.0) as u8, 255])
}

/// Radius in screen pixels of `km` at latitude `lat` and `zoom` (Web Mercator).
pub fn ring_px(km: f64, lat: f64, zoom: u8) -> f64 {
    let meters_per_px = 156_543.033_92 * lat.to_radians().cos() / f64::from(1u32 << zoom);
    km * 1000.0 / meters_per_px
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Node(Icon),
    Selected(Icon),
    Me,
}

/// A map pin whose tip is on (x, y): a tapering body and a round head.
pub(super) fn pin(img: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    let top = y - PIN_HEAD + 8;
    for row in top..=y {
        let half = 12 * (y - row) / (y - top);
        (-half..=half).for_each(|dx| put(img, x + dx, row, color));
    }
    for dy in -17..=17_i64 {
        for dx in -17..=17_i64 {
            if dx * dx + dy * dy <= 17 * 17 {
                put(img, x + dx, y - PIN_HEAD + dy, color);
            }
        }
    }
}

/// The map as a `w` x `h` image, then marks (the selected node on top). The night look
/// recolors the tiles and adds a grid, range rings, a crosshair and scanlines.
pub fn compose(
    view: &MapView,
    w: u32,
    h: u32,
    tiles: &mut Tiles,
    marks: &[(Pos, Mark)],
    look: Look,
) -> RgbaImage {
    let night_look = look == Look::Night;
    let mut img = RgbaImage::from_pixel(
        w,
        h,
        if night_look {
            BACKGROUND
        } else {
            PLAIN_BACKGROUND
        },
    );
    for (x, y, left, top) in view.tiles(w, h) {
        if let Some(tile) = tiles.get((view.zoom, x, y)) {
            if night_look {
                let mut tile = tile.clone();
                tile.pixels_mut().for_each(|p| *p = night(*p));
                imageops::overlay(&mut img, &tile, left, top);
            } else {
                imageops::overlay(&mut img, tile, left, top);
            }
        }
    }
    if night_look {
        radar(&mut img, view, marks);
    }
    // Nodes, then my radio, then the selected node on top.
    for pass in 0..3 {
        for (pos, mark) in marks {
            let layer = match mark {
                Mark::Node(_) => 0,
                Mark::Me => 1,
                Mark::Selected(_) => 2,
            };
            if layer != pass {
                continue;
            }
            let (x, y) = view.screen_px(*pos, w, h);
            let (x, y) = (x.round() as i64, y.round() as i64);
            match (mark, look) {
                (Mark::Node(_), Look::Night) => cross(&mut img, x, y, NODE),
                (Mark::Node(icon), Look::Plain) => draw_icon(&mut img, x, y, icon),
                (Mark::Me, Look::Night) => triangle(&mut img, x, y, ME),
                (Mark::Me, Look::Plain) => triangle(&mut img, x, y, PLAIN_ME),
                (Mark::Selected(_), Look::Night) => {
                    pin(&mut img, x, y, SELECTED);
                    cross(&mut img, x, y - PIN_HEAD, PIN_CROSS);
                }
                (Mark::Selected(icon), Look::Plain) => {
                    pin(&mut img, x, y, PIN_RED);
                    draw_icon(&mut img, x, y - PIN_HEAD, icon);
                }
            }
        }
    }
    img
}

/// Night-look overlays: grid, range rings around my radio, crosshair, scanlines.
pub(super) fn radar(img: &mut RgbaImage, view: &MapView, marks: &[(Pos, Mark)]) {
    let (w, h) = img.dimensions();
    let (cx, cy) = (i64::from(w / 2), i64::from(h / 2));
    let (wi, hi) = (i64::from(w), i64::from(h));
    // Dotted grid, its lines halfway between the center and the next line.
    for x in (0..wi).filter(|x| (x - cx - GRID_PX / 2).rem_euclid(GRID_PX) == 0) {
        (0..hi).step_by(2).for_each(|y| put(img, x, y, GRID));
    }
    for y in (0..hi).filter(|y| (y - cy - GRID_PX / 2).rem_euclid(GRID_PX) == 0) {
        (0..wi).step_by(2).for_each(|x| put(img, x, y, GRID));
    }
    for &(pos, _) in marks.iter().filter(|(_, m)| matches!(m, Mark::Me)) {
        let (x, y) = view.screen_px(pos, w, h);
        for km in RINGS_KM {
            let r = ring_px(km, pos.lat, view.zoom).round() as i64;
            if (6..=2 * wi.max(hi)).contains(&r) {
                circle(img, x.round() as i64, y.round() as i64, r, RING, true);
            }
        }
    }
    for d in 6..=20 {
        for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
            put(img, cx + dx, cy + dy, CROSSHAIR);
        }
    }
    // CRT scanlines.
    for (_, y, pixel) in img.enumerate_pixels_mut() {
        if y % 3 == 2 {
            *pixel = Rgba([pixel[0] / 4 * 3, pixel[1] / 4 * 3, pixel[2] / 4 * 3, 255]);
        }
    }
}

/// Set a pixel, ignoring what falls outside the image.
pub(super) fn put(img: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && x < i64::from(img.width()) && y < i64::from(img.height()) {
        img.put_pixel(x as u32, y as u32, color);
    }
}

/// Midpoint circle outline, dashed or not.
pub(super) fn circle(img: &mut RgbaImage, cx: i64, cy: i64, r: i64, color: Rgba<u8>, dashed: bool) {
    let (mut x, mut y, mut err) = (r, 0, 1 - r);
    while x >= y {
        if !dashed || (y / 4) % 2 == 0 {
            for (dx, dy) in [
                (x, y),
                (y, x),
                (-y, x),
                (-x, y),
                (-x, -y),
                (-y, -x),
                (y, -x),
                (x, -y),
            ] {
                put(img, cx + dx, cy + dy, color);
            }
        }
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

/// A node: an X with a dark outline.
pub(super) fn cross(img: &mut RgbaImage, x: i64, y: i64, fill: Rgba<u8>) {
    for (color, width) in [(OUTLINE, 2), (fill, 1)] {
        for t in -4..=4 {
            for k in -width + 1..width {
                put(img, x + t + k, y + t, color);
                put(img, x + t + k, y - t, color);
            }
        }
    }
}

/// My radio: a red triangle pointing up, outlined.
pub(super) fn triangle(img: &mut RgbaImage, x: i64, y: i64, fill: Rgba<u8>) {
    for (color, grow) in [(OUTLINE, 2), (fill, 0)] {
        for dy in -7 - grow..=5 + grow {
            let half = (dy + 7 + grow) * (6 + grow) / (12 + 2 * grow);
            (-half..=half).for_each(|dx| put(img, x + dx, y + dy, color));
        }
    }
}
