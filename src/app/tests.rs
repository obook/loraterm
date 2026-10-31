//! app/tests.rs
//! Tests of the application state and of the keys.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::*;
use crate::config::{MAX_TEXT_BYTES, SCROLL_STEP};
use crate::map::FOCUS_ZOOM;
use crate::radio::{FromRadio, ToRadio};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const ALICE: &str = "a4cb8fcbd941";

fn node(prefix: &str, name: &str, kind: NodeKind, pos: Option<Pos>) -> Node {
    Node {
        prefix: prefix.into(),
        name: name.into(),
        kind,
        pos,
        key: format!("{prefix}-key"),
        ..Default::default()
    }
}

fn incoming(conv: &str, text: &str) -> Record {
    Record::Msg(Msg {
        ts: 0,
        conv: conv.into(),
        from: "x".into(),
        text: text.into(),
        outgoing: false,
        ack: None,
    })
}

fn press(app: &mut App, code: KeyCode) -> Option<ToRadio> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, KeyCode::Char(c));
    }
}

/// Connected app with Alice and a room, focus on the input, public channel open.
fn connected() -> App {
    let mut app = App::new("MeshCore-bbok", vec![]);
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    app.apply(FromRadio::Nodes(vec![
        node(ALICE, "Alice", NodeKind::Chat, None),
        node("0102030405aa", "Salon", NodeKind::Room, None),
    ]));
    app.focus = Focus::Input;
    app
}

fn labels(app: &App) -> Vec<String> {
    app.rows().into_iter().map(|r| r.label).collect()
}

#[test]
fn public_first_then_nodes_by_name() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Nodes(vec![
        node("01", "bob", NodeKind::Chat, None),
        node("02", "Alice", NodeKind::Chat, None),
    ]));
    assert_eq!(labels(&app), ["# Public", "Alice", "bob"]);
}

#[test]
fn unknown_sender_gets_a_row() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Record(incoming(ALICE, "hi")));
    assert_eq!(labels(&app), ["# Public", ALICE]);
}

#[test]
fn history_is_loaded() {
    let app = App::new(
        "x",
        vec![
            incoming(PUBLIC, "old"),
            Record::Ack {
                tag: "01020304".into(),
            },
        ],
    );
    assert_eq!(app.thread().len(), 1);
    assert!(app.acked.contains("01020304"));
}

#[test]
fn distance_or_dash() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Nodes(vec![
        node(
            "01",
            "Alice",
            NodeKind::Chat,
            Some(Pos {
                lat: 44.8207,
                lon: -0.6,
            }),
        ),
        node(
            "02",
            "Relay",
            NodeKind::Repeater,
            Some(Pos {
                lat: 45.0,
                lon: -0.6,
            }),
        ),
    ]));
    assert_eq!(app.rows()[1].detail, "-");
    app.apply(FromRadio::SelfPos(Some(Pos {
        lat: 44.8,
        lon: -0.6,
    })));
    assert_eq!(app.rows()[1].detail, "2.3 km");
    assert_eq!(app.rows()[2].detail, "repeater");
}

#[test]
fn unread_counts_and_clears() {
    let mut app = connected();
    app.apply(FromRadio::Record(incoming(ALICE, "hi")));
    assert_eq!(app.rows()[1].unread, 1);
    app.apply(FromRadio::Record(incoming(PUBLIC, "seen")));
    assert_eq!(app.rows()[0].unread, 0);
    app.focus = Focus::Contacts;
    press(&mut app, KeyCode::Down);
    assert_eq!(app.current_conv(), ALICE);
    assert_eq!(app.rows()[1].unread, 0);
}

#[test]
fn ack_marks_received() {
    let mut app = connected();
    let sent = Msg {
        ts: 0,
        conv: ALICE.into(),
        from: "me".into(),
        text: "hi".into(),
        outgoing: true,
        ack: Some("01020304".into()),
    };
    assert_eq!(app.status(&sent), Some("sent"));
    app.apply(FromRadio::Record(Record::Ack {
        tag: "01020304".into(),
    }));
    assert_eq!(app.status(&sent), Some("received"));
    let public = Msg {
        conv: PUBLIC.into(),
        ack: None,
        ..sent
    };
    assert_eq!(app.status(&public), None);
}

#[test]
fn enter_sends_to_the_open_conversation() {
    let mut app = connected();
    type_text(&mut app, "hi");
    assert_eq!(
        press(&mut app, KeyCode::Enter),
        Some(ToRadio::SendPublic { text: "hi".into() })
    );
    assert_eq!(app.input, "");
    app.selected = 1;
    type_text(&mut app, "yo");
    assert_eq!(
        press(&mut app, KeyCode::Enter),
        Some(ToRadio::SendDirect {
            prefix: ALICE.into(),
            text: "yo".into()
        })
    );
}

#[test]
fn empty_input_sends_nothing() {
    let mut app = connected();
    assert_eq!(press(&mut app, KeyCode::Enter), None);
}

#[test]
fn enter_offline_keeps_text() {
    let mut app = connected();
    app.apply(FromRadio::Link(Link::Lost("link lost".into())));
    type_text(&mut app, "hi");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert_eq!(app.input, "hi");
    assert_eq!(
        app.notice.as_deref(),
        Some("Not connected, message not sent")
    );
}

fn failed(conv: &str, text: &str) -> FromRadio {
    FromRadio::SendFailed {
        conv: conv.into(),
        text: text.into(),
        error: "timeout".into(),
    }
}

#[test]
fn send_failure_restores_text() {
    let mut app = connected();
    type_text(&mut app, "hi");
    press(&mut app, KeyCode::Enter);
    app.apply(failed(PUBLIC, "hi"));
    assert_eq!(app.input, "hi");
    assert_eq!(app.notice.as_deref(), Some("Send failed: timeout"));
}

#[test]
fn first_of_two_sends_fails() {
    let mut app = connected();
    type_text(&mut app, "A");
    press(&mut app, KeyCode::Enter);
    type_text(&mut app, "B");
    press(&mut app, KeyCode::Enter);
    app.apply(failed(PUBLIC, "A"));
    let b = Msg {
        ts: 0,
        conv: PUBLIC.into(),
        from: "me".into(),
        text: "B".into(),
        outgoing: true,
        ack: None,
    };
    app.apply(FromRadio::Record(Record::Msg(b)));
    assert_eq!(app.input, "A");
}

#[test]
fn failure_goes_back_to_its_conversation() {
    let mut app = connected();
    app.selected = 1;
    type_text(&mut app, "for Alice");
    press(&mut app, KeyCode::Enter);
    app.selected = 0;
    app.apply(failed(ALICE, "for Alice"));
    assert_eq!(
        (app.current_conv().as_str(), app.input.as_str()),
        (ALICE, "for Alice")
    );
}

#[test]
fn failure_while_typing_keeps_both_texts() {
    let mut app = connected();
    type_text(&mut app, "draft");
    app.apply(failed(ALICE, "for Alice"));
    assert_eq!(app.input, "draft");
    assert_eq!(
        app.notice.as_deref(),
        Some("Send to Alice failed: timeout - text: for Alice")
    );
}

#[test]
fn no_send_to_a_room() {
    let mut app = connected();
    app.selected = 2;
    assert_eq!(app.current_conv(), "0102030405aa");
    type_text(&mut app, "hi");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert_eq!(app.input, "hi");
    assert_eq!(
        app.notice.as_deref(),
        Some("Messages to a room are not supported")
    );
}

#[test]
fn no_send_to_a_private_channel() {
    let mut app = connected();
    app.apply(FromRadio::Record(incoming("channel:1", "Bob: yo")));
    app.selected = labels(&app).iter().position(|l| l == "channel:1").unwrap();
    type_text(&mut app, "hi");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert_eq!(
        app.notice.as_deref(),
        Some("Messages to private channels are not supported")
    );
}

#[test]
fn input_capped_in_bytes() {
    let mut app = connected();
    type_text(&mut app, &"é".repeat(61));
    assert_eq!(app.input.len(), MAX_TEXT_BYTES);
    assert_eq!(app.input.chars().count(), 60);
}

#[test]
fn q_in_a_full_input_does_not_quit() {
    let mut app = connected();
    type_text(&mut app, &"a".repeat(MAX_TEXT_BYTES));
    press(&mut app, KeyCode::Char('q'));
    assert!(!app.quit);
}

#[test]
fn backspace_removes_a_char() {
    let mut app = connected();
    type_text(&mut app, "hé");
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.input, "h");
}

#[test]
fn selecting_a_located_node_centers_map() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Nodes(vec![node(
        "01",
        "Alice",
        NodeKind::Chat,
        Some(Pos {
            lat: 44.8,
            lon: -0.6,
        }),
    )]));
    press(&mut app, KeyCode::Down);
    assert_eq!(
        app.view,
        MapView {
            lat: 44.8,
            lon: -0.6,
            zoom: FOCUS_ZOOM
        }
    );
}

#[test]
fn tab_cycles_focus() {
    let mut app = App::new("x", vec![]);
    assert_eq!(app.focus, Focus::Contacts);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Map);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Input);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Contacts);
}

#[test]
fn map_keys_zoom_and_pan() {
    let mut app = App::new("x", vec![]);
    app.focus = Focus::Map;
    let start = app.view;
    press(&mut app, KeyCode::Char('+'));
    assert_eq!(app.view.zoom, start.zoom + 1);
    press(&mut app, KeyCode::Right);
    assert!(app.view.lon > start.lon);
    press(&mut app, KeyCode::Char('-'));
    assert_eq!(app.view.zoom, start.zoom);
}

#[test]
fn map_fits_located_nodes_until_the_user_moves_it() {
    let mut app = App::new("x", vec![]);
    let a = Pos {
        lat: 44.80,
        lon: -0.60,
    };
    let b = Pos {
        lat: 44.85,
        lon: -0.50,
    };
    app.apply(FromRadio::Nodes(vec![
        node("01", "A", NodeKind::Repeater, Some(a)),
        node("02", "B", NodeKind::Repeater, Some(b)),
    ]));
    assert!(app.view.zoom >= 10, "{:?}", app.view);
    assert!(
        app.view.lat > 44.8 && app.view.lat < 44.85,
        "{:?}",
        app.view
    );
    app.focus = Focus::Map;
    press(&mut app, KeyCode::Char('-'));
    let moved = app.view;
    app.apply(FromRadio::SelfPos(Some(Pos {
        lat: 48.85,
        lon: 2.35,
    })));
    assert_eq!(app.view, moved);
}

#[test]
fn q_types_in_input_and_quits_elsewhere() {
    let mut app = connected();
    press(&mut app, KeyCode::Char('q'));
    assert_eq!((app.input.as_str(), app.quit), ("q", false));
    app.focus = Focus::Contacts;
    press(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
    let mut app = connected();
    app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit);
}

#[test]
fn selection_follows_the_contact_when_list_changes() {
    let mut app = connected();
    app.selected = 1;
    app.apply(FromRadio::Nodes(vec![
        node("ffffffffffff", "Aaron", NodeKind::Chat, None),
        node(ALICE, "Alice", NodeKind::Chat, None),
    ]));
    assert_eq!(app.current_conv(), ALICE);
}

#[test]
fn page_keys_scroll_and_selecting_resets() {
    let mut app = App::new(
        "x",
        (0..10).map(|i| incoming(PUBLIC, &i.to_string())).collect(),
    );
    press(&mut app, KeyCode::PageUp);
    assert_eq!(app.scroll, SCROLL_STEP);
    press(&mut app, KeyCode::PageDown);
    press(&mut app, KeyCode::PageDown);
    assert_eq!(app.scroll, 0);
    press(&mut app, KeyCode::PageUp);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.scroll, 0);
}

#[test]
fn scroll_is_bounded_by_the_thread() {
    let mut app = App::new("x", vec![incoming(PUBLIC, "one")]);
    for _ in 0..100 {
        press(&mut app, KeyCode::PageUp);
    }
    assert!(app.scroll <= 4);
}

#[test]
fn map_fits_only_the_nodes_near_me() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::SelfPos(Some(Pos {
        lat: 44.80,
        lon: -0.60,
    })));
    app.apply(FromRadio::Nodes(vec![
        node(
            "01",
            "Near",
            NodeKind::Repeater,
            Some(Pos {
                lat: 44.89,
                lon: -0.60,
            }),
        ),
        node(
            "02",
            "Pyrenees",
            NodeKind::Repeater,
            Some(Pos {
                lat: 43.0,
                lon: -0.60,
            }),
        ),
    ]));
    assert!(app.view.zoom >= 10, "{:?}", app.view);
}

#[test]
fn p_pings_the_selected_room_or_repeater() {
    let mut app = connected();
    app.focus = Focus::Contacts;
    app.selected = 2;
    let prefix = "0102030405aa";
    assert_eq!(
        press(&mut app, KeyCode::Char('p')),
        Some(ToRadio::Ping {
            key: format!("{prefix}-key"),
            prefix: prefix.into()
        })
    );
    assert_eq!(app.pings.get(prefix), Some(&Ping::Waiting));
}

#[test]
fn p_on_a_person_explains() {
    let mut app = connected();
    app.focus = Focus::Map;
    app.selected = 1;
    assert_eq!(press(&mut app, KeyCode::Char('p')), None);
    assert_eq!(
        app.notice.as_deref(),
        Some("PING: repeaters and rooms only")
    );
}

#[test]
fn p_types_in_the_input() {
    let mut app = connected();
    press(&mut app, KeyCode::Char('p'));
    assert_eq!(app.input, "p");
}

#[test]
fn ping_offline_is_refused() {
    let mut app = connected();
    app.apply(FromRadio::Link(Link::Lost("x".into())));
    app.focus = Focus::Contacts;
    app.selected = 2;
    assert_eq!(press(&mut app, KeyCode::Char('p')), None);
    assert_eq!(app.notice.as_deref(), Some("Not connected, ping not sent"));
}

#[test]
fn ping_results_are_kept() {
    let mut app = connected();
    let pong = Ping::Pong {
        rtt_ms: 1800,
        snr: 6.2,
        battery_mv: 4100,
        uptime: 3 * 86400,
        noise: -112,
    };
    app.apply(FromRadio::Pong {
        prefix: "0102030405aa".into(),
        ping: pong.clone(),
    });
    assert_eq!(app.pings.get("0102030405aa"), Some(&pong));
}

fn my_radio() -> MyRadio {
    MyRadio {
        prefix: "abcdef012345".into(),
        name: "bbok".into(),
        ..Default::default()
    }
}

#[test]
fn my_radio_row_comes_first_and_keeps_the_selection() {
    let mut app = connected();
    app.apply(FromRadio::MyRadio(my_radio()));
    assert_eq!(labels(&app)[..2], ["bbok", "# Public"]);
    assert_eq!(app.current_conv(), PUBLIC);
}

#[test]
fn selecting_my_radio_centers_the_map_on_me() {
    let mut app = connected();
    let me = Pos {
        lat: 44.83,
        lon: -0.56,
    };
    app.apply(FromRadio::SelfPos(Some(me)));
    app.apply(FromRadio::MyRadio(my_radio()));
    app.focus = Focus::Contacts;
    press(&mut app, KeyCode::Up);
    assert_eq!(app.current_conv(), MY_RADIO);
    assert_eq!((app.view.lat, app.view.lon), (me.lat, me.lon));
}

#[test]
fn no_message_to_my_own_radio() {
    let mut app = connected();
    app.apply(FromRadio::MyRadio(my_radio()));
    app.selected = 0;
    type_text(&mut app, "hi");
    assert_eq!(press(&mut app, KeyCode::Enter), None);
    assert_eq!(app.notice.as_deref(), Some("That is your own radio"));
}

#[test]
fn battery_details_and_link_time_are_kept() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Battery(Battery {
        pct: 87,
        mv: 3980,
        used_kb: Some(12),
        total_kb: Some(1024),
    }));
    assert_eq!(app.battery.map(|b| (b.pct, b.mv)), Some((87, 3980)));
    assert!(app.connected_since.is_none());
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    assert!(app.connected_since.is_some());
    app.apply(FromRadio::Link(Link::Lost("x".into())));
    assert!(app.connected_since.is_none());
}

#[test]
fn link_lost_is_kept() {
    let mut app = connected();
    app.apply(FromRadio::Link(Link::Lost("x".into())));
    assert_eq!(app.link, Link::Lost("x".into()));
}

fn inside(view: MapView, pos: Pos, (w, h): (u32, u32)) -> bool {
    let (x, y) = view.screen_px(pos, w, h);
    (0.0..f64::from(w)).contains(&x) && (0.0..f64::from(h)).contains(&y)
}

#[test]
fn selecting_my_radio_fits_my_neighbours_in_the_real_map_size() {
    let mut app = connected();
    let me = Pos {
        lat: 44.80,
        lon: -0.60,
    };
    let relay = Pos {
        lat: 44.80,
        lon: -0.50,
    };
    app.apply(FromRadio::SelfPos(Some(me)));
    app.apply(FromRadio::Nodes(vec![node(
        "0a",
        "Relay",
        NodeKind::Repeater,
        Some(relay),
    )]));
    app.apply(FromRadio::MyRadio(MyRadio {
        name: "bbok".into(),
        ..Default::default()
    }));
    app.set_map_px((360, 300));
    app.focus = Focus::Map;
    press(&mut app, KeyCode::Char('+'));
    app.focus = Focus::Contacts;
    app.selected = 1;
    press(&mut app, KeyCode::Up);
    assert_eq!(app.current_conv(), MY_RADIO);
    assert!(
        inside(app.view, me, (360, 300)) && inside(app.view, relay, (360, 300)),
        "{:?}",
        app.view
    );
}

#[test]
fn a_new_map_size_refits_until_the_user_moves_the_map() {
    let mut app = App::new("x", vec![]);
    let a = Pos {
        lat: 44.80,
        lon: -0.60,
    };
    let b = Pos {
        lat: 44.90,
        lon: -0.40,
    };
    app.apply(FromRadio::Nodes(vec![
        node("01", "A", NodeKind::Repeater, Some(a)),
        node("02", "B", NodeKind::Repeater, Some(b)),
    ]));
    app.set_map_px((200, 150));
    assert!(
        inside(app.view, a, (200, 150)) && inside(app.view, b, (200, 150)),
        "{:?}",
        app.view
    );
    app.focus = Focus::Map;
    press(&mut app, KeyCode::Char('-'));
    let moved = app.view;
    app.set_map_px((900, 600));
    assert_eq!(app.view, moved);
}
