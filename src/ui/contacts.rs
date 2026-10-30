//! ui/contacts.rs
//! The list on the left: my radio, the public channel, then the contacts.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use super::format::{cells, node_id, short};
use super::{panel, strong, Theme, BLACK, GREEN};
use crate::app::{App, Focus, MY_RADIO};
use crate::history::PUBLIC;

/// The list on the left: my radio, the public channel, then the contacts.
pub(super) fn draw_contacts(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let items: Vec<ListItem> = app
        .rows()
        .into_iter()
        .map(|r| {
            let text = match theme {
                Theme::Normal => {
                    let unread = if r.unread > 0 {
                        r.unread.to_string()
                    } else {
                        String::new()
                    };
                    let label = match r.conv.as_str() {
                        PUBLIC => "# Public".to_string(),
                        MY_RADIO => format!("◆ {}", r.label),
                        _ => r.label.clone(),
                    };
                    format!("{} {:>9} {unread:>3}", cells(&label, 14), r.detail)
                }
                Theme::Hacker => {
                    let unread = if r.unread > 0 {
                        format!("*{}", r.unread)
                    } else {
                        String::new()
                    };
                    if r.conv == PUBLIC {
                        format!("#PUBLIC{:>21}", unread)
                    } else if r.conv == MY_RADIO {
                        let id = app
                            .me
                            .as_ref()
                            .map_or("", |me| &me.prefix[..6.min(me.prefix.len())]);
                        format!("{id:<6} {} {:>6}", cells(&r.label, 10), "LOCAL")
                    } else {
                        format!(
                            "{:<6} {} {:>6}{unread:>3}",
                            node_id(&r.conv),
                            cells(&r.label, 10),
                            short(&r.detail)
                        )
                    }
                }
            };
            ListItem::new(text)
        })
        .collect();
    let focused = app.focus == Focus::Contacts;
    let (title, symbol, highlight) = match theme {
        Theme::Normal => (
            format!("Contacts ({})", app.nodes.len()),
            "▶ ",
            if focused {
                Style::new()
                    .bg(Color::Rgb(0, 60, 80))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            },
        ),
        Theme::Hacker => (
            format!("NODES ({})", app.nodes.len()),
            ">",
            if focused {
                Style::new()
                    .fg(BLACK)
                    .bg(GREEN)
                    .add_modifier(Modifier::BOLD)
            } else {
                strong(theme)
            },
        ),
    };
    let list = List::new(items)
        .block(panel(theme, focused, &title))
        .highlight_symbol(symbol)
        .highlight_style(highlight);
    frame.render_stateful_widget(
        list,
        area,
        &mut ListState::default().with_selected(Some(app.selected)),
    );
}
