# Chapter 0C: Probability & Fairness in Craps — Complete Implementation Analysis
## Deep Dive into 2d6 distributions, house edge, commit‑reveal, and payout correctness

---

Implementation Status: Implemented
- Lines of code analyzed: ~600 (gaming/*, protocol/craps types)
- Key files: `src/gaming/craps_rules.rs`, `src/gaming/payout_engine.rs`, `src/gaming/consensus_game_manager.rs`
- Gaps/Future Work: Extended statistical tests over long sequences

## Mathematical Foundations

- 2d6 distribution: ways to roll totals; P(7)=6/36, P(2)=1/36, etc.
- House edge derivations: pass line ≈ 1.41%, don’t pass ≈ 1.36%
- Law of large numbers: converge to expected frequencies over time

Worked Example: Pass Line
- Compute probability of immediate win (come‑out 7/11) vs. establish point and win before 7
- Show expected value calculation and compare to implementation payouts

## Code Mapping

- `craps_rules.rs`
  - Bet evaluation functions: pass/don’t pass, field, place, hardways, props
  - GamePhase transitions and correctness invariants

- `payout_engine.rs`
  - Resolution paths per bet type; ensure payouts match true odds where applicable

- `consensus_game_manager.rs`
  - Commit/reveal and distributed dice rolls integration

## Senior Engineering Review
- Add statistical test harness to verify distribution and payouts across 1e6 trials
- Parameterize odds/payout tables for easier audit and regional variants

## Lab Exercise
- Generate 100k dice rolls, plot empirical vs theoretical distribution
- Validate house edge over simulated sessions for pass/don’t pass

## Check Yourself
- Why does pass line have a non‑zero house edge despite fair dice?
- What distribution property explains 7 being most common?
- How does commit‑reveal ensure fairness without a trusted party?
