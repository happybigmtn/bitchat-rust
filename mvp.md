BitCraps MVP (LAN TCP) — Implementation Playbook

Objective
- Deliver a working, zero‑stub MVP that lets multiple nodes on the same LAN play craps using TCP transport with discovery via manual addresses (or mDNS optional), with end‑to‑end create/join/bet flows propagating over the network.
- Scope excludes BLE for MVP to avoid platform stubs; feature‑gate BLE code paths so they don’t affect the default build.

MVP Scope (Feature Flags and Defaults)
- Default build: TCP‑only networking enabled; BLE disabled by default.
- Optional features: `tls` may remain enabled for TCP; if TLS complicates handshake, allow fallback to plain TCP for MVP.
- No use of any “simulated” transports or placeholder sending code in the active code path.

Deliverables
- Functional TCP transport with real read/write, identity handshake, and event emission.
- CLI flags to listen/connect over TCP, and to optionally disable BLE.
- App wiring to start TCP listener and connect to peers.
- Mesh event loop processes TransportEvents and routes `BitchatPacket`s end‑to‑end.
- Commands `create-game`, `join-game`, `bet` operate across peers.
- Documentation and test/validation instructions below executed and green.

Key Risks Addressed
- Remove simplified/stubbed paths in the chosen (TCP) runtime path.
- Ensure peer identity is negotiated (no zeroed PeerId).
- Ensure message framing and backpressure are correct (length‑prefixed frames; per‑connection reader tasks).

Implementation Details (By Area)

1) Transport: TCP (Agent A)
- Files: `src/transport/tcp_transport.rs`, `src/transport/mod.rs`, `src/transport/traits.rs`

- Handshake spec (Hello v1):
  - Frame: 4‑byte big‑endian length N, followed by N bytes bincode‑encoded Hello struct.
  - `struct Hello { version: u16 = 1, peer_id: PeerId }`.
  - Client connect(): immediately sends Hello; server accept(): reads Hello first, then responds with its Hello.
  - If version mismatch, close connection with error.

- Framing for data messages:
  - After handshake, all messages are length‑prefixed frames: `[u32 len][bytes payload]` where `payload` is the raw serialized `BitchatPacket` bytes (via `BitchatPacket::serialize()`), or a small control code for keepalive: `[0,0,0,1][0]` (single byte 0 = keepalive).

- Data structures and concurrency:
  - Change `TcpConnection.stream` to be wrapped in `tokio::sync::Mutex` to allow mutable read/write (`ConnectionStream` inside a `Mutex`), or split into separate reader task (owned) plus a `mpsc::Sender<Vec<u8>>` write channel.
  - Maintain `connections: Arc<RwLock<HashMap<PeerId, TcpConnection>>>` as today, but actually populate `peer_id` from the Hello handshake (remove zeroed PeerId).
  - Implement `is_connected(&self, peer_id)` and `connected_peers(&self)` properly by inspecting `connections` (note: trait requires sync methods; compute from a cached `DashMap` or use `parking_lot::RwLock` to avoid `.await` in these methods).

- Listening/accept loop:
  - In `accept_loop`, after accepting, perform server‑side handshake: read Hello with timeout (e.g., 5s), reply with server Hello.
  - Start a per‑connection reader task: continuously read length‑prefixed frames; on frame receipt:
    - If keepalive frame (len==1, byte==0): update `last_activity`.
    - Else, emit `TransportEvent::DataReceived { peer_id, data }` where `data` is the received packet bytes.
  - On reader error/EOF: remove connection from map; emit `TransportEvent::Disconnected`.

- Connect flow:
  - In `connect()`, establish TCP/TLS, send client Hello, await server Hello, set peer_id, insert into `connections`, spawn reader task as above.
  - Emit `TransportEvent::Connected { peer_id, address }` once handshake succeeds.

- Send path:
  - Implement `send()` to locate connection by `peer_id` and write a length‑prefixed frame. The current `send_via_connection` is a stub; replace with actual writes on the `ConnectionStream` (match and call `write_all`/`flush`). Update `message_count`/`last_activity`.

- Health monitor / keepalive:
  - In `start_health_monitor()`, send a keepalive frame (len=1, byte=0) if no write activity in `keepalive_interval`. Don’t println! in production path; use `log`.

- Events wiring:
  - Ensure `next_event()` returns events from `event_receiver` that reader/connection code sends. Remove placeholder event paths in coordinator that don’t reflect real wire activity.

- Error handling and no unwraps:
  - Replace all unwrap/expect in non‑test paths with proper errors.

Acceptance for Transport:
  - Unit: Add tests for handshake (client/server loopback), frame round‑trip, disconnect behavior.
  - Manual: Two processes locally; one listens, one connects; DataReceived events observed in Mesh; `send()` delivers to peer.

2) Transport Coordinator (Agent A)
- Files: `src/transport/mod.rs`

- Listening:
  - Add `pub async fn listen_tcp(&self, addr: std::net::SocketAddr) -> Result<()>` that: ensures `tcp_transport` exists; calls `listen(TransportAddress::Tcp(addr))` on it; records active transport type.
  - Update `start_listening()` to also start TCP when `tcp_transport` is present (don’t require enhanced BLE). Keep BLE branches intact but not required for MVP.

- Connect API:
  - Add `pub async fn connect_tcp(&self, addr: std::net::SocketAddr) -> Result<PeerId>` that proxies to `tcp_transport.connect(TransportAddress::Tcp(addr))` and returns PeerId.

- Event pump:
  - `next_event()` already exposes events; ensure that underlying TCP events are being pushed through (reader tasks in `TcpTransport` send events to its event_sender; Coordinator should subscribe or wrap those). If Coordinator maintains its own `event_receiver`, bridge the TCP transport’s events into Coordinator’s `event_sender` or standardize on Coordinator’s channel (recommended: pass Coordinator’s `event_sender` into `TcpTransport::new(config)` so TCP posts directly to Coordinator).

Acceptance for Coordinator:
  - Manual: `listen_tcp` binds and logs; `connect_tcp` connects and Mesh sees `Connected` followed by DataReceived when sending.

3) App Wiring + CLI (Agent H)
- Files: `src/app_config.rs`, `src/main.rs`, `src/app_state.rs`

- CLI additions in `app_config.rs`:
  - Global flags:
    - `--listen-tcp <ADDR>`: Optional, default `0.0.0.0:8989` if provided without value.
    - `--connect-tcp <ADDR>...`: Repeatable to connect to one or more peers on startup.
    - `--no-ble`: Optional; default to disabling BLE for MVP.
  - Keep current subcommands; Start command should honor these flags.

- Main startup (`src/main.rs`):
  - Pass new flags into `AppConfig` or into `BitCrapsApp::new` params.

- App state (`src/app_state.rs`):
  - Replace Step 4 (Bluetooth only) with conditional TCP:
    - Always create `TransportCoordinator`.
    - If `--listen-tcp` present, call `enable_tcp(...)` (to init transport) + `listen_tcp(addr)`.
    - If one or more `--connect-tcp` provided, iterate and call `connect_tcp(addr)`.
    - Gate BLE init behind `--no-ble` (default true for MVP). Keep BLE code compiled but not used.
  - Ensure after `mesh_service.start()` the `start_message_processing()` loop in `mesh/mod.rs` receives events and processes.

Acceptance for App/CLI:
  - Manual: `bitcraps start --listen-tcp :8989` on node A, `bitcraps start --connect-tcp 192.168.1.10:8989` on node B connects successfully.

4) Mesh Integration (Agent B)
- Files: `src/mesh/mod.rs`

- Confirm data flow:
  - `start_message_processing()` already consumes `transport.next_event()` and parses `BitchatPacket` from bytes; keep this path.
  - Ensure `broadcast_packet()` calls into Coordinator’s `broadcast_packet()` which sends bytes to all connected peers. Confirm serialization uses `BitchatPacket::serialize()`; it does.
 - For direct sends, `send_packet()` routes using `route_packet_to_peer()`; routing TLV helpers are simplified but acceptable for LAN MVP (single hop). No code path should panic.

MVP Test Gating and Commands

- Features added:
  - `mvp`: enables focused MVP tests only.
  - `legacy-tests`: gates legacy, comprehensive tests that rely on older APIs.
  - `legacy-examples`: gates older examples.

- Run MVP tests:
  - `cargo test --features mvp --test mvp_tcp_e2e`
  - `cargo test --features mvp --test mvp_game_discovery`

- Run legacy tests when needed:
  - `cargo test --features legacy-tests`

- Notes:
  - Some large legacy tests are gated to keep MVP CI green.
  - BLE remains off at runtime by default for MVP; TCP is primary path.

- Clean logging and errors; no placeholder “would be implemented” in executed paths.

Acceptance for Mesh:
  - Manual: When a peer connects, Mesh logs `Peer connected`. Broadcasting a ping from node A is received on node B and logs `MessageReceived`.

5) Protocol/Game Flow (Agent C/D)
- Files: `src/protocol/runtime/game_lifecycle.rs`, `src/commands.rs`

- Peer‑targeted join messaging (minimal viable):
  - In `broadcast_player_join_request()`, replace current “would be sent” log with actual packet creation and broadcast (already uses `mesh_service.broadcast_packet(packet)` in some paths). For MVP, it’s sufficient that game creation/join events are broadcast to all peers; participants filter by game_id.
  - Password validation TODO can be deferred for MVP; do not block join flow if not set.

- Command flows:
  - `create_game` already builds a game and broadcasts a create packet. Ensure receiving side updates local game list. If not currently implemented, add a handler in Mesh processing to call into a game manager to add discovered games (minimal: log discovered game and allow `join-game` using the provided ID).

Acceptance for Protocol/Game:
  - Manual: Node A `create-game` -> Node B `games` shows the game, or at minimum Node B can `join-game <ID>` and both can place bets without errors; state remains consistent for simple cases.

6) Docs and Feature Gates (Agent J)
- Default features in `Cargo.toml`:
  - Ensure BLE optional and off by default for MVP: add a feature `ble` and guard BLE modules (if not already) with `#[cfg(feature = "ble")]` for platform‑specific crates. Do not break existing code; just ensure the default path compiles/runs TCP only.
- Update README/`codex.md` with MVP run instructions (see below).

Validation Plan

Automated
- Unit tests (Transport):
  - Handshake success and version‑mismatch failure.
  - Frame encode/decode round‑trip.
  - Reader task produces `DataReceived` on incoming frame; disconnect on EOF.

- Integration tests (2 processes or multi‑runtime):
  - Start one TCP listener in background task; connect client; exchange ping `BitchatPacket` and assert receipt.
  - Create game on server; client receives creation packet (or at least can join via known ID) and place a bet; server receives bet packet.

Manual
- Node A: `bitcraps start --listen-tcp :8989`
- Node B: `bitcraps start --connect-tcp 192.168.1.10:8989`
- A: `bitcraps create-game --buy-in 100` (copy printed Game ID)
- B: `bitcraps join-game --game-id <ID>`
- A/B: `bitcraps bet --game-id <ID> --bet-type pass --amount 10`
- Observe logs on both nodes for received packets and consistent state.

Performance/Health (MVP targets)
- p95 end‑to‑end packet latency on LAN < 50ms.
- No panics in runtime path; no unwraps in transport/mesh active code.

Post‑MVP (Not Blocking)
- mDNS discovery for TCP peers on LAN.
- TLS certificate management for authenticated peers.
- BLE implementations per platform behind `ble` feature.
- Constant‑time password validation for protected games.

Concrete Patch Points (Checklist)
- Transport/TCP
  - [ ] Replace zeroed PeerId with handshake‑derived PeerId in `connect()` and `accept_loop()`.
  - [ ] Implement `send_via_connection()` actual writes via `ConnectionStream`.
  - [ ] Add per‑connection reader task that emits `TransportEvent::DataReceived`.
  - [ ] Implement `is_connected()` and `connected_peers()` using connection map.
  - [ ] Emit `Disconnected` on errors/EOF; remove from map.

- Coordinator
  - [ ] Add `listen_tcp()` and `connect_tcp()` helpers; include TCP in `start_listening()` if configured.
  - [ ] Ensure events from TCP are surfaced via `next_event()` (bridge channels if needed).

- App/CLI
  - [ ] Add `--listen-tcp`, `--connect-tcp ...`, `--no-ble` flags in `src/app_config.rs`.
  - [ ] Wire flags in `src/main.rs` and `src/app_state.rs` to init TCP and connect peers.

- Protocol/Game
  - [ ] Ensure create/join/bet packets are broadcast and processed (minimal path).
  - [ ] Add a simple in‑memory “discovered games” list on receipt of a game creation packet.

- Features/Build
  - [ ] Ensure default feature set builds without BLE; guard BLE modules with feature flag if necessary.

Runbook
- Build: `cargo build --release`
- Start (listener): `./target/release/bitcraps start --listen-tcp :8989 --no-ble`
- Start (connector): `./target/release/bitcraps start --connect-tcp 192.168.1.10:8989 --no-ble`
- Create/join/bet per Manual Validation above.

Owner Map (Agents)
- Agent A (Transport/Coordinator): TCP handshake, read/write loops, coordinator listen/connect, event bridging.
- Agent B (Mesh): Verify event loop/process flow; minor fixes to routing for single hop.
- Agent C (Protocol): Verify `BitchatPacket` serialization/deserialization and add any missing helpers.
- Agent D (Security): Audit for unwraps/panics in active path; ensure handshake sanity (version, minimal validation).
- Agent H (CLI/App): Flags, wiring, and UX messages.
- Agent J (Docs/Ops): Update docs and ensure default build instructions reflect TCP‑only MVP.

Exit Criteria (All Must Be True)
- Two nodes on LAN can connect via TCP, broadcast/receive packets, create/join a game, and place bets with consistent state.
- No placeholder/stubbed code in the active (TCP + mesh) path; no panics/unwraps along these paths.
- Documentation in this file followed to reproduce the working result.
