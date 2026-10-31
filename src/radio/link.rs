//! radio/link.rs
//! Finding the radios over BLE and telling their firmware apart, connecting and
//! disconnecting a MeshCore radio (meshcore-rs on BlueZ).
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use meshcore_rs::MeshCore;
use uuid::Uuid;

use super::Firmware;

/// MeshCore companion: the Nordic UART service, generic, so the name tells it apart too.
pub const MESHCORE_SERVICE: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e);
pub const MESHTASTIC_SERVICE: Uuid = Uuid::from_u128(0x6ba1b218_15a8_461f_9fa8_5dcae273eafd);

/// How long a scan listens for radios.
const SCAN_TIME: Duration = Duration::from_secs(4);

/// A radio to try: its BLE name (or the address given with --device) and its firmware.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub firmware: Firmware,
}

/// The firmware a radio runs, from its advertised name and services; None when it is
/// neither. The Meshtastic service wins: a stale BlueZ cache can list both, and the session
/// checks the services once connected.
pub fn classify(name: Option<&str>, services: &[Uuid]) -> Option<Firmware> {
    if services.contains(&MESHTASTIC_SERVICE) {
        Some(Firmware::Meshtastic)
    } else if name.is_some_and(|n| n.starts_with("MeshCore-")) {
        Some(Firmware::MeshCore)
    } else {
        None
    }
}

/// Every MeshCore or Meshtastic radio BlueZ reports after a scan, or only `device` (a name
/// or an address). BlueZ also reports radios it only remembers, so the caller tries them in
/// turn. A `device` the scan does not find is tried as a MeshCore radio, as before.
pub async fn candidates(device: Option<&str>) -> anyhow::Result<Vec<Candidate>> {
    let found = scan().await?;
    let mut list: Vec<Candidate> = found
        .into_iter()
        .filter(|(name, address, _)| {
            device.is_none_or(|d| name.as_deref() == Some(d) || address == d)
        })
        .filter_map(|(name, address, services)| {
            let firmware = classify(name.as_deref(), &services)?;
            Some(Candidate {
                name: device.map_or_else(|| name.unwrap_or(address), str::to_string),
                firmware,
            })
        })
        .collect();
    if let (Some(d), true) = (device, list.is_empty()) {
        list.push(Candidate {
            name: d.to_string(),
            firmware: Firmware::MeshCore,
        });
    }
    Ok(list)
}

/// Name, address and advertised services of every radio BlueZ knows after a scan for
/// either service, through the process-wide BLE manager.
async fn scan() -> anyhow::Result<Vec<(Option<String>, String, Vec<Uuid>)>> {
    let adapter = meshcore_rs::ble_manager()
        .await?
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no Bluetooth adapter"))?;
    // All scans share one D-Bus sender, and BlueZ refuses a scan from a sender that left
    // one running.
    let _ = adapter.stop_scan().await;
    adapter
        .start_scan(ScanFilter {
            services: vec![MESHCORE_SERVICE, MESHTASTIC_SERVICE],
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start BLE scan: {e}"))?;
    tokio::time::sleep(SCAN_TIME).await;
    let _ = adapter.stop_scan().await;
    let mut found = vec![];
    for p in adapter.peripherals().await? {
        if let Ok(Some(props)) = p.properties().await {
            found.push((props.local_name, props.address.to_string(), props.services));
        }
    }
    Ok(found)
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

/// The first MeshCore radio of `candidates` that connects, with its BLE name.
pub async fn connect(device: Option<&str>) -> anyhow::Result<(MeshCore, String)> {
    let mut last = anyhow::anyhow!("no radio found");
    for c in candidates(device).await? {
        if c.firmware != Firmware::MeshCore {
            tracing::info!("{} radio {}: not supported yet", c.firmware.name(), c.name);
            continue;
        }
        match connect_to(&c.name).await {
            Ok(radio) => return Ok((radio, c.name)),
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
