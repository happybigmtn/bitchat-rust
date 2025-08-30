//! Deterministic random number generation for consensus
//!
//! Provides a deterministic RNG that produces identical sequences
//! from the same seed, crucial for distributed consensus where all
//! nodes must agree on random values.

use rand::{Error as RandError, RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Deterministic random number generator for consensus
///
/// Uses ChaCha20 algorithm to ensure cryptographic quality
/// while maintaining determinism across all platforms.
#[derive(Clone, Debug)]
pub struct DeterministicRng {
    #[allow(dead_code)]
    seed: [u8; 32],
    inner: ChaCha20Rng,
}

impl DeterministicRng {
    /// Create a new deterministic RNG from a seed
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            seed,
            inner: ChaCha20Rng::from_seed(seed),
        }
    }

    /// Create from consensus data (game ID + round number)
    pub fn from_consensus(game_id: &[u8; 16], round: u64, participants: &[[u8; 32]]) -> Self {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(game_id);
        hasher.update(round.to_le_bytes());

        // Include all participants for determinism
        let mut sorted_participants = participants.to_vec();
        sorted_participants.sort();
        for participant in sorted_participants {
            hasher.update(participant);
        }

        let hash = hasher.finalize();
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&hash);

        Self::from_seed(seed)
    }

    /// Generate a random value in range [min, max)
    pub fn gen_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }

        let range = max - min;
        let mut value = self.inner.next_u64();

        // Avoid modulo bias
        let threshold = u64::MAX - (u64::MAX % range);
        while value >= threshold {
            value = self.inner.next_u64();
        }

        min + (value % range)
    }

    /// Generate dice roll (1-6)
    pub fn roll_die(&mut self) -> u8 {
        self.gen_range(1, 7) as u8
    }

    /// Generate a pair of dice rolls
    pub fn roll_dice(&mut self) -> (u8, u8) {
        (self.roll_die(), self.roll_die())
    }

    /// Shuffle a slice deterministically
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        use rand::seq::SliceRandom;
        slice.shuffle(&mut self.inner);
    }
}

impl RngCore for DeterministicRng {
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
        self.inner.try_fill_bytes(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let seed = [1u8; 32];
        let mut rng1 = DeterministicRng::from_seed(seed);
        let mut rng2 = DeterministicRng::from_seed(seed);

        for _ in 0..1000 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_dice_rolls() {
        let mut rng = DeterministicRng::from_seed([42u8; 32]);

        for _ in 0..1000 {
            let die = rng.roll_die();
            assert!(die >= 1 && die <= 6);

            let (d1, d2) = rng.roll_dice();
            assert!(d1 >= 1 && d1 <= 6);
            assert!(d2 >= 1 && d2 <= 6);
        }
    }

    #[test]
    fn test_consensus_seed() {
        let game_id = [0xAAu8; 16];
        let participants = vec![[1u8; 32], [2u8; 32], [3u8; 32]];

        // Same inputs should produce same RNG
        let mut rng1 = DeterministicRng::from_consensus(&game_id, 1, &participants);
        let mut rng2 = DeterministicRng::from_consensus(&game_id, 1, &participants);

        assert_eq!(rng1.next_u64(), rng2.next_u64());

        // Different round should produce different RNG
        let mut rng3 = DeterministicRng::from_consensus(&game_id, 2, &participants);
        assert_ne!(rng1.next_u64(), rng3.next_u64());
    }

    #[test]
    fn test_range_generation() {
        let mut rng = DeterministicRng::from_seed([99u8; 32]);

        for _ in 0..1000 {
            let value = rng.gen_range(10, 20);
            assert!(value >= 10 && value < 20);
        }
    }
}
