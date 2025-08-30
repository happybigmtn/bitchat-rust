//! Commit-reveal scheme for fair randomness generation

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::SecureKeystore;
use crate::error::Result;
use crate::protocol::{Hash256, PeerId, Signature};

use super::RoundId;

/// Randomness commitment for dice rolls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessCommit {
    pub player: PeerId,
    pub round_id: RoundId,
    pub commitment: Hash256, // SHA256(nonce || round_id)
    pub timestamp: u64,
    pub signature: Signature,
}

/// Randomness reveal for dice rolls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomnessReveal {
    pub player: PeerId,
    pub round_id: RoundId,
    pub nonce: [u8; 32],
    pub timestamp: u64,
    pub signature: Signature,
}

/// Entropy pool for secure randomness
#[derive(Debug, Clone)]
pub struct EntropyPool {
    /// Collected entropy from all participants
    entropy_sources: Vec<[u8; 32]>,

    /// Combined entropy hash
    combined_entropy: Option<Hash256>,

    /// Round counter for unique seeds
    round_counter: u64,
}

impl RandomnessCommit {
    /// Create new randomness commitment
    pub fn new(player: PeerId, round_id: RoundId, nonce: [u8; 32]) -> Self {
        let commitment = Self::create_commitment(nonce, round_id);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            player,
            round_id,
            commitment,
            timestamp,
            signature: crate::protocol::Signature([0u8; 64]), // Would implement proper signing
        }
    }

    /// Create commitment hash
    fn create_commitment(nonce: [u8; 32], round_id: RoundId) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update(round_id.to_le_bytes());
        hasher.finalize().into()
    }

    /// Verify commitment against reveal
    pub fn verify_reveal(&self, reveal: &RandomnessReveal) -> bool {
        if self.player != reveal.player || self.round_id != reveal.round_id {
            return false;
        }

        let expected_commitment = Self::create_commitment(reveal.nonce, reveal.round_id);
        self.commitment == expected_commitment
    }
}

impl RandomnessReveal {
    /// Create new randomness reveal with proper cryptographic signature
    pub fn new(
        player: PeerId,
        round_id: RoundId,
        nonce: [u8; 32],
        keystore: &mut SecureKeystore,
    ) -> Result<Self> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create signature data
        let mut signature_data = Vec::new();
        signature_data.extend_from_slice(&player);
        signature_data.extend_from_slice(&round_id.to_le_bytes());
        signature_data.extend_from_slice(&nonce);
        signature_data.extend_from_slice(&timestamp.to_le_bytes());

        // Sign with randomness commitment context
        let signature = keystore.sign(&signature_data)?;

        Ok(Self {
            player,
            round_id,
            nonce,
            timestamp,
            signature,
        })
    }
}

impl Default for EntropyPool {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropyPool {
    /// Create new entropy pool
    pub fn new() -> Self {
        Self {
            entropy_sources: Vec::new(),
            combined_entropy: None,
            round_counter: 0,
        }
    }

    /// Add entropy from a participant
    pub fn add_entropy(&mut self, entropy: [u8; 32]) {
        self.entropy_sources.push(entropy);
        self.combined_entropy = None; // Invalidate cached entropy
    }

    /// Get combined entropy
    pub fn get_combined_entropy(&mut self) -> Hash256 {
        if let Some(entropy) = self.combined_entropy {
            return entropy;
        }

        let mut hasher = Sha256::new();

        // Add all entropy sources
        for source in &self.entropy_sources {
            hasher.update(source);
        }

        // Add round counter for uniqueness
        hasher.update(self.round_counter.to_le_bytes());

        let entropy = hasher.finalize().into();
        self.combined_entropy = Some(entropy);
        entropy
    }

    /// Generate dice roll from entropy using unbiased method
    pub fn generate_dice_roll(&mut self) -> (u8, u8) {
        // Generate secure random bytes
        let random_bytes = self.generate_bytes(16);

        // Use unbiased method to convert to dice values
        let die1 = Self::bytes_to_die_value(&random_bytes[0..8]);
        let die2 = Self::bytes_to_die_value(&random_bytes[8..16]);

        (die1, die2)
    }

    /// Convert bytes to unbiased die value (1-6) using rejection sampling
    fn bytes_to_die_value(bytes: &[u8]) -> u8 {
        // Convert bytes to u64
        let mut value = 0u64;
        for (i, &byte) in bytes.iter().enumerate().take(8) {
            value |= (byte as u64) << (i * 8);
        }

        // Rejection sampling to avoid modulo bias
        const MAX_VALID: u64 = u64::MAX - (u64::MAX % 6);

        while value >= MAX_VALID {
            // Re-hash to get new randomness
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"DICE_REROLL");
            hasher.update(value.to_le_bytes());
            let new_hash = hasher.finalize();

            value = 0u64;
            for (i, &byte) in new_hash.iter().enumerate().take(8) {
                value |= (byte as u64) << (i * 8);
            }
        }

        ((value % 6) + 1) as u8
    }

    /// Clear entropy sources (for new round)
    pub fn clear(&mut self) {
        self.entropy_sources.clear();
        self.combined_entropy = None;
    }

    /// Get number of entropy sources
    pub fn entropy_count(&self) -> usize {
        self.entropy_sources.len()
    }

    /// Check if enough entropy is collected
    pub fn has_sufficient_entropy(&self, min_sources: usize) -> bool {
        self.entropy_sources.len() >= min_sources
    }

    /// Generate cryptographically secure bytes using OS entropy
    pub fn generate_bytes(&mut self, num_bytes: usize) -> Vec<u8> {
        // Use OS-provided cryptographically secure randomness
        let mut os_rng = OsRng;
        let mut os_bytes = vec![0u8; 32];
        os_rng.fill_bytes(&mut os_bytes);

        // Combine with existing entropy sources
        let mut hasher = Sha256::new();

        // Add OS entropy first
        hasher.update(&os_bytes);

        // Add collected entropy
        for source in &self.entropy_sources {
            hasher.update(source);
        }

        // Add round counter for uniqueness
        hasher.update(self.round_counter.to_le_bytes());
        self.round_counter = self.round_counter.wrapping_add(1);

        // Add current timestamp for additional entropy
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        hasher.update(timestamp.to_be_bytes());

        let seed = hasher.finalize();

        // Use seed to generate requested amount of random data
        let mut output = Vec::new();
        let mut counter = 0u32;

        while output.len() < num_bytes {
            let mut round_hasher = Sha256::new();
            round_hasher.update(&seed);
            round_hasher.update(counter.to_be_bytes());

            let round_hash = round_hasher.finalize();
            output.extend_from_slice(&round_hash);
            counter = counter.wrapping_add(1);
        }

        output.truncate(num_bytes);
        output
    }
}
