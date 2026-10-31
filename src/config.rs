//! config.rs
//! The settings worth tuning, in one place: radio timing, input limits, map and tile
//! behavior, display thresholds. Each module imports what it needs from here.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::time::Duration;

// ===============================================================
//  RADIO LINK
// ===============================================================

/// Wait between two connection attempts.
pub const RETRY: Duration = Duration::from_secs(5);
/// Battery read, also the link check: meshcore-rs reports no event when BlueZ drops the link.
pub const CHECK_EVERY: Duration = Duration::from_secs(15);
/// The radio sends one contact every 60 ms: a few hundred contacts take seconds.
pub const CONTACTS_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a ping waits for its answer: multi-hop answers take seconds.
pub const PING_TIMEOUT: Duration = Duration::from_secs(20);

// ===============================================================
//  MESSAGES
// ===============================================================

/// shortcut: conservative, under the firmware's 160-byte text limit including the "name: "
/// prefix of channel messages; check MAX_TEXT_LEN in the firmware to raise it.
pub const MAX_TEXT_BYTES: usize = 120;
/// Screen rows moved by PageUp / PageDown in the conversation.
pub const SCROLL_STEP: usize = 5;

// ===============================================================
//  MAP
// ===============================================================

/// Zoom used when centering on a node: about 10 km across.
pub const FOCUS_ZOOM: u8 = 13;
/// Distance moved by one arrow key, in screen pixels.
pub const PAN_PX: f64 = 200.0;
/// shortcut: the map's size in pixels is only known while drawing; fit to a typical panel.
pub const FIT_PX: (u32, u32) = (800, 450);
/// The first view shows the nodes within this distance of my radio.
pub const FIT_RADIUS_KM: f64 = 50.0;
/// Range rings around my radio on the Hacker map, in km.
pub const RINGS_KM: [f64; 3] = [1.0, 5.0, 10.0];
/// Days since a node was last heard at which its icon turns from fresh to stale, old and
/// extinct, as on map.meshcore.io.
pub const FRESHNESS_DAYS: [u32; 3] = [5, 10, 20];

// ===============================================================
//  OPENSTREETMAP TILES
// ===============================================================

/// OSM usage policy: identify the app, cache tiles, at most two downloads at once (we do one).
pub const TILE_URL: &str = "https://tile.openstreetmap.org";
/// Names the app to the tile server, as the OSM policy requires.
pub const TILE_USER_AGENT: &str = concat!(
    "loraterm/",
    env!("CARGO_PKG_VERSION"),
    " (terminal MeshCore and Meshtastic client)"
);
/// A cached tile older than this is shown, then downloaded again.
pub const TILE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// A failed tile download is tried again after this.
pub const TILE_RETRY: Duration = Duration::from_secs(60);
/// shortcut: decoded tiles kept in memory; the cache is simply emptied past this.
pub const TILES_IN_MEMORY: usize = 512;

// ===============================================================
//  DISPLAY
// ===============================================================

/// How long the Hacker boot sequence lasts; a key skips it.
pub const BOOT: Duration = Duration::from_millis(1500);
/// Battery under this percentage is shown as an alert.
pub const LOW_BATTERY: u8 = 20;
