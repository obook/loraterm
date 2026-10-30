//! main.rs
//! meshcore-m5: terminal client for a MeshCore companion radio (ThinkNode M5) over Bluetooth.
//! This file reads the command line, sets up the log and the single-instance lock, then runs
//! either the probe or the interface:
//!   app/ (state and keys), radio/ (the BLE link, in a tokio task), ui/ (drawing),
//!   map.rs and tiles/ (the OpenStreetMap map), history.rs (messages on disk),
//!   tui.rs (screens and main loop), config.rs (settings).
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

mod app;
mod config;
mod geo;
mod history;
mod map;
mod probe;
mod radio;
mod tiles;
mod tui;
mod ui;

use std::io::Write;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// BLE name or address of the radio (default: the first "MeshCore-..." found).
    #[arg(long)]
    device: Option<String>,
    /// Connect, print the radio's state and exit.
    #[arg(long)]
    probe: bool,
    /// How the map image is shown; "auto" asks the terminal (iTerm2 images in Konsole).
    #[arg(long, value_enum, default_value = "auto")]
    image: ImageMode,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ImageMode {
    Auto,
    Sixel,
    Kitty,
    Iterm2,
    Halfblocks,
}

/// Set while the TUI runs: logs go to this file, opened once.
static TUI_LOG: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();

fn main() -> anyhow::Result<()> {
    // RUST_LOG prints to stderr. The TUI owns the terminal: it logs to a file instead.
    let verbose = std::env::var_os("RUST_LOG").is_some();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("meshcore_m5=info")),
        )
        .with_ansi(false)
        .with_writer(move || -> Box<dyn Write> {
            if let Some(file) = TUI_LOG.get() {
                Box::new(file)
            } else if verbose {
                Box::new(std::io::stderr())
            } else {
                Box::new(std::io::sink())
            }
        })
        .init();

    let cli = Cli::parse();
    let _lock = single_instance(&runtime_dir().join("meshcore-m5.lock"))?;
    // tokio runs asynchronous tasks; the BLE library (meshcore-rs on btleplug) needs it.
    let rt = tokio::runtime::Runtime::new()?;
    if cli.probe {
        return rt.block_on(probe::probe(cli.device.as_deref()));
    }

    let history_path = history::default_path();
    let records = history::load(&history_path)?;
    let label = cli
        .device
        .clone()
        .unwrap_or_else(|| radio::ANY_RADIO.into());
    let mut app = app::App::new(&label, records);
    let _ = TUI_LOG.set(std::fs::File::create(log_path())?);
    // A channel: the radio task sends its updates (FromRadio), the interface reads them.
    let (tx, rx) = std::sync::mpsc::channel();
    let cli_device = cli.device.clone();
    let cli_image = cli.image;
    let (cmds, radio_task) = radio::spawn(&rt, cli.device, history_path, tx);

    let mut terminal = ratatui::init();
    // Asks the terminal how it shows images (Kitty, Sixel...); half blocks work everywhere.
    let mut picker = ratatui_image::picker::Picker::from_query_stdio().unwrap_or_else(|e| {
        tracing::warn!("terminal image query failed: {e}");
        ratatui_image::picker::Picker::halfblocks()
    });
    use ratatui_image::picker::ProtocolType;
    match cli_image {
        ImageMode::Auto => {
            // ratatui-image keeps Konsole on half blocks (its old Sixel bugs); Konsole 26.08 shows
            // iTerm2 images well, in full color.
            if std::env::var_os("KONSOLE_VERSION").is_some()
                && picker.protocol_type() == ProtocolType::Halfblocks
            {
                picker.set_protocol_type(ProtocolType::Iterm2);
            }
        }
        ImageMode::Sixel => picker.set_protocol_type(ProtocolType::Sixel),
        ImageMode::Kitty => picker.set_protocol_type(ProtocolType::Kitty),
        ImageMode::Iterm2 => picker.set_protocol_type(ProtocolType::Iterm2),
        ImageMode::Halfblocks => picker.set_protocol_type(ProtocolType::Halfblocks),
    }
    tracing::info!(
        "map: {:?}, font {:?}",
        picker.protocol_type(),
        picker.font_size()
    );
    let mut map = ui::MapState::new(picker, tiles::Tiles::new(tiles::Tiles::default_dir()));
    let result = tui::run_tui(&mut terminal, &mut app, &mut map, &rx, &cmds);
    ratatui::restore();

    // Let the radio task disconnect, so a phone can connect to the radio again. It cannot
    // while still connecting or starting: drop the link from here then.
    drop(cmds);
    while let Ok(update) = rx.try_recv() {
        app.apply(update);
    }
    drop(rx);
    let finished = rt
        .block_on(async { tokio::time::timeout(Duration::from_secs(3), radio_task).await })
        .is_ok();
    // The radio task announces each radio before connecting to it.
    let name = match &app.link {
        radio::Link::Connected(name) => Some(name.clone()),
        radio::Link::Searching(name) if name != radio::ANY_RADIO => Some(name.clone()),
        _ => cli_device,
    };
    if let (false, Some(name)) = (finished, name) {
        let _ = rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(3), radio::ble_disconnect(&name)).await
        });
    }
    result
}

/// TUI log, on the per-user tmpfs (private, cleared at logout).
fn log_path() -> std::path::PathBuf {
    runtime_dir().join("meshcore-m5.log")
}

fn runtime_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, Into::into)
}

/// Two instances fight over the radio and BlueZ sends every notification to both: allow one.
/// The lock lives as long as the returned file.
fn single_instance(path: &std::path::Path) -> anyhow::Result<std::fs::File> {
    let file = std::fs::File::create(path)?;
    if file.try_lock().is_err() {
        anyhow::bail!("meshcore-m5 is already running (quit the other one first)");
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_one_instance_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = super::single_instance(&path).unwrap();
        assert!(super::single_instance(&path).is_err());
        drop(first);
        assert!(super::single_instance(&path).is_ok());
    }
}
