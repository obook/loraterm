//! tiles/mod.rs
//! OpenStreetMap tiles: disk cache (~/.cache/loraterm/tiles) and downloads in a
//! background thread, following the OSM tile usage policy. compose.rs draws the map image,
//! icons.rs the node icons.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

mod compose;
mod font;
mod icons;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime};

use image::RgbaImage;

use crate::config::{TILES_IN_MEMORY, TILE_MAX_AGE, TILE_RETRY, TILE_URL, TILE_USER_AGENT};
pub use compose::{compose, ring_px, Look, Mark};
pub use icons::{initials, Icon};

type Key = (u8, u32, u32);

/// A tile in the download queue, or one whose download failed (retried after TILE_RETRY).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Asked {
    Waiting,
    Failed(Instant),
}

pub struct Tiles {
    dir: PathBuf,
    loaded: HashMap<Key, RgbaImage>,
    /// Tiles asked for and not on disk yet.
    asked: HashMap<Key, Asked>,
    requests: Option<Sender<Key>>,
    done: Receiver<(Key, bool)>,
    done_tx: Sender<(Key, bool)>,
}

impl Tiles {
    /// Tiles cached in `dir`, downloading the missing ones in a background thread.
    pub fn new(dir: PathBuf) -> Tiles {
        let mut tiles = Tiles::offline(dir.clone());
        let (tx, rx) = mpsc::channel();
        let done = tiles.done_tx.clone();
        std::thread::spawn(move || download(&dir, &rx, &done));
        tiles.requests = Some(tx);
        tiles
    }

    /// Cached tiles only: nothing is downloaded.
    pub fn offline(dir: PathBuf) -> Tiles {
        let (done_tx, done) = mpsc::channel();
        Tiles {
            dir,
            loaded: HashMap::new(),
            asked: HashMap::new(),
            requests: None,
            done,
            done_tx,
        }
    }

    pub fn default_dir() -> PathBuf {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
            })
            .join("loraterm/tiles")
    }

    /// Downloads asked for and not done yet.
    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.asked.len()
    }

    /// True when new tiles arrived since the last call.
    pub fn poll(&mut self) -> bool {
        let mut arrived = false;
        while let Ok((key, ok)) = self.done.try_recv() {
            if ok {
                self.asked.remove(&key);
                self.loaded.remove(&key);
                arrived = true;
            } else {
                self.asked.insert(key, Asked::Failed(Instant::now()));
            }
        }
        arrived
    }

    fn ask(&mut self, key: Key) {
        match self.asked.get(&key) {
            // Still in the queue: asking again would only add a duplicate download.
            Some(Asked::Waiting) => return,
            Some(Asked::Failed(at)) if at.elapsed() < TILE_RETRY => return,
            _ => {}
        }
        self.asked.insert(key, Asked::Waiting);
        if let Some(tx) = &self.requests {
            let _ = tx.send(key);
        }
    }

    /// The tile if it is on disk; asks for it when missing or stale.
    fn get(&mut self, key: Key) -> Option<&RgbaImage> {
        if !self.loaded.contains_key(&key) {
            let (z, x, y) = key;
            let path = self.dir.join(format!("{z}/{x}/{y}.png"));
            let age = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok());
            if age.is_none_or(|a| a > TILE_MAX_AGE) {
                self.ask(key);
            }
            let Ok(img) = image::open(&path) else {
                self.ask(key);
                return None;
            };
            let img = img.to_rgba8();
            if self.loaded.len() >= TILES_IN_MEMORY {
                self.loaded.clear();
            }
            self.loaded.insert(key, img);
        }
        self.loaded.get(&key)
    }
}

/// One download at a time; each tile is written whole (temp file, then rename).
fn download(dir: &Path, requests: &Receiver<Key>, done: &Sender<(Key, bool)>) {
    // A stalled connection must not hold the queue.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    for (z, x, y) in requests {
        let path = dir.join(format!("{z}/{x}/{y}.png"));
        let result = (|| -> anyhow::Result<()> {
            let bytes = agent
                .get(format!("{TILE_URL}/{z}/{x}/{y}.png"))
                .header("User-Agent", TILE_USER_AGENT)
                .call()?
                .into_body()
                .read_to_vec()?;
            std::fs::create_dir_all(path.parent().unwrap_or(dir))?;
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, &path)?;
            Ok(())
        })();
        if let Err(e) = &result {
            tracing::warn!("tile {z}/{x}/{y}: {e:#}");
        }
        if done.send(((z, x, y), result.is_ok())).is_err() {
            return;
        }
    }
}
