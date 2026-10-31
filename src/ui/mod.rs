//! ui/mod.rs
//! Drawing, in two looks: Normal (rounded boxes, cyan accent, like sccast) and Hacker
//! (phosphor green on black, radar map). This file holds the colors, the styles and the
//! screen layout; the other files draw one panel each.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

mod card;
mod contacts;
mod conversation;
mod format;
mod map_panel;
mod menu;
mod panels;
mod radar;
mod radio_card;
#[cfg(test)]
mod tests;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType};
use ratatui::Frame;

use crate::app::App;
use card::draw_card;
use contacts::draw_contacts;
use conversation::draw_conversation;
use map_panel::draw_map;
pub use map_panel::MapState;
pub use menu::{draw_boot, draw_menu, menu_key, Menu, BOOT};
use panels::{draw_footer, draw_header};

const GREEN: Color = Color::Rgb(0, 255, 65);
const DIM: Color = Color::Rgb(0, 140, 40);
const RED: Color = Color::Rgb(255, 60, 60);
const BLACK: Color = Color::Rgb(0, 0, 0);
const CYAN: Color = Color::Cyan;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Theme {
    Normal,
    Hacker,
}

fn text(theme: Theme) -> Style {
    match theme {
        Theme::Normal => Style::new(),
        Theme::Hacker => Style::new().fg(GREEN).bg(BLACK),
    }
}

fn faint(theme: Theme) -> Style {
    match theme {
        Theme::Normal => Style::new().fg(Color::DarkGray),
        Theme::Hacker => Style::new().fg(DIM).bg(BLACK),
    }
}

fn strong(theme: Theme) -> Style {
    match theme {
        Theme::Normal => Style::new().fg(CYAN).add_modifier(Modifier::BOLD),
        Theme::Hacker => text(theme).add_modifier(Modifier::BOLD),
    }
}

fn alert(theme: Theme) -> Style {
    match theme {
        Theme::Normal => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        Theme::Hacker => Style::new().fg(RED).bg(BLACK).add_modifier(Modifier::BOLD),
    }
}

/// Normal: rounded box, " title ", cyan when focused. Hacker: square box, "[ TITLE ]".
/// The Linux console (`ascii`) has no rounded corners: square boxes there.
fn panel(theme: Theme, ascii: bool, focused: bool, title: &str) -> Block<'static> {
    let corners = if ascii {
        BorderType::Plain
    } else {
        BorderType::Rounded
    };
    match theme {
        Theme::Normal => Block::bordered()
            .border_type(corners)
            .border_style(Style::new().fg(if focused { CYAN } else { Color::DarkGray }))
            .title(format!(" {title} ")),
        Theme::Hacker => Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(if focused { text(theme) } else { faint(theme) })
            .style(text(theme))
            .title(Span::styled(format!("[ {title} ]"), strong(theme))),
    }
}

pub fn draw(frame: &mut Frame, app: &App, map: &mut MapState, theme: Theme) {
    if theme == Theme::Hacker {
        frame.render_widget(Block::new().style(text(theme)), frame.area());
    }
    let [header, middle, conversation, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(8),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [contacts, map_area, card] = Layout::horizontal([
        Constraint::Length(32),
        Constraint::Min(0),
        Constraint::Length(32),
    ])
    .areas(middle);
    draw_header(frame, app, header, theme);
    draw_contacts(frame, app, contacts, theme);
    draw_map(frame, app, map_area, map, theme);
    draw_card(frame, app, card, theme);
    draw_conversation(frame, app, conversation, theme);
    draw_footer(frame, footer, theme, app.ascii);
}
