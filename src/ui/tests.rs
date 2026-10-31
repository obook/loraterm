//! ui/tests.rs
//! Tests of the drawing, on a test terminal.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: GPL-3.0

use super::format::*;
use super::menu::*;
use super::*;
use crate::history::{Msg, Record};
use crate::radio::{Battery, Firmware, FromRadio, Link, MyRadio, Node, NodeKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn render(draw_fn: impl FnOnce(&mut Frame)) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal.draw(draw_fn).unwrap();
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen_with(app: &App, theme: Theme) -> String {
    let dir = tempfile::tempdir().unwrap();
    let mut map = MapState::new(
        ratatui_image::picker::Picker::halfblocks(),
        crate::tiles::Tiles::offline(dir.path().into()),
    );
    render(|f| draw(f, app, &mut map, theme))
}

fn screen(app: &App) -> String {
    screen_with(app, Theme::Hacker)
}

fn normal(app: &App) -> String {
    screen_with(app, Theme::Normal)
}

fn nodes(n: usize) -> Vec<Node> {
    (0..n)
        .map(|i| Node {
            prefix: format!("{i:012x}"),
            name: format!("node{i}"),
            kind: NodeKind::Chat,
            pos: None,
            ..Default::default()
        })
        .collect()
}

fn public(text: &str) -> Record {
    Record::Msg(Msg {
        ts: 0,
        conv: "public".into(),
        from: "Bob".into(),
        text: text.into(),
        outgoing: false,
        ack: None,
    })
}

fn sent(text: &str) -> Msg {
    Msg {
        ts: 0,
        conv: "a4cb8fcbd941".into(),
        from: "me".into(),
        text: text.into(),
        outgoing: true,
        ack: Some("01020304".into()),
    }
}

#[test]
fn banner_shows_connected() {
    let mut app = App::new("MeshCore-bbok", vec![]);
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    app.apply(FromRadio::Battery(Battery {
        pct: 87,
        mv: 3980,
        used_kb: Some(12),
        total_kb: Some(1024),
    }));
    app.apply(FromRadio::Nodes(nodes(12)));
    assert!(screen(&app)
        .contains(">> LINK ESTABLISHED :: MeshCore-bbok :: MESHCORE :: PWR ▮▮▮▮▮▮▮▯ 87% :: 12 NODES"));
}

#[test]
fn banner_shows_searching() {
    let s = screen(&App::new("MeshCore-bbok", vec![]));
    assert!(
        s.contains(">> SCANNING FOR MeshCore-bbok") && s.contains("IS A PHONE CONNECTED TO IT?"),
        "{s}"
    );
}

#[test]
fn banner_shows_notice() {
    let mut app = App::new("MeshCore-bbok", vec![]);
    app.notice = Some("Not connected, message not sent".into());
    assert!(screen(&app).contains(">> ERR :: Not connected, message not sent"));
}

#[test]
fn thread_shows_ack_state() {
    let mut app = App::new("x", vec![Record::Msg(sent("hello"))]);
    app.selected = 1;
    assert!(screen(&app).contains("[TX]"));
    app.apply(FromRadio::Record(Record::Ack {
        tag: "01020304".into(),
    }));
    let s = screen(&app);
    assert!(s.contains("hello") && s.contains("[ACK]"), "{s}");
}

#[test]
fn lines_look_like_irc() {
    let s = screen(&App::new("x", vec![public("hi")]));
    assert!(s.contains("<Bob> hi"), "{s}");
}

#[test]
fn my_messages_carry_my_node_name() {
    let mut app = App::new("x", vec![Record::Msg(sent("hello"))]);
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    app.selected = 1;
    assert!(screen(&app).contains("<bbok> hello"));
}

#[test]
fn long_message_wraps_instead_of_being_cut() {
    let app = App::new("x", vec![public(&format!("{} END", "x".repeat(110)))]);
    assert!(screen(&app).contains("END"));
}

#[test]
fn older_messages_are_reachable_by_scrolling() {
    let mut app = App::new(
        "x",
        (1..=10).map(|i| public(&format!("msg{i:02}"))).collect(),
    );
    let s = screen(&app);
    assert!(s.contains("msg10") && !s.contains("msg01"), "{s}");
    app.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert!(screen(&app).contains("msg01"));
    app.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(screen(&app).contains("msg10"));
}

#[test]
fn status_of_a_long_message_stays_visible() {
    let long = Msg {
        ack: Some("01".into()),
        ..sent(&"y".repeat(110))
    };
    let mut app = App::new(
        "x",
        vec![Record::Msg(long), Record::Ack { tag: "01".into() }],
    );
    app.selected = 1;
    assert!(screen(&app).contains("[ACK]"));
}

#[test]
fn end_of_a_long_input_stays_visible() {
    let mut app = App::new("x", vec![]);
    app.input = format!("{}TAIL", "z".repeat(110));
    let s = screen(&app);
    assert!(s.contains("TAIL") && s.contains("root@mesh:~$"), "{s}");
}

#[test]
fn map_title_shows_the_radar_and_the_selected_node() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::SelfPos(Some(crate::geo::Pos {
        lat: 44.8,
        lon: -0.6,
    })));
    let alice = crate::geo::Pos {
        lat: 44.8207,
        lon: -0.6,
    };
    app.apply(FromRadio::Nodes(vec![Node {
        prefix: "01".into(),
        name: "Alice".into(),
        kind: NodeKind::Chat,
        pos: Some(alice),
        ..Default::default()
    }]));
    app.selected = 1;
    let s = screen(&app);
    assert!(
        s.contains("[ RADAR :: ") && s.contains("[ NODE :: Alice ]") && s.contains("2.3km"),
        "{s}"
    );
}

#[test]
fn coordinates_read_like_a_gps() {
    assert_eq!(coord(44.8267, -0.5571), "44.8267N 0.5571W");
    assert_eq!(coord(-33.9, 151.2), "33.9000S 151.2000E");
}

#[test]
fn map_credits_openstreetmap() {
    assert!(screen(&App::new("x", vec![])).contains("(c) OpenStreetMap contributors"));
}

#[test]
fn nodes_show_id_name_and_type() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Nodes(vec![Node {
        prefix: "a4cb8fcbd941".into(),
        name: "Relay-Bdx".into(),
        kind: NodeKind::Repeater,
        pos: None,
        ..Default::default()
    }]));
    let s = screen(&app);
    assert!(
        s.contains("#PUBLIC") && s.contains("a4cb8f Relay-Bdx") && s.contains("RPT"),
        "{s}"
    );
}

#[test]
fn a_name_with_an_emoji_keeps_the_columns_aligned() {
    let mut app = App::new("x", vec![]);
    let relay = |prefix: &str, name: &str| Node {
        prefix: prefix.into(),
        name: name.into(),
        kind: NodeKind::Repeater,
        pos: None,
        ..Default::default()
    };
    app.apply(FromRadio::Nodes(vec![
        relay("aaaaaaaaaaaa", "🐱 SHA-40-GMD"),
        relay("bbbbbbbbbbbb", "33CSTR-REP-NASH"),
    ]));
    let dir = tempfile::tempdir().unwrap();
    let mut map = MapState::new(
        ratatui_image::picker::Picker::halfblocks(),
        crate::tiles::Tiles::offline(dir.path().into()),
    );
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| draw(f, &app, &mut map, Theme::Hacker))
        .unwrap();
    let buf = terminal.backend().buffer();
    // Column of "RPT" on each row that has it, counted in terminal cells.
    let columns: Vec<u16> = (0..30)
        .filter_map(|y| {
            (0..30).find(|&x| {
                buf[(x, y)].symbol() == "R"
                    && buf[(x + 1, y)].symbol() == "P"
                    && buf[(x + 2, y)].symbol() == "T"
            })
        })
        .collect();
    assert_eq!(columns.len(), 2, "{columns:?}");
    assert_eq!(columns[0], columns[1]);
}

fn relay_app() -> App {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::SelfPos(Some(crate::geo::Pos {
        lat: 44.80,
        lon: -0.60,
    })));
    let heard = (chrono::Utc::now().timestamp() - 300) as u32;
    let relay = Node {
        prefix: "5545fc8a91d2".into(),
        name: "33EYSI".into(),
        kind: NodeKind::Repeater,
        pos: Some(crate::geo::Pos {
            lat: 44.86,
            lon: -0.52,
        }),
        heard,
        path_len: 2,
        path: vec![0x3f, 0x12],
        ..Default::default()
    };
    app.apply(FromRadio::Nodes(vec![relay]));
    app.selected = 1;
    app
}

#[test]
fn card_shows_a_repeater() {
    let s = screen(&relay_app());
    for want in [
        "[ NODE :: 33EYSI ]",
        "REPEATER",
        "2 HOPS (3f > 12)",
        "5 MIN AGO",
        "[P] PING",
    ] {
        assert!(s.contains(want), "{want} missing in\n{s}");
    }
}

#[test]
fn card_shows_the_ping_result() {
    let mut app = relay_app();
    let pong = crate::app::Ping::Pong {
        rtt_ms: 1800,
        snr: 6.2,
        battery_mv: 4100,
        uptime: 3 * 86400,
        noise: -112,
    };
    app.apply(FromRadio::Pong {
        prefix: "5545fc8a91d2".into(),
        ping: pong,
    });
    let s = screen(&app);
    for want in ["PONG", "1.8 s", "4.10 V", "-112 dBm", "3 D 0 H"] {
        assert!(s.contains(want), "{want} missing in\n{s}");
    }
    app.apply(FromRadio::Pong {
        prefix: "5545fc8a91d2".into(),
        ping: crate::app::Ping::Failed("timeout".into()),
    });
    assert!(screen(&app).contains("TIMEOUT"));
    app.apply(FromRadio::Pong {
        prefix: "5545fc8a91d2".into(),
        ping: crate::app::Ping::Failed("login refused".into()),
    });
    let s = screen(&app);
    assert!(s.contains("LOGIN REFUSED") && !s.contains("TIMEOUT"), "{s}");
}

#[test]
fn card_for_the_public_channel() {
    let mut app = relay_app();
    app.selected = 0;
    assert!(screen(&app).contains("CHANNEL #PUBLIC"));
}

#[test]
fn card_helpers() {
    assert_eq!(
        [age(30), age(300), age(3 * 3600 + 5), age(2 * 86400)],
        ["JUST NOW", "5 MIN AGO", "3 H AGO", "2 D AGO"]
    );
    assert_eq!(
        [route(-1, &[]), route(0, &[]), route(2, &[0x3f, 0x12])],
        ["FLOOD", "DIRECT", "2 HOPS (3f > 12)"]
    );
    assert_eq!(
        [
            uptime(3 * 86400 + 4 * 3600),
            uptime(5 * 3600 + 12 * 60),
            uptime(720)
        ],
        ["3 D 4 H", "5 H 12 MIN", "12 MIN"]
    );
}

#[test]
fn normal_banner_is_plain_english() {
    let mut app = App::new("MeshCore-bbok", vec![]);
    assert!(normal(&app).contains("Searching for MeshCore-bbok... (is a phone connected to it?)"));
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    app.apply(FromRadio::Battery(Battery {
        pct: 87,
        mv: 3980,
        used_kb: Some(12),
        total_kb: Some(1024),
    }));
    app.apply(FromRadio::Nodes(nodes(12)));
    let s = normal(&app);
    assert!(
        s.contains("Connected to MeshCore-bbok (MeshCore) - battery 87 % ▮▮▮▮▮▮▮▯ - 12 contacts")
            && !s.contains(">>"),
        "{s}"
    );
}

#[test]
fn normal_thread_and_prompt() {
    let mut app = App::new("x", vec![Record::Msg(sent("hello"))]);
    app.selected = 1;
    let s = normal(&app);
    assert!(
        s.contains("me  hello")
            && s.contains("sent")
            && s.contains("> ")
            && !s.contains("root@mesh"),
        "{s}"
    );
    app.apply(FromRadio::Record(Record::Ack {
        tag: "01020304".into(),
    }));
    assert!(normal(&app).contains("received"));
}

#[test]
fn normal_panels_are_plain() {
    let s = normal(&relay_app());
    for want in [
        "Contacts (1)",
        "# Public",
        "repeater",
        " Map ",
        "Node: 33EYSI",
        "REPEATER",
    ] {
        assert!(s.contains(want), "{want} missing in\n{s}");
    }
    assert!(!s.contains("RADAR") && !s.contains("[ NODES"), "{s}");
}

#[test]
fn menu_offers_both_looks() {
    let s = render(|f| draw_menu(f, Theme::Normal, false));
    assert!(s.contains("▶ Normal") && s.contains("Hacker"), "{s}");
    assert!(render(|f| draw_menu(f, Theme::Hacker, false)).contains("▶ Hacker"));
    assert!(render(|f| draw_menu(f, Theme::Hacker, true)).contains("> Hacker"));
}

#[test]
fn menu_keys() {
    assert_eq!(
        menu_key(Theme::Normal, KeyCode::Down),
        Menu::Choose(Theme::Hacker)
    );
    assert_eq!(
        menu_key(Theme::Hacker, KeyCode::Up),
        Menu::Choose(Theme::Normal)
    );
    assert_eq!(
        menu_key(Theme::Hacker, KeyCode::Enter),
        Menu::Start(Theme::Hacker)
    );
    assert_eq!(menu_key(Theme::Normal, KeyCode::Char('q')), Menu::Quit);
    assert_eq!(menu_key(Theme::Normal, KeyCode::Esc), Menu::Quit);
}

#[test]
fn battery_gauge() {
    assert_eq!(
        [gauge(0, false), gauge(87, false), gauge(100, false)],
        ["▯▯▯▯▯▯▯▯", "▮▮▮▮▮▮▮▯", "▮▮▮▮▮▮▮▮"]
    );
    assert_eq!(
        gauge(87, true),
        "[#######-]",
        "the Linux console has no such blocks"
    );
    assert_eq!([khz(250_000), khz(62_500)], ["250", "62.5"]);
}

fn radio_app() -> App {
    let mut app = App::new("MeshCore-bbok", vec![]);
    app.apply(FromRadio::Link(Link::Connected("MeshCore-bbok".into())));
    app.apply(FromRadio::Battery(Battery {
        pct: 87,
        mv: 3980,
        used_kb: Some(12),
        total_kb: Some(1024),
    }));
    app.apply(FromRadio::SelfPos(Some(crate::geo::Pos {
        lat: 44.8267,
        lon: -0.5571,
    })));
    app.apply(FromRadio::MyRadio(MyRadio {
        prefix: "abcdef012345".into(),
        name: "bbok".into(),
        model: Some("ThinkNode M5".into()),
        version: Some("v1.18.0".into()),
        build: Some("12-Oct-2026".into()),
        freq_khz: 869_525,
        bw_hz: 250_000,
        sf: 11,
        cr: 5,
        tx_power: 22,
        max_tx_power: 22,
        shares_position: true,
        preset: None,
    }));
    app.apply(FromRadio::Nodes(nodes(11)));
    app.selected = 0;
    app
}

#[test]
fn card_for_my_radio() {
    let s = screen(&radio_app());
    let want = [
        "RADIO :: bbok",
        "ThinkNode M5",
        "v1.18.0",
        "BAT    3.98 V ",
        "12 / 1024 KB",
        "869.525 MHz",
        "BW 250 kHz SF 11 CR 5",
        "22 dBm (max 22)",
        "GPS    SHARED",
        "NODES  11",
        "LINK",
    ];
    for w in want {
        assert!(s.contains(w), "{w} missing in\n{s}");
    }
    assert!(normal(&radio_app()).contains("My radio: bbok"));
}

#[test]
fn my_radio_row_in_both_looks() {
    assert!(screen(&radio_app()).contains("abcdef bbok"));
    assert!(normal(&radio_app()).contains("◆ bbok"));
}

#[test]
fn footer_lists_the_keys() {
    assert!(screen(&App::new("x", vec![])).contains("[TAB] CYCLE"));
}

#[test]
fn boot_sequence_ends_ready() {
    let first = render(|f| draw_boot(f, std::time::Duration::ZERO));
    assert!(
        first.contains("Initializing LoRa mesh stack") && !first.contains("READY"),
        "{first}"
    );
    let last = render(|f| draw_boot(f, BOOT));
    assert!(
        last.contains(">> READY :: 73") && !last.contains("Decrypting"),
        "{last}"
    );
}

// ===============================================================
//  LINUX CONSOLE (ASCII GLYPHS, NO IMAGES)
// ===============================================================

fn console(mut app: App, theme: Theme) -> String {
    app.ascii = true;
    screen_with(&app, theme)
}

#[test]
fn console_names_lose_their_emoji_but_keep_their_accents() {
    assert_eq!(ascii_safe("🐱 SHA-40 Réseau", true), "? SHA-40 Réseau");
    assert_eq!(ascii_safe("🐱 cat", false), "🐱 cat");
}

#[test]
fn console_uses_plain_boxes_and_ascii_marks() {
    let s = console(radio_app(), Theme::Normal);
    assert!(s.contains("┌") && !s.contains("╭"), "{s}");
    assert!(
        s.contains("> * bbok") && !s.contains("▶") && !s.contains("◆"),
        "{s}"
    );
    assert!(s.contains("battery 87 % [#######-]"), "{s}");
}

#[test]
fn console_message_text_is_safe() {
    let msg = Record::Msg(Msg {
        ts: 0,
        conv: "public".into(),
        from: "Bob 🐱".into(),
        text: "hi 🐱".into(),
        outgoing: false,
        ack: None,
    });
    let s = console(App::new("x", vec![msg]), Theme::Hacker);
    assert!(s.contains("<Bob ?> hi ?"), "{s}");
    let mut app = App::new("x", vec![]);
    let cat = Node {
        prefix: "01".into(),
        name: "🐱 Cat".into(),
        ..Default::default()
    };
    app.apply(FromRadio::Nodes(vec![cat]));
    app.selected = 1;
    let s = console(app, Theme::Hacker);
    assert!(s.contains("COMMS // ? Cat") && !s.contains('🐱'), "{s}");
}

#[test]
fn console_map_is_a_radar_of_letters() {
    let mut app = App::new("x", vec![]);
    let me = crate::geo::Pos {
        lat: 44.80,
        lon: -0.60,
    };
    app.apply(FromRadio::SelfPos(Some(me)));
    let north = crate::geo::Pos {
        lat: 44.82,
        lon: -0.60,
    };
    let relay = Node {
        prefix: "01".into(),
        name: "Quebec".into(),
        kind: NodeKind::Repeater,
        pos: Some(north),
        ..Default::default()
    };
    app.apply(FromRadio::Nodes(vec![relay]));
    let s = console(app, Theme::Normal);
    // The radar is the middle panel: columns 33 to 66 of the 100-column test screen.
    let radar: Vec<String> = s
        .lines()
        .map(|line| line.chars().skip(33).take(33).collect())
        .collect();
    let row_of = |mark: char| radar.iter().position(|line| line.contains(mark));
    let (me_row, relay_row) = (row_of('+'), row_of('Q'));
    assert!(me_row.is_some() && relay_row.is_some(), "{s}");
    assert!(
        relay_row < me_row,
        "the node to the north is drawn above me:\n{s}"
    );
    assert!(s.contains("1 cell = "), "a scale:\n{s}");
    assert!(!s.contains("OpenStreetMap"), "no tiles, no credit:\n{s}");
}

#[test]
fn the_map_panel_reports_its_size_in_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let mut map = MapState::new(
        ratatui_image::picker::Picker::halfblocks(),
        crate::tiles::Tiles::offline(dir.path().into()),
    );
    assert_eq!(map.size_px(), None);
    let app = App::new("x", vec![]);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| draw(f, &app, &mut map, Theme::Normal))
        .unwrap();
    let (w, h) = map.size_px().unwrap();
    assert!(w > 0 && h > 0, "{w} x {h}");
}

#[test]
fn header_names_the_firmware() {
    let mut app = App::new("x", vec![]);
    app.apply(FromRadio::Firmware(Firmware::Meshtastic));
    app.apply(FromRadio::Link(Link::Connected("Meshtastic_d940".into())));
    let normal = screen_with(&app, Theme::Normal);
    assert!(normal.contains("Connected to Meshtastic_d940 (Meshtastic)"), "{normal}");
    let hacker = screen(&app);
    assert!(hacker.contains(">> LINK ESTABLISHED :: Meshtastic_d940 :: MESHTASTIC"), "{hacker}");
}

#[test]
fn cards_show_meshtastic_results() {
    let mut app = relay_app();
    app.apply(FromRadio::Pong {
        prefix: "5545fc8a91d2".into(),
        ping: crate::app::Ping::Trace {
            rtt_ms: 1500,
            hops: 2,
            snr: Some(4.5),
        },
    });
    let s = screen(&app);
    for want in ["TRACE", "1.5 s  2 HOPS", "SNR", "4.5 dB"] {
        assert!(s.contains(want), "{want} missing in\n{s}");
    }
    app.apply(FromRadio::Pong {
        prefix: "5545fc8a91d2".into(),
        ping: crate::app::Ping::Trace {
            rtt_ms: 900,
            hops: 0,
            snr: None,
        },
    });
    let s = screen(&app);
    assert!(s.contains("0.9 s  DIRECT") && !s.contains("0 HOPS"), "{s}");
    let mut radio = radio_app();
    let me = MyRadio {
        preset: Some("EU_868 LONG_FAST".into()),
        freq_khz: 0,
        ..radio.me.clone().unwrap()
    };
    radio.apply(FromRadio::MyRadio(me));
    let s = screen(&radio);
    assert!(s.contains("EU_868 LONG_FAST") && !s.contains("MHz"), "{s}");
}

#[test]
fn recent_contacts_are_marked_in_the_list() {
    let now = chrono::Utc::now().timestamp();
    let node = |name: &str, ago: i64| Node {
        prefix: format!("{:012x}", name.len()),
        name: name.into(),
        heard: if ago < 0 { 0 } else { (now - ago) as u32 },
        ..Default::default()
    };
    let mut app = App::new("x", vec![]);
    // Different name lengths give different prefixes.
    app.apply(FromRadio::Nodes(vec![node("aa", 60), node("bbb", 7200), node("cccc", -1)]));
    let s = normal(&app);
    let line = |name: &str| s.lines().find(|l| l.contains(name)).unwrap().to_string();
    assert!(line("aa").contains("● aa"), "{s}");
    assert!(!line("bbb").contains('●'), "{s}");
    assert!(!line("cccc").contains('●'), "{s}");
    assert!(screen(&app).lines().any(|l| l.contains("+aa")), "{s}");
}
