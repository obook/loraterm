# Meshtastic support: implementation plan

**Goal:** loraterm (then called meshcore-m5) detects whether the radio runs MeshCore or Meshtastic
and offers the same screens with either.

**Architecture:** the scan classifies each radio by its advertised service.
A new `src/radio/meshtastic/` module (BLE transport, conversions, session)
sends the same `FromRadio` and takes the same `ToRadio` as the MeshCore
session; `task.rs` picks the session from the candidate's firmware. The app
learns the firmware from a new `FromRadio::Firmware` event and only shows
that firmware's conversations, Meshtastic keys being prefixed with `mt:`.

**Tech stack:** Rust 2021, tokio, btleplug 0.12 (through the shared manager
of `vendor/meshcore-rs`), `meshtastic` 0.1.9 protobuf types, prost 0.14,
ratatui.

**Spec:** `docs/meshtastic.md`. Read it first; this plan does not repeat
its reasons.

## Global constraints

- One btleplug version in the binary: `cargo tree -d` must not list
  btleplug. Add `btleplug = "0.12"` and `uuid = "1"` to `Cargo.toml`.
- `meshtastic = { version = "0.1.9", default-features = false }` (protobuf
  types only, module `meshtastic::protobufs`) and `prost = "0.14"`.
- One BLE manager per process: every btleplug access goes through
  `meshcore_rs::ble_manager()` (Task 1). Never call `Manager::new()`.
- MeshCore behavior unchanged; the history file format unchanged.
- Every Meshtastic key written to the history starts with `mt:`: node
  conversation `mt:!a1b2c3d4`, primary channel `mt:public`, secondary
  channel `mt:channel:N`, ack tag `mt:a1b2c3d4` (packet id, 8 hex digits).
- Every received record goes through the existing `session::save()` (write,
  then report; a write failure stops the session).
- Code, comments, docs and commit messages in English; no mention of AI
  tools anywhere. New files get the usual header (`//! path`, one-line role,
  Author: O. Booklage, Date: October 2026, Licence: GPL-3.0).
- `cargo test` and `cargo clippy --all-targets` clean after every task.

## Review focus

1. The radio is reflashed while the app runs: on reconnect the other
   firmware's nodes, battery, card and conversations must disappear.
   Test in Task 2 (`switching_firmware_clears_the_radio_state`).
2. BlueZ still caches the old firmware's service (no re-pairing): one clear
   "re-pair the radio with bluetoothctl" message, no endless retry. Test in
   Task 5 (`repair_needed_says_what_to_do`).
3. Messages queued in the radio arrive during the config handshake: they
   must be saved, not dropped. Test in Task 5
   (`packets_during_the_handshake_are_kept`).
4. An ack sent by a relay, or for a packet we did not send: no "received".
   Test in Task 4 (`only_the_destination_acks`).
5. A NodeDB of hundreds of nodes: at most one `FromRadio::Nodes` per check
   period, never one per packet (Task 5, `nodes_stale` flag). Text length
   needs no change: `MAX_TEXT_BYTES` (120) is under Meshtastic's 233 bytes.

---

### Task 1: Shared BLE manager and a scan that tells the firmwares apart

**Files:**
- Modify: `vendor/meshcore-rs/src/meshcore/ble.rs`, `vendor/meshcore-rs/src/lib.rs`
- Modify: `Cargo.toml`, `src/radio/mod.rs`, `src/radio/link.rs`, `src/radio/task.rs`, `src/probe.rs`
- Test: `src/radio/tests.rs`

**Interfaces:**
- Produces:
  - `meshcore_rs::ble_manager() -> meshcore_rs::Result<&'static btleplug::platform::Manager>` (wraps the private `manager()`).
  - In `src/radio/mod.rs`: `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub enum Firmware { #[default] MeshCore, Meshtastic }` with `pub fn name(self) -> &'static str` ("MeshCore", "Meshtastic"); `ANY_RADIO` becomes `"a MeshCore or Meshtastic radio"`.
  - In `src/radio/link.rs`: `pub const MESHCORE_SERVICE: Uuid = Uuid::from_u128(0x6e400001_b5a3_f393_e0a9_e50e24dcca9e)`, `pub const MESHTASTIC_SERVICE: Uuid = Uuid::from_u128(0x6ba1b218_15a8_461f_9fa8_5dcae273eafd)`, `#[derive(Clone, Debug, PartialEq)] pub struct Candidate { pub name: String, pub firmware: Firmware }`, `pub fn classify(name: Option<&str>, services: &[Uuid]) -> Option<Firmware>`, `pub async fn candidates(device: Option<&str>) -> anyhow::Result<Vec<Candidate>>`.

- [ ] **Step 1: Write the failing test** `classify_tells_the_firmwares_apart` in `src/radio/tests.rs`:

```rust
assert_eq!(classify(Some("Meshtastic_d940"), &[MESHTASTIC_SERVICE]), Some(Firmware::Meshtastic));
assert_eq!(classify(Some("bbok_d940"), &[MESHTASTIC_SERVICE]), Some(Firmware::Meshtastic));
assert_eq!(classify(Some("MeshCore-bbok"), &[MESHCORE_SERVICE]), Some(Firmware::MeshCore));
assert_eq!(classify(Some("MeshCore-bbok"), &[]), Some(Firmware::MeshCore));
assert_eq!(classify(Some("Nordic_UART"), &[MESHCORE_SERVICE]), None);
assert_eq!(classify(None, &[]), None);
// A stale BlueZ cache can list both: the Meshtastic service wins, the session checks it.
assert_eq!(classify(Some("MeshCore-bbok"), &[MESHTASTIC_SERVICE]), Some(Firmware::Meshtastic));
```

- [ ] **Step 2: Run** `cargo test classify` and see it fail to compile.

- [ ] **Step 3: Implement.** `classify`: the Meshtastic service gives Meshtastic, else a name starting with `MeshCore-` gives MeshCore (the Nordic UART UUID alone is generic), else `None`. `candidates`: scan 4 s through `ble_manager()` with `ScanFilter { services: vec![MESHCORE_SERVICE, MESHTASTIC_SERVICE] }` (stop any running scan first, as `ble_discover` does), then classify every `adapter.peripherals()` entry from its properties (`local_name`, `address`, `services`); this keeps the radios BlueZ only remembers, as today. With `device`, keep the entries whose name or address equals it; if none, return `Candidate { name: device, firmware: MeshCore }` (today's behavior). `task.rs` and `probe.rs` take `Candidate`s; a Meshtastic candidate is skipped with `tracing::info!("Meshtastic radio {name}: not supported yet")` until Task 5. The "no MeshCore radio found" errors become "no radio found".

- [ ] **Step 4: Run** `cargo test` and `cargo clippy --all-targets`: all pass, and `cargo tree -d` lists no btleplug.

- [ ] **Step 5: Check on hardware.** With the M5 under Meshtastic, the TUI log shows the skip line for it.

- [ ] **Step 6: Commit** `feat(radio): Tell MeshCore and Meshtastic radios apart during the scan`.

### Task 2: The app follows the firmware

**Files:**
- Modify: `src/radio/mod.rs`, `src/radio/session.rs`, `src/history.rs`, `src/app/mod.rs`, `src/app/update.rs`, `src/app/keys.rs`, `src/ui/panels.rs`, `src/ui/contacts.rs`, `src/ui/conversation.rs`, `src/ui/card.rs`
- Test: `src/app/tests.rs`, `src/ui/tests.rs`

**Interfaces:**
- Consumes: `Firmware` (Task 1).
- Produces:
  - `FromRadio::Firmware(Firmware)`, sent by every session before `Link::Connected`. The MeshCore session sends `Firmware::MeshCore`.
  - In `src/history.rs`: `pub const MT_PUBLIC: &str = "mt:public";` and `pub fn is_meshtastic(key: &str) -> bool` (prefix `mt:`).
  - `App.firmware: Firmware` and `App::public_conv(&self) -> &'static str` (`PUBLIC` or `MT_PUBLIC`).

- [ ] **Step 1: Write the failing tests** in `src/app/tests.rs`:
  - `firmware_comes_from_the_last_history_record`: `App::new` with records ending in a `Msg` whose conv is `mt:public` gives `Firmware::Meshtastic`; ending in conv `a1b2c3d4e5f6` gives MeshCore; no records gives MeshCore.
  - `rows_show_only_the_current_firmware`: history with messages in `public`, `a1b2c3d4e5f6`, `mt:public`, `mt:!0badcafe`; under MeshCore the row convs are `public` and `a1b2c3d4e5f6`; after `apply(FromRadio::Firmware(Firmware::Meshtastic))` they are `mt:public` and `mt:!0badcafe`, the public row first and labeled "# Public".
  - `switching_firmware_clears_the_radio_state`: with nodes, battery and `me` set under MeshCore, `apply(FromRadio::Firmware(Firmware::Meshtastic))` leaves `nodes` empty and `battery`, `me` at `None`; applying the same firmware again changes nothing.
  - `enter_on_the_meshtastic_public_row_sends_public`: firmware Meshtastic, public row selected, typing "hi" and Enter gives `ToRadio::SendPublic { text: "hi" }`.
  - `meshtastic_pings_any_node`: firmware Meshtastic, a `NodeKind::Chat` node selected, `p` gives `ToRadio::Ping`; under MeshCore the same gives the notice "PING: repeaters and rooms only" (unchanged).
  - In `src/ui/tests.rs`, `header_names_the_firmware`: connected under Meshtastic, the Normal header contains "(Meshtastic)" and the Hacker header contains "MESHTASTIC".

- [ ] **Step 2: Run** `cargo test` and see these fail.

- [ ] **Step 3: Implement.** `App::new` sets `firmware` from the last record's key (`Msg.conv` or `Ack.tag`). `apply(Firmware(f))` with `f != self.firmware` clears `nodes`, `battery`, `me`, `pings` and sets `selected = 0`. `rows()` uses `public_conv()` for the public row and skips history conversations whose `is_meshtastic` differs from the current firmware. Every comparison with `PUBLIC` in `keys.rs`, `contacts.rs` and `conversation.rs` uses `app.public_conv()`; the private-channel refusal in `send()` tests `conv.contains("channel:")`. The ping key accepts any node under Meshtastic. `card.rs` shows the ping section for every Meshtastic node (the `NodeKind::Chat` early return stays for MeshCore only). Headers: Normal `Connected to {name} ({firmware})`, Hacker `>> LINK ESTABLISHED :: {name} :: {FIRMWARE}` in capitals.

- [ ] **Step 4: Run** `cargo test` and `cargo clippy --all-targets`: all pass.

- [ ] **Step 5: Commit** `feat(app): Show only the detected firmware's conversations`.

### Task 3: Meshtastic BLE transport

**Files:**
- Create: `src/radio/meshtastic/mod.rs`, `src/radio/meshtastic/transport.rs`
- Modify: `src/radio/mod.rs` (`mod meshtastic;`), `Cargo.toml`
- Test: `src/radio/meshtastic/tests.rs`

**Interfaces:**
- Consumes: `ble_manager()`, `MESHTASTIC_SERVICE` (Task 1).
- Produces, in `transport.rs`:
  - UUIDs `TO_RADIO = 0xf75c76d2_129e_4dad_a1dd_7866124401e7` (write), `FROM_RADIO = 0x2c55e69e_4993_11ed_b878_0242ac120002` (read), `FROM_NUM = 0xed9da18c_a800_4f66_a670_aa7547e34453` (notify).
  - `#[derive(Debug)] pub struct RepairNeeded(pub String);` implementing `Display` as `"{0} does not offer the Meshtastic service: re-pair the radio with bluetoothctl"` and `std::error::Error`.
  - `pub struct Transport` with `pub async fn connect(name: &str) -> anyhow::Result<Transport>`, `pub async fn send(&self, msg: &protobufs::ToRadio) -> anyhow::Result<()>`, `pub async fn drain(&self) -> anyhow::Result<Vec<protobufs::FromRadio>>`, `pub async fn notified(&mut self) -> bool` (false once the notification stream ends), `pub async fn disconnect(&self)`.
  - `pub(super) async fn read_until_empty<F, Fut>(read: F) -> anyhow::Result<Vec<Vec<u8>>>` where `F: FnMut() -> Fut, Fut: Future<Output = anyhow::Result<Vec<u8>>>`.

- [ ] **Step 1: Write the failing tests** in `src/radio/meshtastic/tests.rs`:
  - `read_until_empty_stops_at_the_first_empty_read`: a fake reader returning `[1]`, `[2, 3]`, `[]`, `[9]` gives `vec![vec![1], vec![2, 3]]` and is called 3 times.
  - `read_until_empty_passes_errors_on`: a reader failing on its first call gives `Err`.
  - `repair_needed_says_what_to_do`: `RepairNeeded("Meshtastic_d940".into()).to_string()` contains "re-pair the radio with bluetoothctl".

- [ ] **Step 2: Run** `cargo test meshtastic` and see it fail.

- [ ] **Step 3: Implement.** `connect`: find the peripheral whose `local_name` or address equals `name` through `ble_manager()`, connect, `discover_services()`; without `MESHTASTIC_SERVICE`, disconnect and return `RepairNeeded`; else subscribe to `FROM_NUM` and keep `peripheral.notifications()` filtered on it. `send` writes `msg.encode_to_vec()` with `WriteType::WithResponse` (no 4-byte header over BLE). `drain` decodes each chunk of `read_until_empty` with `prost::Message::decode`. BlueZ negotiates the MTU.

- [ ] **Step 4: Run** `cargo test` and `cargo clippy --all-targets`: all pass.

- [ ] **Step 5: Commit** `feat(radio): Add the Meshtastic Bluetooth transport`.

### Task 4: Meshtastic conversions

**Files:**
- Create: `src/radio/meshtastic/convert.rs`
- Modify: `src/radio/mod.rs` (`Ping::Trace`, `MyRadio.preset`), `src/radio/convert.rs` (`preset: None`), `src/geo.rs`, `src/ui/card.rs`, `src/ui/radio_card.rs`
- Test: `src/radio/meshtastic/tests.rs`, `src/ui/tests.rs`

**Interfaces:**
- Produces:
  - `Ping::Trace { rtt_ms: u64, hops: u8, snr: Option<f32> }`; `MyRadio.preset: Option<String>`; `geo::pos_from_e7(lat_i: i32, lon_i: i32) -> Option<Pos>` (1e-7 degrees, `(0, 0)` gives `None`).
  - In `convert.rs`: `node_key(num: u32) -> String` (`mt:!{num:08x}`), `node_num(key: &str) -> Option<u32>`, `ack_tag(id: u32) -> String` (`mt:{id:08x}`), `kind(role: Role) -> NodeKind`, `node(info: &NodeInfo) -> Node`, `battery(m: &DeviceMetrics) -> Option<Battery>`, `text_msg(p: &MeshPacket, text: &str, my_num: u32, names: &HashMap<u32, String>) -> Msg`, `my_radio(my_num: u32, me: Option<&NodeInfo>, meta: Option<&DeviceMetadata>, lora: Option<&config::LoRaConfig>) -> MyRadio`, `ack_for(p: &MeshPacket, routing: &Routing, sent: &HashMap<u32, u32>) -> Option<String>`, `trace(route: &RouteDiscovery, rtt_ms: u64) -> Ping`. Types come from `meshtastic::protobufs`.

- [ ] **Step 1: Write the failing tests** in `src/radio/meshtastic/tests.rs`:
  - `node_keys_round_trip`: `node_key(0x0badcafe) == "mt:!0badcafe"`, `node_num("mt:!0badcafe") == Some(0x0badcafe)`, `node_num("a1b2c3d4e5f6") == None`, `ack_tag(0x1234) == "mt:00001234"`.
  - `roles_map_to_node_kinds`: ROUTER, ROUTER_LATE, ROUTER_CLIENT and REPEATER give Repeater; SENSOR gives Sensor; CLIENT, CLIENT_MUTE and TRACKER give Chat.
  - `node_from_node_info`: num `0x0badcafe`, long name "Bob", `latitude_i` 448266630, `longitude_i` -5571380, `last_heard` 1793451000, `hops_away` Some(2) gives prefix `mt:!0badcafe`, name "Bob", pos (44.826663, -0.557138), heard 1793451000, `path_len` 2, empty path; without a user the name is `!0badcafe`; without `hops_away`, `path_len` is -1.
  - `battery_level_101_is_full`: `battery_level` 101 and voltage 4.2 give pct 100 and mv 4200; no `battery_level` gives `None`.
  - `text_goes_to_the_right_conversation`: `to == my_num` gives conv `mt:!<from>`; `to == 0xffffffff` on channel 0 gives `mt:public`; on channel 2 gives `mt:channel:2`; `from` is the known name, else `!<from>`.
  - `my_radio_shows_the_preset`: LoRa config with `use_preset`, region EU_868 and preset LONG_FAST gives `preset == Some("EU_868 LONG_FAST")` and `freq_khz == 0`.
  - `only_the_destination_acks`: with `sent = {7: 0x0badcafe}`, a Routing NONE answering request 7 from `0x0badcafe` gives `Some("mt:00000007")`; the same from `0x0000beef` (a relay) gives `None`; error `MAX_RETRANSMIT` gives `None`; request 8 gives `None`.
  - `trace_counts_hops`: a route of two node numbers with `snr_back` `[22, 18]` (quarter dB) and rtt 1500 gives `Ping::Trace { rtt_ms: 1500, hops: 2, snr: Some(4.5) }`.
  - In `src/ui/tests.rs`, `cards_show_meshtastic_results`: a node card with `Ping::Trace { rtt_ms: 1500, hops: 2, snr: Some(4.5) }` shows "1.5 s", "2 HOPS" and "4.5 dB"; a radio card with `preset` "EU_868 LONG_FAST" shows that text.

- [ ] **Step 2: Run** `cargo test` and see these fail.

- [ ] **Step 3: Implement** the functions. `battery`: pct is `battery_level` capped at 100, mv is `voltage * 1000`, storage fields `None`. `my_radio`: name from the user's long name, model from `hw_model` (`as_str_name()`), version from `firmware_version`, `freq_khz` from `override_frequency` (MHz) when set, bandwidth, spreading factor and coding rate from the config when `use_preset` is false, `tx_power` from the config, `max_tx_power` 0, `shares_position` when my node has a position. `trace`: `hops` is the route length, `snr` the last `snr_back` (else the last `snr_towards`) divided by 4. The card shows `TRACE  1.5 s  2 HOPS  SNR 4.5 dB`; the radio card shows a `PRESET` line when `preset` is set and the frequency lines only when `freq_khz` is not 0.

- [ ] **Step 4: Run** `cargo test` and `cargo clippy --all-targets`: all pass.

- [ ] **Step 5: Commit** `feat(radio): Convert Meshtastic data to our types`.

### Task 5: Meshtastic session

**Files:**
- Create: `src/radio/meshtastic/session.rs`
- Modify: `src/radio/task.rs`, `src/radio/meshtastic/mod.rs`
- Test: `src/radio/meshtastic/tests.rs`

**Interfaces:**
- Consumes: `Transport`, `RepairNeeded` (Task 3), the Task 4 conversions, `session::save` and `HistoryWriteFailed` (existing), `config::{CHECK_EVERY, CONTACTS_TIMEOUT, PING_TIMEOUT}`.
- Produces:
  - `#[derive(Default)] pub(in crate::radio) struct Snapshot { pub my_num: u32, pub nodes: HashMap<u32, NodeInfo>, pub lora: Option<config::LoRaConfig>, pub meta: Option<DeviceMetadata>, pub pending: Vec<MeshPacket> }`.
  - `pub(in crate::radio) fn absorb(snap: &mut Snapshot, msg: protobufs::FromRadio, config_id: u32) -> bool` (true once `ConfigCompleteId(config_id)` arrives).
  - `pub(in crate::radio) async fn handshake(t: &mut Transport) -> anyhow::Result<Snapshot>`.
  - `pub(in crate::radio) async fn session(t: &mut Transport, name: &str, history: &Path, tx: &Sender<FromRadio>, cmds: &mut UnboundedReceiver<ToRadio>) -> anyhow::Result<bool>`, same contract as the MeshCore `session`.

- [ ] **Step 1: Write the failing tests:**
  - `packets_during_the_handshake_are_kept`: `absorb` with `MyInfo` (num 5), a `NodeInfo` (num 9), a text `Packet`, then `ConfigCompleteId(42)` with `config_id` 42: the first three return false, the last true; `my_num == 5`, `nodes` has 9, `pending` has the packet.
  - `another_config_id_does_not_end_the_handshake`: `ConfigCompleteId(41)` with `config_id` 42 returns false.

- [ ] **Step 2: Run** `cargo test handshake` and see it fail.

- [ ] **Step 3: Implement `absorb` and `handshake`.** `handshake` sends `ToRadio { want_config_id }` with a non-zero id, then drains and absorbs until done, waiting on `notified()` between drains; past `CONTACTS_TIMEOUT` it fails with "config handshake timed out".

- [ ] **Step 4: Implement `session`.**
  - **Start:** handshake, then send `Firmware(Meshtastic)`, `MyRadio`, `SelfPos` (my node's position), `Link::Connected(name)`, `Nodes`, and `Battery` when my node has metrics. Set the clock with an `AdminMessage` `set_time_only(now)` in a packet to `my_num` on port `ADMIN_APP` (a failure is only logged). Then handle `pending` like live packets.
  - **Packet ids:** a `u32` counter seeded from the clock's nanoseconds, never 0.
  - **Commands:**
    - `SendDirect { prefix }`: to `node_num(prefix)` with `want_ack` on `TEXT_MESSAGE_APP`; save the `Msg` with `ack: Some(ack_tag(id))` and remember `id -> destination` in `sent`. A prefix that is not a Meshtastic key fails with `cmd.failed("not a Meshtastic node")`.
    - `SendPublic`: to `0xffffffff`, channel 0; save under `mt:public`.
    - `Ping { prefix }`: traceroute packet (`TRACEROUTE_APP`, empty `RouteDiscovery`, `want_response`); remember `id -> (prefix, Instant)`.
  - **Live packets**, by port:
    - `TEXT_MESSAGE_APP`: `text_msg`, then `save`;
    - `NODEINFO_APP`, `POSITION_APP`, `TELEMETRY_APP`: update the node map and set `nodes_stale` (telemetry from `my_num` also updates the battery);
    - `ROUTING_APP`: `ack_for`, then `save(Record::Ack)`;
    - `TRACEROUTE_APP` answering a remembered id: `Pong` with `trace(...)`.
    
    A `NodeInfo` arriving outside the handshake also updates the map.
  - **Every `CHECK_EVERY`:** send `ToRadio { heartbeat }`; if the write fails, return `Ok(true)` (link lost). Send `Nodes` if `nodes_stale`. Turn traceroutes older than `PING_TIMEOUT` into `Ping::Failed("no answer")`.
  - **End:** `notified()` returning false means the link is lost: return `Ok(true)`. A closed command channel returns `Ok(false)`.

- [ ] **Step 5: Wire `task.rs`.** A Meshtastic candidate connects with `Transport::connect` and runs this `session`, then `disconnect()` and `ble_disconnect(name)`. A `RepairNeeded` error is handled like `HistoryWriteFailed`: `Link::Lost("<message> - fix it, then restart")` and the task stops.

- [ ] **Step 6: Run** `cargo test` and `cargo clippy --all-targets`: all pass.

- [ ] **Step 7: Check on hardware** (M5 under Meshtastic): the TUI connects, the header says Meshtastic, the nodes and the battery show, a public message is saved under `mt:public`, `p` on a node gives a trace or "no answer".

- [ ] **Step 8: Commit** `feat(radio): Run a Meshtastic session`.

### Task 6: Probe, documentation, final checks

**Files:**
- Modify: `src/probe.rs`, `README.md`, `ROADMAP.md`
- Add: `docs/meshtastic.md`, `docs/meshtastic-plan.md` (already written)

- [ ] **Step 1: `--probe` for Meshtastic:** connect, `handshake`, print `device`, `firmware Meshtastic <version>`, `battery`, `pos` and `nodes <count>` in the existing format, then disconnect. MeshCore output stays the same, plus a `firmware MeshCore` line.

- [ ] **Step 2: Check on hardware:** `loraterm --probe` with the M5 under Meshtastic prints the five lines.

- [ ] **Step 3: Docs.** README: detection, pairing for both firmwares, what differs under Meshtastic (traceroute, nodes found automatically, conversations kept apart, the clock set at each connection). ROADMAP: tick the done items of "Meshtastic support".

- [ ] **Step 4: MeshCore regression check**, with the owner's go-ahead: restore the MeshCore backup on the M5, re-pair, then check `--probe` and the TUI (contacts, public channel, a ping to a repeater).

- [ ] **Step 5: Commit** `docs: Describe Meshtastic support`.
