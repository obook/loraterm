//! ui/card.rs
//! The card on the right: everything known about the selected node, my radio or the public channel.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::format::{age, ascii_safe, coord, hhmm, kind_name, route, spinner, uptime};
use super::radio_card::radio_lines;
use super::{alert, faint, panel, strong, text, Theme};
use crate::app::{App, Ping, MY_RADIO};
use crate::geo::{bearing_deg, cardinal, distance_km, format_distance};
use crate::history::{Msg, PUBLIC};
use crate::radio::{Node, NodeKind};

/// "LABEL  value" row of the node card.
pub(super) fn field<'a>(theme: Theme, label: &'a str, value: String, style: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<7}"), faint(theme)),
        Span::styled(value, style),
    ])
}

/// "3 (LAST 22:39)" for a conversation.
pub(super) fn msgs_summary(app: &App, conv: &str) -> String {
    let msgs: Vec<&Msg> = app.msgs.iter().filter(|m| m.conv == conv).collect();
    match msgs.last() {
        Some(last) => format!("{} (LAST {})", msgs.len(), hhmm(last.ts)),
        None => "0".into(),
    }
}

/// Card lines for a contact: identity, distance, freshness, route, messages, then ping
/// results for a repeater or room.
pub(super) fn node_lines<'a>(app: &App, node: &Node, theme: Theme) -> Vec<Line<'a>> {
    let mut lines = vec![
        field(theme, "ID", node.prefix.clone(), text(theme)),
        field(theme, "TYPE", kind_name(node.kind).into(), text(theme)),
    ];
    match (app.self_pos, node.pos) {
        (Some(me), Some(p)) => {
            let b = bearing_deg(me, p);
            let dist = format_distance(distance_km(me, p)).replace(' ', "");
            lines.push(field(
                theme,
                "DIST",
                format!("{dist}  CAP {b:03.0} {}", cardinal(b)),
                text(theme),
            ));
        }
        _ => lines.push(field(theme, "DIST", "-".into(), faint(theme))),
    }
    lines.push(match node.pos {
        Some(p) => field(theme, "POS", coord(p.lat, p.lon), text(theme)),
        None => field(theme, "POS", "UNKNOWN".into(), faint(theme)),
    });
    if node.heard == 0 {
        lines.push(field(theme, "HEARD", "NEVER".into(), faint(theme)));
    } else {
        let secs = chrono::Utc::now().timestamp() - i64::from(node.heard);
        // Bright within the hour, plain within the day, faint beyond.
        let style = if secs < 3600 {
            strong(theme)
        } else if secs < 86_400 {
            text(theme)
        } else {
            faint(theme)
        };
        lines.push(field(theme, "HEARD", age(secs.max(0)), style));
    }
    lines.push(field(
        theme,
        "ROUTE",
        route(node.path_len, &node.path),
        text(theme),
    ));
    lines.push(field(
        theme,
        "MSGS",
        msgs_summary(app, &node.prefix),
        text(theme),
    ));
    if node.kind == NodeKind::Chat {
        let acked = app
            .msgs
            .iter()
            .rev()
            .find(|m| m.conv == node.prefix && app.status(m) == Some("received"));
        lines.push(field(
            theme,
            "ACK",
            acked.map_or("-".into(), |m| format!("LAST {}", hhmm(m.ts))),
            text(theme),
        ));
        return lines;
    }
    lines.push(Line::styled("-- PING ------------------", faint(theme)));
    match app.pings.get(&node.prefix) {
        None => lines.push(Line::styled("[P] PING", strong(theme))),
        Some(Ping::Waiting) => lines.push(Line::styled(
            format!("PINGING {}", spinner()),
            strong(theme),
        )),
        Some(Ping::Failed(reason)) => {
            lines.push(Line::styled(reason.to_uppercase(), alert(theme)));
            lines.push(Line::styled("[P] PING AGAIN", text(theme)));
        }
        Some(Ping::Pong {
            rtt_ms,
            snr,
            battery_mv,
            uptime: up,
            noise,
        }) => {
            let rtt = *rtt_ms as f64 / 1000.0;
            lines.push(field(
                theme,
                "PONG",
                format!("{rtt:.1} s  SNR {snr:.1} dB"),
                strong(theme),
            ));
            lines.push(field(
                theme,
                "BAT",
                format!("{:.2} V", f64::from(*battery_mv) / 1000.0),
                text(theme),
            ));
            lines.push(field(theme, "UP", uptime(*up), text(theme)));
            lines.push(field(theme, "NOISE", format!("{noise} dBm"), text(theme)));
            lines.push(Line::styled("[P] PING AGAIN", text(theme)));
        }
    }
    lines
}

/// Everything known about the selected node.
pub(super) fn draw_card(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let conv = app.current_conv();
    if let (MY_RADIO, Some(radio)) = (conv.as_str(), &app.me) {
        let title = match theme {
            Theme::Normal => format!("My radio: {}", ascii_safe(&radio.name, app.ascii)),
            Theme::Hacker => format!("RADIO :: {}", ascii_safe(&radio.name, app.ascii)),
        };
        frame.render_widget(
            Paragraph::new(radio_lines(app, radio, theme))
                .style(text(theme))
                .block(panel(theme, app.ascii, false, &title)),
            area,
        );
        return;
    }
    let node_title = |name: &str| match theme {
        Theme::Normal => format!("Node: {name}"),
        Theme::Hacker => format!("NODE :: {name}"),
    };
    let (title, lines) = if conv == PUBLIC {
        let title = if theme == Theme::Normal {
            "Public channel"
        } else {
            "CHANNEL #PUBLIC"
        };
        let lines = vec![
            field(theme, "NODES", app.nodes.len().to_string(), text(theme)),
            field(theme, "MSGS", msgs_summary(app, PUBLIC), text(theme)),
        ];
        (title.to_string(), lines)
    } else if let Some(node) = app.nodes.iter().find(|n| n.prefix == conv) {
        (
            node_title(&ascii_safe(&node.name, app.ascii)),
            node_lines(app, node, theme),
        )
    } else {
        let lines = vec![
            field(theme, "ID", conv.clone(), text(theme)),
            field(theme, "TYPE", "UNKNOWN".into(), faint(theme)),
            field(theme, "MSGS", msgs_summary(app, &conv), text(theme)),
        ];
        (node_title(&conv), lines)
    };
    frame.render_widget(
        Paragraph::new(lines)
            .style(text(theme))
            .block(panel(theme, app.ascii, false, &title)),
        area,
    );
}
