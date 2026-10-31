//! ui/panels.rs
//! Header banner, contact list and key footer.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::format::{gauge, spinner, LOW_BATTERY};
use super::{alert, faint, panel, strong, text, Theme};
use crate::app::App;
use crate::radio::Link;

/// The banner: link state, battery and node count, or the current notice.
pub(super) fn draw_header(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let low = app.battery.is_some_and(|b| b.pct < LOW_BATTERY);
    let status = match (theme, &app.notice, &app.link) {
        (Theme::Normal, Some(notice), _) => Span::styled(notice.clone(), alert(theme)),
        (Theme::Hacker, Some(notice), _) => {
            Span::styled(format!(">> ERR :: {notice}"), alert(theme))
        }
        (Theme::Normal, None, Link::Connected(name)) => {
            let battery = app
                .battery
                .map(|b| format!(" - battery {} % {}", b.pct, gauge(b.pct, app.ascii)))
                .unwrap_or_default();
            let line = format!(
                "Connected to {name} ({}){battery} - {} contacts",
                app.firmware.name(),
                app.nodes.len()
            );
            Span::styled(
                line,
                if low {
                    alert(theme)
                } else {
                    Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
                },
            )
        }
        (Theme::Normal, None, Link::Searching(name)) => Span::styled(
            format!("Searching for {name}... (is a phone connected to it?)"),
            Style::new().fg(Color::Yellow),
        ),
        (Theme::Normal, None, Link::Lost(reason)) => Span::styled(
            format!("Link lost: {reason} - retrying"),
            Style::new().fg(Color::Yellow),
        ),
        (Theme::Hacker, None, Link::Connected(name)) => {
            let power = app
                .battery
                .map(|b| format!(" :: PWR {} {}%", gauge(b.pct, app.ascii), b.pct))
                .unwrap_or_default();
            Span::styled(
                format!(
                    ">> LINK ESTABLISHED :: {name} :: {}{power} :: {} NODES",
                    app.firmware.name().to_uppercase(),
                    app.nodes.len()
                ),
                if low { alert(theme) } else { strong(theme) },
            )
        }
        (Theme::Hacker, None, Link::Searching(name)) => Span::styled(
            format!(
                ">> SCANNING FOR {name} {} :: IS A PHONE CONNECTED TO IT?",
                spinner()
            ),
            strong(theme),
        ),
        (Theme::Hacker, None, Link::Lost(reason)) => Span::styled(
            format!(">> LINK LOST :: {reason} :: RETRYING"),
            strong(theme),
        ),
    };
    let line = match theme {
        Theme::Normal => Line::from(status),
        Theme::Hacker => Line::from(vec![
            status,
            Span::styled(
                if app.ascii { " _" } else { " █" },
                text(theme).add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
    };
    let block = match theme {
        Theme::Normal => panel(
            theme,
            app.ascii,
            false,
            "meshcore-m5  MeshCore over Bluetooth",
        ),
        Theme::Hacker => {
            let clock = chrono::Local::now().format(" %H:%M:%S ").to_string();
            panel(theme, app.ascii, false, "MESHCORE-M5")
                .title(Line::styled(clock, text(theme)).right_aligned())
        }
    };
    frame.render_widget(Paragraph::new(line).block(block), area);
}

/// The keys line at the bottom.
pub(super) fn draw_footer(frame: &mut Frame, area: Rect, theme: Theme, ascii: bool) {
    let spans: Vec<Span> = match theme {
        Theme::Normal => {
            let keys = [
                (if ascii { "up/down" } else { "↑↓" }, "move"),
                ("Tab", "switch"),
                ("Enter", "send"),
                ("+/-", "zoom"),
                ("arrows", "pan"),
                ("p", "ping"),
                ("PgUp/PgDn", "scroll"),
                ("q", "quit"),
            ];
            keys.iter()
                .flat_map(|(k, what)| {
                    [
                        Span::styled(
                            format!(" {k} "),
                            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("{what}  ")),
                    ]
                })
                .collect()
        }
        Theme::Hacker => {
            let keys = [
                ("TAB", "CYCLE"),
                ("ENTER", "TRANSMIT"),
                ("+/-", "ZOOM"),
                ("ARROWS", "PAN"),
                ("P", "PING"),
                ("PGUP/PGDN", "SCROLLBACK"),
                ("Q", "EXIT"),
            ];
            keys.iter()
                .flat_map(|(k, what)| {
                    [
                        Span::styled(format!(" [{k}]"), strong(theme)),
                        Span::styled(format!(" {what} "), faint(theme)),
                    ]
                })
                .collect()
        }
    };
    frame.render_widget(Paragraph::new(Line::from(spans)).style(text(theme)), area);
}
