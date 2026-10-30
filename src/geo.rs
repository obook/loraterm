//! geo.rs
//! Positions, distances (haversine formula) and bearings between nodes.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub lat: f64,
    pub lon: f64,
}

/// MeshCore sends positions in microdegrees; (0, 0) means "no position".
pub fn pos_from_micro(lat: i32, lon: i32) -> Option<Pos> {
    (lat != 0 || lon != 0).then(|| Pos {
        lat: lat as f64 / 1e6,
        lon: lon as f64 / 1e6,
    })
}

/// Great-circle distance (haversine).
pub fn distance_km(a: Pos, b: Pos) -> f64 {
    let (la, lb) = (a.lat.to_radians(), b.lat.to_radians());
    let h = ((lb - la) / 2.0).sin().powi(2)
        + la.cos() * lb.cos() * ((b.lon - a.lon).to_radians() / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().asin()
}

/// Initial bearing from `a` to `b`, in degrees clockwise from north (0..360).
pub fn bearing_deg(a: Pos, b: Pos) -> f64 {
    let (la, lb) = (a.lat.to_radians(), b.lat.to_radians());
    let dl = (b.lon - a.lon).to_radians();
    let y = dl.sin() * lb.cos();
    let x = la.cos() * lb.sin() - la.sin() * lb.cos() * dl.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

/// Eight-point compass direction of a bearing.
pub fn cardinal(deg: f64) -> &'static str {
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]
        [((deg.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

/// "850 m", "2.3 km" or "140 km".
pub fn format_distance(km: f64) -> String {
    if km < 1.0 {
        format!("{:.0} m", km * 1000.0)
    } else if km < 10.0 {
        format!("{km:.1} km")
    } else {
        format!("{km:.0} km")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_zero_means_no_position() {
        assert_eq!(pos_from_micro(0, 0), None);
    }

    #[test]
    fn microdegrees_to_degrees() {
        assert_eq!(
            pos_from_micro(44_837_800, -579_200),
            Some(Pos {
                lat: 44.8378,
                lon: -0.5792
            })
        );
    }

    #[test]
    fn bordeaux_to_paris_is_about_499_km() {
        let d = distance_km(
            Pos {
                lat: 44.8378,
                lon: -0.5792,
            },
            Pos {
                lat: 48.8566,
                lon: 2.3522,
            },
        );
        assert!((d - 499.0).abs() < 3.0, "{d}");
    }

    #[test]
    fn bearing_due_east_at_the_equator() {
        assert!(
            (bearing_deg(Pos { lat: 0.0, lon: 0.0 }, Pos { lat: 0.0, lon: 1.0 }) - 90.0).abs()
                < 1e-6
        );
    }

    #[test]
    fn bordeaux_to_paris_is_north_east() {
        let b = bearing_deg(
            Pos {
                lat: 44.8378,
                lon: -0.5792,
            },
            Pos {
                lat: 48.8566,
                lon: 2.3522,
            },
        );
        assert!((20.0..35.0).contains(&b), "{b}");
        assert_eq!(cardinal(b), "NE");
    }

    #[test]
    fn cardinal_points() {
        assert_eq!(
            [
                cardinal(0.0),
                cardinal(350.0),
                cardinal(90.0),
                cardinal(225.0)
            ],
            ["N", "N", "E", "SW"]
        );
    }

    #[test]
    fn distance_format() {
        assert_eq!(format_distance(0.85), "850 m");
        assert_eq!(format_distance(2.34), "2.3 km");
        assert_eq!(format_distance(140.4), "140 km");
    }
}
