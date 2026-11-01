//! probe.rs
//! --probe: connect to the radio, print its name, firmware, battery, position and contact
//! (or node) count, then drop the link. Checks the Bluetooth setup without the interface.
//! --cli: send one text command to a MeshCore radio ("set ui.backlight_on_keys 1"...) and
//! print its reply.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::time::Duration;

use meshcore_rs::{EventPayload, EventType};

use crate::radio::{self, Candidate, Firmware};

/// MeshCore companion protocol v14+: a text command, and the radio's reply to it.
const CMD_RUN_CLI_COMMAND: u8 = 66;
const RESP_CODE_CLI_REPLY: u8 = 29;

/// Send `command` to the first MeshCore radio that connects and print its reply. Only a
/// failed connect moves on to the next radio: a command is never sent to two radios.
pub async fn cli(device: Option<&str>, command: &str) -> anyhow::Result<()> {
    let mut last = anyhow::anyhow!("no MeshCore radio found");
    for Candidate { name, firmware } in radio::candidates(device).await? {
        if firmware != Firmware::MeshCore {
            continue;
        }
        let radio = match radio::connect_to(&name).await {
            Ok(radio) => radio,
            Err(e) => {
                last = e;
                continue;
            }
        };
        let reply = async {
            let cmd = radio.commands().lock().await;
            cmd.send_appstart().await?;
            let mut frame = vec![CMD_RUN_CLI_COMMAND];
            frame.extend_from_slice(command.as_bytes());
            // meshcore-rs does not know code 29: it arrives as an Unknown event, code included.
            // An older firmware answers Error.
            let event = cmd
                .send_multi(
                    &frame,
                    &[EventType::Unknown, EventType::Error],
                    Duration::from_secs(10),
                )
                .await?;
            match event.payload {
                EventPayload::Bytes(b) if b.first() == Some(&RESP_CODE_CLI_REPLY) => {
                    Ok(String::from_utf8_lossy(&b[1..]).into_owned())
                }
                _ => Err(anyhow::anyhow!(
                    "the radio did not run the command (firmware older than v14?)"
                )),
            }
        }
        .await;
        // Drop the link even when the command failed, or a phone cannot reach the radio.
        let _ = radio.disconnect().await;
        let dropped = radio::ble_disconnect(&name).await;
        println!("{}", reply?);
        return dropped;
    }
    Err(last)
}

/// Probe the first radio that answers, of either firmware.
pub async fn probe(device: Option<&str>) -> anyhow::Result<()> {
    let mut last = anyhow::anyhow!("no radio found");
    for Candidate { name, firmware } in radio::candidates(device).await? {
        let result = match firmware {
            Firmware::MeshCore => meshcore(&name).await,
            Firmware::Meshtastic => radio::probe_meshtastic(&name).await,
        };
        match result {
            Ok(()) => return Ok(()),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Connect to the MeshCore radio `name`, print its state, then drop the link, even when a
/// read failed.
async fn meshcore(name: &str) -> anyhow::Result<()> {
    let radio = radio::connect_to(name).await?;
    let read = async {
        let cmd = radio.commands().lock().await;
        let me = cmd.send_appstart().await?;
        let battery = cmd.get_bat().await?;
        let contacts = cmd
            .get_contacts_with_timeout(0, Duration::from_secs(30))
            .await?;
        println!("device   {}", me.name);
        println!("firmware MeshCore");
        println!(
            "battery  {} %  ({} mV)",
            radio::lipo_percent(battery.battery_mv),
            battery.battery_mv
        );
        if me.adv_lat == 0 && me.adv_lon == 0 {
            println!("pos      none");
        } else {
            println!(
                "pos      {:.6}, {:.6}",
                me.adv_lat as f64 / 1e6,
                me.adv_lon as f64 / 1e6
            );
        }
        println!("contacts {}", contacts.len());
        let now = chrono::Utc::now().timestamp();
        let mut list: Vec<_> = contacts.iter().collect();
        list.sort_by_key(|c| c.adv_name.to_lowercase());
        for c in list {
            let heard = match c.last_modification_timestamp.max(c.last_advert) {
                0 => "never".to_string(),
                t => format!("{} min ago", (now - i64::from(t)).max(0) / 60),
            };
            println!("  {:<24} type {}  heard {heard}", c.adv_name, c.contact_type);
        }
        anyhow::Ok(())
    }
    .await;
    // Drop the link even when a read failed, or a phone cannot reach the radio.
    let _ = radio.disconnect().await;
    radio::ble_disconnect(name).await?;
    read
}
