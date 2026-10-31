//! app/update.rs
//! apply(): the radio's updates (radio::FromRadio) into the application state, keeping the
//! selection on its conversation and the map fitted to the nodes until the user moves it.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::App;
use crate::config::FIT_RADIUS_KM;
use crate::geo::{distance_km, Pos};
use crate::history::Record;
use crate::map::MapView;
use crate::radio::{FromRadio, Link};

impl App {
    pub fn apply(&mut self, update: FromRadio) {
        // Keep the selection on the same conversation when rows move.
        let conv = self.current_conv();
        let mut keep = conv.clone();
        match update {
            FromRadio::Firmware(firmware) => {
                if firmware != self.firmware {
                    // The radio was reflashed: what we knew of it belongs to the other firmware.
                    self.firmware = firmware;
                    self.nodes.clear();
                    self.battery = None;
                    self.me = None;
                    self.pings.clear();
                    keep = self.public_conv().into();
                }
            }
            FromRadio::Link(link) => {
                self.connected_since = match link {
                    Link::Connected(_) => self.connected_since.or(Some(std::time::Instant::now())),
                    _ => None,
                };
                self.link = link;
            }
            FromRadio::MyRadio(me) => self.me = Some(me),
            FromRadio::SelfPos(pos) => self.self_pos = pos,
            FromRadio::Battery(b) => self.battery = Some(b),
            FromRadio::Nodes(nodes) => self.nodes = nodes,
            FromRadio::Record(Record::Ack { tag }) => {
                self.acked.insert(tag);
            }
            FromRadio::Record(Record::Msg(msg)) => {
                if !msg.outgoing && msg.conv != conv {
                    *self.unread.entry(msg.conv.clone()).or_default() += 1;
                }
                self.msgs.push(msg);
            }
            FromRadio::Pong { prefix, ping } => {
                self.pings.insert(prefix, ping);
            }
            FromRadio::SendFailed {
                conv: failed,
                text,
                error,
            } => {
                if self.input.is_empty() {
                    // Back to the conversation it was meant for, so Enter cannot send it elsewhere.
                    self.input = text;
                    keep = failed;
                    self.notice = Some(format!("Send failed: {error}"));
                } else {
                    let label = self
                        .rows()
                        .into_iter()
                        .find(|r| r.conv == failed)
                        .map_or(failed, |r| r.label);
                    self.notice = Some(format!("Send to {label} failed: {error} - text: {text}"));
                }
            }
        }
        self.selected = self.rows().iter().position(|r| r.conv == keep).unwrap_or(0);
        if !self.map_moved {
            if let Some(view) = self.nearby_view() {
                self.view = view;
            }
        }
    }

    /// The view on my radio and the nodes within FIT_RADIUS_KM of it (every located node
    /// when my position is unknown), fitted to the map's real size; None without positions.
    pub(super) fn nearby_view(&self) -> Option<MapView> {
        let near = |pos: &Pos| {
            self.self_pos
                .is_none_or(|me| distance_km(me, *pos) <= FIT_RADIUS_KM)
        };
        let points: Vec<Pos> = self
            .nodes
            .iter()
            .filter_map(|node| node.pos)
            .filter(near)
            .chain(self.self_pos)
            .collect();
        if points.is_empty() {
            return None;
        }
        Some(MapView::fit(&points, self.map_px.0, self.map_px.1))
    }

    /// The map panel's size in pixels, measured by the last drawing: the view is fitted to
    /// it until the user moves the map.
    pub fn set_map_px(&mut self, px: (u32, u32)) {
        if px == self.map_px {
            return;
        }
        self.map_px = px;
        if !self.map_moved {
            if let Some(view) = self.nearby_view() {
                self.view = view;
            }
        }
    }
}
