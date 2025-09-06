Randomness: VRF and Commit‑Reveal

Modes

- VRF: Leader produces `(output, proof)` over `round_id || game_id`; clients verify
- Commit‑Reveal: Validators commit to nonce and reveal; combined entropy → dice

Implementation

- Trait: `protocol_randomness::RandomnessProvider`
- VRF provider: `VrfProvider::from_seed(seed)`; maps output to unbiased dice
- Proof: embedded as JSON in `GameOperation::ProcessRoll.entropy_proof` (hashed to 32 bytes for transport)

Client Verification

- VRF: verify `(input, output, proof, pk)` with SDK method (to be implemented)
- Commit‑Reveal: verify commitments match reveals and combined hash

Operator Notes

- Prefer VRF in high‑latency environments; commit‑reveal in closed validator rings
- Timeouts: `reveal_window_ms`, `reveal_grace_ms` (see CLI flags)

