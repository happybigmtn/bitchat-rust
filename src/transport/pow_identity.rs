use crate::protocol::PeerId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

/// Standalone Proof of Work for NodeId validation
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProofOfWork {
    pub nonce: u64,
    pub timestamp: u64,
    pub difficulty: u32,
    pub hash: [u8; 32],
}

impl ProofOfWork {
    /// Generate proof of work for given data
    pub fn generate(data: &[u8], difficulty: u32) -> Result<Self, &'static str> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_secs();

        let mut nonce = 0u64;
        let mut hasher = Sha256::new();

        // Limit attempts to prevent infinite loops
        let max_attempts = 1_000_000;

        for _ in 0..max_attempts {
            hasher.update(data);
            hasher.update(nonce.to_le_bytes());
            hasher.update(timestamp.to_le_bytes());

            let hash: [u8; 32] = hasher.finalize_reset().into();

            if Self::check_difficulty(&hash, difficulty) {
                return Ok(Self {
                    nonce,
                    timestamp,
                    difficulty,
                    hash,
                });
            }

            nonce += 1;
        }

        Err("Failed to generate proof of work within attempt limit")
    }

    /// Verify proof of work
    pub fn verify(&self, data: &[u8]) -> bool {
        // Check timestamp is reasonable (within 24 hours)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_secs();

        if self.timestamp > now + 3600 || self.timestamp < now.saturating_sub(86400) {
            return false;
        }

        // Verify hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(self.nonce.to_le_bytes());
        hasher.update(self.timestamp.to_le_bytes());

        let computed_hash: [u8; 32] = hasher.finalize().into();

        if computed_hash.ct_ne(&self.hash).into() {
            return false;
        }

        // Check difficulty
        Self::check_difficulty(&self.hash, self.difficulty)
    }

    /// Check if hash meets difficulty requirement
    fn check_difficulty(hash: &[u8; 32], difficulty: u32) -> bool {
        let required_zeros = difficulty / 8;
        let remainder_bits = difficulty % 8;

        // Check full zero bytes
        for i in 0..required_zeros as usize {
            if i >= hash.len() || hash[i] != 0 {
                return false;
            }
        }

        // Check partial byte
        if remainder_bits > 0 && (required_zeros as usize) < hash.len() {
            let mask = 0xFF << (8 - remainder_bits);
            if hash[required_zeros as usize] & mask != 0 {
                return false;
            }
        }

        true
    }
}

/// Proof of Work identity - prevents Sybil attacks
///
/// Feynman: Creating a new identity should cost something, otherwise
/// one bad actor could create millions of fake identities and overwhelm
/// the network. We make identity creation "expensive" by requiring
/// computational work - like making someone solve puzzles before joining.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfWorkIdentity {
    pub peer_id: PeerId,
    pub nonce: u64,
    pub timestamp: u64,
    pub difficulty: u32,
    pub hash: [u8; 32],
}

impl ProofOfWorkIdentity {
    /// Generate a new PoW identity with specified difficulty
    ///
    /// Feynman: This is like mining bitcoin, but for identities.
    /// We try different nonces until we find one that makes our
    /// identity hash start with enough zero bits. Higher difficulty
    /// means more zeros needed, which takes exponentially longer.
    pub fn generate(peer_id: PeerId, difficulty: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_secs();

        let mut nonce = 0u64;
        let mut hasher = Sha256::new();

        loop {
            hasher.update(peer_id);
            hasher.update(nonce.to_le_bytes());
            hasher.update(timestamp.to_le_bytes());

            let hash: [u8; 32] = hasher.finalize_reset().into();

            if Self::check_difficulty(&hash, difficulty) {
                return Self {
                    peer_id,
                    nonce,
                    timestamp,
                    difficulty,
                    hash,
                };
            }

            nonce += 1;
        }
    }

    /// Verify a PoW identity
    ///
    /// Feynman: Verification is fast - we just check that:
    /// 1. The hash is correct (matches peer_id + nonce + timestamp)
    /// 2. The hash has enough leading zeros (meets difficulty)
    /// 3. The timestamp is reasonable (not too old or future)
    pub fn verify(&self) -> bool {
        // Check timestamp is reasonable (within 24 hours)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_secs();

        if self.timestamp > now + 3600 {
            return false; // Too far in future
        }

        if self.timestamp < now.saturating_sub(86400) {
            return false; // Too old
        }

        // Verify hash
        let mut hasher = Sha256::new();
        hasher.update(self.peer_id);
        hasher.update(self.nonce.to_le_bytes());
        hasher.update(self.timestamp.to_le_bytes());

        let computed_hash: [u8; 32] = hasher.finalize().into();

        if computed_hash.ct_ne(&self.hash).into() {
            return false;
        }

        // Check difficulty
        Self::check_difficulty(&self.hash, self.difficulty)
    }

    /// Check if a hash meets the difficulty requirement
    fn check_difficulty(hash: &[u8; 32], difficulty: u32) -> bool {
        let required_zeros = difficulty / 8;
        let remainder_bits = difficulty % 8;

        // Check full zero bytes
        for i in 0..required_zeros as usize {
            if hash[i] != 0 {
                return false;
            }
        }

        // Check partial byte
        if remainder_bits > 0 && required_zeros < 32 {
            let mask = 0xFF << (8 - remainder_bits);
            if hash[required_zeros as usize] & mask != 0 {
                return false;
            }
        }

        true
    }

    /// Calculate the actual difficulty of a hash
    pub fn calculate_difficulty(hash: &[u8; 32]) -> u32 {
        let mut leading_zeros = 0;

        for &byte in hash {
            if byte == 0 {
                leading_zeros += 8;
            } else {
                leading_zeros += byte.leading_zeros();
                break;
            }
        }

        leading_zeros
    }
}

/// Adaptive PoW difficulty manager
///
/// Feynman: We want to keep identity creation at a steady rate.
/// If too many identities are being created, we increase difficulty.
/// If too few, we decrease it. This self-regulates the network growth.
pub struct AdaptiveDifficulty {
    current_difficulty: u32,
    target_rate: Duration,    // Target time between identities
    adjustment_window: usize, // Number of identities to average
    recent_identities: Vec<Instant>,
}

impl AdaptiveDifficulty {
    pub fn new(initial_difficulty: u32, target_rate: Duration) -> Self {
        Self {
            current_difficulty: initial_difficulty,
            target_rate,
            adjustment_window: 100,
            recent_identities: Vec::new(),
        }
    }

    /// Record a new identity creation and adjust difficulty
    pub fn record_identity(&mut self) -> u32 {
        let now = Instant::now();
        self.recent_identities.push(now);

        // Keep only recent identities
        if self.recent_identities.len() > self.adjustment_window {
            self.recent_identities.remove(0);
        }

        // Adjust difficulty if we have enough data
        if self.recent_identities.len() >= self.adjustment_window {
            let time_span = now - self.recent_identities[0];
            let actual_rate = time_span / self.adjustment_window as u32;

            if actual_rate < self.target_rate * 9 / 10 {
                // Too fast - increase difficulty
                self.current_difficulty = (self.current_difficulty + 1).min(32);
            } else if actual_rate > self.target_rate * 11 / 10 {
                // Too slow - decrease difficulty
                self.current_difficulty = self.current_difficulty.saturating_sub(1).max(8);
            }
        }

        self.current_difficulty
    }
}

/// Identity cache with verification
///
/// Feynman: We cache verified identities so we don't have to
/// re-verify them every time. But we also track "reputation" -
/// good behavior increases trust, bad behavior decreases it.
pub struct IdentityCache {
    verified: HashMap<PeerId, ProofOfWorkIdentity>,
    reputation: HashMap<PeerId, i32>,
    min_difficulty: u32,
}

impl IdentityCache {
    pub fn new(min_difficulty: u32) -> Self {
        Self {
            verified: HashMap::new(),
            reputation: HashMap::new(),
            min_difficulty,
        }
    }

    /// Verify and cache an identity
    pub fn verify_and_cache(&mut self, identity: ProofOfWorkIdentity) -> bool {
        // Check minimum difficulty
        if identity.difficulty < self.min_difficulty {
            return false;
        }

        // Verify the PoW
        if !identity.verify() {
            return false;
        }

        // Cache the identity
        let peer_id = identity.peer_id;
        self.verified.insert(peer_id, identity);
        self.reputation.insert(peer_id, 0);

        true
    }

    /// Update reputation for a peer
    pub fn update_reputation(&mut self, peer_id: &PeerId, delta: i32) {
        let rep = self.reputation.entry(*peer_id).or_insert(0);
        *rep = (*rep + delta).max(-100).min(100);
    }

    /// Check if a peer should be trusted
    pub fn is_trusted(&self, peer_id: &PeerId) -> bool {
        self.verified.contains_key(peer_id)
            && self.reputation.get(peer_id).copied().unwrap_or(0) >= -10
    }

    /// Evict untrusted peers
    pub fn cleanup(&mut self) {
        let to_remove: Vec<PeerId> = self
            .reputation
            .iter()
            .filter(|(_, &rep)| rep < -50)
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            self.verified.remove(&id);
            self.reputation.remove(&id);
        }
    }
}
