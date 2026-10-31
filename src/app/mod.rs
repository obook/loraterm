//! app/mod.rs
//! Application state, without any I/O: link, my radio, contacts, messages, pings, selection
//! and map view. apply() takes the radio's updates (radio::FromRadio) and the ui/ module draws
//! the state. keys.rs turns keys into state changes and radio commands.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

mod keys;
#[cfg(test)]
mod tests;
mod update;

use std::collections::{HashMap, HashSet};

use crate::config::FIT_PX;
use crate::geo::{distance_km, format_distance, Pos};
use crate::history::{Msg, Record, PUBLIC};
use crate::map::MapView;
pub use crate::radio::Ping;
use crate::radio::{Battery, Link, MyRadio, Node, NodeKind};

/// Conversation key of the row for my own radio.
pub const MY_RADIO: &str = "radio";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    Contacts,
    Map,
    Input,
}

pub struct Row {
    pub conv: String,
    pub label: String,
    pub detail: String,
    pub unread: usize,
}

pub struct App {
    pub link: Link,
    pub self_pos: Option<Pos>,
    pub battery: Option<Battery>,
    pub me: Option<MyRadio>,
    /// When the current link came up.
    pub connected_since: Option<std::time::Instant>,
    pub nodes: Vec<Node>,
    pub msgs: Vec<Msg>,
    pub acked: HashSet<String>,
    /// Last ping of each node, by prefix.
    pub pings: HashMap<String, Ping>,
    pub unread: HashMap<String, usize>,
    pub selected: usize,
    pub focus: Focus,
    pub input: String,
    /// Conversation rows hidden below the view; 0 shows the latest messages.
    pub scroll: usize,
    pub view: MapView,
    /// Size of the map panel in pixels, as last drawn (config::FIT_PX before the first frame).
    pub map_px: (u32, u32),
    pub notice: Option<String>,
    pub quit: bool,
    /// The terminal only shows ASCII, accented letters and line drawing (the Linux
    /// console): no images, no emoji, no symbols.
    pub ascii: bool,
    /// Once the user zooms, pans or picks a node, the map stops fitting the nodes.
    map_moved: bool,
}

fn kind_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Chat => "chat",
        NodeKind::Repeater => "repeater",
        NodeKind::Room => "room",
        NodeKind::Sensor => "sensor",
    }
}

impl App {
    pub fn new(device: &str, history: Vec<Record>) -> App {
        let mut app = App {
            link: Link::Searching(device.into()),
            self_pos: None,
            battery: None,
            me: None,
            connected_since: None,
            nodes: vec![],
            msgs: vec![],
            acked: HashSet::new(),
            pings: HashMap::new(),
            unread: HashMap::new(),
            selected: 0,
            focus: Focus::Contacts,
            input: String::new(),
            scroll: 0,
            view: MapView::default(),
            map_px: FIT_PX,
            notice: None,
            quit: false,
            ascii: false,
            map_moved: false,
        };
        for rec in history {
            match rec {
                Record::Msg(m) => app.msgs.push(m),
                Record::Ack { tag } => {
                    app.acked.insert(tag);
                }
            }
        }
        app
    }

    /// Public channel, then nodes by name, then conversations with no known node.
    pub fn rows(&self) -> Vec<Row> {
        let unread = |conv: &str| self.unread.get(conv).copied().unwrap_or(0);
        let mut rows: Vec<Row> = self
            .me
            .iter()
            .map(|me| Row {
                conv: MY_RADIO.into(),
                label: me.name.clone(),
                detail: String::new(),
                unread: 0,
            })
            .collect();
        rows.push(Row {
            conv: PUBLIC.into(),
            label: "# Public".into(),
            detail: String::new(),
            unread: unread(PUBLIC),
        });
        let mut nodes: Vec<&Node> = self.nodes.iter().collect();
        nodes.sort_by_key(|n| n.name.to_lowercase());
        for n in nodes {
            let detail = match (n.kind, self.self_pos, n.pos) {
                (NodeKind::Chat, Some(a), Some(b)) => format_distance(distance_km(a, b)),
                (NodeKind::Chat, _, _) => "-".into(),
                (kind, _, _) => kind_label(kind).into(),
            };
            rows.push(Row {
                conv: n.prefix.clone(),
                label: n.name.clone(),
                detail,
                unread: unread(&n.prefix),
            });
        }
        // A set, not a scan of the rows: the history grows for months.
        let mut listed: HashSet<String> = rows.iter().map(|r| r.conv.clone()).collect();
        for m in &self.msgs {
            if !listed.contains(&m.conv) {
                listed.insert(m.conv.clone());
                rows.push(Row {
                    conv: m.conv.clone(),
                    label: m.conv.clone(),
                    detail: String::new(),
                    unread: unread(&m.conv),
                });
            }
        }
        rows
    }

    pub fn current_conv(&self) -> String {
        self.rows()
            .into_iter()
            .nth(self.selected)
            .map_or_else(|| PUBLIC.into(), |r| r.conv)
    }

    pub fn thread(&self) -> Vec<&Msg> {
        let conv = self.current_conv();
        self.msgs.iter().filter(|m| m.conv == conv).collect()
    }

    /// "sent" or "received" for a direct message we sent; nothing otherwise.
    pub fn status(&self, m: &Msg) -> Option<&'static str> {
        let tag = m.ack.as_ref().filter(|_| m.outgoing)?;
        Some(if self.acked.contains(tag) {
            "received"
        } else {
            "sent"
        })
    }
}
