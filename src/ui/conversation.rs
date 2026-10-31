//! ui/conversation.rs
//! The conversation panel: the selected thread (wrapped and scrollable) and the input line.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::format::{ascii_safe, hhmm};
use super::{faint, panel, strong, text, Theme, CYAN};
use crate::app::{App, Focus};
use crate::history::Msg;
use crate::radio::Link;

/// My handle in the thread: the node name inside the BLE name "MeshCore-<name>".
pub(super) fn my_handle(app: &App) -> String {
    match &app.link {
        Link::Connected(name) => name.strip_prefix("MeshCore-").unwrap_or(name).to_string(),
        _ => "me".into(),
    }
}

/// One message of the thread: time, sender and text, plus the delivery state of a direct
/// message I sent.
pub(super) fn thread_line<'a>(
    app: &App,
    msg: &'a Msg,
    me: &str,
    width: usize,
    theme: Theme,
) -> Line<'a> {
    let time = hhmm(msg.ts);
    let from = if msg.outgoing { me } else { &msg.from };
    let from = ascii_safe(from, app.ascii);
    let body = ascii_safe(&msg.text, app.ascii).into_owned();
    let mut line = match theme {
        Theme::Normal => Line::from(vec![
            Span::styled(format!("{time} "), faint(theme)),
            Span::styled(
                format!("{from}  "),
                Style::new()
                    .fg(if msg.outgoing { CYAN } else { Color::White })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(body),
        ]),
        Theme::Hacker => Line::from(vec![
            Span::styled(format!("[{time}] "), faint(theme)),
            Span::styled(format!("<{from}> "), strong(theme)),
            Span::styled(body, text(theme)),
        ]),
    };
    if let Some(status) = app.status(msg) {
        let received = status == "received";
        let tag = match theme {
            Theme::Normal => status,
            Theme::Hacker if received => "[ACK]",
            Theme::Hacker => "[TX]",
        };
        // Right-aligned when it fits on the row, else after the text (wrapped with it).
        let pad = width
            .checked_sub(line.width() + tag.len())
            .filter(|&p| p >= 2)
            .unwrap_or(2);
        line.push_span(Span::raw(" ".repeat(pad)));
        line.push_span(Span::styled(
            tag,
            if received && theme == Theme::Hacker {
                strong(theme)
            } else {
                faint(theme)
            },
        ));
    }
    line
}

/// The selected conversation (anchored at its latest message) and the input line.
pub(super) fn draw_conversation(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = app.rows();
    let public = if theme == Theme::Normal {
        "# Public"
    } else {
        "#PUBLIC"
    };
    let label = rows.get(app.selected).map_or(String::new(), |r| {
        if r.conv == app.public_conv() {
            public.into()
        } else {
            ascii_safe(&r.label, app.ascii).into_owned()
        }
    });
    let focused = app.focus == Focus::Input;
    let title = if theme == Theme::Normal {
        label
    } else {
        format!("COMMS // {label}")
    };
    let block = panel(theme, app.ascii, focused, &title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [thread_area, input_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    let width = usize::from(thread_area.width);

    let me = my_handle(app);
    let lines: Vec<Line> = app
        .thread()
        .into_iter()
        .map(|m| thread_line(app, m, &me, width, theme))
        .collect();
    let thread = Paragraph::new(lines)
        .style(text(theme))
        .wrap(Wrap { trim: false });
    let hidden_above = thread
        .line_count(thread_area.width)
        .saturating_sub(usize::from(thread_area.height));
    let top = hidden_above - app.scroll.min(hidden_above);
    frame.render_widget(
        thread.scroll((u16::try_from(top).unwrap_or(u16::MAX), 0)),
        thread_area,
    );

    // The end of a long input stays in view.
    let (prompt, cursor) = match theme {
        Theme::Normal => ("> ", "_"),
        Theme::Hacker => ("root@mesh:~$ ", if app.ascii { "_" } else { "█" }),
    };
    let room = usize::from(input_area.width).saturating_sub(prompt.len() + 1);
    let typed = app.input.chars().count();
    let shown: String = app.input.chars().skip(typed.saturating_sub(room)).collect();
    let mut input = vec![
        Span::styled(prompt, strong(theme)),
        Span::styled(shown, text(theme)),
    ];
    if focused {
        input.push(Span::styled(
            cursor,
            text(theme).add_modifier(Modifier::SLOW_BLINK),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(input)).style(text(theme)),
        input_area,
    );
}
