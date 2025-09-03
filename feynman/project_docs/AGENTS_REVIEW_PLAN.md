# Multi‑Agent Review Plan (Semester)

This plan assigns review roles (per `agents.md`) to each week’s topics. Agents verify walkthrough accuracy vs code and raise PRs to fix drift.

## Roles
- Security, Code Quality, Performance, Networking/Transport, Consensus/Game Logic, Mobile, Database/Storage, CI/CD & Tooling, Documentation

## Week‑by‑Week Checklist

Week 1: Error, Architecture
- Documentation: Confirm error variants and results match code
- Code Quality: Scan `unwrap/expect` in core modules; file issues

Week 2: Config, Main, State
- Documentation: Cross‑link `src/app_config.rs`, `src/app.rs`, `src/app_state.rs`
- Performance: Identify startup path hot spots

Week 3: Crypto
- Security: Verify X25519+HKDF+ChaCha20Poly1305 flows and nonce handling
- Documentation: Align walkthrough line counts and code paths

Week 4: Protocol
- Networking/Transport: Validate message format walkthrough vs actual types
- Documentation: Update any stale paths

Week 5: Transport/NAT
- Networking/Transport: MTU budgeting, fragmentation; add metrics if missing
- Performance: Benchmark fragment sizes vs throughput

Week 6: Mesh/Discovery
- Networking/Transport: Peer discovery and DHT correctness review
- Documentation: Ensure Kademlia paths match code

Week 7: Consensus
- Consensus/Game Logic: Verify 2/3 thresholds and double‑vote protections
- Documentation: Mark Future Work where features are design‑only

Week 8: Sync/Resilience
- Consensus/Game Logic: Partition recovery invariants; TODOs to tracked tasks

Week 9: Game/Fairness
- Documentation: Match payouts to `craps_rules.rs`
- Security: Validate commit‑reveal is enforced

Week 10: Storage
- Database/Storage: Persistence interfaces and recovery tests
- CI/CD: Add storage tests to CI if feasible

Week 11: Security/Sessions
- Security: Keystore and session lifecycle reviews
- Code Quality: Remove non‑test `unwrap/expect`

Week 12: Observability/Perf
- Performance: Profile hot paths; cache hit/miss analyses
- Documentation: Ensure metrics names/labels documented

Week 13: Mobile
- Mobile: JNI/Swift/UniFFI correctness and lifecycle
- CI/CD: Device/instrumentation test gates (scoped plan)

Week 14: Advanced/Launch
- Documentation: Confirm deployment walkthrough matches Docker/K8s manifests
- CI/CD: Release checklist and gates

## Deliverables
- PRs updating walkthroughs for accuracy
- issues.md entries for notable drift
- Optional small code changes to expose metrics/config for labs
