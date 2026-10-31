# Meshtastic support: design

## Intent

The ThinkNode M5 runs either MeshCore or Meshtastic, and its owner switches
between them (full flash backup, then restore). meshcore-m5 should work the
same way with both: it connects to the radio it finds, detects the firmware
on its own, and shows the same screens (contact list, map, conversations,
node card, radio card, battery) with the same keys.

The roadmap's "Meshtastic support" section lists the steps. This document
gives what a builder needs to plan them.

## Goals

- **Detection during the scan**, before connecting, from the advertised
  service UUIDs, which btleplug exposes in `PeripheralProperties.services`:
  - MeshCore: Nordic UART service `6e400001-b5a3-f393-e0a9-e50e24dcca9e`
    and a name starting with `MeshCore-` (the UUID alone is generic);
  - Meshtastic: `6ba1b218-15a8-461f-9fa8-5dcae273eafd`. The name is
    `Meshtastic_xxxx` or `<short name>_xxxx`; do not rely on it.
  - `--device <name or address>` keeps working for both, and so does
    `--probe`.
- **Same interface contract**: the Meshtastic session sends the same
  `FromRadio` events and takes the same `ToRadio` commands as the MeshCore
  session (`src/radio/mod.rs`). The UI changes only where Meshtastic has
  different data (see below).
- **Nodes**: from the node database the radio sends during the config
  handshake (`NodeInfo`: user names, role, position, `last_heard`,
  `hops_away`, device metrics), then kept up to date from `NODEINFO_APP`,
  `POSITION_APP` and `TELEMETRY_APP` packets. Positions go on the map.
- **Messages**:
  - channel 0 (the primary channel) is the public channel;
  - direct messages go to a node number;
  - a direct message asks for an ack (`want_ack`); the receipt is the
    `ROUTING_APP` packet whose `request_id` is our packet id, with
    `error_reason` NONE, sent by the destination node. An implicit ack from
    a relay does not count. Public messages get no receipt, as with
    MeshCore. Delivery failures reported by the firmware are not shown.
- **Ping key `p`**: sends a traceroute (`TRACEROUTE_APP`) to the selected
  node, whatever its role. The node card shows the result: round-trip time,
  hop count and SNR. Direct messages may go to any node, routers included.
- **Battery**: the percentage and voltage the firmware reports in my node's
  device metrics. `battery_level` 101 means external power.
- **Node types**: Meshtastic roles mapped to `NodeKind` for the card and the
  map icons. ROUTER, ROUTER_LATE, ROUTER_CLIENT and REPEATER become
  Repeater; SENSOR becomes Sensor; every other role becomes Chat. Meshtastic
  has no rooms.
- **Radio card**: name, hardware model, firmware version, LoRa settings
  (region and modem preset, or frequency, bandwidth, spreading factor and
  coding rate) and TX power.
- **Detected firmware** shown in the header ("MeshCore" or "Meshtastic").
- **Clock**: at each connection, set the radio time with the admin message
  `set_time_only` sent to my own node. Meshtastic has no command to read the
  radio time, so the MeshCore rule (set only when behind) cannot apply.
- **Reconnection and link check**: same behavior as with MeshCore. The
  session ends when the link drops, and the radio task reconnects on its own.
- **Tests** for every conversion from Meshtastic data to our types.

## Non-goals

- Secondary channels, channel or device configuration (except the time),
  sending my position, Meshtastic over serial or TCP, MQTT, store and
  forward, favorite or ignored nodes.
- MeshCore-only data under Meshtastic: stored routes (only the hop count is
  shown), rooms, repeater login.

## Anti-goals

- Any change in behavior with a MeshCore radio.
- A second btleplug `Manager` or a new D-Bus connection per connect. This
  caused duplicated notifications with MeshCore (see the fixes in
  `vendor/meshcore-rs`). Both protocols share the one process-wide manager.
- A received message shown but not saved: as today, each record is written
  to the history file before the UI gets it, and a write failure stops the
  session.
- A second btleplug version in the binary.

## Constraints

- Rust, tokio, btleplug 0.12 (the version meshcore-rs uses).
- GPL-3.0. The `meshtastic` crate is GPL-3.0 too.
- Linux and BlueZ only. Pairing is done beforehand with `bluetoothctl`.
- Target: Meshtastic 2.7.26 on the ThinkNode M5.
- Code, comments, docs and commits in English.

## Decisions

Made by the owner (conversation and roadmap): automatic detection; the same
features as with MeshCore; MeshCore keeps working; channel 0 as the public
channel; traceroute for the ping key; battery percentage from the firmware;
roles on the card and the map; firmware name in the header. Also:

- **Separate histories.** The history file stays single, but each firmware
  only shows its own conversations. The MeshCore public channel and the
  Meshtastic primary channel are two different conversations: they are two
  networks with different people.
- **Any radio in range.** Without `--device`, the app may connect to any
  MeshCore radio (named `MeshCore-...`, as today) or any Meshtastic radio
  (by service UUID), the first that connects. Accepted risk: it can connect
  to a stranger's Meshtastic node that needs no PIN, and then set that
  node's clock.
- **Re-pairing on a firmware switch.** The owner removes the bond and pairs
  again with `bluetoothctl` after each switch (documented procedure). The
  M5 keeps the same Bluetooth address under both firmwares.

Made by the designer, open to review:

- **Bluetooth transport written with btleplug**, using the `meshtastic`
  crate (0.1.9, `default-features = false`) only for its generated protobuf
  types. Reasons: 0.1.9 has a BLE bug that is fixed only on its unreleased
  main branch (packets stuck until the next write); the crate opens its own
  btleplug manager; its next release moves to btleplug 0.13. Expected size:
  150 to 300 lines.
- **Protocol** (Meshtastic client API): BlueZ negotiates the MTU (btleplug
  0.12 cannot request one); write a
  `ToRadio` with `want_config_id`; read the FromRadio characteristic until
  it returns empty; then, on each FromNum notification, read until empty
  again. Over BLE, packets have no 4-byte serial header.
  - Service `6ba1b218-15a8-461f-9fa8-5dcae273eafd`
  - ToRadio `f75c76d2-129e-4dad-a1dd-7866124401e7` (write)
  - FromRadio `2c55e69e-4993-11ed-b878-0242ac120002` (read)
  - FromNum `ed9da18c-a800-4f66-a670-aa7547e34453` (notify)
- **Shared manager**: `vendor/meshcore-rs` exposes its process-wide
  `manager()` so the Meshtastic transport and the scan use it.
- **History keys**: every Meshtastic key written to the history starts with
  `mt:`, so no record format change is needed and old records stay
  MeshCore's. A node's conversation is `mt:` plus its node id
  (`mt:!a1b2c3d4`), the primary channel is `mt:public`, an ack tag is `mt:`
  plus the packet id in hex. Without the prefix, MeshCore ack tags and
  Meshtastic packet ids (both 8 hex digits) would share one set.
- **Which conversations show**: those of the detected firmware. Before a
  radio is detected, those of the firmware of the last record in the
  history. The list of conversations without a known node only takes the
  current firmware's.
- **Stale Bluetooth cache**: if the bond was not renewed, BlueZ may still
  report the old firmware's service. After connecting, the session checks
  that the expected GATT service is there; if not, it ends with the message
  "re-pair the radio with bluetoothctl" instead of retrying in a loop.

## Separate work

The project needs a new name: "meshcore-m5" no longer fits once it speaks
Meshtastic too. Renaming is its own task, after this one. It touches the
binary and crate names, the GitHub repository, the README, and the data
paths (`~/.local/share/meshcore-m5/history.jsonl`, the tile cache, the log
and the lock file); the history must be carried over to the new path.

## Left to the builder

- Module layout under `src/radio/` (the roadmap suggests
  `src/radio/meshtastic/`) and how the radio task picks the session.
- How the traceroute result fits the `Ping` type, and how the card shows it.
- How the link loss is detected (btleplug disconnect event, failed read,
  or the `heartbeat` message of `ToRadio`).
- How the node card and the radio card adapt their fields to Meshtastic.
- What `--probe` prints for a Meshtastic radio.
