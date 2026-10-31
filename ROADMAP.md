# Roadmap

meshcore-m5 is a terminal MeshCore client that talks over Bluetooth to an
ELECROW ThinkNode M5 (firmware MeshCore Companion Bluetooth v1.18.0).

## Done

- [x] Bluetooth link to the M5, automatic reconnection, single instance
- [x] Direct messages and the public channel, with delivery receipts
- [x] Message history on disk (JSONL), written before display
- [x] Contact list and node card: reachability, route, ping
- [x] M5 card: battery (LiPo curve), radio settings, connection time
- [x] OpenStreetMap map with a tile cache, zoom and pan
- [x] map.meshcore.io icons, pin on the selected node
- [x] Normal and Hacker looks, picked from the start menu
- [x] Linux console mode (`--console` or `TERM=linux`): ASCII display and
  a radar of letters instead of the map
- [x] Map fitted to the panel size; selecting the M5 shows its neighbours
- [x] Fixes to meshcore-rs 0.2.0 (copy in `vendor/`): one D-Bus
  connection, no more duplicated notifications after a reconnection

## To check on the hardware

- [ ] Ping a repeater with the guest login ("33LPOR Pier Ré" failed
  before the fix)
- [ ] Console mode on a real Linux console (TTY), then over SSH
- [ ] Map fit when selecting the M5
- [ ] Battery percentage compared with the Android app

## Meshtastic support

The M5 runs either firmware. The program would pick the right protocol on
its own, from what the M5 advertises over Bluetooth.

- [ ] Choose the licence: the `meshtastic` crate 0.1.9 is GPL-3.0 while
  this project is MIT. Move the project to GPL-3.0 or keep Meshtastic
  behind a build feature
- [ ] Check the licence of the Meshtastic Protobuf definitions
- [ ] Read the crate's code: Bluetooth on Linux, a D-Bus connection opened
  on each connect (risk of duplicated notifications)
- [ ] Detect the firmware during the Bluetooth scan from the advertised
  service UUID: MeshCore `6e400001-...`, Meshtastic `6ba1b218-...`
- [ ] `src/radio/meshtastic/` module that sends the same `FromRadio` and
  takes the same `ToRadio` as the MeshCore side
- [ ] Nodes and positions from the Meshtastic node database
- [ ] Messages: channel 0 as the public channel, direct messages, receipts
- [ ] Battery: percentage given by the firmware
- [ ] Ping: traceroute instead of the repeater status request
- [ ] Node types: Meshtastic roles (CLIENT, ROUTER...) on the card and
  the map
- [ ] Detected firmware shown in the header
- [ ] Tests for the conversions from Meshtastic to our types
- [ ] Try it on the M5 flashed with Meshtastic

## Documentation and release

- [ ] README: install, pairing (`bluetoothctl` with the `KeyboardOnly`
  agent and the PIN shown on the M5), usage, keys
- [ ] LICENSE file (`Cargo.toml` says MIT)
- [ ] Offer our Bluetooth fixes to the meshcore-rs project, so `vendor/`
  can go
- [ ] Tag v0.1.0 and release notes
- [ ] GitHub Actions CI: tests and clippy

## Known small debts

- [ ] The M5's position is read only at connect: it does not follow a move
- [ ] Tiles that left the view stay in the download queue
- [ ] Guest login replies do not name the node: one ping at a time
- [ ] Maximum contact count not shown: meshcore-rs caps the firmware's
  value at 255
- [ ] Long path hashes show byte by byte
- [ ] Conversation scrolling assumes at most 4 rows per message
- [ ] The node type goes from `app` to `ui` as a string ("repeater",
  "room"...)
- [ ] The time is read three different ways (chrono `Utc`, `Local`,
  `SystemTime`)
- [ ] The XDG directory lookup is repeated in three files
