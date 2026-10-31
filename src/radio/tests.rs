//! radio/tests.rs
//! Tests of the radio conversions, of saving and of failure reports.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::convert::*;
use super::session::*;
use super::*;
use meshcore_rs::events::{ChannelMessage, Contact, ContactMessage};
use std::collections::HashMap;

fn channel(text: &str) -> ChannelMessage {
    ChannelMessage {
        channel_idx: 0,
        path_len: 0,
        txt_type: 0,
        sender_timestamp: 1_700_000_000,
        text: text.into(),
        snr: None,
    }
}

fn direct() -> ContactMessage {
    ContactMessage {
        sender_prefix: [0xa4, 0xcb, 0x8f, 0xcb, 0xd9, 0x41],
        path_len: 0,
        txt_type: 0,
        sender_timestamp: 1_700_000_000,
        text: "hi".into(),
        snr: None,
        signature: None,
    }
}

fn contact(contact_type: u8, lat: i32, lon: i32) -> Contact {
    let mut public_key = [0u8; 32];
    public_key[..6].copy_from_slice(&[0xa4, 0xcb, 0x8f, 0xcb, 0xd9, 0x41]);
    Contact {
        public_key,
        contact_type,
        flags: 0,
        path_len: 0,
        out_path: vec![],
        adv_name: "Relay".into(),
        last_advert: 0,
        adv_lat: lat,
        adv_lon: lon,
        last_modification_timestamp: 0,
    }
}

#[test]
fn channel_text_is_sender_then_text() {
    let m = channel_msg(&channel("Alice: Salut"));
    assert_eq!(
        (
            m.conv.as_str(),
            m.from.as_str(),
            m.text.as_str(),
            m.outgoing
        ),
        (PUBLIC, "Alice", "Salut", false)
    );
}

#[test]
fn channel_text_without_sender() {
    let m = channel_msg(&channel("no separator"));
    assert_eq!((m.from.as_str(), m.text.as_str()), ("?", "no separator"));
}

#[test]
fn other_channels_are_kept_apart() {
    let m = channel_msg(&ChannelMessage {
        channel_idx: 1,
        ..channel("Bob: yo")
    });
    assert_eq!(m.conv, "channel:1");
}

#[test]
fn direct_message_uses_known_name() {
    let names = HashMap::from([("a4cb8fcbd941".to_string(), "Bob".to_string())]);
    let m = contact_msg(&direct(), &names);
    assert_eq!(
        (m.conv.as_str(), m.from.as_str(), m.text.as_str()),
        ("a4cb8fcbd941", "Bob", "hi")
    );
}

#[test]
fn direct_message_from_unknown_sender_uses_prefix() {
    assert_eq!(contact_msg(&direct(), &HashMap::new()).from, "a4cb8fcbd941");
}

#[test]
fn save_reports_history_failure_and_still_shows_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    // A directory cannot be opened for append.
    let err = save(dir.path(), &tx, Record::Ack { tag: "01".into() }).unwrap_err();
    assert!(err.is::<HistoryWriteFailed>(), "{err:#}");
    assert!(matches!(
        rx.try_recv(),
        Ok(FromRadio::Record(Record::Ack { .. }))
    ));
}

#[test]
fn failed_send_names_its_conversation() {
    let direct = ToRadio::SendDirect {
        prefix: "a4cb8fcbd941".into(),
        text: "hi".into(),
    }
    .failed("timeout".into());
    assert!(matches!(
        direct,
        FromRadio::SendFailed { conv, text, error }
            if conv == "a4cb8fcbd941" && text == "hi" && error == "timeout"
    ));
    let public = ToRadio::SendPublic { text: "yo".into() }.failed("x".into());
    assert!(
        matches!(public, FromRadio::SendFailed { conv, text, .. } if conv == PUBLIC && text == "yo")
    );
    let ping = ToRadio::Ping {
        key: "k".into(),
        prefix: "abc".into(),
    }
    .failed("radio task stopped".into());
    assert!(
        matches!(ping, FromRadio::Pong { prefix, ping: Ping::Failed(_) } if prefix == "abc"),
        "a lost ping must not stay Waiting"
    );
}

#[test]
fn node_keeps_key_route_and_last_heard() {
    let c = Contact {
        path_len: 2,
        out_path: vec![0x3f, 0x12],
        last_advert: 100,
        last_modification_timestamp: 200,
        ..contact(2, 0, 0)
    };
    let n = Node::from(&c);
    assert_eq!(n.key, format!("a4cb8fcbd941{}", "0".repeat(52)));
    assert_eq!(
        (n.heard, n.path_len, n.path.clone()),
        (200, 2, vec![0x3f, 0x12])
    );
    let unsynced = Contact {
        last_modification_timestamp: 0,
        ..c
    };
    assert_eq!(
        Node::from(&unsynced).heard,
        100,
        "falls back on the advert time"
    );
}

#[test]
fn my_radio_from_self_and_device_info() {
    let mut public_key = [0u8; 32];
    public_key[..6].copy_from_slice(&[0xab, 0xcd, 0xef, 0x01, 0x23, 0x45]);
    let me = meshcore_rs::events::SelfInfo {
        name: "bbok".into(),
        public_key,
        radio_freq: 869_525,
        radio_bw: 250_000,
        sf: 11,
        cr: 5,
        tx_power: 22,
        max_tx_power: 22,
        adv_loc_policy: 1,
        ..Default::default()
    };
    let dev = meshcore_rs::events::DeviceInfoData {
        model: Some("ThinkNode M5".into()),
        version: Some("v1.18.0".into()),
        fw_build: Some("12-Oct-2026".into()),
        ..Default::default()
    };
    let r = my_radio(&me, Some(&dev));
    assert_eq!(
        (
            r.prefix.as_str(),
            r.name.as_str(),
            r.freq_khz,
            r.bw_hz,
            r.sf,
            r.cr
        ),
        ("abcdef012345", "bbok", 869_525, 250_000, 11, 5)
    );
    assert_eq!(
        (r.tx_power, r.max_tx_power, r.shares_position),
        (22, 22, true)
    );
    assert_eq!(
        (r.model.as_deref(), r.version.as_deref(), r.build.as_deref()),
        (Some("ThinkNode M5"), Some("v1.18.0"), Some("12-Oct-2026"))
    );
    let hidden = meshcore_rs::events::SelfInfo {
        adv_loc_policy: 0,
        ..me
    };
    assert!(!my_radio(&hidden, None).shares_position);
}

#[test]
fn battery_percent_follows_a_lipo_curve() {
    assert_eq!(lipo_percent(4200), 100);
    assert_eq!(lipo_percent(4300), 100, "on USB power");
    assert_eq!(lipo_percent(3840), 50);
    assert_eq!(lipo_percent(4000), 78, "meshcore-rs said 100");
    assert_eq!(lipo_percent(3930), 68);
    assert_eq!(lipo_percent(3270), 0);
    assert_eq!(lipo_percent(3000), 0);
}

#[test]
fn node_kind_and_missing_position() {
    let n = Node::from(&contact(2, 0, 0));
    assert_eq!(
        (n.prefix.as_str(), n.name.as_str(), n.kind, n.pos),
        ("a4cb8fcbd941", "Relay", NodeKind::Repeater, None)
    );
    assert_eq!(Node::from(&contact(3, 0, 0)).kind, NodeKind::Room);
    assert_eq!(Node::from(&contact(4, 0, 0)).kind, NodeKind::Sensor);
    assert_eq!(
        Node::from(&contact(1, 44_800_000, -600_000)).pos,
        Some(crate::geo::Pos {
            lat: 44.8,
            lon: -0.6
        })
    );
}

#[test]
fn classify_tells_the_firmwares_apart() {
    use super::link::{classify, MESHCORE_SERVICE, MESHTASTIC_SERVICE};
    assert_eq!(
        classify(Some("Meshtastic_d940"), &[MESHTASTIC_SERVICE]),
        Some(Firmware::Meshtastic)
    );
    assert_eq!(
        classify(Some("bbok_d940"), &[MESHTASTIC_SERVICE]),
        Some(Firmware::Meshtastic)
    );
    assert_eq!(
        classify(Some("MeshCore-bbok"), &[MESHCORE_SERVICE]),
        Some(Firmware::MeshCore)
    );
    assert_eq!(classify(Some("MeshCore-bbok"), &[]), Some(Firmware::MeshCore));
    assert_eq!(classify(Some("Nordic_UART"), &[MESHCORE_SERVICE]), None);
    assert_eq!(classify(None, &[]), None);
    // BlueZ keeps the services of the other firmware in its cache; the advertised name is
    // fresh, so it decides.
    assert_eq!(
        classify(Some("MeshCore-bbok"), &[MESHTASTIC_SERVICE]),
        Some(Firmware::MeshCore)
    );
}

#[test]
fn announce_sends_the_firmware_first() {
    let (tx, rx) = std::sync::mpsc::channel();
    announce(&tx, Firmware::Meshtastic, MyRadio::default(), None, "Meshtastic_d940");
    let got: Vec<FromRadio> = rx.try_iter().collect();
    assert!(matches!(got[0], FromRadio::Firmware(Firmware::Meshtastic)));
    assert!(matches!(got[1], FromRadio::MyRadio(_)));
    assert!(
        matches!(got.last(), Some(FromRadio::Link(Link::Connected(n))) if n == "Meshtastic_d940")
    );
}

#[test]
fn a_stale_bluetooth_cache_is_reported_when_nothing_else_connects() {
    use super::meshtastic::RepairNeeded;
    let repair = Some(RepairNeeded("Meshtastic_d940".into()).into());
    assert!(super::task::not_found(anyhow::anyhow!("timeout"), repair).is::<RepairNeeded>());
    assert!(!super::task::not_found(anyhow::anyhow!("timeout"), None).is::<RepairNeeded>());
}
