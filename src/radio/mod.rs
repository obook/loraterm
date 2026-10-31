//! radio/mod.rs
//! The radio task: holds the BLE link to the MeshCore companion radio, reconnects on its own,
//! reports to the interface (FromRadio) and takes its commands (ToRadio).
//! link.rs connects and disconnects, session.rs runs one connection, convert.rs turns
//! meshcore-rs types into ours.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

mod convert;
mod link;
mod ping;
mod session;
mod task;
#[cfg(test)]
mod tests;

use crate::geo::Pos;
use crate::history::{Record, PUBLIC};
pub use convert::lipo_percent;
pub use link::{ble_disconnect, connect};
pub use task::spawn;

#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// Device name, or a description when none was given.
    Searching(String),
    /// BLE name of the radio.
    Connected(String),
    /// Why the link is down.
    Lost(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub pct: u8,
    pub mv: u16,
    pub used_kb: Option<u32>,
    pub total_kb: Option<u32>,
}

/// What my radio says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MyRadio {
    pub prefix: String,
    pub name: String,
    pub model: Option<String>,
    pub version: Option<String>,
    pub build: Option<String>,
    pub freq_khz: u32,
    pub bw_hz: u32,
    pub sf: u8,
    pub cr: u8,
    pub tx_power: u8,
    pub max_tx_power: u8,
    pub shares_position: bool,
}

pub enum FromRadio {
    Link(Link),
    SelfPos(Option<Pos>),
    Battery(Battery),
    MyRadio(MyRadio),
    Nodes(Vec<Node>),
    Record(Record),
    /// The radio did not take the message: `conv` and `text` let the UI give it back.
    SendFailed {
        conv: String,
        text: String,
        error: String,
    },
    Pong {
        prefix: String,
        ping: Ping,
    },
}

#[derive(Debug, PartialEq)]
pub enum ToRadio {
    SendDirect {
        prefix: String,
        text: String,
    },
    SendPublic {
        text: String,
    },
    /// Status request to a repeater or room; `key` is its full public key in hex.
    Ping {
        key: String,
        prefix: String,
    },
}

/// Outcome of a ping (status request).
#[derive(Clone, Debug, PartialEq)]
pub enum Ping {
    Waiting,
    Pong {
        rtt_ms: u64,
        snr: f32,
        battery_mv: u16,
        uptime: u32,
        noise: i16,
    },
    Failed(String),
}

impl ToRadio {
    pub fn failed(&self, error: String) -> FromRadio {
        let (conv, text) = match self {
            ToRadio::SendDirect { prefix, text } => (prefix.clone(), text.clone()),
            ToRadio::SendPublic { text } => (PUBLIC.to_string(), text.clone()),
            ToRadio::Ping { prefix, .. } => {
                return FromRadio::Pong {
                    prefix: prefix.clone(),
                    ping: Ping::Failed(error),
                }
            }
        };
        FromRadio::SendFailed { conv, text, error }
    }
}

/// The history file cannot be written: fetching more messages would lose them.
#[derive(Debug)]
pub struct HistoryWriteFailed(pub(super) std::io::Error);

impl std::fmt::Display for HistoryWriteFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "history write failed: {}", self.0)
    }
}

impl std::error::Error for HistoryWriteFailed {}

/// Shown while searching when no device name was given.
pub const ANY_RADIO: &str = "a MeshCore or Meshtastic radio";

/// What the radio runs, found from what it advertises over Bluetooth.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Firmware {
    #[default]
    MeshCore,
    Meshtastic,
}

impl Firmware {
    pub fn name(self) -> &'static str {
        match self {
            Firmware::MeshCore => "MeshCore",
            Firmware::Meshtastic => "Meshtastic",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NodeKind {
    #[default]
    Chat,
    Repeater,
    Room,
    Sensor,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    /// First 6 bytes of the public key, in hex: the conversation key.
    pub prefix: String,
    pub name: String,
    pub kind: NodeKind,
    pub pos: Option<Pos>,
    /// Full public key in hex, needed for a status request.
    pub key: String,
    /// Last time my radio heard of it (Unix seconds, my radio's clock); 0 if never.
    pub heard: u32,
    /// Hops on the known route; -1 when none is known (messages flood the mesh).
    pub path_len: i8,
    /// The route's relay hashes.
    pub path: Vec<u8>,
}
