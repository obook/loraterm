//! radio/meshtastic/tests.rs
//! Tests of the Meshtastic transport, conversions and handshake.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::transport::*;
use std::cell::Cell;

#[tokio::test]
async fn read_until_empty_stops_at_the_first_empty_read() {
    let chunks = [vec![1u8], vec![2, 3], vec![], vec![9]];
    let calls = Cell::new(0);
    let read = || {
        let i = calls.get();
        calls.set(i + 1);
        let chunk = chunks[i].clone();
        async move { anyhow::Ok(chunk) }
    };
    assert_eq!(read_until_empty(read).await.unwrap(), vec![vec![1], vec![2, 3]]);
    assert_eq!(calls.get(), 3);
}

#[tokio::test]
async fn read_until_empty_passes_errors_on() {
    let read = || async { Err::<Vec<u8>, _>(anyhow::anyhow!("link lost")) };
    assert!(read_until_empty(read).await.is_err());
}

#[test]
fn repair_needed_says_what_to_do() {
    let msg = RepairNeeded("Meshtastic_d940".into()).to_string();
    assert!(msg.contains("re-pair the radio with bluetoothctl"), "{msg}");
}
