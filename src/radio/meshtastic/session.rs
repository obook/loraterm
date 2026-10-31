//! radio/meshtastic/session.rs
//! One connection to a Meshtastic radio: the config handshake, then a loop on the radio's
//! notifications, the interface's commands and a periodic check. Every received message is
//! written to disk before it is shown.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Instant;

use meshtastic::protobufs::{
    self as pb, admin_message, config, from_radio, mesh_packet::PayloadVariant, telemetry,
    to_radio, PortNum,
};
use prost::Message;
use tokio::sync::mpsc::UnboundedReceiver;

use super::convert::{ack_for, ack_tag, battery, my_radio, node, node_num, text_msg, trace};
use super::transport::Transport;
use crate::config::{CHECK_EVERY, CONTACTS_TIMEOUT, PING_TIMEOUT};
use crate::geo::pos_from_e7;
use crate::history::{Msg, Record, MT_PUBLIC};
use crate::radio::link::now;
use crate::radio::session::save;
use crate::radio::{Firmware, FromRadio, HistoryWriteFailed, Link, Ping, ToRadio};

/// Destination of a channel message.
const BROADCAST: u32 = 0xffff_ffff;

/// What the radio sends during the config handshake.
#[derive(Default)]
pub(in crate::radio) struct Snapshot {
    pub my_num: u32,
    pub nodes: HashMap<u32, pb::NodeInfo>,
    pub lora: Option<config::LoRaConfig>,
    pub meta: Option<pb::DeviceMetadata>,
    /// Packets queued in the radio: handled once the session starts, never dropped.
    pub pending: Vec<pb::MeshPacket>,
}

/// Take one message of the handshake; true once the radio has sent everything.
pub(in crate::radio) fn absorb(snap: &mut Snapshot, msg: pb::FromRadio, config_id: u32) -> bool {
    match msg.payload_variant {
        Some(from_radio::PayloadVariant::MyInfo(my)) => snap.my_num = my.my_node_num,
        Some(from_radio::PayloadVariant::NodeInfo(n)) => {
            snap.nodes.insert(n.num, n);
        }
        Some(from_radio::PayloadVariant::Config(pb::Config {
            payload_variant: Some(config::PayloadVariant::Lora(lora)),
        })) => snap.lora = Some(lora),
        Some(from_radio::PayloadVariant::Metadata(meta)) => snap.meta = Some(meta),
        Some(from_radio::PayloadVariant::Packet(p)) => snap.pending.push(p),
        Some(from_radio::PayloadVariant::ConfigCompleteId(id)) => return id == config_id,
        _ => {}
    }
    false
}

/// Ask for the config and the node database, and wait for all of it.
pub(in crate::radio) async fn handshake(
    t: &mut Transport,
    ids: &mut PacketIds,
) -> anyhow::Result<Snapshot> {
    let id = ids.next();
    t.send(&pb::ToRadio {
        payload_variant: Some(to_radio::PayloadVariant::WantConfigId(id)),
    })
    .await?;
    tracing::debug!("want_config {id} sent");
    let mut snap = Snapshot::default();
    let wait = async {
        loop {
            let mut done = false;
            // The whole batch: packets can follow the end of the config.
            let batch = t.drain().await?;
            tracing::debug!("handshake: {} messages", batch.len());
            for msg in batch {
                done |= absorb(&mut snap, msg, id);
            }
            if done {
                return anyhow::Ok(());
            }
            if !t.notified().await {
                anyhow::bail!("link lost during the config handshake");
            }
        }
    };
    tokio::time::timeout(CONTACTS_TIMEOUT, wait)
        .await
        .map_err(|_| anyhow::anyhow!("config handshake timed out"))??;
    Ok(snap)
}

/// Packet ids: a counter seeded from the clock, never 0.
pub(in crate::radio) struct PacketIds(u32);

impl PacketIds {
    pub fn new() -> PacketIds {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos());
        PacketIds(seed)
    }

    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(1).max(1);
        self.0
    }
}

/// A send that did not go out; a public one goes back to the Meshtastic channel.
pub(in crate::radio) fn failed(cmd: &ToRadio, error: String) -> FromRadio {
    match cmd {
        ToRadio::SendPublic { text } => FromRadio::SendFailed {
            conv: MT_PUBLIC.into(),
            text: text.clone(),
            error,
        },
        _ => cmd.failed(error),
    }
}

/// A packet for the mesh, on the primary channel.
fn packet(to: u32, id: u32, port: PortNum, payload: Vec<u8>, want_ack: bool) -> pb::ToRadio {
    pb::ToRadio {
        payload_variant: Some(to_radio::PayloadVariant::Packet(pb::MeshPacket {
            to,
            id,
            want_ack,
            payload_variant: Some(PayloadVariant::Decoded(pb::Data {
                portnum: port as i32,
                payload,
                // A traceroute needs an answer; nothing else asks for one.
                want_response: port == PortNum::TracerouteApp,
                ..Default::default()
            })),
            ..Default::default()
        })),
    }
}

/// What the session keeps between packets.
struct Live {
    my_num: u32,
    nodes: HashMap<u32, pb::NodeInfo>,
    /// Direct messages waiting for their ack: packet id -> destination.
    sent: HashMap<u32, u32>,
    /// Traceroutes waiting for their answer: packet id -> (node key, sent at).
    traces: HashMap<u32, (String, Instant)>,
    /// The node list changed since the UI last got it.
    nodes_stale: bool,
}

impl Live {
    fn names(&self) -> HashMap<u32, String> {
        self.nodes
            .iter()
            .filter_map(|(num, n)| Some((*num, n.user.as_ref()?.long_name.clone())))
            .filter(|(_, name)| !name.is_empty())
            .collect()
    }

    fn entry(&mut self, num: u32) -> &mut pb::NodeInfo {
        self.nodes.entry(num).or_insert_with(|| pb::NodeInfo {
            num,
            ..Default::default()
        })
    }

    fn send_nodes(&mut self, tx: &Sender<FromRadio>) {
        let list = self
            .nodes
            .values()
            .filter(|n| n.num != self.my_num)
            .map(node)
            .collect();
        let _ = tx.send(FromRadio::Nodes(list));
        self.nodes_stale = false;
    }

    fn take(
        &mut self,
        msg: pb::FromRadio,
        history: &Path,
        tx: &Sender<FromRadio>,
    ) -> anyhow::Result<()> {
        match msg.payload_variant {
            Some(from_radio::PayloadVariant::Packet(p)) => self.packet(p, history, tx)?,
            Some(from_radio::PayloadVariant::NodeInfo(n)) => {
                self.nodes.insert(n.num, n);
                self.nodes_stale = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn packet(
        &mut self,
        p: pb::MeshPacket,
        history: &Path,
        tx: &Sender<FromRadio>,
    ) -> anyhow::Result<()> {
        let Some(PayloadVariant::Decoded(data)) = &p.payload_variant else {
            return Ok(());
        };
        if p.from != self.my_num {
            self.entry(p.from).last_heard = now();
            self.nodes_stale = true;
        }
        match PortNum::try_from(data.portnum) {
            Ok(PortNum::TextMessageApp) if p.from != self.my_num => {
                let text = String::from_utf8_lossy(&data.payload);
                let msg = text_msg(&p, &text, self.my_num, &self.names());
                save(history, tx, Record::Msg(msg))?;
            }
            Ok(PortNum::NodeinfoApp) => {
                if let Ok(user) = pb::User::decode(data.payload.as_slice()) {
                    self.entry(p.from).user = Some(user);
                }
            }
            Ok(PortNum::PositionApp) => {
                if let Ok(pos) = pb::Position::decode(data.payload.as_slice()) {
                    self.entry(p.from).position = Some(pos);
                }
            }
            Ok(PortNum::TelemetryApp) => {
                if let Ok(pb::Telemetry {
                    variant: Some(telemetry::Variant::DeviceMetrics(m)),
                    ..
                }) = pb::Telemetry::decode(data.payload.as_slice())
                {
                    if p.from == self.my_num {
                        if let Some(b) = battery(&m) {
                            let _ = tx.send(FromRadio::Battery(b));
                        }
                    }
                    self.entry(p.from).device_metrics = Some(m);
                }
            }
            Ok(PortNum::RoutingApp) => {
                if let Ok(routing) = pb::Routing::decode(data.payload.as_slice()) {
                    if let Some(tag) = ack_for(&p, &routing, &self.sent) {
                        self.sent.remove(&data.request_id);
                        save(history, tx, Record::Ack { tag })?;
                    }
                }
            }
            Ok(PortNum::TracerouteApp) => {
                if let Some((prefix, at)) = self.traces.remove(&data.request_id) {
                    let ping = match pb::RouteDiscovery::decode(data.payload.as_slice()) {
                        Ok(route) => trace(&route, at.elapsed().as_millis() as u64),
                        Err(e) => Ping::Failed(format!("bad answer: {e}")),
                    };
                    let _ = tx.send(FromRadio::Pong { prefix, ping });
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn command(
        &mut self,
        t: &Transport,
        ids: &mut PacketIds,
        cmd: &ToRadio,
        history: &Path,
        tx: &Sender<FromRadio>,
    ) -> anyhow::Result<()> {
        let ts = chrono::Utc::now().timestamp();
        let outgoing = |conv: &str, text: &str, ack| {
            Record::Msg(Msg {
                ts,
                conv: conv.into(),
                from: "me".into(),
                text: text.into(),
                outgoing: true,
                ack,
            })
        };
        match cmd {
            ToRadio::SendDirect { prefix, text } => {
                let to = node_num(prefix).ok_or_else(|| anyhow::anyhow!("not a Meshtastic node"))?;
                let id = ids.next();
                let bytes = text.as_bytes().to_vec();
                t.send(&packet(to, id, PortNum::TextMessageApp, bytes, true)).await?;
                self.sent.insert(id, to);
                save(history, tx, outgoing(prefix, text, Some(ack_tag(id))))
            }
            ToRadio::SendPublic { text } => {
                let bytes = text.as_bytes().to_vec();
                t.send(&packet(BROADCAST, ids.next(), PortNum::TextMessageApp, bytes, false))
                    .await?;
                save(history, tx, outgoing(MT_PUBLIC, text, None))
            }
            ToRadio::Ping { prefix, .. } => {
                let to = node_num(prefix).ok_or_else(|| anyhow::anyhow!("not a Meshtastic node"))?;
                let id = ids.next();
                let route = pb::RouteDiscovery::default().encode_to_vec();
                t.send(&packet(to, id, PortNum::TracerouteApp, route, false)).await?;
                self.traces.insert(id, (prefix.clone(), Instant::now()));
                Ok(())
            }
        }
    }

    /// The periodic part: the node list when it changed, and traceroutes that got no answer.
    fn check(&mut self, tx: &Sender<FromRadio>) {
        if self.nodes_stale {
            self.send_nodes(tx);
        }
        let late: Vec<u32> = self
            .traces
            .iter()
            .filter(|(_, (_, at))| at.elapsed() > PING_TIMEOUT)
            .map(|(id, _)| *id)
            .collect();
        for id in late {
            if let Some((prefix, _)) = self.traces.remove(&id) {
                let ping = Ping::Failed("no answer".into());
                let _ = tx.send(FromRadio::Pong { prefix, ping });
            }
        }
    }
}

/// One connection. Ok(true): the link dropped; Ok(false): the UI is gone.
pub(in crate::radio) async fn session(
    t: &mut Transport,
    name: &str,
    history: &Path,
    tx: &Sender<FromRadio>,
    cmds: &mut UnboundedReceiver<ToRadio>,
) -> anyhow::Result<bool> {
    let mut ids = PacketIds::new();
    let snap = handshake(t, &mut ids).await?;
    let me = snap.nodes.get(&snap.my_num);
    let my_pos = me
        .and_then(|m| m.position.as_ref())
        .and_then(|p| pos_from_e7(p.latitude_i?, p.longitude_i?));
    let _ = tx.send(FromRadio::Firmware(Firmware::Meshtastic));
    let radio = my_radio(snap.my_num, me, snap.meta.as_ref(), snap.lora.as_ref());
    let _ = tx.send(FromRadio::MyRadio(radio));
    let _ = tx.send(FromRadio::SelfPos(my_pos));
    let _ = tx.send(FromRadio::Link(Link::Connected(name.to_string())));
    if let Some(b) = me.and_then(|m| m.device_metrics.as_ref()).and_then(battery) {
        let _ = tx.send(FromRadio::Battery(b));
    }
    // Meshtastic cannot tell its time: set it at each connection.
    let admin = pb::AdminMessage {
        payload_variant: Some(admin_message::PayloadVariant::SetTimeOnly(now())),
        ..Default::default()
    };
    let set_time = packet(snap.my_num, ids.next(), PortNum::AdminApp, admin.encode_to_vec(), false);
    if let Err(e) = t.send(&set_time).await {
        tracing::info!("radio clock not set: {e:#}");
    }
    let mut live = Live {
        my_num: snap.my_num,
        nodes: snap.nodes,
        sent: HashMap::new(),
        traces: HashMap::new(),
        nodes_stale: false,
    };
    live.send_nodes(tx);
    for p in snap.pending {
        live.packet(p, history, tx)?;
    }

    let mut tick = tokio::time::interval(CHECK_EVERY);
    tick.tick().await;
    loop {
        tokio::select! {
            alive = t.notified() => {
                if !alive {
                    return Ok(true);
                }
                for msg in t.drain().await? {
                    live.take(msg, history, tx)?;
                }
            }
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { return Ok(false) };
                if let Err(e) = live.command(t, &mut ids, &cmd, history, tx).await {
                    // Sent but not saved: the message is shown, do not invite a resend.
                    if e.is::<HistoryWriteFailed>() {
                        return Err(e);
                    }
                    let _ = tx.send(failed(&cmd, format!("{e:#}")));
                }
            }
            _ = tick.tick() => {
                // Also the link check: a write to a dead link fails.
                let beat = pb::ToRadio {
                    payload_variant: Some(to_radio::PayloadVariant::Heartbeat(pb::Heartbeat::default())),
                };
                if t.send(&beat).await.is_err() {
                    return Ok(true);
                }
                live.check(tx);
            }
        }
    }
}

/// --probe: the radio's state from the handshake, printed, then the link dropped.
pub(crate) async fn probe(name: &str) -> anyhow::Result<()> {
    let mut t = Transport::connect(name).await?;
    let snap = handshake(&mut t, &mut PacketIds::new()).await;
    t.disconnect().await;
    let snap = snap?;
    let me = snap.nodes.get(&snap.my_num);
    let radio = my_radio(snap.my_num, me, snap.meta.as_ref(), snap.lora.as_ref());
    println!("device   {}", radio.name);
    println!(
        "firmware Meshtastic {}",
        radio.version.as_deref().unwrap_or("?")
    );
    match me.and_then(|m| m.device_metrics.as_ref()).and_then(battery) {
        Some(b) => println!("battery  {} %  ({} mV)", b.pct, b.mv),
        None => println!("battery  unknown"),
    }
    let pos = me
        .and_then(|m| m.position.as_ref())
        .and_then(|p| pos_from_e7(p.latitude_i?, p.longitude_i?));
    match pos {
        Some(p) => println!("pos      {:.6}, {:.6}", p.lat, p.lon),
        None => println!("pos      none"),
    }
    println!("nodes    {}", snap.nodes.len().saturating_sub(1));
    Ok(())
}
