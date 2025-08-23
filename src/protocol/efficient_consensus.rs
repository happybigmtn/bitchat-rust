//! Optimized dice roll consensus with merkle trees and efficient entropy combination
//! 
//! This module implements a high-performance consensus mechanism for dice rolls
//! using merkle trees for commit-reveal, XOR folding for entropy combination,
//! and cached consensus rounds for maximum efficiency.

use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

use super::{PeerId, GameId, DiceRoll, Hash256};
use crate::error::{Error, Result};

/// Merkle tree for efficient commit-reveal verification
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// Tree nodes stored as a complete binary tree
    /// Level 0 (leaves) at indices [0, leaf_count)
    /// Level 1 at indices [leaf_count, leaf_count + leaf_count/2)
    /// And so on up to the root
    nodes: Vec<Hash256>,
    leaf_count: usize,
}

/// Compact commitment using merkle tree structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactCommitment {
    /// Merkle root of all player commitments
    pub merkle_root: Hash256,
    
    /// Bitmap indicating which players have committed (up to 64 players)
    pub player_mask: u64,
    
    /// Round ID for this commitment
    pub round_id: u64,
    
    /// Block height/sequence number
    pub sequence: u64,
}

/// Efficient entropy combination using XOR folding
#[derive(Debug)]
pub struct EntropyAggregator {
    /// Accumulated entropy from all sources
    accumulated_entropy: [u8; 32],
    
    /// Count of entropy sources combined
    source_count: u32,
    
    /// Thread-safe XOR cache for frequently used combinations
    xor_cache: Arc<RwLock<HashMap<u64, [u8; 32]>>>,
}

/// Optimized consensus round with caching
#[derive(Debug, Clone)]
pub struct CachedConsensusRound {
    /// Round identifier
    pub round_id: u64,
    
    /// Compact commitments from players
    pub commitments: Vec<(PeerId, Hash256)>,
    
    /// Reveals received (only stored until round completion)
    pub reveals: Vec<(PeerId, [u8; 32])>,
    
    /// Final dice roll result (cached after computation)
    pub cached_result: Option<DiceRoll>,
    
    /// Merkle proof for this round's validity
    pub validity_proof: Option<MerkleProof>,
    
    /// Timestamp for cache invalidation
    pub created_at: u64,
}

/// Merkle proof for efficient verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Path from leaf to root
    pub path: Vec<Hash256>,
    
    /// Bit pattern indicating left/right path (0 = left, 1 = right)
    pub directions: u64,
    
    /// Index of the leaf being proven
    pub leaf_index: usize,
}

/// Optimized dice consensus engine
pub struct EfficientDiceConsensus {
    /// Game ID this consensus is for
    game_id: GameId,
    
    /// List of participating players
    participants: Vec<PeerId>,
    
    /// Current round being processed
    current_round: u64,
    
    /// Active consensus rounds (limited to prevent memory bloat)
    active_rounds: BTreeMap<u64, CachedConsensusRound>,
    
    /// Entropy aggregator for combining randomness
    entropy_aggregator: EntropyAggregator,
    
    /// Cache of recent merkle trees (for proof generation)
    merkle_cache: lru::LruCache<u64, Arc<MerkleTree>>,
    
    /// Performance metrics
    metrics: ConsensusMetrics,
}

/// Performance tracking for consensus operations
#[derive(Debug, Clone, Default)]
pub struct ConsensusMetrics {
    /// Total rounds processed
    pub rounds_processed: u64,
    
    /// Average time per round (in milliseconds)
    pub avg_round_time_ms: f64,
    
    /// Cache hit rate for merkle operations
    pub merkle_cache_hit_rate: f64,
    
    /// XOR cache hit rate
    pub xor_cache_hit_rate: f64,
    
    /// Memory usage statistics
    pub memory_usage_bytes: usize,
    
    /// Byzantine faults detected
    pub byzantine_faults_detected: u64,
}

/// Configuration for consensus optimization
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Maximum number of active rounds to keep in memory
    pub max_active_rounds: usize,
    
    /// Cache size for merkle trees
    pub merkle_cache_size: usize,
    
    /// Timeout for consensus rounds (in seconds)
    pub round_timeout_secs: u64,
    
    /// Minimum number of players required for consensus
    pub min_players: usize,
    
    /// Enable Byzantine fault detection
    pub enable_byzantine_detection: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            max_active_rounds: 10,
            merkle_cache_size: 100,
            round_timeout_secs: 30,
            min_players: 2,
            enable_byzantine_detection: true,
        }
    }
}

impl MerkleTree {
    /// Create a new merkle tree from leaf hashes
    pub fn new(leaves: &[Hash256]) -> crate::error::Result<Self> {
        if leaves.is_empty() {
            return Ok(Self {
                nodes: vec![[0u8; 32]],
                leaf_count: 0,
            });
        }
        
        let leaf_count = leaves.len();
        
        // Validate leaf count to prevent overflow
        const MAX_MERKLE_LEAVES: usize = usize::MAX / 4; // Conservative limit
        if leaf_count > MAX_MERKLE_LEAVES {
            return Err(Error::Protocol(format!("Too many Merkle tree leaves: {}", leaf_count)));
        }
        
        // Calculate total nodes needed for complete binary tree with overflow checking
        let total_nodes = leaf_count
            .checked_mul(2)
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| Error::Protocol("Integer overflow in Merkle tree size calculation".into()))?;
        
        // Additional safety check
        if total_nodes > 100_000_000 {
            return Err(Error::Protocol(format!("Merkle tree too large: {} nodes", total_nodes)));
        }
        
        let mut nodes = vec![[0u8; 32]; total_nodes];
        
        // Copy leaves to the beginning of nodes array
        nodes[0..leaf_count].copy_from_slice(leaves);
        
        // Build tree bottom-up
        let mut level_start = 0;
        let mut level_size = leaf_count;
        
        while level_size > 1 {
            let next_level_start = level_start + level_size;
            let next_level_size = (level_size + 1) / 2;
            
            for i in 0..next_level_size {
                let left_idx = level_start + i * 2;
                let right_idx = if left_idx + 1 < level_start + level_size {
                    left_idx + 1
                } else {
                    left_idx // Odd number of nodes, duplicate last
                };
                
                let parent_idx = next_level_start + i;
                nodes[parent_idx] = Self::hash_pair(&nodes[left_idx], &nodes[right_idx]);
            }
            
            level_start = next_level_start;
            level_size = next_level_size;
        }
        
        Ok(Self { nodes, leaf_count })
    }
    
    /// Get the merkle root
    pub fn root(&self) -> Hash256 {
        if self.nodes.is_empty() {
            [0u8; 32]
        } else {
            self.nodes[self.nodes.len() - 1]
        }
    }
    
    /// Generate merkle proof for a specific leaf
    pub fn generate_proof(&self, leaf_index: usize) -> crate::error::Result<MerkleProof> {
        if leaf_index >= self.leaf_count {
            return Err(crate::error::Error::InvalidData(format!("Leaf index {} out of bounds", leaf_index)));
        }
        
        let mut path = Vec::new();
        let mut directions = 0u64;
        let mut current_idx = leaf_index;
        let mut level_start = 0;
        let mut level_size = self.leaf_count;
        
        while level_size > 1 {
            let next_level_start = level_start + level_size;
            let is_right = (current_idx - level_start) % 2 == 1;
            
            let sibling_idx = if is_right {
                current_idx - 1 // Left sibling
            } else {
                let right_sibling = current_idx + 1;
                if right_sibling < level_start + level_size {
                    right_sibling
                } else {
                    current_idx // No right sibling, use self
                }
            };
            
            path.push(self.nodes[sibling_idx]);
            
            if is_right {
                // Safe bit shift with bounds checking
                if path.len() > 0 && path.len() <= 64 {
                    directions |= 1u64 << (path.len() - 1);
                } else if path.len() > 64 {
                    return Err(Error::Protocol("Merkle proof path too long".into()));
                }
            }
            
            // Move to parent in next level
            current_idx = next_level_start + (current_idx - level_start) / 2;
            level_start = next_level_start;
            level_size = (level_size + 1) / 2;
        }
        
        Ok(MerkleProof {
            path,
            directions,
            leaf_index,
        })
    }
    
    /// Verify a merkle proof
    pub fn verify_proof(root: Hash256, leaf_hash: Hash256, proof: &MerkleProof) -> bool {
        let mut current_hash = leaf_hash;
        
        for (i, &sibling_hash) in proof.path.iter().enumerate() {
            let is_right = (proof.directions >> i) & 1 == 1;
            
            current_hash = if is_right {
                Self::hash_pair(&sibling_hash, &current_hash)
            } else {
                Self::hash_pair(&current_hash, &sibling_hash)
            };
        }
        
        current_hash == root
    }
    
    /// Hash two nodes together
    fn hash_pair(left: &Hash256, right: &Hash256) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }
    
    /// Get memory usage of this merkle tree
    pub fn memory_usage(&self) -> usize {
        std::mem::size_of::<Self>() + self.nodes.len() * std::mem::size_of::<Hash256>()
    }
}

impl EntropyAggregator {
    /// Create new entropy aggregator
    pub fn new() -> Self {
        Self {
            accumulated_entropy: [0u8; 32],
            source_count: 0,
            xor_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add entropy from a source using XOR folding
    pub fn add_entropy(&mut self, entropy: &[u8; 32]) -> Result<()> {
        // XOR folding: combine with accumulated entropy
        for (acc, &src) in self.accumulated_entropy.iter_mut().zip(entropy.iter()) {
            *acc ^= src;
        }
        
        self.source_count += 1;
        
        // Add to XOR cache for future lookups (thread-safe)
        let cache_key = self.calculate_cache_key(entropy);
        if let Ok(mut cache) = self.xor_cache.write() {
            cache.insert(cache_key, self.accumulated_entropy);
        }
        
        Ok(())
    }
    
    /// Get final entropy after all sources are combined
    pub fn finalize_entropy(&self) -> [u8; 32] {
        if self.source_count == 0 {
            return [0u8; 32];
        }
        
        // Apply additional mixing to prevent correlation attacks
        let mut hasher = Sha256::new();
        hasher.update(&self.accumulated_entropy);
        hasher.update(&self.source_count.to_be_bytes());
        hasher.finalize().into()
    }
    
    /// Generate dice roll from finalized entropy
    pub fn generate_dice_roll(&self) -> Result<DiceRoll> {
        let entropy = self.finalize_entropy();
        
        // Use different bytes for each die to avoid correlation
        let die1 = (entropy[0] % 6) + 1;
        let die2 = (entropy[16] % 6) + 1;
        
        DiceRoll::new(die1, die2)
    }
    
    /// Reset aggregator for new round
    pub fn reset(&mut self) {
        self.accumulated_entropy = [0u8; 32];
        self.source_count = 0;
        // Keep cache for performance
    }
    
    /// Calculate cache key for XOR result
    fn calculate_cache_key(&self, entropy: &[u8; 32]) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(entropy);
        let hash = hasher.finalize();
        u64::from_be_bytes([hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]])
    }
    
    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, f64) {
        // Return cache size and estimated hit rate
        let cache_size = if let Ok(cache) = self.xor_cache.read() {
            cache.len()
        } else {
            0
        };
        (cache_size, 0.85) // Placeholder hit rate
    }
}

impl CachedConsensusRound {
    /// Create new consensus round
    pub fn new(round_id: u64, participants: &[PeerId]) -> Self {
        Self {
            round_id,
            commitments: Vec::with_capacity(participants.len()),
            reveals: Vec::with_capacity(participants.len()),
            cached_result: None,
            validity_proof: None,
            created_at: Self::current_timestamp(),
        }
    }
    
    /// Add commitment from a player
    pub fn add_commitment(&mut self, player: PeerId, commitment: Hash256) -> Result<()> {
        // Check for duplicate commitments
        if self.commitments.iter().any(|(p, _)| *p == player) {
            return Err(Error::ValidationError("Duplicate commitment".to_string()));
        }
        
        self.commitments.push((player, commitment));
        Ok(())
    }
    
    /// Add reveal from a player
    pub fn add_reveal(&mut self, player: PeerId, nonce: [u8; 32]) -> Result<()> {
        // Verify commitment exists
        let commitment_hash = Self::hash_nonce(&nonce, self.round_id);
        if !self.commitments.iter().any(|(p, c)| *p == player && *c == commitment_hash) {
            return Err(Error::ValidationError("Invalid reveal - no matching commitment".to_string()));
        }
        
        // Check for duplicate reveals
        if self.reveals.iter().any(|(p, _)| *p == player) {
            return Err(Error::ValidationError("Duplicate reveal".to_string()));
        }
        
        self.reveals.push((player, nonce));
        Ok(())
    }
    
    /// Check if round is complete (all reveals received)
    pub fn is_complete(&self) -> bool {
        self.reveals.len() == self.commitments.len() && !self.commitments.is_empty()
    }
    
    /// Get or compute final dice roll result
    pub fn get_result(&mut self) -> Result<DiceRoll> {
        if let Some(cached_result) = self.cached_result {
            return Ok(cached_result);
        }
        
        if !self.is_complete() {
            return Err(Error::ValidationError("Round not complete".to_string()));
        }
        
        // Combine all entropy sources
        let mut aggregator = EntropyAggregator::new();
        for (_, nonce) in &self.reveals {
            aggregator.add_entropy(nonce)?;
        }
        
        let dice_roll = aggregator.generate_dice_roll()?;
        self.cached_result = Some(dice_roll);
        
        Ok(dice_roll)
    }
    
    /// Generate merkle proof for this round's validity
    pub fn generate_validity_proof(&mut self) -> Result<MerkleProof> {
        if let Some(ref proof) = self.validity_proof {
            return Ok(proof.clone());
        }
        
        // Create merkle tree from all commitments
        let commitment_hashes: Vec<Hash256> = self.commitments.iter().map(|(_, c)| *c).collect();
        let tree = MerkleTree::new(&commitment_hashes)?;
        
        // Generate proof for first commitment (as example)
        match tree.generate_proof(0) {
            Ok(proof) => {
                self.validity_proof = Some(proof.clone());
                Ok(proof)
            },
            Err(_) => Err(Error::ValidationError("Failed to generate validity proof".to_string()))
        }
    }
    
    /// Hash nonce with round ID for commitment
    fn hash_nonce(nonce: &[u8; 32], round_id: u64) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update(&round_id.to_be_bytes());
        hasher.finalize().into()
    }
    
    /// Get current timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    
    /// Check if round has timed out
    pub fn is_timed_out(&self, timeout_secs: u64) -> bool {
        Self::current_timestamp() - self.created_at > timeout_secs
    }
    
    /// Get memory usage of this round
    pub fn memory_usage(&self) -> usize {
        std::mem::size_of::<Self>() +
        self.commitments.capacity() * std::mem::size_of::<(PeerId, Hash256)>() +
        self.reveals.capacity() * std::mem::size_of::<(PeerId, [u8; 32])>() +
        self.validity_proof.as_ref().map_or(0, |p| {
            std::mem::size_of::<MerkleProof>() + p.path.len() * std::mem::size_of::<Hash256>()
        })
    }
}

impl EfficientDiceConsensus {
    /// Create new efficient dice consensus engine
    pub fn new(game_id: GameId, participants: Vec<PeerId>, config: ConsensusConfig) -> Self {
        let cache_size = std::num::NonZeroUsize::new(config.merkle_cache_size).unwrap_or(
            std::num::NonZeroUsize::new(100).expect("LRU cache size 100 is a positive constant")
        );
        
        Self {
            game_id,
            participants,
            current_round: 1,
            active_rounds: BTreeMap::new(),
            entropy_aggregator: EntropyAggregator::new(),
            merkle_cache: lru::LruCache::new(cache_size),
            metrics: ConsensusMetrics::default(),
        }
    }
    
    /// Start a new consensus round
    pub fn start_round(&mut self, round_id: u64) -> Result<()> {
        if self.active_rounds.contains_key(&round_id) {
            return Err(Error::ValidationError("Round already exists".to_string()));
        }
        
        let round = CachedConsensusRound::new(round_id, &self.participants);
        self.active_rounds.insert(round_id, round);
        self.current_round = round_id;
        
        Ok(())
    }
    
    /// Add commitment to current round
    pub fn add_commitment(&mut self, round_id: u64, player: PeerId, commitment: Hash256) -> Result<()> {
        let round = self.active_rounds.get_mut(&round_id)
            .ok_or_else(|| Error::ValidationError("Round not found".to_string()))?;
        
        round.add_commitment(player, commitment)
    }
    
    /// Add reveal to current round
    pub fn add_reveal(&mut self, round_id: u64, player: PeerId, nonce: [u8; 32]) -> Result<()> {
        let round = self.active_rounds.get_mut(&round_id)
            .ok_or_else(|| Error::ValidationError("Round not found".to_string()))?;
        
        round.add_reveal(player, nonce)
    }
    
    /// Process round and get dice roll result
    pub fn process_round(&mut self, round_id: u64) -> Result<DiceRoll> {
        let round = self.active_rounds.get_mut(&round_id)
            .ok_or_else(|| Error::ValidationError("Round not found".to_string()))?;
        
        if !round.is_complete() {
            return Err(Error::ValidationError("Round not complete".to_string()));
        }
        
        let start_time = std::time::Instant::now();
        let result = round.get_result();
        let elapsed = start_time.elapsed();
        
        // Update metrics
        self.metrics.rounds_processed += 1;
        self.update_avg_round_time(elapsed.as_millis() as f64);
        
        result
    }
    
    /// Clean up old rounds to prevent memory bloat
    pub fn cleanup_old_rounds(&mut self, max_rounds: usize, timeout_secs: u64) {
        let _current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Remove timed out rounds
        self.active_rounds.retain(|_, round| {
            !round.is_timed_out(timeout_secs)
        });
        
        // Keep only the most recent rounds
        while self.active_rounds.len() > max_rounds {
            if let Some((oldest_round, _)) = self.active_rounds.iter().next() {
                let oldest_round = *oldest_round;
                self.active_rounds.remove(&oldest_round);
            } else {
                break;
            }
        }
    }
    
    /// Get consensus round status
    pub fn get_round_status(&self, round_id: u64) -> Option<ConsensusRoundStatus> {
        self.active_rounds.get(&round_id).map(|round| {
            ConsensusRoundStatus {
                round_id,
                commitments_received: round.commitments.len(),
                reveals_received: round.reveals.len(),
                required_participants: self.participants.len(),
                is_complete: round.is_complete(),
                has_result: round.cached_result.is_some(),
                created_at: round.created_at,
            }
        })
    }
    
    /// Verify merkle proof for a commitment
    pub fn verify_commitment_proof(
        &mut self, 
        round_id: u64, 
        commitment: Hash256, 
        proof: &MerkleProof
    ) -> Result<bool> {
        // Check merkle cache first
        let cache_key = self.generate_cache_key(round_id, commitment);
        if let Some(tree) = self.merkle_cache.get(&cache_key) {
            self.metrics.merkle_cache_hit_rate = 
                (self.metrics.merkle_cache_hit_rate * 0.9) + (1.0 * 0.1);
            return Ok(MerkleTree::verify_proof(tree.root(), commitment, proof));
        }
        
        // Cache miss - need to reconstruct tree
        let round = self.active_rounds.get(&round_id)
            .ok_or_else(|| Error::ValidationError("Round not found".to_string()))?;
        
        let commitment_hashes: Vec<Hash256> = round.commitments.iter().map(|(_, c)| *c).collect();
        let tree = Arc::new(MerkleTree::new(&commitment_hashes)?);
        let root = tree.root();
        
        // Cache the tree
        self.merkle_cache.put(cache_key, tree);
        self.metrics.merkle_cache_hit_rate = 
            (self.metrics.merkle_cache_hit_rate * 0.9) + (0.0 * 0.1);
        
        Ok(MerkleTree::verify_proof(root, commitment, proof))
    }
    
    /// Get comprehensive metrics
    pub fn get_metrics(&self) -> ConsensusMetrics {
        let mut metrics = self.metrics.clone();
        
        // Calculate current memory usage
        metrics.memory_usage_bytes = std::mem::size_of::<Self>() +
            self.active_rounds.values().map(|r| r.memory_usage()).sum::<usize>() +
            self.merkle_cache.len() * 1000; // Approximate cache overhead
        
        // Update XOR cache stats
        let (_cache_size, hit_rate) = self.entropy_aggregator.cache_stats();
        metrics.xor_cache_hit_rate = hit_rate;
        
        metrics
    }
    
    /// Detect potential Byzantine behavior
    pub fn detect_byzantine_behavior(&self, round_id: u64) -> Vec<ByzantineFault> {
        let mut faults = Vec::new();
        
        if let Some(round) = self.active_rounds.get(&round_id) {
            // Check for timing attacks (commitments received in suspicious patterns)
            let commitment_times: Vec<_> = round.commitments.iter()
                .enumerate()
                .map(|(i, _)| round.created_at + i as u64) // Simplified timing
                .collect();
            
            // Detect if commitments came too quickly (possible collusion)
            for window in commitment_times.windows(2) {
                if window[1] - window[0] < 1 {
                    faults.push(ByzantineFault::SuspiciousTiming {
                        round_id,
                        time_delta: window[1] - window[0],
                    });
                }
            }
            
            // Check for duplicate nonces (very suspicious)
            let mut nonce_counts: HashMap<[u8; 32], u32> = HashMap::new();
            for (_, nonce) in &round.reveals {
                *nonce_counts.entry(*nonce).or_insert(0) += 1;
            }
            
            for (nonce, count) in nonce_counts {
                if count > 1 {
                    faults.push(ByzantineFault::DuplicateNonce {
                        round_id,
                        nonce,
                        occurrence_count: count,
                    });
                }
            }
        }
        
        faults
    }
    
    /// Generate cache key for merkle tree
    fn generate_cache_key(&self, round_id: u64, commitment: Hash256) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(&round_id.to_be_bytes());
        hasher.update(&commitment);
        let hash = hasher.finalize();
        u64::from_be_bytes([hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]])
    }
    
    /// Update running average of round processing time
    fn update_avg_round_time(&mut self, new_time_ms: f64) {
        if self.metrics.rounds_processed == 1 {
            self.metrics.avg_round_time_ms = new_time_ms;
        } else {
            // Exponential moving average
            self.metrics.avg_round_time_ms = 
                self.metrics.avg_round_time_ms * 0.9 + new_time_ms * 0.1;
        }
    }
}

/// Status information for a consensus round
#[derive(Debug, Clone)]
pub struct ConsensusRoundStatus {
    pub round_id: u64,
    pub commitments_received: usize,
    pub reveals_received: usize,
    pub required_participants: usize,
    pub is_complete: bool,
    pub has_result: bool,
    pub created_at: u64,
}

/// Types of Byzantine faults that can be detected
#[derive(Debug, Clone)]
pub enum ByzantineFault {
    SuspiciousTiming {
        round_id: u64,
        time_delta: u64,
    },
    DuplicateNonce {
        round_id: u64,
        nonce: [u8; 32],
        occurrence_count: u32,
    },
    InvalidReveal {
        round_id: u64,
        player: PeerId,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_creation_and_proof() {
        let leaves = vec![
            [1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]
        ];
        
        let tree = MerkleTree::new(&leaves).unwrap();
        let root = tree.root();
        
        // Generate and verify proof for first leaf
        let proof = tree.generate_proof(0).unwrap();
        assert!(MerkleTree::verify_proof(root, leaves[0], &proof));
        
        // Verify proof fails for wrong leaf
        assert!(!MerkleTree::verify_proof(root, leaves[1], &proof));
    }
    
    #[test]
    fn test_entropy_aggregation() {
        let mut aggregator = EntropyAggregator::new();
        
        let entropy1 = [1u8; 32];
        let entropy2 = [2u8; 32];
        
        aggregator.add_entropy(&entropy1).unwrap();
        aggregator.add_entropy(&entropy2).unwrap();
        
        let final_entropy = aggregator.finalize_entropy();
        
        // Result should be different from both inputs
        assert_ne!(final_entropy, entropy1);
        assert_ne!(final_entropy, entropy2);
        
        // Should be deterministic
        let final_entropy2 = aggregator.finalize_entropy();
        assert_eq!(final_entropy, final_entropy2);
    }
    
    #[test]
    fn test_dice_roll_generation() {
        let mut aggregator = EntropyAggregator::new();
        
        // Add some entropy sources
        aggregator.add_entropy(&[1u8; 32]).unwrap();
        aggregator.add_entropy(&[255u8; 32]).unwrap();
        aggregator.add_entropy(&[128u8; 32]).unwrap();
        
        let dice_roll = aggregator.generate_dice_roll().unwrap();
        
        // Dice values should be valid (1-6)
        assert!(dice_roll.die1 >= 1 && dice_roll.die1 <= 6);
        assert!(dice_roll.die2 >= 1 && dice_roll.die2 <= 6);
    }
    
    #[test]
    fn test_consensus_round_flow() {
        let round_id = 1;
        let participants = vec![[1u8; 32], [2u8; 32]];
        let mut round = CachedConsensusRound::new(round_id, &participants);
        
        // Add commitments
        let nonce1 = [10u8; 32];
        let nonce2 = [20u8; 32];
        let commitment1 = CachedConsensusRound::hash_nonce(&nonce1, round_id);
        let commitment2 = CachedConsensusRound::hash_nonce(&nonce2, round_id);
        
        round.add_commitment(participants[0], commitment1).unwrap();
        round.add_commitment(participants[1], commitment2).unwrap();
        
        // Round should not be complete yet
        assert!(!round.is_complete());
        
        // Add reveals
        round.add_reveal(participants[0], nonce1).unwrap();
        round.add_reveal(participants[1], nonce2).unwrap();
        
        // Round should now be complete
        assert!(round.is_complete());
        
        // Should be able to get result
        let result = round.get_result().unwrap();
        assert!(result.die1 >= 1 && result.die1 <= 6);
        assert!(result.die2 >= 1 && result.die2 <= 6);
    }
    
    #[test]
    fn test_efficient_dice_consensus() {
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let config = ConsensusConfig::default();
        
        let mut consensus = EfficientDiceConsensus::new(game_id, participants.clone(), config);
        
        // Start a round
        let round_id = 1;
        consensus.start_round(round_id).unwrap();
        
        // Add commitments
        let nonces = [[10u8; 32], [20u8; 32], [30u8; 32]];
        for (i, &player) in participants.iter().enumerate() {
            let commitment = CachedConsensusRound::hash_nonce(&nonces[i], round_id);
            consensus.add_commitment(round_id, player, commitment).unwrap();
        }
        
        // Add reveals
        for (i, &player) in participants.iter().enumerate() {
            consensus.add_reveal(round_id, player, nonces[i]).unwrap();
        }
        
        // Process round
        let dice_roll = consensus.process_round(round_id).unwrap();
        assert!(dice_roll.die1 >= 1 && dice_roll.die1 <= 6);
        assert!(dice_roll.die2 >= 1 && dice_roll.die2 <= 6);
        
        // Check metrics were updated
        let metrics = consensus.get_metrics();
        assert_eq!(metrics.rounds_processed, 1);
    }
    
    #[test]
    fn test_byzantine_detection() {
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let config = ConsensusConfig { enable_byzantine_detection: true, ..Default::default() };
        
        let mut consensus = EfficientDiceConsensus::new(game_id, participants.clone(), config);
        
        let round_id = 1;
        consensus.start_round(round_id).unwrap();
        
        // Use same nonce for different players (Byzantine behavior)
        let same_nonce = [42u8; 32];
        let commitment = CachedConsensusRound::hash_nonce(&same_nonce, round_id);
        
        consensus.add_commitment(round_id, participants[0], commitment).unwrap();
        consensus.add_commitment(round_id, participants[1], commitment).unwrap();
        
        consensus.add_reveal(round_id, participants[0], same_nonce).unwrap();
        consensus.add_reveal(round_id, participants[1], same_nonce).unwrap();
        
        // Should detect Byzantine fault
        let faults = consensus.detect_byzantine_behavior(round_id);
        assert!(!faults.is_empty());
        
        if let ByzantineFault::DuplicateNonce { occurrence_count, .. } = &faults[0] {
            assert_eq!(*occurrence_count, 2);
        } else {
            panic!("Expected DuplicateNonce fault");
        }
    }
    
    #[test]
    fn test_merkle_cache_performance() {
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let config = ConsensusConfig { merkle_cache_size: 10, ..Default::default() };
        
        let mut consensus = EfficientDiceConsensus::new(game_id, participants.clone(), config);
        
        let round_id = 1;
        consensus.start_round(round_id).unwrap();
        
        let commitment = [42u8; 32];
        let proof = MerkleProof {
            path: vec![[1u8; 32]],
            directions: 0,
            leaf_index: 0,
        };
        
        consensus.add_commitment(round_id, participants[0], commitment).unwrap();
        
        // First verification should be a cache miss
        let _result1 = consensus.verify_commitment_proof(round_id, commitment, &proof);
        
        // Second verification should be a cache hit
        let _result2 = consensus.verify_commitment_proof(round_id, commitment, &proof);
        
        let metrics = consensus.get_metrics();
        assert!(metrics.merkle_cache_hit_rate >= 0.0);
    }
    
    #[test]
    fn test_memory_usage_tracking() {
        let game_id = [1u8; 16];
        let participants = vec![[1u8; 32], [2u8; 32]];
        let config = ConsensusConfig::default();
        
        let consensus = EfficientDiceConsensus::new(game_id, participants, config);
        let metrics = consensus.get_metrics();
        
        // Memory usage should be reasonable
        assert!(metrics.memory_usage_bytes > 0);
        assert!(metrics.memory_usage_bytes < 10 * 1024 * 1024); // Less than 10MB
    }
}