//! ui/menu.rs
//! Start menu (Normal or Hacker look) and the Hacker boot log.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use std::time::Duration;

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use super::{faint, panel, strong, text, Theme, CYAN};
pub use crate::config::BOOT;

/// The fake boot log of the Hacker look, one line every 300 ms.
pub(super) const BOOT_LINES: [&str; 5] = [
    "[ OK ] Initializing LoRa mesh stack",
    "[ OK ] Loading contacts",
    "[ OK ] Tuning to channel #PUBLIC",
    "[ OK ] Bringing up BLE uplink (Nordic UART)",
    ">> READY :: 73",
];

/// What a key does in the start menu.
#[derive(Debug, PartialEq)]
pub enum Menu {
    Choose(Theme),
    Start(Theme),
    Quit,
}

/// What a key does in the start menu.
pub fn menu_key(selected: Theme, code: KeyCode) -> Menu {
    match code {
        KeyCode::Up | KeyCode::Char('k') => Menu::Choose(Theme::Normal),
        KeyCode::Down | KeyCode::Char('j') => Menu::Choose(Theme::Hacker),
        KeyCode::Enter => Menu::Start(selected),
        KeyCode::Char('q') | KeyCode::Esc => Menu::Quit,
        _ => Menu::Choose(selected),
    }
}

/// Start menu: pick the look.
pub fn draw_menu(frame: &mut Frame, selected: Theme, ascii: bool) {
    // The Linux console fonts have no arrows or triangles.
    let (pointer, arrows) = if ascii {
        (">", "up/down")
    } else {
        ("▶", "↑↓")
    };
    let [area] = Layout::horizontal([Constraint::Length(44)])
        .flex(Flex::Center)
        .areas(frame.area());
    let [area] = Layout::vertical([Constraint::Length(10)])
        .flex(Flex::Center)
        .areas(area);
    let choice = |theme: Theme, name: &str, what: &str| {
        let on = theme == selected;
        let style = if on {
            Style::new()
                .fg(Color::Black)
                .bg(CYAN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new()
        };
        Line::from(vec![
            Span::raw("   "),
            Span::styled(
                format!("{} {name:<8}", if on { pointer } else { " " }),
                style,
            ),
            Span::styled(format!(" {what}"), faint(Theme::Normal)),
        ])
    };
    let lines = vec![
        Line::raw(""),
        Line::raw("   Choose your interface"),
        Line::raw(""),
        choice(Theme::Normal, "Normal", "clean, light map"),
        choice(Theme::Hacker, "Hacker", "green terminal, radar"),
        Line::raw(""),
        Line::styled(
            format!("   {arrows} choose   Enter start   q quit"),
            faint(Theme::Normal),
        ),
    ];
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(panel(Theme::Normal, ascii, true, "meshcore-m5")),
        area,
    );
}

/// Hacker boot log, one line every 300 ms.
pub fn draw_boot(frame: &mut Frame, elapsed: Duration) {
    let theme = Theme::Hacker;
    let shown = (elapsed.as_millis() / 300) as usize + 1;
    let lines: Vec<Line> = BOOT_LINES
        .iter()
        .take(shown)
        .map(|l| {
            Line::styled(
                *l,
                if l.starts_with(">>") {
                    strong(theme)
                } else {
                    text(theme)
                },
            )
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .style(text(theme))
            .block(Block::new().style(text(theme))),
        frame.area(),
    );
}
