# Chapter 0E: Distributed Systems Math — Quorums, Thresholds, and Failure Models
## Deep Dive into 2f+1 quorums, Byzantine thresholds, and probabilistic liveness

---

Implementation Status: Partial/Implemented (varies by module)
- Lines of code analyzed: ~700 (protocol/*, gaming/consensus_game_manager.rs)
- Key files: `src/protocol/*`, `src/gaming/consensus_game_manager.rs`
- Gaps/Future Work: Formal specs and model checking (out of scope for semester)

## Fundamentals

- Crash vs Byzantine failures, quorum math: with f faulty, need 2f+1 out of 3f+1
- Threshold signatures (conceptual) and voting correctness
- Tradeoffs: latency vs safety; partitions and rejoin policies

## Code Mapping

- Voting and state transitions in consensus paths
- Partition recovery sequencing and safety checks

## Senior Engineering Review
- Make thresholds explicit constants; centralize safety invariants in types
- Add simulation harness to explore partitions/latency distributions

## Lab Exercise
- Implement a majority vote with double‑vote prevention; prove 2/3 threshold
- Simulate partitions and verify failure behavior matches design

## Check Yourself
- Why 2f+1 for Byzantine agreements?
- How do partitions impact safety vs liveness?
- What invariants must hold for state synchronization to be correct?
