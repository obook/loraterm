//! radio/link.rs
//! Finding, connecting and disconnecting the radio over BLE (meshcore-rs on BlueZ).
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::time::Duration;

use meshcore_rs::MeshCore;

/// `device`, or every radio named "MeshCore-..." BlueZ reports. BlueZ also reports radios
/// it only remembers, so the caller tries them in turn.
pub async fn candidates(device: Option<&str>) -> anyhow::Result<Vec<String>> {
    Ok(match device {
        Some(d) => vec![d.to_string()],
        None => MeshCore::ble_discover(Duration::from_secs(4))
            .await?
            .into_iter()
            .filter(|n| n.starts_with("MeshCore-"))
            .collect(),
    })
}

/// Connect to the radio `name`; a link left half open by a failed connect is dropped, or the
/// radio, which takes a single central, would refuse a phone.
pub async fn connect_to(name: &str) -> anyhow::Result<MeshCore> {
    match MeshCore::ble_connect(name).await {
        Ok(radio) => Ok(radio),
        Err(e) => {
            let _ = ble_disconnect(name).await;
            Err(e.into())
        }
    }
}

/// The first radio of `candidates` that connects, with its BLE name.
pub async fn connect(device: Option<&str>) -> anyhow::Result<(MeshCore, String)> {
    let mut last = anyhow::anyhow!("no MeshCore radio found");
    for name in candidates(device).await? {
        match connect_to(&name).await {
            Ok(radio) => return Ok((radio, name)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// meshcore-rs 0.2.0 `disconnect()` only stops its own tasks: the BLE link stays up and a
/// phone cannot connect to the radio. Drop the link of the radio named `name` (or address).
pub async fn ble_disconnect(name: &str) -> anyhow::Result<()> {
    Ok(MeshCore::ble_disconnect(name).await?)
}

/// Unix time in seconds, as the radio counts it.
pub fn now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as u32)
}
