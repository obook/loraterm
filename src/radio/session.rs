//! radio/session.rs
//! One connection to the radio: start-up, then a loop on radio events, interface commands
//! and a periodic check. Every received message is written to disk before it is shown.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;

use futures::StreamExt;
use meshcore_rs::commands::Destination;
use meshcore_rs::events::{EventPayload, EventType};
use meshcore_rs::parsing::hex_encode;
use meshcore_rs::MeshCore;
use tokio::sync::mpsc::UnboundedReceiver;

use super::convert::{channel_msg, contact_msg, lipo_percent, my_radio};
use super::link::now;
use super::ping::ping;
use super::{Battery, FromRadio, HistoryWriteFailed, Link, Node, ToRadio};
use crate::config::{CHECK_EVERY, CONTACTS_TIMEOUT};
use crate::geo::pos_from_micro;
use crate::history::{self, Msg, Record, PUBLIC};

/// One connection. Ok(true): the link dropped; Ok(false): the UI is gone.
pub(super) async fn session(
    radio: &MeshCore,
    name: &str,
    history: &Path,
    tx: &Sender<FromRadio>,
    cmds: &mut UnboundedReceiver<ToRadio>,
) -> anyhow::Result<bool> {
    // Subscribe first, so no push is missed while starting.
    let mut events = radio.event_stream();
    let me = radio.commands().lock().await.send_appstart().await?;
    let _ = tx.send(FromRadio::SelfPos(pos_from_micro(me.adv_lat, me.adv_lon)));
    let dev = radio.commands().lock().await.send_device_query().await;
    if let Err(e) = &dev {
        tracing::info!("device query: {e}");
    }
    let _ = tx.send(FromRadio::MyRadio(my_radio(&me, dev.as_ref().ok())));
    let _ = tx.send(FromRadio::Link(Link::Connected(name.to_string())));
    // The firmware refuses to move its clock back (it answers an error, which meshcore-rs only
    // sees as a timeout); a GPS radio is often a second ahead. A clock not set is no link failure.
    let device_time = radio.commands().lock().await.get_time().await?;
    if device_time < now() {
        if let Err(e) = radio.commands().lock().await.set_time(now()).await {
            tracing::info!("radio clock not set: {e}");
        }
    }
    battery(radio, tx).await?;
    let mut names = contacts(radio, tx).await?;
    drain(radio, history, tx, &names).await?;

    let mut tick = tokio::time::interval(CHECK_EVERY);
    tick.tick().await;
    // Reloading all contacts takes 60 ms per contact: once per check, not once per advert.
    let mut contacts_stale = false;
    loop {
        tokio::select! {
            event = events.next() => {
                let Some(event) = event else { return Ok(true) };
                match event.event_type {
                    EventType::MessagesWaiting => drain(radio, history, tx, &names).await?,
                    EventType::Advertisement | EventType::NewContact | EventType::PathUpdate => {
                        contacts_stale = true;
                    }
                    EventType::Ack => {
                        if let EventPayload::Ack { tag } = event.payload {
                            save(history, tx, Record::Ack { tag: hex_encode(&tag) })?;
                        }
                    }
                    EventType::Disconnected => return Ok(true),
                    _ => {}
                }
            }
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { return Ok(false) };
                if let ToRadio::Ping { key, prefix } = &cmd {
                    ping(radio, key, prefix, tx);
                    continue;
                }
                if let Err(e) = send(radio, history, tx, &cmd).await {
                    // Sent but not saved: the message is shown, do not invite a resend.
                    if e.is::<HistoryWriteFailed>() {
                        return Err(e);
                    }
                    let _ = tx.send(cmd.failed(format!("{e:#}")));
                }
            }
            _ = tick.tick() => {
                battery(radio, tx).await?;
                if std::mem::take(&mut contacts_stale) {
                    names = contacts(radio, tx).await?;
                }
                // A MessagesWaiting push lost while the loop was busy must not strand messages.
                drain(radio, history, tx, &names).await?;
            }
        }
    }
}

/// Read the battery; doubles as the link check, since a dead link makes it fail.
pub(super) async fn battery(radio: &MeshCore, tx: &Sender<FromRadio>) -> anyhow::Result<()> {
    let info = radio.commands().lock().await.get_bat().await?;
    let _ = tx.send(FromRadio::Battery(Battery {
        pct: lipo_percent(info.battery_mv),
        mv: info.battery_mv,
        used_kb: info.used_kb,
        total_kb: info.total_kb,
    }));
    Ok(())
}

/// Send the contact list to the UI; return prefix -> name.
pub(super) async fn contacts(
    radio: &MeshCore,
    tx: &Sender<FromRadio>,
) -> anyhow::Result<HashMap<String, String>> {
    let list = radio
        .commands()
        .lock()
        .await
        .get_contacts_with_timeout(0, CONTACTS_TIMEOUT)
        .await?;
    let nodes: Vec<Node> = list.iter().map(Node::from).collect();
    let names = nodes
        .iter()
        .map(|n| (n.prefix.clone(), n.name.clone()))
        .collect();
    let _ = tx.send(FromRadio::Nodes(nodes));
    Ok(names)
}

/// Fetch every waiting message. The radio forgets a message once fetched, so each one is
/// written to disk before the next fetch.
pub(super) async fn drain(
    radio: &MeshCore,
    history: &Path,
    tx: &Sender<FromRadio>,
    names: &HashMap<String, String>,
) -> anyhow::Result<()> {
    loop {
        let Some(event) = radio.commands().lock().await.get_msg().await? else {
            return Ok(());
        };
        let msg = match &event.payload {
            EventPayload::ContactMessage(m) => contact_msg(m, names),
            EventPayload::ChannelMessage(m) => channel_msg(m),
            _ => continue,
        };
        save(history, tx, Record::Msg(msg))?;
    }
}

/// Write then report. On a write failure the record is still shown, and the error stops
/// the session so no more messages leave the radio.
pub(super) fn save(history: &Path, tx: &Sender<FromRadio>, rec: Record) -> anyhow::Result<()> {
    let written = history::append(history, &rec);
    let _ = tx.send(FromRadio::Record(rec));
    Ok(written.map_err(HistoryWriteFailed)?)
}

/// Send a direct or public message, then save it as sent.
pub(super) async fn send(
    radio: &MeshCore,
    history: &Path,
    tx: &Sender<FromRadio>,
    cmd: &ToRadio,
) -> anyhow::Result<()> {
    let ts = chrono::Utc::now().timestamp();
    let msg = match cmd {
        ToRadio::SendDirect { prefix, text } => {
            let info = radio
                .commands()
                .lock()
                .await
                .send_msg(Destination::Hex(prefix.clone()), text, None)
                .await?;
            Msg {
                ts,
                conv: prefix.clone(),
                from: "me".into(),
                text: text.clone(),
                outgoing: true,
                ack: Some(hex_encode(&info.expected_ack)),
            }
        }
        ToRadio::SendPublic { text } => {
            radio
                .commands()
                .lock()
                .await
                .send_channel_msg(0, text, None)
                .await?;
            Msg {
                ts,
                conv: PUBLIC.into(),
                from: "me".into(),
                text: text.clone(),
                outgoing: true,
                ack: None,
            }
        }
        ToRadio::Ping { .. } => return Ok(()), // handled by ping()
    };
    save(history, tx, Record::Msg(msg))
}
