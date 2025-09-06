//! Randomness providers and proof types (VRF and Commit-Reveal)

use crate::crypto::vrf::{self, VRFKeypair, VRFOutput, VRFProof};
use crate::protocol::{DiceRoll, Hash256};

/// Proof bundle clients can verify
#[derive(Debug, Clone)]
pub enum RandomnessProof {
    VRF { input: Vec<u8>, output: [u8;32], proof: Vec<u8>, pk: [u8;32] },
    CommitReveal { combined_entropy: Hash256 },
}

/// Randomness provider trait
pub trait RandomnessProvider {
    fn roll_with_proof(&self, round_id: u64, game_id: [u8;16]) -> (DiceRoll, RandomnessProof);
}

/// Map 32 bytes to unbiased dice using rejection sampling
fn bytes_to_two_dice(bytes: &[u8;32]) -> (u8, u8) {
    fn one_die(slice: &[u8;16]) -> u8 {
        // Convert first 8 bytes to u64
        let mut v = 0u64;
        for (i, b) in slice.iter().take(8).enumerate() { v |= (*b as u64) << (i*8); }
        const MAX_VALID: u64 = u64::MAX - (u64::MAX % 6);
        let mut cur = v;
        while cur >= MAX_VALID {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b"REROLL");
            h.update(cur.to_le_bytes());
            let d = h.finalize();
            cur = 0;
            for (i, b) in d.iter().take(8).enumerate() { cur |= (*b as u64) << (i*8); }
        }
        ((cur % 6) + 1) as u8
    }
    let mut left = [0u8;16]; left.copy_from_slice(&bytes[0..16]);
    let mut right = [0u8;16]; right.copy_from_slice(&bytes[16..32]);
    (one_die(&left), one_die(&right))
}

/// VRF-based provider using stub implementation for now
pub struct VrfProvider {
    pub keypair: VRFKeypair,
}

impl VrfProvider {
    pub fn from_seed(seed: u64) -> Self { Self { keypair: VRFKeypair::generate_deterministic(seed) } }
}

impl RandomnessProvider for VrfProvider {
    fn roll_with_proof(&self, round_id: u64, game_id: [u8;16]) -> (DiceRoll, RandomnessProof) {
        let mut input = Vec::with_capacity(8+16);
        input.extend_from_slice(&round_id.to_le_bytes());
        input.extend_from_slice(&game_id);
        let (out, proof) = vrf::prove(&self.keypair.sk, &input).expect("vrf prove");
        let VRFOutput(out_bytes) = out;
        let (d1, d2) = bytes_to_two_dice(&out_bytes);
        let roll = DiceRoll::new(d1, d2).expect("valid dice");
        let vf = RandomnessProof::VRF { input, output: out_bytes, proof: proof.0, pk: (self.keypair.pk.0) };
        (roll, vf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_vrf_provider_roll() {
        let p = VrfProvider::from_seed(42);
        let (roll, proof) = p.roll_with_proof(1, [0u8;16]);
        assert!((1..=6).contains(&roll.die1()));
        assert!((1..=6).contains(&roll.die2()));
        match proof { RandomnessProof::VRF { .. } => {}, _ => panic!("expected vrf") }
    }
}
