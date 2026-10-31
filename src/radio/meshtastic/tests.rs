//! radio/meshtastic/tests.rs
//! Tests of the Meshtastic transport, conversions and handshake.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::transport::*;
use std::cell::Cell;

#[tokio::test]
async fn read_until_empty_stops_at_the_first_empty_read() {
    let chunks = [vec![1u8], vec![2, 3], vec![], vec![9]];
    let calls = Cell::new(0);
    let read = || {
        let i = calls.get();
        calls.set(i + 1);
        let chunk = chunks[i].clone();
        async move { anyhow::Ok(chunk) }
    };
    assert_eq!(read_until_empty(read).await.unwrap(), vec![vec![1], vec![2, 3]]);
    assert_eq!(calls.get(), 3);
}

#[tokio::test]
async fn read_until_empty_passes_errors_on() {
    let read = || async { Err::<Vec<u8>, _>(anyhow::anyhow!("link lost")) };
    assert!(read_until_empty(read).await.is_err());
}

#[test]
fn repair_needed_says_what_to_do() {
    let msg = RepairNeeded("Meshtastic_d940".into()).to_string();
    assert!(msg.contains("re-pair the radio with bluetoothctl"), "{msg}");
}

use super::convert::*;
use crate::history::MT_PUBLIC;
use crate::radio::{NodeKind, Ping};
use meshtastic::protobufs::config::device_config::Role;
use meshtastic::protobufs::config::lo_ra_config::{ModemPreset, RegionCode};
use meshtastic::protobufs::{self as pb, mesh_packet::PayloadVariant, routing};
use std::collections::HashMap;

#[test]
fn node_keys_round_trip() {
    assert_eq!(node_key(0x0badcafe), "mt:!0badcafe");
    assert_eq!(node_num("mt:!0badcafe"), Some(0x0badcafe));
    assert_eq!(node_num("a1b2c3d4e5f6"), None);
    assert_eq!(ack_tag(0x1234), "mt:00001234");
}

#[test]
#[allow(deprecated)]
fn roles_map_to_node_kinds() {
    for r in [Role::Router, Role::RouterLate, Role::RouterClient, Role::Repeater] {
        assert_eq!(kind(r), NodeKind::Repeater, "{r:?}");
    }
    assert_eq!(kind(Role::Sensor), NodeKind::Sensor);
    for r in [Role::Client, Role::ClientMute, Role::Tracker] {
        assert_eq!(kind(r), NodeKind::Chat, "{r:?}");
    }
}

fn bob() -> pb::NodeInfo {
    pb::NodeInfo {
        num: 0x0badcafe,
        user: Some(pb::User {
            long_name: "Bob".into(),
            short_name: "BOB".into(),
            ..Default::default()
        }),
        position: Some(pb::Position {
            latitude_i: Some(448_266_630),
            longitude_i: Some(-5_571_380),
            ..Default::default()
        }),
        last_heard: 1_793_451_000,
        hops_away: Some(2),
        ..Default::default()
    }
}

#[test]
fn node_from_node_info() {
    let n = node(&bob());
    assert_eq!(n.prefix, "mt:!0badcafe");
    assert_eq!(n.name, "Bob");
    let pos = n.pos.unwrap();
    assert!((pos.lat - 44.826663).abs() < 1e-7 && (pos.lon + 0.557138).abs() < 1e-7, "{pos:?}");
    assert_eq!(n.heard, 1_793_451_000);
    assert_eq!(n.path_len, 2);
    assert!(n.path.is_empty());
    let anon = node(&pb::NodeInfo {
        user: None,
        hops_away: None,
        ..bob()
    });
    assert_eq!(anon.name, "!0badcafe");
    assert_eq!(anon.path_len, -1);
}

#[test]
fn battery_level_101_is_full() {
    let b = battery(&pb::DeviceMetrics {
        battery_level: Some(101),
        voltage: Some(4.2),
        ..Default::default()
    })
    .unwrap();
    assert_eq!((b.pct, b.mv), (100, 4200));
    assert!(battery(&pb::DeviceMetrics::default()).is_none());
}

fn packet(from: u32, to: u32, channel: u32) -> pb::MeshPacket {
    pb::MeshPacket {
        from,
        to,
        channel,
        ..Default::default()
    }
}

#[test]
fn text_goes_to_the_right_conversation() {
    let names = HashMap::from([(0x0badcafe, "Bob".to_string())]);
    let me = 5;
    let dm = text_msg(&packet(0x0badcafe, me, 0), "hi", me, &names);
    assert_eq!(
        (dm.conv.as_str(), dm.from.as_str(), dm.text.as_str()),
        ("mt:!0badcafe", "Bob", "hi")
    );
    assert!(!dm.outgoing && dm.ack.is_none());
    let public = text_msg(&packet(0x0badcafe, 0xffff_ffff, 0), "hi", me, &names);
    assert_eq!(public.conv, MT_PUBLIC);
    let other = text_msg(&packet(0x0000beef, 0xffff_ffff, 2), "hi", me, &names);
    assert_eq!(
        (other.conv.as_str(), other.from.as_str()),
        ("mt:channel:2", "!0000beef")
    );
}

#[test]
fn my_radio_shows_the_preset() {
    let lora = pb::config::LoRaConfig {
        use_preset: true,
        region: RegionCode::Eu868 as i32,
        modem_preset: ModemPreset::LongFast as i32,
        tx_power: 27,
        ..Default::default()
    };
    let r = my_radio(5, Some(&bob()), None, Some(&lora));
    assert_eq!(r.preset.as_deref(), Some("EU_868 LONG_FAST"));
    assert_eq!(r.freq_khz, 0);
    assert_eq!((r.prefix.as_str(), r.name.as_str()), ("mt:!00000005", "Bob"));
    assert_eq!(r.tx_power, 27);
    assert!(r.shares_position);
}

fn routing_from(from: u32, request_id: u32, error: routing::Error) -> (pb::MeshPacket, pb::Routing) {
    let p = pb::MeshPacket {
        from,
        payload_variant: Some(PayloadVariant::Decoded(pb::Data {
            portnum: pb::PortNum::RoutingApp as i32,
            request_id,
            ..Default::default()
        })),
        ..Default::default()
    };
    let r = pb::Routing {
        variant: Some(routing::Variant::ErrorReason(error as i32)),
    };
    (p, r)
}

#[test]
fn only_the_destination_acks() {
    let sent = HashMap::from([(7, 0x0badcafe)]);
    let ack = |from, id, err| {
        let (p, r) = routing_from(from, id, err);
        ack_for(&p, &r, &sent)
    };
    assert_eq!(
        ack(0x0badcafe, 7, routing::Error::None).as_deref(),
        Some("mt:00000007")
    );
    assert_eq!(ack(0x0000beef, 7, routing::Error::None), None, "a relay");
    assert_eq!(ack(0x0badcafe, 7, routing::Error::MaxRetransmit), None);
    assert_eq!(ack(0x0badcafe, 8, routing::Error::None), None, "not ours");
}

#[test]
fn trace_counts_hops() {
    let route = pb::RouteDiscovery {
        route: vec![1, 2],
        snr_back: vec![22, 18],
        ..Default::default()
    };
    assert_eq!(
        trace(&route, 1500),
        Ping::Trace {
            rtt_ms: 1500,
            hops: 2,
            snr: Some(4.5)
        }
    );
}
