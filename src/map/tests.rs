//! map/tests.rs
//! Tests of the Web Mercator viewport.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use super::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn world_pixels_of_the_origin() {
    assert_eq!(to_px(Pos { lat: 0.0, lon: 0.0 }, 0), (128.0, 128.0));
    assert_eq!(to_px(Pos { lat: 0.0, lon: 0.0 }, 1), (256.0, 256.0));
}

#[test]
fn pixels_round_trip() {
    let p = Pos {
        lat: 44.8378,
        lon: -0.5792,
    };
    let (x, y) = to_px(p, 13);
    let q = from_px(x, y, 13);
    assert!(close(p.lat, q.lat) && close(p.lon, q.lon), "{q:?}");
    // Bordeaux is in tile x 4082 at zoom 13.
    assert_eq!((x / TILE) as u32, 4082);
}

#[test]
fn tiles_cover_the_view() {
    let v = MapView {
        lat: 0.0,
        lon: 0.0,
        zoom: 1,
    };
    let mut t = v.tiles(512, 512);
    t.sort();
    assert_eq!(
        t,
        [
            (0, 0, 0, 0),
            (0, 1, 0, 256),
            (1, 0, 256, 0),
            (1, 1, 256, 256)
        ]
    );
}

#[test]
fn tiles_wrap_around_the_date_line() {
    let v = MapView {
        lat: 0.0,
        lon: 180.0,
        zoom: 1,
    };
    let xs: Vec<u32> = v.tiles(256, 256).iter().map(|t| t.0).collect();
    assert!(xs.contains(&0) && xs.contains(&1), "{xs:?}");
}

#[test]
fn center_is_the_middle_of_the_screen() {
    let v = MapView {
        lat: 44.8,
        lon: -0.6,
        zoom: 12,
    };
    let (x, y) = v.screen_px(
        Pos {
            lat: 44.8,
            lon: -0.6,
        },
        800,
        400,
    );
    assert!(close(x, 400.0) && close(y, 200.0));
}

#[test]
fn zoom_stays_in_range() {
    let mut v = MapView {
        zoom: MIN_ZOOM,
        ..Default::default()
    };
    v.zoom_out();
    assert_eq!(v.zoom, MIN_ZOOM);
    let mut v = MapView {
        zoom: MAX_ZOOM,
        ..Default::default()
    };
    v.zoom_in();
    assert_eq!(v.zoom, MAX_ZOOM);
}

#[test]
fn pan_moves_by_a_fixed_number_of_pixels() {
    let mut v = MapView {
        lat: 0.0,
        lon: 0.0,
        zoom: 1,
    };
    v.pan(1, 0);
    assert!(close(v.lon, PAN_PX / 512.0 * 360.0), "{}", v.lon);
    let mut v = MapView {
        lat: 0.0,
        lon: 0.0,
        zoom: 5,
    };
    v.pan(0, 1);
    assert!(v.lat > 0.0, "north is up");
}

#[test]
fn center_on_zooms_to_the_town() {
    let mut v = MapView::default();
    v.center_on(Pos {
        lat: 44.8,
        lon: -0.6,
    });
    assert_eq!((v.lat, v.lon, v.zoom), (44.8, -0.6, FOCUS_ZOOM));
}

#[test]
fn fit_shows_every_point() {
    let a = Pos {
        lat: 44.80,
        lon: -0.60,
    };
    let b = Pos {
        lat: 44.90,
        lon: -0.45,
    };
    let v = MapView::fit(&[a, b], 800, 400);
    for p in [a, b] {
        let (x, y) = v.screen_px(p, 800, 400);
        assert!(
            (0.0..800.0).contains(&x) && (0.0..400.0).contains(&y),
            "{v:?}"
        );
    }
    assert!(v.zoom >= 10, "{v:?}");
}

#[test]
fn fit_of_one_point_is_a_town_view() {
    let v = MapView::fit(
        &[Pos {
            lat: 44.8,
            lon: -0.6,
        }],
        800,
        400,
    );
    assert_eq!((v.lat, v.lon, v.zoom), (44.8, -0.6, FOCUS_ZOOM));
}
