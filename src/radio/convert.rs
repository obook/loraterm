//! radio/convert.rs
//! Conversions from meshcore-rs types: contacts to nodes, received messages to history
//! records, self info to MyRadio, battery voltage to a charge percentage.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use std::collections::HashMap;

use meshcore_rs::events::{ChannelMessage, Contact, ContactMessage};
use meshcore_rs::parsing::hex_encode;

use super::{MyRadio, Node, NodeKind};
use crate::geo::pos_from_micro;
use crate::history::{Msg, PUBLIC};

/// `dev` is missing on firmwares that do not answer the device query.
pub fn my_radio(
    me: &meshcore_rs::events::SelfInfo,
    dev: Option<&meshcore_rs::events::DeviceInfoData>,
) -> MyRadio {
    MyRadio {
        prefix: hex_encode(&me.public_key[..6]),
        name: me.name.clone(),
        model: dev.and_then(|device| device.model.clone()),
        version: dev.and_then(|device| device.version.clone()),
        build: dev.and_then(|device| device.fw_build.clone()),
        freq_khz: me.radio_freq,
        bw_hz: me.radio_bw,
        sf: me.sf,
        cr: me.cr,
        tx_power: me.tx_power,
        max_tx_power: me.max_tx_power,
        // Firmware ADVERT_LOC_NONE (0): the advert carries no position.
        shares_position: me.adv_loc_policy != 0,
    }
}

/// Charge of a single-cell LiPo from its voltage, interpolated on a typical resting
/// discharge curve. meshcore-rs maps 3.00-3.93 V linearly, so a battery above 3.93 V
/// (still a third of its charge left to lose) always read 100 %.
pub fn lipo_percent(mv: u16) -> u8 {
    const CURVE: [(u16, u8); 21] = [
        (3270, 0),
        (3610, 5),
        (3690, 10),
        (3710, 15),
        (3730, 20),
        (3750, 25),
        (3770, 30),
        (3790, 35),
        (3800, 40),
        (3820, 45),
        (3840, 50),
        (3850, 55),
        (3870, 60),
        (3910, 65),
        (3950, 70),
        (3980, 75),
        (4020, 80),
        (4080, 85),
        (4110, 90),
        (4150, 95),
        (4200, 100),
    ];
    let i = CURVE
        .iter()
        .position(|&(v, _)| mv < v)
        .unwrap_or(CURVE.len());
    match i {
        0 => 0,
        i if i == CURVE.len() => 100,
        i => {
            let ((v0, p0), (v1, p1)) = (CURVE[i - 1], CURVE[i]);
            let frac = f32::from(mv - v0) / f32::from(v1 - v0);
            (f32::from(p0) + frac * f32::from(p1 - p0)).round() as u8
        }
    }
}

impl From<&Contact> for Node {
    fn from(contact: &Contact) -> Node {
        let kind = match contact.contact_type {
            2 => NodeKind::Repeater,
            3 => NodeKind::Room,
            4 => NodeKind::Sensor,
            _ => NodeKind::Chat,
        };
        Node {
            prefix: contact.prefix_hex(),
            name: contact.adv_name.clone(),
            kind,
            pos: pos_from_micro(contact.adv_lat, contact.adv_lon),
            key: contact.public_key_hex(),
            // The sender's advert clock can be off; my radio's update time is not.
            heard: if contact.last_modification_timestamp != 0 {
                contact.last_modification_timestamp
            } else {
                contact.last_advert
            },
            path_len: contact.path_len,
            path: contact.out_path.clone(),
        }
    }
}

/// Our clock, not the sender's: a node's clock can be far off.
pub(super) fn received_now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Channel text arrives as "<sender>: <text>". Channel 0 is the public one.
pub fn channel_msg(message: &ChannelMessage) -> Msg {
    let (from, text) = message
        .text
        .split_once(": ")
        .unwrap_or(("?", &message.text));
    let conv = if message.channel_idx == 0 {
        PUBLIC.to_string()
    } else {
        format!("channel:{}", message.channel_idx)
    };
    Msg {
        ts: received_now(),
        conv,
        from: from.into(),
        text: text.into(),
        outgoing: false,
        ack: None,
    }
}

/// `names` maps a contact prefix (hex) to its name.
pub fn contact_msg(message: &ContactMessage, names: &HashMap<String, String>) -> Msg {
    let prefix = hex_encode(&message.sender_prefix);
    let from = names
        .get(&prefix)
        .cloned()
        .unwrap_or_else(|| prefix.clone());
    Msg {
        ts: received_now(),
        conv: prefix,
        from,
        text: message.text.clone(),
        outgoing: false,
        ack: None,
    }
}
