//! radio/meshtastic/transport.rs
//! The Meshtastic Bluetooth protocol: write ToRadio, read FromRadio until it is empty, and
//! read again on each FromNum notification. Goes through the process-wide BLE manager.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::future::Future;
use std::pin::Pin;

use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ValueNotification, WriteType,
};
use btleplug::platform::Peripheral;
use futures::{Stream, StreamExt};
use meshtastic::protobufs;
use prost::Message;
use uuid::Uuid;

use crate::radio::link::MESHTASTIC_SERVICE;

const TO_RADIO: Uuid = Uuid::from_u128(0xf75c76d2_129e_4dad_a1dd_7866124401e7);
const FROM_RADIO: Uuid = Uuid::from_u128(0x2c55e69e_4993_11ed_b878_0242ac120002);
const FROM_NUM: Uuid = Uuid::from_u128(0xed9da18c_a800_4f66_a670_aa7547e34453);

/// BlueZ said the radio offers the Meshtastic service, the radio does not: the cache comes
/// from a bond made under the other firmware.
#[derive(Debug)]
pub struct RepairNeeded(pub String);

impl std::fmt::Display for RepairNeeded {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} does not offer the Meshtastic service: re-pair the radio with bluetoothctl",
            self.0
        )
    }
}

impl std::error::Error for RepairNeeded {}

/// An open link to a Meshtastic radio.
pub struct Transport {
    radio: Peripheral,
    to_radio: Characteristic,
    from_radio: Characteristic,
    notes: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
}

impl Transport {
    /// Connect to the radio named `name` (or its address); a half-open link is dropped.
    pub async fn connect(name: &str) -> anyhow::Result<Transport> {
        let radio = find(name).await?;
        radio.connect().await?;
        match Transport::open(radio.clone(), name).await {
            Ok(transport) => Ok(transport),
            Err(e) => {
                let _ = radio.disconnect().await;
                Err(e)
            }
        }
    }

    async fn open(radio: Peripheral, name: &str) -> anyhow::Result<Transport> {
        radio.discover_services().await?;
        if !radio.services().iter().any(|s| s.uuid == MESHTASTIC_SERVICE) {
            return Err(RepairNeeded(name.to_string()).into());
        }
        let chars = radio.characteristics();
        let get = |uuid: Uuid| {
            chars
                .iter()
                .find(|c| c.uuid == uuid)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{name}: no characteristic {uuid}"))
        };
        let (to_radio, from_radio, from_num) = (get(TO_RADIO)?, get(FROM_RADIO)?, get(FROM_NUM)?);
        // A subscription left by an earlier session would deliver every notification twice.
        let _ = radio.unsubscribe(&from_num).await;
        radio.subscribe(&from_num).await?;
        let notes = radio
            .notifications()
            .await?
            .filter(|n| futures::future::ready(n.uuid == FROM_NUM))
            .boxed();
        Ok(Transport {
            radio,
            to_radio,
            from_radio,
            notes,
        })
    }

    /// One ToRadio, as is: no 4-byte header over Bluetooth.
    pub async fn send(&self, msg: &protobufs::ToRadio) -> anyhow::Result<()> {
        self.radio
            .write(&self.to_radio, &msg.encode_to_vec(), WriteType::WithResponse)
            .await?;
        Ok(())
    }

    /// Everything the radio has queued for us.
    pub async fn drain(&self) -> anyhow::Result<Vec<protobufs::FromRadio>> {
        let chunks =
            read_until_empty(|| async { Ok(self.radio.read(&self.from_radio).await?) }).await?;
        chunks
            .iter()
            .map(|c| Ok(protobufs::FromRadio::decode(c.as_slice())?))
            .collect()
    }

    /// Wait for the radio to say it has something; false once the link is gone.
    pub async fn notified(&mut self) -> bool {
        self.notes.next().await.is_some()
    }

    pub async fn disconnect(&self) {
        let _ = self.radio.disconnect().await;
    }
}

/// The peripheral BlueZ knows under `name` or with that address.
async fn find(name: &str) -> anyhow::Result<Peripheral> {
    for adapter in meshcore_rs::ble_manager().await?.adapters().await? {
        for p in adapter.peripherals().await? {
            if let Ok(Some(props)) = p.properties().await {
                if props.local_name.as_deref() == Some(name) || props.address.to_string() == name {
                    return Ok(p);
                }
            }
        }
    }
    Err(anyhow::anyhow!("{name} not found"))
}

/// Read until the radio answers an empty value: that is how it says its queue is empty.
pub(super) async fn read_until_empty<F, Fut>(mut read: F) -> anyhow::Result<Vec<Vec<u8>>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<Vec<u8>>>,
{
    let mut chunks = vec![];
    loop {
        let chunk = read().await?;
        if chunk.is_empty() {
            return Ok(chunks);
        }
        chunks.push(chunk);
    }
}
