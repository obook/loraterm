# meshcore-m5

A terminal client for a MeshCore companion radio over Bluetooth. You can
send and receive messages, direct or on the public channel, and see the
nodes on a map.

Tested on Linux (BlueZ) with an ELECROW ThinkNode M5 running the MeshCore
Companion Bluetooth v1.18.0 firmware from https://meshcore.io/flasher.

## Build

You need Rust.

```
cargo build --release
```

The program is `target/release/meshcore-m5`.

## Pair the radio

Pair once from `bluetoothctl`. The radio wants a bond made with its PIN;
pairing from the KDE Bluetooth settings leaves a bond the radio rejects.

```
bluetoothctl
agent KeyboardOnly
default-agent
scan on
pair A4:CB:8F:CB:D9:41
```

Put your radio's address in the last line, then type the PIN shown on the
radio's screen (the M5 picks a new one at each boot). To check the link:

```
meshcore-m5 --probe
```

## Run

```
meshcore-m5
```

A menu asks for the look, Normal or Hacker. The program then connects to
the first "MeshCore-..." radio it finds and reconnects on its own.

Options:

- `--device <name or address>`: the radio to use
- `--probe`: print the radio's state and exit
- `--image auto|sixel|kitty|iterm2|halfblocks`: how the map image is shown
- `--console`: ASCII look with a radar of letters instead of the map;
  automatic on the Linux console (`TERM=linux`)

The map looks best in a terminal that shows images with the Kitty, Sixel
or iTerm2 protocol, like Konsole. Other terminals get a coarser picture
made of half blocks.

## Keys

- Tab: go from the contact list to the map, then to the input line
- Arrows: pick a contact in the list, pan the map
- `+` and `-`: zoom the map
- Enter: send the message you typed
- PgUp and PgDn: scroll the conversation
- `p`: ping the selected node
- `q` or Ctrl+C: quit

Outside the input line only, `p` pings and `q` quits.

## Files

- Messages: `~/.local/share/meshcore-m5/history.jsonl`
- Map tiles: `~/.cache/meshcore-m5/tiles/`
- Log: `$XDG_RUNTIME_DIR/meshcore-m5.log`

The plan is in [ROADMAP.md](ROADMAP.md).

## License

GPL-3.0, see [LICENSE](LICENSE).

- Map tiles: (c) OpenStreetMap contributors.
- Node icons: map.meshcore.io by Rastislav Vysoky, MIT, see
  `assets/node_types/LICENSE`.
- `vendor/meshcore-rs`: meshcore-rs 0.2.0 (MIT) with our Bluetooth fixes.
