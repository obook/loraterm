//! radio/meshtastic/mod.rs
//! The Meshtastic side of the radio task: Bluetooth transport, conversions to our types and
//! the session, which sends the same FromRadio and takes the same ToRadio as MeshCore's.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

// shortcut: the session (plan task 5) uses this module; until then nothing calls it.
#![allow(dead_code)]

mod convert;
mod transport;
#[cfg(test)]
mod tests;
