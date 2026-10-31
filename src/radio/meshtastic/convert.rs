//! radio/meshtastic/convert.rs
//! Meshtastic data (protobufs) to our types: nodes, messages, battery, my radio, acks and
//! traceroutes. Every key starts with "mt:", so the histories of the two firmwares stay apart.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::collections::HashMap;

use meshtastic::protobufs::config::device_config::Role;
use meshtastic::protobufs::config::lo_ra_config::{ModemPreset, RegionCode};
use meshtastic::protobufs::config::LoRaConfig;
use meshtastic::protobufs::{
    mesh_packet::PayloadVariant, routing, DeviceMetadata, DeviceMetrics, HardwareModel,
    MeshPacket, NodeInfo, RouteDiscovery, Routing,
};

use crate::geo::pos_from_e7;
use crate::history::{Msg, MT_PUBLIC};
use crate::radio::convert::received_now;
use crate::radio::{Battery, MyRadio, Node, NodeKind, Ping};

/// Conversation key of a node: "mt:" and its node id ("!" and 8 hex digits).
pub fn node_key(num: u32) -> String {
    format!("mt:!{num:08x}")
}

/// The node number in a key made by `node_key`.
pub fn node_num(key: &str) -> Option<u32> {
    u32::from_str_radix(key.strip_prefix("mt:!")?, 16).ok()
}

/// Ack tag of a packet we sent: "mt:" and its id in hex.
pub fn ack_tag(id: u32) -> String {
    format!("mt:{id:08x}")
}

/// "!0badcafe": how Meshtastic shows a node with no name.
fn node_id(num: u32) -> String {
    format!("!{num:08x}")
}

// Repeater and RouterClient are deprecated, but older nodes still report them.
#[allow(deprecated)]
pub fn kind(role: Role) -> NodeKind {
    match role {
        Role::Router | Role::RouterLate | Role::RouterClient | Role::Repeater => {
            NodeKind::Repeater
        }
        Role::Sensor => NodeKind::Sensor,
        _ => NodeKind::Chat,
    }
}

pub fn node(info: &NodeInfo) -> Node {
    let user = info.user.as_ref();
    let role = user.map_or(Role::Client, |u| {
        Role::try_from(u.role).unwrap_or(Role::Client)
    });
    Node {
        prefix: node_key(info.num),
        name: user
            .map(|u| u.long_name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| node_id(info.num)),
        kind: kind(role),
        pos: info
            .position
            .as_ref()
            .and_then(|p| pos_from_e7(p.latitude_i?, p.longitude_i?)),
        key: node_id(info.num),
        heard: info.last_heard,
        path_len: info.hops_away.map_or(-1, |h| h.min(i8::MAX as u32) as i8),
        path: vec![],
    }
}

/// The firmware's own percentage; 101 means external power.
pub fn battery(m: &DeviceMetrics) -> Option<Battery> {
    let level = m.battery_level?;
    Some(Battery {
        pct: level.min(100) as u8,
        mv: m.voltage.map_or(0, |v| (v * 1000.0).round() as u16),
        used_kb: None,
        total_kb: None,
    })
}

/// A text packet: to me, it is a direct message; else a channel message, channel 0 being
/// the public one. `names` maps node numbers to names.
pub fn text_msg(p: &MeshPacket, text: &str, my_num: u32, names: &HashMap<u32, String>) -> Msg {
    let conv = if p.to == my_num {
        node_key(p.from)
    } else if p.channel == 0 {
        MT_PUBLIC.to_string()
    } else {
        format!("mt:channel:{}", p.channel)
    };
    Msg {
        ts: received_now(),
        conv,
        from: names
            .get(&p.from)
            .cloned()
            .unwrap_or_else(|| node_id(p.from)),
        text: text.into(),
        outgoing: false,
        ack: None,
    }
}

/// `me` is my own node's entry in the node database.
pub fn my_radio(
    my_num: u32,
    me: Option<&NodeInfo>,
    meta: Option<&DeviceMetadata>,
    lora: Option<&LoRaConfig>,
) -> MyRadio {
    let own = lora.filter(|l| !l.use_preset);
    MyRadio {
        prefix: node_key(my_num),
        name: me
            .and_then(|m| m.user.as_ref())
            .map_or_else(|| node_id(my_num), |u| u.long_name.clone()),
        model: meta.and_then(|m| HardwareModel::try_from(m.hw_model).ok()).map(|h| h.as_str_name().into()),
        version: meta.map(|m| m.firmware_version.clone()),
        build: None,
        freq_khz: lora.map_or(0, |l| (l.override_frequency * 1000.0).round() as u32),
        bw_hz: own.map_or(0, |l| l.bandwidth * 1000),
        sf: own.map_or(0, |l| l.spread_factor as u8),
        cr: own.map_or(0, |l| l.coding_rate as u8),
        tx_power: lora.map_or(0, |l| l.tx_power.clamp(0, 255) as u8),
        max_tx_power: 0,
        shares_position: me.is_some_and(|m| m.position.is_some()),
        preset: lora.filter(|l| l.use_preset).map(|l| {
            let region = RegionCode::try_from(l.region).map_or("?", |r| r.as_str_name());
            let preset = ModemPreset::try_from(l.modem_preset).map_or("?", |p| p.as_str_name());
            format!("{region} {preset}")
        }),
    }
}

/// The ack tag a routing packet confirms: only from the node we sent to (a relay's
/// implicit ack does not count), and only without an error.
pub fn ack_for(p: &MeshPacket, routing: &Routing, sent: &HashMap<u32, u32>) -> Option<String> {
    let Some(PayloadVariant::Decoded(data)) = &p.payload_variant else {
        return None;
    };
    let ok = matches!(routing.variant, Some(routing::Variant::ErrorReason(e)) if e == routing::Error::None as i32);
    (ok && sent.get(&data.request_id) == Some(&p.from)).then(|| ack_tag(data.request_id))
}

/// A traceroute answer: hops on the way there, and the SNR of the last hop back (quarter
/// dB), or of the last hop there.
pub fn trace(route: &RouteDiscovery, rtt_ms: u64) -> Ping {
    let snr = route
        .snr_back
        .last()
        .or(route.snr_towards.last())
        .map(|&q| q as f32 / 4.0);
    Ping::Trace {
        rtt_ms,
        hops: route.route.len().min(u8::MAX as usize) as u8,
        snr,
    }
}
