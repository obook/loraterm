//! app/update.rs
//! apply(): the radio's updates (radio::FromRadio) into the application state, keeping the
//! selection on its conversation and the map fitted to the nodes until the user moves it.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use super::App;
use crate::config::{FIT_PX, FIT_RADIUS_KM};
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
            let near = |p: &Pos| {
                self.self_pos
                    .is_none_or(|me| distance_km(me, *p) <= FIT_RADIUS_KM)
            };
            let points: Vec<Pos> = self
                .nodes
                .iter()
                .filter_map(|n| n.pos)
                .filter(near)
                .chain(self.self_pos)
                .collect();
            if !points.is_empty() {
                self.view = MapView::fit(&points, FIT_PX.0, FIT_PX.1);
            }
        }
    }
}
