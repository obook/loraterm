//! radio/task.rs
//! The radio task: connects to a radio, runs a session, reconnects after a loss and stops
//! for good when the history cannot be written or the interface is gone.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use super::link::{ble_disconnect, candidates, connect_to, Candidate};
use super::session::session;
use super::{FromRadio, Firmware, HistoryWriteFailed, Link, ToRadio, ANY_RADIO};
use crate::config::RETRY;

/// Run the radio in `rt`. It reconnects on its own and stops once the UI is gone;
/// the handle lets the caller wait for its disconnect.
pub fn spawn(
    rt: &tokio::runtime::Runtime,
    device: Option<String>,
    history: PathBuf,
    tx: Sender<FromRadio>,
) -> (UnboundedSender<ToRadio>, tokio::task::JoinHandle<()>) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let task = rt.spawn(run(device, history, tx, cmd_rx));
    (cmd_tx, task)
}

pub(super) async fn run(
    device: Option<String>,
    history: PathBuf,
    tx: Sender<FromRadio>,
    mut cmds: UnboundedReceiver<ToRadio>,
) {
    let label = device.clone().unwrap_or_else(|| ANY_RADIO.into());
    loop {
        if tx
            .send(FromRadio::Link(Link::Searching(label.clone())))
            .is_err()
        {
            return;
        }
        // Each candidate is announced first: the UI shows it, and a quit during the connect
        // knows which radio to drop.
        let mut found = Err(anyhow::anyhow!("no radio found"));
        let list = candidates(device.as_deref()).await.unwrap_or_else(|e| {
            tracing::info!("BLE scan failed: {e:#}");
            vec![]
        });
        for Candidate { name, firmware } in list {
            if firmware != Firmware::MeshCore {
                tracing::info!("{} radio {name}: not supported yet", firmware.name());
                continue;
            }
            if tx
                .send(FromRadio::Link(Link::Searching(name.clone())))
                .is_err()
            {
                return;
            }
            found = connect_to(&name).await.map(|radio| (radio, name));
            if found.is_ok() {
                break;
            }
        }
        let reason = match found {
            Ok((radio, name)) => {
                let end = session(&radio, &name, &history, &tx, &mut cmds).await;
                let _ = radio.disconnect().await;
                if let Err(e) = ble_disconnect(&name).await {
                    tracing::warn!("BLE disconnect failed: {e:#}");
                }
                match end {
                    Ok(true) => "link lost".to_string(),
                    Ok(false) => return, // UI gone
                    Err(e) if e.is::<HistoryWriteFailed>() => {
                        // Stop for good: every new fetch would lose a message.
                        tracing::error!("radio: {e:#}");
                        let _ = tx.send(FromRadio::Link(Link::Lost(format!(
                            "{e:#} - fix it, then restart"
                        ))));
                        return;
                    }
                    Err(e) => format!("{e:#}"),
                }
            }
            Err(e) => {
                // Most often the radio is busy with a phone: keep the "Searching" banner
                // and its hint.
                tracing::info!("radio not reached: {e:#}");
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        tracing::warn!("radio: {reason}");
        if tx.send(FromRadio::Link(Link::Lost(reason))).is_err() {
            return;
        }
        tokio::time::sleep(RETRY).await;
    }
}
