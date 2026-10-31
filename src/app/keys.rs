//! app/keys.rs
//! Keyboard handling: focus, selection, map moves, scrolling, sending and pinging. key()
//! returns the radio command (radio::ToRadio) a key asks for, if any.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{kind_label, App, Focus, Ping, MY_RADIO};
use crate::config::{MAX_TEXT_BYTES, SCROLL_STEP};
use crate::history::PUBLIC;
use crate::map::MapView;
use crate::radio::{Link, NodeKind, ToRadio};

impl App {
    /// Handle one key press; returns the radio command it asks for, if any.
    pub fn key(&mut self, key: KeyEvent) -> Option<ToRadio> {
        self.notice = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit |= key.code == KeyCode::Char('c');
            return None;
        }
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Contacts => Focus::Map,
                Focus::Map => Focus::Input,
                Focus::Input => Focus::Contacts,
            };
            return None;
        }
        match (self.focus, key.code) {
            (Focus::Input, KeyCode::Char(c))
                if self.input.len() + c.len_utf8() <= MAX_TEXT_BYTES =>
            {
                self.input.push(c)
            }
            (Focus::Input, KeyCode::Char(_)) => {} // input full
            (Focus::Input, KeyCode::Backspace) => {
                self.input.pop();
            }
            (Focus::Input, KeyCode::Enter) => return self.send(),
            // shortcut: bound assumes a message takes at most 4 rows (120 bytes, 40+ columns);
            // the drawing clamps the real offset anyway.
            (_, KeyCode::PageUp) => {
                self.scroll = (self.scroll + SCROLL_STEP).min(self.thread().len() * 4)
            }
            (_, KeyCode::PageDown) => self.scroll = self.scroll.saturating_sub(SCROLL_STEP),
            (_, KeyCode::Char('q')) => self.quit = true,
            (_, KeyCode::Char('p')) => return self.ping(),
            (Focus::Contacts, KeyCode::Up) => self.select(self.selected.saturating_sub(1)),
            (Focus::Contacts, KeyCode::Down) => self.select(self.selected + 1),
            (Focus::Map, KeyCode::Char('+' | '=')) => self.move_map(MapView::zoom_in),
            (Focus::Map, KeyCode::Char('-')) => self.move_map(MapView::zoom_out),
            (Focus::Map, KeyCode::Left) => self.move_map(|v| v.pan(-1, 0)),
            (Focus::Map, KeyCode::Right) => self.move_map(|v| v.pan(1, 0)),
            (Focus::Map, KeyCode::Up) => self.move_map(|v| v.pan(0, 1)),
            (Focus::Map, KeyCode::Down) => self.move_map(|v| v.pan(0, -1)),
            _ => {}
        }
        None
    }

    fn move_map(&mut self, change: impl FnOnce(&mut MapView)) {
        change(&mut self.view);
        self.map_moved = true;
    }

    fn select(&mut self, index: usize) {
        self.selected = index.min(self.rows().len() - 1);
        self.scroll = 0;
        let conv = self.current_conv();
        self.unread.remove(&conv);
        if conv == MY_RADIO {
            // My radio with its neighbours: centering on it alone could leave them all off-screen.
            if let Some(view) = self.nearby_view() {
                self.move_map(|v| *v = view);
            }
            return;
        }
        let pos = self
            .nodes
            .iter()
            .find(|n| n.prefix == conv)
            .and_then(|n| n.pos);
        if let Some(pos) = pos {
            self.move_map(|v| v.center_on(pos));
        }
    }

    /// Status request to the selected repeater or room.
    fn ping(&mut self) -> Option<ToRadio> {
        if !matches!(self.link, Link::Connected(_)) {
            self.notice = Some("Not connected, ping not sent".into());
            return None;
        }
        let conv = self.current_conv();
        let Some(node) = self
            .nodes
            .iter()
            .find(|n| n.prefix == conv && n.kind != NodeKind::Chat)
        else {
            self.notice = Some("PING: repeaters and rooms only".into());
            return None;
        };
        let cmd = ToRadio::Ping {
            key: node.key.clone(),
            prefix: node.prefix.clone(),
        };
        self.pings.insert(conv, Ping::Waiting);
        Some(cmd)
    }

    fn send(&mut self) -> Option<ToRadio> {
        if self.input.is_empty() {
            return None;
        }
        if !matches!(self.link, Link::Connected(_)) {
            self.notice = Some("Not connected, message not sent".into());
            return None;
        }
        let conv = self.current_conv();
        if conv == MY_RADIO {
            self.notice = Some("That is your own radio".into());
            return None;
        }
        if conv.starts_with("channel:") {
            self.notice = Some("Messages to private channels are not supported".into());
            return None;
        }
        if let Some(n) = self
            .nodes
            .iter()
            .find(|n| n.prefix == conv && n.kind != NodeKind::Chat)
        {
            self.notice = Some(format!(
                "Messages to a {} are not supported",
                kind_label(n.kind)
            ));
            return None;
        }
        let text = std::mem::take(&mut self.input);
        self.scroll = 0;
        Some(if conv == PUBLIC {
            ToRadio::SendPublic { text }
        } else {
            ToRadio::SendDirect { prefix: conv, text }
        })
    }
}
