# Roadmap

loraterm (formerly meshcore-m5) is a terminal client that talks over
Bluetooth to a MeshCore or Meshtastic radio. It is tested on an ELECROW
ThinkNode M5 (MeshCore Companion Bluetooth v1.18.0, Meshtastic 2.7.26).

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
- [x] Map fitted to the panel size; selecting the M5 shows its neighbors
- [x] Fixes to meshcore-rs 0.2.0 (copy in `vendor/`): one D-Bus
  connection, no more duplicated notifications after a reconnection

## To check on the hardware

- [ ] Ping a repeater with the guest login ("33LPOR Pier Ré" failed
  before the fix)
- [ ] Console mode on a real Linux console (TTY), then over SSH
- [ ] Map fit when selecting the M5
- [ ] Battery percentage compared with the Android app

## Meshtastic support

The M5 runs either firmware; the program picks the protocol on its own,
from what the radio advertises over Bluetooth. Design:
[docs/meshtastic-design.md](docs/meshtastic-design.md).

- [x] License: the project is GPL-3.0, like the `meshtastic` crate 0.1.9
- [x] Read the crate: its released Bluetooth code has a bug and its own
  BLE manager, so only its protobuf types are used; the transport is ours,
  on the shared manager
- [x] Detect the firmware during the Bluetooth scan from the advertised
  service UUID: MeshCore `6e400001-...`, Meshtastic `6ba1b218-...`
- [x] `src/radio/meshtastic/` module that sends the same `FromRadio` and
  takes the same `ToRadio` as the MeshCore side
- [x] Nodes and positions from the Meshtastic node database
- [x] Messages: channel 0 as the public channel, direct messages, receipts
- [x] Battery: percentage given by the firmware
- [x] Ping: traceroute instead of the repeater status request
- [x] Node types: Meshtastic roles (CLIENT, ROUTER...) on the card and
  the map
- [x] Detected firmware shown in the header
- [x] Tests for the conversions from Meshtastic to our types
- [x] Try it on the M5 flashed with Meshtastic: `--probe`, connection,
  header and battery
- [ ] Message exchange and traceroute with another Meshtastic node
- [ ] MeshCore check after the switch back (backup restored; re-pair with
  `scripts/pair-radio.py`, then probe)
- [x] New name for the project: loraterm; the history and tile cache move
  to the new data paths on the first start

## Documentation and release

- [x] README: build, pairing, usage, keys
- [x] LICENSE file (GPL-3.0)
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
