# BitCraps Agent Operations Guide

This guide defines how agents (and humans using agent tooling) should collaborate on BitCraps. It distills guardrails, review roles, and deliverables from CLAUDE.md into a vendor‑neutral, repeatable process.

## Critical Code Standards

### Forbidden Patterns (never use)
- SQL string formatting: always use parameterized queries.
- `unwrap/expect` in production paths: use `Result` and `?` with contextual errors.
- Untracked async tasks: use tracked spawn utilities.
- Non-constant-time secret comparisons: use constant-time equality.
- Unbounded collections/buffers: enforce capacities or use bounded/LRU structures.
- Unbounded decompression: enforce strict maximum output sizes.

### Mandatory Patterns (always use)
- Error handling with typed errors and `?`.
- SQL safety with parameter binding and validated identifiers.
- Tracked async tasks and graceful shutdown.
- Constant-time operations for secrets.
- Explicit resource limits: timeouts, caps, rate limits.
- Input validation and sanitization at boundaries.

### Pre‑Commit Checklist
- No SQL string formatting.
- No `unwrap()`/`expect()` outside tests.
- All spawned tasks tracked.
- Constant‑time secret comparisons.
- Collections and decompression bounded.
- External input validated.
- I/O timeouts set.

## Multi‑Agent Review Protocol

### Roles
- Security: cryptography, key storage, auth, abuse resistance.
- Code Quality: correctness, error handling, API cohesion, dead code.
- Performance: allocations, hot paths, contention, caching, SIMD.
- Networking/Transport: BLE/TCP/NAT traversal, retries, backoff, TURN.
- Consensus/Game Logic: state machine, validation, anti‑cheat, sync.
- Mobile: Android/iOS bindings, lifecycle, background limits, battery.
- Database/Storage: migrations, repo layer, transactions, WAL, backups.
- CI/CD & Tooling: build, tests, coverage, SAST/DAST, release gates.
- Documentation: accuracy, single source of truth, diagrams, guides.

### Inputs
- Source tree, `Cargo.toml`, docs in `docs/`, tests in `tests/`.
- Known risk markers: `TODO|FIXME|HACK`, `unwrap/expect`, unsafe.

### Outputs (per role)
- Findings: issues with code references.
- Risk: severity (Critical/High/Medium/Low) + rationale.
- Remediation: concrete diffs/tasks, acceptance criteria.
- Verification: test strategy (unit/integration/e2e), metrics.

### Process
1. Triage: grep for hotspots (TODO/FIXME/unwrap) and critical modules.
2. Deep‑Dive: inspect modules by role ownership.
3. Synthesis: deduplicate, prioritize, assign owners & estimates.
4. Plan: convert into actionable tasks with acceptance tests.

## Collaboration & Safety
- Keep changes minimally scoped and reversible.
- Prefer small PRs with tests and docs.
- Follow repo style; avoid unrelated refactors.
- Document risks and rollback steps for non‑trivial changes.

## Execution Hints (CLI)
- Search: `rg "TODO|FIXME|HACK|unwrap\(|expect\(" src`.
- Read large files in chunks; avoid network-bound builds without caches.
- When adding code, include targeted tests near changed areas.

## Current Hotspots (2025‑09‑02 Snapshot)
Derived from repo scan and TODOs; see “Engineering Next Steps” for details.
- Transport: key exchange, peer ID protocol, recovery config, Kademlia pings, metrics counters.
- Protocol: state sync send path, game lifecycle broadcasts, partition recovery steps.
- Mobile: UniFFI configuration fixes, JNI/Swift bridging details, network stats sourcing.
- Discovery & Mesh: heartbeat/peer timeout configurability, announcement flow integration.
- SDK: template scaffolds and validation helpers.

## Deliverable Templates

### Finding
- Location: `src/path.rs:123`
- Summary: concise problem description.
- Risk: Critical/High/Medium/Low.
- Remediation: stepwise implementation notes.
- Verification: tests and metrics to prove fixed.

### Task
- Title: imperative scope name.
- Owner: team/role.
- Steps: 3–5 concrete actions.
- Acceptance: explicit, testable criteria.

---

See also: `CLAUDE.md` for historical session context and progress logs. Future status updates should prefer a single authoritative plan file rather than scattering progress across multiple documents.
