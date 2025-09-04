Scaling BitCraps to a Massive Global Table

Overview

- Goal: Evolve BitCraps from small-group PBFT to a tiered architecture that supports 100k–1M+ concurrent spectators with tens of validators, while preserving provable fairness and low end-to-end latency per round.
- Approach: Limit consensus to a validator set; treat everyone else as clients via regional gateways; batch inputs/outputs; provide quorum certificates for client verification; keep commit-reveal among validators (with VRF fallback); and harden gateway scaling, fees, and monitoring.
- Grounding: This plan references the current codebase modules and proposes precise changes with tests at each milestone so engineers can implement incrementally.

Current State (as implemented)

- Consensus engines: `src/protocol/consensus/optimized_pbft.rs` (pipelined PBFT with batching/compression), `engine.rs`, `state_machine.rs` (checkpoints), `validation.rs`, `voting.rs`, `commit_reveal.rs`.
- Mesh + gateway: `src/mesh/*` including `gateway.rs` (TCP/UDP/WS QUIC gateway skeleton), `service.rs`, `consensus_message_handler.rs` (bridge to consensus messages), `message_queue.rs`.
- Services/API: `src/services/api_gateway/*` (Axum gateway with routing, LB, rate-limits, auth), `src/services/consensus/*` (consensus microservice, PBFT/Tendermint/HotStuff variants), `src/services/game_engine/*`.
- SDK v2 (client): `src/sdk_v2/*` including `rest.rs`, `networking.rs`, `consensus.rs`.
- Game orchestration: `src/gaming/game_orchestrator.rs`, `src/gaming/consensus_game_manager.rs` with commit-reveal hooks.
- Economics/fees + token: `src/economics/*`, `src/token/*`.

Target Architecture (Tiered)

- Validators: Fixed-size validator set (10–50, stretch to ≤100) run PBFT (`optimized_pbft.rs`) and apply game ops. Validators sign commits; their signatures form a quorum certificate (QC) attached to outcomes.
- Gateways: Regional gateways (Axum-based service + `mesh/gateway.rs`) accept client bets via REST/WS, aggregate them, and stream state updates. Fan-in bets to validators, fan-out updates to clients.
- Clients (spectators): Use SDK v2 to submit signed bets to gateways; subscribe to updates and verify QC + dice fairness proofs locally. No heavy consensus workload on clients.
- Randomness: Validator-only commit-reveal per round (or VRF from leader with fallback) with client-verifiable proofs.
- Batching: Aggregate identical bets, commit batches per round; use `UpdateBalances` to distribute payouts in a single consensus operation, with inclusion proofs.

Milestone Plan (with concrete code changes and tests)

M0. Baseline + Roles Skeleton

- Changes
  - Add node roles to config: `src/app_config.rs`
    - Add `NodeRole { Validator, Gateway, Client }` and `role: NodeRole` to runtime config.
    - Wire into `src/main.rs` startup to initialize services based on role.
  - Expose validator set in consensus service: `src/services/consensus/service.rs`
    - Maintain active validator set and enforce `vote`/`propose` only for validators.
    - Leverage existing request types in `src/services/consensus/types.rs` (UpdateValidatorRequest/Response) to manage membership (operator-driven for now).
  - Mesh coordinator: ensure participant list == validators for consensus lanes: `src/protocol/consensus_coordinator.rs` and `src/mesh/consensus_message_handler.rs`.

- Tests
  - Unit: `src/services/consensus/service.rs`
    - Non-validator vote rejected with `ConsensusServiceError::InvalidVote`.
    - Validator add/remove updates active set and affects `votes_required`.
  - Integration: `tests/roles_routing.rs`
    - Spawn 3 roles (validator, gateway, client); client bet hits gateway; gateway forwards to consensus service; non-validator votes ignored.
  - Bench baseline: `benches/global_scale_baseline.rs`
    - Baseline PBFT throughput with n={4,10,20} validators and varying `batch_size`.

M1. PBFT Core Tuning for Validators

- Changes
  - `src/protocol/consensus/optimized_pbft.rs`
    - Expose config via app config and CLI flags: `pipeline_depth`, `batch_size`, `base_timeout`, `view_change_timeout`, `max_pending_operations`.
    - Increase default `batch_size` from 100 -> 2000 (configurable) and reduce batch interval tick if pending grows.
    - Ensure `start_view_monitor` implements minimal leader rotation on timeout (already stubbed).
  - `src/services/consensus/service.rs`
    - Set `ConsensusConfig { algorithm: PBFT, min_validators: 4 }` and ensure `calculate_required_votes()` = ceil(2n/3).

- Tests
  - Unit: `optimized_pbft.rs` existing tests + add
    - Batch creation triggers on size/time thresholds; compression toggles respected.
    - Quorum computation for various n (4, 7, 10, 50, 100).
  - Integration: `tests/pbft_50_validators.rs`
    - Simulate 50 validators processing 100k ops in batches, assert commit rates and latency envelopes.
  - Bench: `benches/pbft_batching.rs`
    - Throughput vs `batch_size` and `pipeline_depth`.

M2. Quorum Certificates (QC) for Client Verification

- Changes
  - Add QC type: `src/protocol/consensus/optimized_pbft.rs`
    - Define `QuorumCertificate { view, sequence, batch_hash, commit_signatures: Vec<(PeerId, Signature)> }`.
    - On commit, aggregate commit signatures exceeding threshold into QC; emit as part of commit event.
  - Surface QC via services:
    - `src/services/consensus/service.rs`: extend `ConsensusResult` to include QC bytes.
    - `src/services/game_engine/service.rs`: attach QC to round outcomes.
  - SDK v2: `src/sdk_v2/consensus.rs`
    - Add `get_quorum_certificate(proposal_id)` and `verify_quorum_certificate(qc, validator_pubkeys)`.

- Tests
  - Unit: `optimized_pbft.rs`
    - QC produced only when commit signatures >= threshold; malformed QC rejected by verifier.
  - Integration: `tests/qc_client_verify.rs`
    - Client fetches QC via REST; verifies with distributed validator pubkeys.

M3. Validator-Only Commit-Reveal, VRF Fallback

- Changes
  - Restrict commit-reveal participants to validators only:
    - `src/protocol/consensus/commit_reveal.rs`: add API to validate participating set against active validators.
    - `src/services/consensus/service.rs`: gate commit/reveal collection to validator set; proceed on >2/3 reveals with timeout.
  - Fallback VRF:
    - New: `src/crypto/vrf.rs` (Ed25519-based VRF via RFC9380 libs later; stub in phase 1 with interface and deterministic test mode).
    - Round leader proposes VRF output + proof; validators and clients verify; fallback to commit-reveal if leader stalls.
  - Game orchestrator hook:
    - `src/gaming/game_orchestrator.rs`: switch dice roll source to the validator randomness provider; publish proofs in game log.

- Tests
  - Unit: `commit_reveal.rs`
    - Commit -> reveal flow with 5 validators; proceed with 4-of-5; mismatch reveal rejected.
  - Unit: `crypto/vrf.rs`
    - Deterministic proof verification for known key/input vector.
  - Integration: `tests/randomness_fairness.rs`
    - Clients verify either validator commit-reveal or VRF proof; outcomes match protocol definition.

M4. Gateway Fan-in/Fan-out and Client API

- Changes
  - API gateway: `src/services/api_gateway/gateway.rs`
    - Add routes: `POST /api/v1/games/{id}/bets` (accept signed bet), `GET /api/v1/games/{id}/subscribe` (WS topics), `GET /api/v1/games/{id}/proofs` (QC + randomness proofs).
    - Enforce rate-limits per IP/user; integrate auth middleware.
  - Mesh gateway: `src/mesh/gateway.rs`
    - Implement WebSocket server variant; add topic-based broadcast to clients; shard clients across nodes.
  - Consensus bridge: ensure validators only receive aggregated bet batches, not per-client micro-tx.
    - Path: `src/protocol/network_consensus_bridge.rs` + `src/services/game_engine/service.rs`.
  - SDK v2: `src/sdk_v2/networking.rs` and `consensus.rs`
    - Add `place_bet(game_id, bet)` to post to API; `subscribe_updates(game_id)` WS subscription; `fetch_proofs(game_id, round)`.

- Tests
  - Integration: `tests/gateway_bet_flow.rs`
    - 1000 synthetic clients submit signed bets to `/bets`; gateway aggregates and forwards one batch op; validators commit; clients receive broadcast update.
  - Integration: `tests/ws_broadcast.rs`
    - WS broadcast delivers updates to N subscribers with bounded latency and no per-client consensus load.

M5. Bet Aggregation + Merkle Inclusion

- Changes
  - Aggregator module: `src/services/game_engine/aggregator.rs`
    - Aggregate identical bets per round into `AggregatedBet { bet_type, total_amount, contributors: Vec<(PeerId, amount, sig)> }`.
    - Generate contribution Merkle tree using `src/protocol/consensus/merkle_cache.rs` and persist Merkle root in consensus op.
  - Consensus operation:
    - Extend `GameOperation::PlaceBet`/`UpdateBalances` in `src/protocol/consensus/engine.rs` to accept aggregated forms and Merkle root.
  - Proof endpoint: API gateway exposes inclusion proof per player.

- Tests
  - Unit: `aggregator.rs`
    - Aggregation correctness; contributors reconstructed from Merkle proof; tampered leaf fails.
  - Integration: `tests/agg_round_commit.rs`
    - Single consensus op replaces 10k identical bets; payouts via `UpdateBalances` in one op.

M6. State Checkpoints + Fast Sync

- Changes
  - Use existing checkpoints: `src/protocol/consensus/state_machine.rs` (`checkpoint_interval`) to snapshot state hashes.
  - Expose snapshot API: `GET /api/v1/games/{id}/snapshot` returning compact state + latest QC.
  - Client fast sync: SDK pulls snapshot + QC instead of replaying entire history.

- Tests
  - Unit: `state_machine.rs`
    - Checkpoint created at configured interval; pruning respects retention.
  - Integration: `tests/client_fast_sync.rs`
    - New client syncs from snapshot and verifies QC/state hash <500ms for typical snapshot size.

M7. Economics + Anti-DoS

- Changes
  - Dynamic micro-fees for bets: leverage `src/economics/mod.rs` to compute base fee per bet category; expose policy in config.
  - Rate-limiting: tighten per-IP/user limits in `src/services/api_gateway/middleware.rs` based on role/suspicion.
  - Optional min bet enforcement in game engine to deter spam (`src/services/game_engine/service.rs`).

- Tests
  - Unit: `economics/mod.rs`
    - Fee increases with congestion; zero fee for trusted internal gateways.
  - Integration: `tests/antidos.rs`
    - Simulate bursty clients; ensure rate-limits kick in without starving legit traffic.

M8. Observability + SLOs

- Changes
  - Metrics: ensure consensus, gateway, game engine expose Prometheus metrics (existing monitoring wiring: `monitoring/`, `grafana/`).
  - Add latency histograms for: bet ingress->commit, commit->broadcast, WS fan-out per region.
  - Health endpoints already exist in API gateway; extend for validator health rollups.

- Tests
  - Integration: `tests/metrics_visibility.rs`
    - Scrape metrics endpoints; verify presence of key histograms/counters; simple threshold assertions.

M9. Regional Gateways + Routing

- Changes
  - Extend gateway discovery and routing in `src/mesh/gateway.rs` to add region/weight and register in `ApiGateway` service discovery.
  - Add sticky routing by region in `src/services/api_gateway/load_balancer.rs` (e.g., IP-hash or JWT region claim).

- Tests
  - Integration: `tests/region_routing.rs`
    - Clients pinned to nearest gateway; failover to next region on outage.

M10. Long‑Term: Committee Rotation (Optional)

- Changes (future)
  - New module: `src/services/consensus/committee.rs` implementing secure random selection (using previous QC/randomness) of K-of-N committee per round, with ephemeral keys and short-lived comms; bridges into `ConsensusService`.
  - Start with testnet flag.

- Tests
  - Integration: `tests/committee_rotation.rs` with 1000 mock nodes selecting 50 committee members per round; verify safety (no equivocation) and liveness under churn.

Key Implementation Notes per File

- `src/protocol/consensus/optimized_pbft.rs`
  - Add `QuorumCertificate` and QC assembly in commit handler; expose getter for QC by `sequence`.
  - Increase `batch_size` via config; maintain EMA of batch sizes (already present) and expose in metrics.
  - Implement simple `start_view_monitor` with timeout-based leader bump.

- `src/services/consensus/service.rs`
  - Enforce validator-only propose/vote; expose validator admin API using existing `UpdateValidatorRequest`.
  - Extend `ConsensusResult` to include QC and randomness proof bundle.

- `src/sdk_v2/consensus.rs`
  - New methods: `get_quorum_certificate`, `verify_quorum_certificate`, `get_randomness_proofs(round_id)`.

- `src/mesh/gateway.rs` and `src/services/api_gateway/gateway.rs`
  - Implement WS subscription path and topic fan-out; integrate rate limiter and auth; add `/bets` endpoint.

- `src/services/game_engine/service.rs`
  - Introduce aggregation pipeline and `UpdateBalances` batch payouts; attach inclusion proof retrieval.

- `src/protocol/consensus/commit_reveal.rs`
  - Validator-only mode and >2/3 reveals rule with timeout; evidence record for missing reveals.

Rollout and Testing Strategy

1) Local validator ring: 5–10 validators on dev machines; synthetic 10k clients via load tool hitting `/bets`; verify end-to-end QC verification and WS updates.
2) Staged perf runs: scale validators to 50; push `batch_size` and `pipeline_depth`; measure commit latency and throughput with benches in `benches/`.
3) Canary in one region: 1 gateway, 5 validators; then multi-region with 3 gateways; use SLOs from metrics to dial configs.

Acceptance Criteria per Milestone

- M0: Non-validators cannot affect consensus; roles initialize correctly; baseline metrics captured.
- M1: PBFT processes ≥50k ops/sec with `batch_size>=1000` on dev hardware; commit latency p50 < 500ms.
- M2: QC delivered and client SDK verifies; invalid QC rejected.
- M3: Validator commit-reveal or VRF proof present for each roll; clients verify.
- M4: Gateway throughput ≥100k bet requests/min sustained; broadcast latency p95 < 300ms to 10k subs in lab.
- M5: Aggregated bets reduce on-chain ops by ≥1000x for common lines; users retrieve inclusion proofs.
- M6: Fresh client syncs via snapshot+QC in <1s for typical snapshot.
- M7: Rate-limit/fees cap attack traffic while maintaining ≥95% success for legit requests in mixed loads.
- M8: Metrics dashboards show stable SLOs; alerts configured for regressions.
- M9: Regional affinity working; single-region failure fails over within 5s.

Risks and Mitigations

- Validator collusion: Keep validator set diverse; publish QC and randomness proofs; invite third parties to run validators; enable audits.
- Gateway overload: Horizontal scale behind LB; rate-limit; shard WS; use backpressure; compress payloads (already supported).
- State bloat: Use `checkpoint_interval` and compact snapshots; batch updates; prune old checkpoints with retention.
- Latency spikes: Adaptive timeouts (`TimeoutController`), regional gateways, and batch knobs.

Ownership (“Agents”)

- Consensus Core + QC: Owner A. Files: `src/protocol/consensus/optimized_pbft.rs`, `src/services/consensus/service.rs`.
- Gateway + API: Owner B. Files: `src/services/api_gateway/*`, `src/mesh/gateway.rs`, SDK v2 endpoints.
- Randomness: Owner C. Files: `src/protocol/consensus/commit_reveal.rs`, `src/crypto/vrf.rs` (new), orchestrator wiring.
- Aggregation + Merkle: Owner D. Files: `src/services/game_engine/aggregator.rs` (new), `src/protocol/consensus/merkle_cache.rs`.
- Checkpointing + Sync: Owner E. Files: `src/protocol/consensus/state_machine.rs`, snapshot APIs.
- Economics/Anti-DoS: Owner F. Files: `src/economics/*`, `src/services/api_gateway/middleware.rs`.

Getting Started Checklist

- Add `NodeRole` to `app_config.rs` and wire role-based boot in `main.rs`.
- Implement validator gating in `ConsensusService` and update tests.
- Add QC struct and commit assembly in `optimized_pbft.rs` and expose via service.
- Add `/bets`, `/subscribe`, `/proofs` endpoints in gateway; wiring to game engine + consensus bridge.
- Create aggregator module and wire Merkle roots into consensus ops.
- Expose snapshot + QC API; add SDK fast-sync call.
- Add test files listed above and hook into CI.

