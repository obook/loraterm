//! tui.rs
//! Screens and main loop: start menu (Normal or Hacker look), the Hacker boot log, then the
//! app. Each turn applies the radio's updates, draws and turns a key into an app action or
//! a radio command.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use std::time::Duration;

use crate::{app, radio, ui};

/// Start menu, then (Hacker only) the boot log, then the app.
#[derive(Clone, Copy)]
enum Screen {
    Menu(ui::Theme),
    Boot(std::time::Instant),
    Main(ui::Theme),
}

/// Run the screens until the user quits; radio updates arrive on `rx`, commands leave on `cmds`.
pub fn run_tui(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut app::App,
    map: &mut ui::MapState,
    rx: &std::sync::mpsc::Receiver<radio::FromRadio>,
    cmds: &tokio::sync::mpsc::UnboundedSender<radio::ToRadio>,
) -> anyhow::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    let mut screen = Screen::Menu(ui::Theme::Normal);
    while !app.quit {
        while let Ok(update) = rx.try_recv() {
            app.apply(update);
        }
        if let Screen::Boot(start) = screen {
            if start.elapsed() >= ui::BOOT {
                screen = Screen::Main(ui::Theme::Hacker);
            }
        }
        terminal.draw(|frame| match screen {
            Screen::Menu(theme) => ui::draw_menu(frame, theme),
            Screen::Boot(start) => ui::draw_boot(frame, start.elapsed()),
            Screen::Main(theme) => ui::draw(frame, app, map, theme),
        })?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match screen {
            Screen::Menu(_)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c') =>
            {
                app.quit = true
            }
            Screen::Menu(theme) => match ui::menu_key(theme, key.code) {
                ui::Menu::Choose(theme) => screen = Screen::Menu(theme),
                ui::Menu::Start(ui::Theme::Normal) => screen = Screen::Main(ui::Theme::Normal),
                ui::Menu::Start(ui::Theme::Hacker) => {
                    screen = Screen::Boot(std::time::Instant::now())
                }
                ui::Menu::Quit => app.quit = true,
            },
            Screen::Boot(_) => screen = Screen::Main(ui::Theme::Hacker), // a key skips the boot log
            Screen::Main(_) => {
                if let Some(cmd) = app.key(key) {
                    if let Err(e) = cmds.send(cmd) {
                        app.apply(e.0.failed("radio task stopped".into()));
                    }
                }
            }
        }
    }
    Ok(())
}
