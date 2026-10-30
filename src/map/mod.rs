//! map/mod.rs
//! Map viewport in Web Mercator, the projection of OpenStreetMap tiles: world pixels, the
//! tiles of a view, zoom, pan and the view that fits the nodes. Drawing is in tiles/.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use std::f64::consts::PI;

pub use crate::config::{FOCUS_ZOOM, PAN_PX};
pub use crate::geo::Pos;

/// Tile side in pixels.
pub const TILE: f64 = 256.0;
/// Smallest zoom: most of the world in one view.
pub const MIN_ZOOM: u8 = 1;
/// Largest zoom of the OpenStreetMap tiles: a street.
pub const MAX_ZOOM: u8 = 18;
/// Mercator stops here.
const MAX_LAT: f64 = 85.051_128_78;

/// Position in world pixels at `zoom`.
pub fn to_px(pos: Pos, zoom: u8) -> (f64, f64) {
    let world_px = TILE * f64::from(1u32 << zoom);
    let lat = pos.lat.clamp(-MAX_LAT, MAX_LAT).to_radians();
    (
        (pos.lon + 180.0) / 360.0 * world_px,
        (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0 * world_px,
    )
}

/// Position of the world pixel (x, y) at `zoom`; the inverse of `to_px`.
pub fn from_px(x: f64, y: f64, zoom: u8) -> Pos {
    let world_px = TILE * f64::from(1u32 << zoom);
    Pos {
        lat: (PI * (1.0 - 2.0 * y / world_px)).sinh().atan().to_degrees(),
        lon: x / world_px * 360.0 - 180.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapView {
    pub lat: f64,
    pub lon: f64,
    pub zoom: u8,
}

impl Default for MapView {
    /// Most of the world.
    fn default() -> Self {
        MapView {
            lat: 30.0,
            lon: 0.0,
            zoom: 2,
        }
    }
}

impl MapView {
    fn center_px(&self) -> (f64, f64) {
        to_px(
            Pos {
                lat: self.lat,
                lon: self.lon,
            },
            self.zoom,
        )
    }

    /// Where `p` falls in a view of `w` x `h` screen pixels.
    pub fn screen_px(&self, pos: Pos, w: u32, h: u32) -> (f64, f64) {
        let (cx, cy) = self.center_px();
        let (x, y) = to_px(pos, self.zoom);
        (x - cx + f64::from(w) / 2.0, y - cy + f64::from(h) / 2.0)
    }

    /// Tiles of a `w` x `h` view: (x, y, left, top), left/top being the screen pixel of the
    /// tile's corner. x wraps around the date line.
    pub fn tiles(&self, w: u32, h: u32) -> Vec<(u32, u32, i64, i64)> {
        let (cx, cy) = self.center_px();
        let (ox, oy) = (cx - f64::from(w) / 2.0, cy - f64::from(h) / 2.0);
        let n = 1i64 << self.zoom;
        let span = |o: f64, len: u32| {
            (o / TILE).floor() as i64..=((o + f64::from(len) - 1.0) / TILE).floor() as i64
        };
        let mut tiles = vec![];
        for ty in span(oy, h).filter(|ty| (0..n).contains(ty)) {
            for tx in span(ox, w) {
                let (left, top) = (
                    (tx as f64 * TILE - ox).round() as i64,
                    (ty as f64 * TILE - oy).round() as i64,
                );
                tiles.push((tx.rem_euclid(n) as u32, ty as u32, left, top));
            }
        }
        tiles
    }

    /// One zoom level closer, up to MAX_ZOOM.
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom + 1).min(MAX_ZOOM);
    }

    /// One zoom level farther, down to MIN_ZOOM.
    pub fn zoom_out(&mut self) {
        self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
    }

    /// dx > 0 is east, dy > 0 is north.
    pub fn pan(&mut self, dx: i8, dy: i8) {
        let (cx, cy) = self.center_px();
        let p = from_px(
            cx + f64::from(dx) * PAN_PX,
            cy - f64::from(dy) * PAN_PX,
            self.zoom,
        );
        self.lat = p.lat.clamp(-MAX_LAT, MAX_LAT);
        self.lon = p.lon.clamp(-180.0, 180.0);
    }

    /// Center on `pos`, and zoom in to at least the town scale.
    pub fn center_on(&mut self, pos: Pos) {
        self.lat = pos.lat;
        self.lon = pos.lon;
        self.zoom = self.zoom.max(FOCUS_ZOOM);
    }

    /// The closest view, up to FOCUS_ZOOM, that shows every point in a `w` x `h` view.
    pub fn fit(points: &[Pos], w: u32, h: u32) -> MapView {
        let Some(&first) = points.first() else {
            return MapView::default();
        };
        for zoom in (MIN_ZOOM..=FOCUS_ZOOM).rev() {
            let px: Vec<(f64, f64)> = points.iter().map(|&point| to_px(point, zoom)).collect();
            let (x0, x1) = px
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), &(x, _)| (a.min(x), b.max(x)));
            let (y0, y1) = px
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), &(_, y)| (a.min(y), b.max(y)));
            if x1 - x0 == 0.0 && y1 - y0 == 0.0 {
                return MapView {
                    lat: first.lat,
                    lon: first.lon,
                    zoom,
                };
            }
            // A margin keeps the dots off the edges.
            if x1 - x0 <= 0.8 * f64::from(w) && y1 - y0 <= 0.8 * f64::from(h) {
                let c = from_px((x0 + x1) / 2.0, (y0 + y1) / 2.0, zoom);
                return MapView {
                    lat: c.lat,
                    lon: c.lon,
                    zoom,
                };
            }
        }
        MapView {
            zoom: MIN_ZOOM,
            ..MapView::default()
        }
    }
}

#[cfg(test)]
mod tests;
