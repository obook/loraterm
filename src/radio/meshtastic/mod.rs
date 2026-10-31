//! radio/meshtastic/mod.rs
//! The Meshtastic side of the radio task: Bluetooth transport, conversions to our types and
//! the session, which sends the same FromRadio and takes the same ToRadio as MeshCore's.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

mod convert;
mod session;
mod transport;

pub(crate) use session::probe;
pub(super) use session::session;
pub(super) use transport::{RepairNeeded, Transport};
#[cfg(test)]
mod tests;
