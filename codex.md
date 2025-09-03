BitCraps Project Plan (Agents, Milestones, Validation)

Summary
- Goal: Ship a stable, secure decentralized craps casino over a Bluetooth-first mesh with CRAP tokens and a cross-platform SDK.
- Scope covers: transport/mesh, consensus/gameflow, protocol, security, storage, treasury, SDK/mobile, UI, performance, monitoring, and compliance.
- This plan assigns agents, defines milestones with concrete deliverables, and specifies validation gates required before closing each milestone.

Architecture Snapshot (from repo survey)
- Transport: `src/transport/*` (BLE platform backends, NAT traversal, TCP, Kademlia, PoW identity, keystore, coordinator, MTU discovery, bounded queues).
- Mesh: `src/mesh/*` (service orchestrator, deduplication, advanced routing, game sessions, anti-cheat, consensus handler, gateway, message queue, resilience).
- Protocol: `src/protocol/*` (binary/TLV, compression, consensus, runtime, craps game logic, versioning, state sync, reputation; optimized/zero-copy paths).
- Session: `src/session/*` (Noise/forward secrecy, lifecycle/state; session limits).
- Security: `src/security/*` (input validation, rate limiting, DoS protection, quotas, constant-time utilities, security events).
- Storage: `src/storage/*` + `src/persistence.rs` (persistent store, encryption, PostgreSQL backend optional, rusqlite via tokio-rusqlite).
- Treasury/Token: `src/treasury/*`, `src/token/*`, `src/contracts/*` (AMM, reserves, governance, staking, on-chain bridges).
- SDK: `src/sdk/*` + UDLs (client, codegen, game dev kit; UniFFI bindings; mobile bridges in `src/mobile/`).
- UI: CLI/TUI/mobile views in `src/ui/*` and mobile components in `src/ui/mobile/*`.
- App: `src/app.rs`, `src/app_state.rs`, `src/app_config.rs`, `src/main.rs` wiring.
- Utilities: `src/utils/*` (loop budget, adaptive intervals, lock ordering, task tracker, buffers, timeout, etc.).
- Tests: extensive under `tests/*` (security, transport, integration, gaming, performance, compliance, mobile, consensus).

Agents and Ownership
- Agent A – Transport/Networking: `src/transport/*`, `src/discovery/*`, `src/coordinator/*`.
- Agent B – Mesh/Consensus/Game Flow: `src/mesh/*`, `src/gaming/*`, `src/protocol/consensus*`, `src/protocol/state_sync*`.
- Agent C – Protocol/Serialization: `src/protocol/*` (binary/optimized/zero-copy, versioning, packets).
- Agent D – Security/Crypto: `src/security/*`, `src/session/*`, cryptographic usage in `src/crypto/*`.
- Agent E – Storage/Persistence: `src/storage/*`, `src/persistence.rs`, DB backends.
- Agent F – Treasury/Token/Economics: `src/treasury/*`, `src/token/*`, `src/contracts/*`.
- Agent G – SDK/Mobile: `src/sdk/*`, UDLs, `src/mobile/*`, platform bridges.
- Agent H – UI/UX: `src/ui/*` (CLI/TUI/Mobile views/components).
- Agent I – Performance/Profiling: `src/performance/*`, `src/profiling/*`, `benches/*`.
- Agent J – Monitoring/Operations/Compliance: `src/monitoring/*`, `docs/*`, `k8s/*`, `grafana/*`, `tests/compliance/*`.

Cross-Cutting Priorities and Known Risks
- Dead/unused code allowances (`#![allow(dead_code, unused_*)]`) hide drift; enforce lints gradually.
- Multi-transport complexity (BLE + TCP + NAT) increases statefulness, MTU fragmentation, and dedup logic risks.
- Security surfaces: PoW identity, Noise sessions, rate limiting, DoS/quotas, key handling.
- Economics correctness (payoffs, AMM invariants) requires precise math with `rust_decimal` and robust tests.
- Storage encryption and key lifecycle must be audited; SQLite vs Postgres parity.
- UniFFI/mobile builds can regress easily; ensure CI matrix covers platforms.

Milestone Plan with Validation Gates

M0. Baseline Hardening and CI Gates (1 week)
- Deliverables:
  - Cargo features clarified; default feature minimalism; ensure `uniffi`, `android`, `tls` gated properly.
  - CI pipeline running: fmt, clippy (deny high-impact), unit tests, doc build, benches compile.
  - Test suite triage: tag/organize slow vs fast tests and skip platform-dependent tests in default CI.
  - Crash-safe logging and panic hook are already present; route to file + STDOUT with env control.
- Validation:
  - Run: `cargo fmt --all --check`; `cargo clippy --all-targets -- -D warnings` (allowlist exceptions documented).
  - Run unit tests fast set: `cargo test --lib --bins -- --exclude-should-panic`.
  - Ensure benches compile: `cargo bench --no-run`.
  - Produce minimal build matrix doc per-platform.
  - Exit criteria: CI green on Linux; clippy baseline established with documented allow list; test runtime < 8 min.

M1. Transport Stability and Mesh Fundamentals (2–3 weeks)
- Deliverables:
  - Transport MTU/fragmentation policy finalized; bounded queues/connection pool tuned; backpressure applied.
  - Peer discovery paths validated (BLE + optional TCP). NAT traversal feature-flagged.
  - Mesh service event flow documented; dedup window and TTL tuned; heartbeat/timeout verified.
  - Routing table structure sanity; metrics on peer churn and message flow.
- Validation:
  - Unit: `tests/unit_tests/transport_module_tests.rs` and `src/transport/*` specific tests pass.
  - Integration: `tests/transport_security_tests.rs`, `tests/transport_security_comprehensive_test.rs`, `tests/multi_peer_integration.rs` pass.
  - BLE-specific: guard with feature flag; run `src/transport/ble_integration_test.rs` where supported.
  - Profiling: measure p50/p95 end-to-end message latency and message loss under 100 peers synthetic.
  - Exit criteria: <1% message loss at 100 peers synthetic; p95 < 300ms over BLE lab; no deadlocks under 30 min soak.

M2. Protocol and Session Security (2 weeks)
- Deliverables:
  - Protocol versioning and feature toggles validated end-to-end; TLV and compression interop confirmed.
  - Noise session handshake paths audited; forward secrecy enforced; key rotation policy documented.
  - Input validation hardened across API surfaces; rate limits and quotas tuned.
- Validation:
  - Unit: `tests/security/crypto_security.rs`, `tests/security/consensus_crypto_integration.rs` pass.
  - Fuzz: add `proptest` or `arbitrary` for protocol frames; run 1M cases on packet parsers (`protocol/binary`, `zero_copy`).
  - Constant-time checks: review critical compare paths (`security/constant_time.rs`).
  - Exit criteria: zero panics on malformed frames; handshake downgrade attempts detected; all security tests green.

M3. Game Flow, Fairness, and Consensus (2–3 weeks)
- Deliverables:
  - Craps game logic and payouts validated across all bet types; RNG policy with audit trail.
  - Consensus coordination for game state transitions consistent across mesh; partition recovery logic validated.
  - Anti-cheat hooks integrated with session and mesh events.
- Validation:
  - Unit/integration: `tests/gaming/*`, `tests/comprehensive_p2p_game_flow_test.rs`, `tests/consensus_test.rs`, `tests/adversarial_consensus_test.rs` pass.
  - Economics correctness: deterministic seeds reproduce outcomes; edge-case dice paths covered.
  - Chaos: `tests/security/byzantine_tests.rs`, `tests/security/chaos_engineering.rs` for adversarial peers.
  - Exit criteria: no divergence in state across 50 simulated peers; payouts match spec for 100k simulated rolls.

M4. Treasury, Token Economics, and Contracts (2 weeks)
- Deliverables:
  - Treasury AMM and reserves logic validated; staking/governance hooks; off-chain token ledger consistency.
  - Contract interfaces optional and feature-gated (`ethers`, `web3`, `bitcoin`).
- Validation:
  - Unit: `tests/unit_tests/token_tests.rs`, `tests/token_economics_comprehensive_test.rs` pass.
  - Decimal math invariants (no precision loss for payouts, AMM curves preserve invariants).
  - Exit criteria: ledger/P&L reconcile after 100k simulated sessions; invariants hold across stake/unstake flows.

M5. Storage, Encryption, and Persistence (1–2 weeks)
- Deliverables:
  - Encrypted persistent storage; key lifecycle documented; SQLite default, Postgres optional.
  - Backup/restore and migration path validated; WAL/backup for rusqlite configured.
- Validation:
  - Integration: `tests/database_*` and `tests/database_comprehensive_test.rs` pass.
  - Fault injection: kill during write, ensure recovery without corruption.
  - Exit criteria: zero data loss on crash tests; restore replays consistent states; Postgres feature passes parity tests.

M6. SDK and Mobile Integrations (2–3 weeks)
- Deliverables:
  - UniFFI codegen stable; Android/iOS bridges functional; example apps compile.
  - SDK ergonomics: client flows for discovery, join/create game, bet placement.
- Validation:
  - Tests: `tests/mobile/*` and `src/transport/*_ble.rs` guarded by platform flags.
  - Build: Android AAR/iOS framework build succeeds; smoke test on device for discovery + join + single round.
  - Exit criteria: SDK quickstarts run; API stability notes published; mobile integration tests green on CI matrix.

M7. UI/UX and Operator Experience (1 week)
- Deliverables:
  - CLI/TUI flows polished: start/join/bet/stats/ping; errors actionable.
  - Monitoring: Prometheus metrics and `monitoring` dashboards wired; health checks exposed.
- Validation:
  - Snapshot tests for CLI output; latency/peer metrics visible in Grafana locally.
  - Exit criteria: Operators can observe peers, routes, messages, and game KPIs live.

M8. Performance, Soak, and Production Readiness (2 weeks)
- Deliverables:
  - Loop budget adherence, adaptive interval tuning; memory pool health; lock ordering sanity.
  - Soak tests and resource limits verified; deployment docs hardened (Docker, k8s/helm).
- Validation:
  - Benches: `benches/*` run with reports; target throughput/latency budgets documented.
  - Soak: 8-hour multi-peer simulation; no memory leaks; p95 latency steady-state.
  - Compliance: `tests/compliance/*` pass; security hardening report revisited.
  - Exit criteria: Meets defined SLOs; deployment checklist satisfied; incident runbooks drafted.

Execution Details per Agent

Agent A – Transport/Networking
- Tasks: MTU discovery tuning, bounded queues sizing, connection pool limits, NAT traversal behind feature, Kademlia routes sanity, PoW identity gating.
- Tests to own: `tests/transport_*`, `src/transport/*_test.rs`, BLE integration guarded.
- Metrics: p50/p95 one-way latency, message drop %, reconnection rate.

Agent B – Mesh/Consensus/Game Flow
- Tasks: finalize `MeshService` event model, dedup window, heartbeat/timeout; integrate anti-cheat and game session lifecycle; consensus handler in `protocol/consensus*`.
- Tests to own: `tests/consensus_test.rs`, `tests/adversarial_consensus_test.rs`, full `tests/comprehensive_*`.
- Metrics: consensus round trip, reorg/partition recovery time, duplicate drop rate.

Agent C – Protocol/Serialization
- Tasks: verify TLV schema, zero-copy/optimized paths equivalence, version negotiation and feature gating, compression ratios vs CPU.
- Tests to own: protocol unit tests, add proptests for frame encode/decode and fuzz boundaries.
- Metrics: decode failure rate under fuzz, compression ratio, CPU time per frame.

Agent D – Security/Crypto
- Tasks: audit key handling, `constant_time` usage, input validation, rate limiting and quotas; Noise handshake and rekey rotation; DoS protections.
- Tests to own: `tests/security/*` plus targeted unit tests for validators.
- Metrics: handshake failure modes coverage, throttled request %, suspected DoS events.

Agent E – Storage/Persistence
- Tasks: encryption at rest, backup/restore, migration; SQLite vs Postgres parity; recovery on crash.
- Tests to own: `tests/database_*`; write kill/restart harness.
- Metrics: write latency, WAL growth, recovery time, data loss incidents.

Agent F – Treasury/Token/Economics
- Tasks: AMM/reserve invariants, payout math, staking flows, governance hooks; feature-gate external chains.
- Tests to own: token/treasury unit and integration; long-run Monte Carlo.
- Metrics: invariant violations, rounding errors, reconciliation mismatches.

Agent G – SDK/Mobile
- Tasks: unify UniFFI codegen, Android/iOS bridges, example apps; simplify client API for core flows; docs.
- Tests to own: `tests/mobile/*`, smoke tests per platform; build scripts.
- Metrics: build success rate, device discovery success, API call latency.

Agent H – UI/UX
- Tasks: polish CLI/TUI flows, error messages, non-interactive mode; TUI widgets stability; help/docs.
- Tests to own: snapshot tests for CLI/TUI outputs; manual scripts for flows.
- Metrics: task completion time, user error rate, crash-free sessions.

Agent I – Performance/Profiling
- Tasks: instrument loop budget, adaptive intervals, lock ordering; memory pool tuning; CPU/mem profiles.
- Tests to own: benches + load tests; flamegraphs.
- Metrics: CPU%, RSS, GC/alloc churn, lock contention.

Agent J – Monitoring/Operations/Compliance
- Tasks: wire Prometheus, dashboards; deployment scripts; revisit compliance tests and docs; incident runbooks.
- Tests to own: `tests/compliance/*`, e2e health checks.
- Metrics: SLOs, alert coverage, dashboard completeness.

Validation Matrix (where to run what)
- Fast local: `cargo test --all -- --skip slow` for non-platform-specific modules.
- Security suite: `cargo test --test security -- --nocapture` or individual files under `tests/security/*`.
- Transport integration: behind features/platform guards; BLE/TCP as available.
- Database: `tests/database_*` require local SQLite; enable Postgres feature for parity tests.
- Performance: `cargo bench` with HTML reports (criterion) for trends.
- Compliance: run `tests/compliance/*` as part of release candidates.

PR Checklist (every milestone)
- Documentation updated for user/operator and API changes.
- Unit + integration tests added/updated; coverage stable or improved.
- Lints: no new `clippy`/`rustc` warnings (allowlists justified in code or docs).
- Feature flags respected; default build minimal compiles and tests.
- Benchmarks compile; performance budgets noted in PR.

How To Run (commands)
- Format/lint: `cargo fmt --all --check`; `cargo clippy --all-targets -- -D warnings`.
- Test (fast): `cargo test --lib --bins`.
- Test (full, Linux): `cargo test --all -- --nocapture`.
- Benches: `cargo bench` (criterion with HTML reports).
- Docs: `cargo doc --no-deps --open`.

Exit Criteria Summary
- Each milestone requires: all listed tests green, metrics targets achieved, and docs updated.
- A milestone cannot close with flaky tests, undefined feature flags, or undocumented operator impacts.

Notes and Follow-ups Discovered During Survey
- `#![allow(dead_code, unused_*)]` appears in `src/lib.rs`; plan to ratchet down after M0 to reveal drift.
- Many modules have rich functionality; ensure CI job filters long-running and platform tests to keep signal high.
- Consider adding `proptest` dev-dependency in M2 for protocol parsing/property tests.

