//! radio/ping.rs
//! Ping of a repeater or room: a guest login, then a status request, in a task of its own.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use meshcore_rs::commands::{CommandHandler, Destination};
use meshcore_rs::events::{EventDispatcher, EventPayload, EventType, MeshCoreEvent};
use meshcore_rs::parsing::hex_encode;
use meshcore_rs::{BinaryReqType, MeshCore};
use tokio::sync::Mutex;

use super::{FromRadio, Ping};
use crate::config::PING_TIMEOUT;

/// Ping a repeater or room in a task of its own, so the session keeps handling messages
/// meanwhile. The result reaches the interface as FromRadio::Pong.
pub(super) fn ping(radio: &MeshCore, key: &str, prefix: &str, tx: &Sender<FromRadio>) {
    let commands = radio.commands().clone();
    let dispatcher = radio.dispatcher().clone();
    let (key, prefix, tx) = (key.to_string(), prefix.to_string(), tx.clone());
    tokio::spawn(async move {
        let ping = match login_then_status(commands, dispatcher, key).await {
            Ok(pong) => pong,
            Err(e) => {
                tracing::info!("ping {prefix}: {e:#}");
                Ping::Failed(format!("{e:#}"))
            }
        };
        let _ = tx.send(FromRadio::Pong { prefix, ping });
    });
}

/// A repeater only answers clients in its access list, and a login with an empty password
/// joins it as a guest unless the owner set a guest password. Each answer is awaited at
/// most PING_TIMEOUT.
async fn login_then_status(
    commands: Arc<Mutex<CommandHandler>>,
    dispatcher: Arc<EventDispatcher>,
    key: String,
) -> anyhow::Result<Ping> {
    commands
        .lock()
        .await
        .send_login(Destination::Hex(key.clone()), "")
        .await?;
    let no_filter = HashMap::new;
    let success =
        dispatcher.wait_for_event(Some(EventType::LoginSuccess), no_filter(), PING_TIMEOUT);
    let failure =
        dispatcher.wait_for_event(Some(EventType::LoginFailed), no_filter(), PING_TIMEOUT);
    // shortcut: login results name no node; fine with one ping at a time.
    tokio::select! {
        Some(_) = success => {}
        Some(_) = failure => anyhow::bail!("login refused"),
        else => anyhow::bail!("timeout"),
    }

    let start = Instant::now();
    let request = Destination::Hex(key);
    let sent = commands
        .lock()
        .await
        .send_binary_req(request, BinaryReqType::Status)
        .await?;
    let filters = HashMap::from([("tag".to_string(), hex_encode(&sent.expected_ack))]);
    let answer = dispatcher.wait_for_event(Some(EventType::StatusResponse), filters, PING_TIMEOUT);
    match answer.await {
        Some(MeshCoreEvent {
            payload: EventPayload::Status(status),
            ..
        }) => Ok(Ping::Pong {
            rtt_ms: start.elapsed().as_millis() as u64,
            snr: status.snr,
            battery_mv: status.battery_mv,
            uptime: status.uptime,
            noise: status.noise_floor,
        }),
        _ => anyhow::bail!("timeout"),
    }
}
