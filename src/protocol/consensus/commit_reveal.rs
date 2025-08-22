//! Commit-reveal scheme for fair randomness generation

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use crate::protocol::{PeerId, Hash256, Signature};

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
        hasher.update(&nonce);
        hasher.update(&round_id.to_le_bytes());
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
    /// Create new randomness reveal
    pub fn new(player: PeerId, round_id: RoundId, nonce: [u8; 32]) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            player,
            round_id,
            nonce,
            timestamp,
            signature: crate::protocol::Signature([0u8; 64]), // Would implement proper signing
        }
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
        hasher.update(&self.round_counter.to_le_bytes());
        
        let entropy = hasher.finalize().into();
        self.combined_entropy = Some(entropy);
        entropy
    }
    
    /// Generate dice roll from entropy
    pub fn generate_dice_roll(&mut self) -> (u8, u8) {
        let entropy = self.get_combined_entropy();
        
        // Use entropy to generate dice values
        let die1 = (entropy[0] % 6) + 1;
        let die2 = (entropy[1] % 6) + 1;
        
        // Advance round counter
        self.round_counter += 1;
        self.combined_entropy = None;
        
        (die1, die2)
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
    
    /// Generate random bytes from entropy
    pub fn generate_bytes(&mut self, num_bytes: usize) -> Vec<u8> {
        let entropy = self.get_combined_entropy();
        let mut result = Vec::with_capacity(num_bytes);
        
        // Extend entropy if needed
        for i in 0..num_bytes {
            result.push(entropy[i % entropy.len()]);
        }
        
        // Advance round counter for next generation
        self.round_counter += 1;
        self.combined_entropy = None;
        
        result
    }
}