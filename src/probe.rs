//! probe.rs
//! --probe: connect to the radio, print its name, battery, position and contact count,
//! then drop the link. Checks the Bluetooth setup without the interface.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use std::time::Duration;

use crate::radio;

/// Connect to `device` (or the first MeshCore radio), print its state, then drop the link,
/// even when a read failed.
pub async fn probe(device: Option<&str>) -> anyhow::Result<()> {
    let (radio, name) = radio::connect(device).await?;
    let read = async {
        let cmd = radio.commands().lock().await;
        let me = cmd.send_appstart().await?;
        let battery = cmd.get_bat().await?;
        let contacts = cmd
            .get_contacts_with_timeout(0, Duration::from_secs(30))
            .await?;
        println!("device   {}", me.name);
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
        anyhow::Ok(())
    }
    .await;
    // Drop the link even when a read failed, or a phone cannot reach the radio.
    let _ = radio.disconnect().await;
    radio::ble_disconnect(&name).await?;
    read
}
