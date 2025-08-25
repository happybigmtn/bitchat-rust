//! Merkle tree operations for efficient state verification

use std::collections::HashMap;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use subtle::ConstantTimeEq;

use crate::protocol::{GameId, Hash256};
use crate::error::Result;

use super::state_manager::GameStateNode;

/// Merkle tree for state synchronization
pub struct StateMerkleTree {
    /// Tree nodes organized by level
    levels: Vec<Vec<MerkleNode>>,
    
    /// Mapping from game ID to leaf position
    game_positions: HashMap<GameId, usize>,
    
    /// Root hash of the entire tree
    root_hash: Hash256,
    
    /// Last update timestamp
    last_updated: u64,
}

/// Node in the merkle tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Hash value for this node
    pub hash: Hash256,
    
    /// Game IDs covered by this subtree (for leaves, single game)
    pub game_ids: Vec<GameId>,
    
    /// Child node indices (empty for leaves)
    pub children: Vec<usize>,
    
    /// Metadata for sync optimization
    pub metadata: NodeMetadata,
}

/// Metadata for merkle tree nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Number of games in subtree
    pub game_count: u32,
    
    /// Total size of states in subtree
    pub total_size: u64,
    
    /// Latest update timestamp in subtree
    pub latest_update: u64,
    
    /// Depth in the tree (0 = leaf)
    pub depth: u8,
}

/// Merkle proof for state verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash path to root
    pub path: Vec<Hash256>,
    
    /// Binary directions (0=left, 1=right)
    pub directions: u64,
    
    /// Index of leaf in tree
    pub leaf_index: usize,
}

impl Default for StateMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMerkleTree {
    /// Create new merkle tree for state synchronization
    pub fn new() -> Self {
        Self {
            levels: Vec::new(),
            game_positions: HashMap::new(),
            root_hash: [0u8; 32],
            last_updated: Self::current_timestamp(),
        }
    }
    
    /// Get current timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    
    /// Get root hash
    pub fn root(&self) -> Hash256 {
        self.root_hash
    }
    
    /// Update tree with new game state
    pub fn update_state(&mut self, game_id: GameId, state_node: &GameStateNode) -> Result<()> {
        // Add or update leaf node
        if let Some(&position) = self.game_positions.get(&game_id) {
            // Update existing game
            if self.levels.is_empty() {
                self.levels.push(Vec::new());
            }
            
            if position < self.levels[0].len() {
                self.levels[0][position] = self.create_leaf_node(game_id, state_node);
            }
        } else {
            // Add new game
            if self.levels.is_empty() {
                self.levels.push(Vec::new());
            }
            
            let position = self.levels[0].len();
            let leaf_node = self.create_leaf_node(game_id, state_node);
            self.levels[0].push(leaf_node);
            self.game_positions.insert(game_id, position);
        }
        
        // Rebuild tree from bottom up
        self.rebuild_tree()?;
        self.last_updated = Self::current_timestamp();
        
        Ok(())
    }
    
    /// Create leaf node for a game state
    fn create_leaf_node(&self, game_id: GameId, state_node: &GameStateNode) -> MerkleNode {
        MerkleNode {
            hash: state_node.state_hash,
            game_ids: vec![game_id],
            children: Vec::new(),
            metadata: NodeMetadata {
                game_count: 1,
                total_size: state_node.size_bytes as u64,
                latest_update: state_node.timestamp,
                depth: 0,
            },
        }
    }
    
    /// Rebuild internal nodes of the tree
    fn rebuild_tree(&mut self) -> Result<()> {
        if self.levels.is_empty() || self.levels[0].is_empty() {
            self.root_hash = [0u8; 32];
            return Ok(());
        }
        
        let mut current_level = 0;
        
        // Build levels until we reach the root
        while self.levels[current_level].len() > 1 {
            let next_level = current_level + 1;
            
            // Ensure next level exists
            if self.levels.len() <= next_level {
                self.levels.push(Vec::new());
            } else {
                self.levels[next_level].clear();
            }
            
            // Create parent nodes
            let mut i = 0;
            while i < self.levels[current_level].len() {
                let left_child = &self.levels[current_level][i];
                let right_child = if i + 1 < self.levels[current_level].len() {
                    Some(&self.levels[current_level][i + 1])
                } else {
                    None
                };
                
                let parent_node = self.create_parent_node(left_child, right_child, (current_level + 1) as u8);
                self.levels[next_level].push(parent_node);
                
                i += 2;
            }
            
            current_level = next_level;
        }
        
        // Set root hash
        if !self.levels[current_level].is_empty() {
            self.root_hash = self.levels[current_level][0].hash;
        }
        
        Ok(())
    }
    
    /// Create parent node from children
    fn create_parent_node(&self, left: &MerkleNode, right: Option<&MerkleNode>, depth: u8) -> MerkleNode {
        let mut hasher = Sha256::new();
        hasher.update(left.hash);
        
        let mut game_ids = left.game_ids.clone();
        let mut total_size = left.metadata.total_size;
        let mut game_count = left.metadata.game_count;
        let mut latest_update = left.metadata.latest_update;
        
        if let Some(right_child) = right {
            hasher.update(right_child.hash);
            game_ids.extend(right_child.game_ids.iter());
            total_size += right_child.metadata.total_size;
            game_count += right_child.metadata.game_count;
            latest_update = latest_update.max(right_child.metadata.latest_update);
        }
        
        MerkleNode {
            hash: hasher.finalize().into(),
            game_ids,
            children: if right.is_some() { vec![0, 1] } else { vec![0] }, // Simplified child indices
            metadata: NodeMetadata {
                game_count,
                total_size,
                latest_update,
                depth,
            },
        }
    }
    
    /// Get merkle proof for a specific game
    pub fn get_proof(&self, game_id: GameId) -> Option<MerkleProof> {
        let position = self.game_positions.get(&game_id)?;
        
        let mut proof = Vec::new();
        let mut current_pos = *position;
        
        for level in 0..self.levels.len() - 1 {
            let sibling_pos = if current_pos % 2 == 0 {
                current_pos + 1
            } else {
                current_pos - 1
            };
            
            if sibling_pos < self.levels[level].len() {
                proof.push(self.levels[level][sibling_pos].hash);
            }
            
            current_pos /= 2;
        }
        
        Some(MerkleProof {
            path: proof,
            directions: 0, // Simplified
            leaf_index: *position,
        })
    }
    
    /// Get nodes that differ from another tree
    pub fn find_differences(&self, other_root: Hash256, max_depth: usize) -> Vec<Vec<usize>> {
        let mut differences = Vec::new();
        
        if self.root_hash != other_root {
            // Trees differ - would implement recursive comparison here
            // Recursively find differences in the tree
            self.find_differences_recursive(&mut differences, 0, 0, max_depth);
        }
        
        differences
    }
    
    /// Recursively find differences in tree nodes
    fn find_differences_recursive(&self, differences: &mut Vec<Vec<usize>>, level: usize, index: usize, max_depth: usize) {
        if level >= self.levels.len() || level >= max_depth {
            return;
        }
        
        // Add current position as a difference
        let path = vec![level, index];
        differences.push(path);
        
        // If not at leaf level, check children
        if level > 0 {
            let child_level = level - 1;
            let left_child = index * 2;
            let right_child = index * 2 + 1;
            
            if left_child < self.levels[child_level].len() {
                self.find_differences_recursive(differences, child_level, left_child, max_depth);
            }
            if right_child < self.levels[child_level].len() {
                self.find_differences_recursive(differences, child_level, right_child, max_depth);
            }
        }
    }
    
    /// Get memory usage
    pub fn memory_usage(&self) -> usize {
        let levels_size = self.levels.iter()
            .map(|level| level.len() * std::mem::size_of::<MerkleNode>())
            .sum::<usize>();
        
        let positions_size = self.game_positions.len() * (std::mem::size_of::<GameId>() + std::mem::size_of::<usize>());
        
        levels_size + positions_size + std::mem::size_of::<Self>()
    }
}

impl MerkleProof {
    /// Verify proof against root hash
    pub fn verify(&self, leaf_hash: Hash256, root_hash: Hash256) -> bool {
        let mut current_hash = leaf_hash;
        let mut directions = self.directions;
        
        for sibling_hash in &self.path {
            let mut hasher = Sha256::new();
            
            if directions & 1 == 0 {
                // Current node is left child
                hasher.update(current_hash);
                hasher.update(sibling_hash);
            } else {
                // Current node is right child
                hasher.update(sibling_hash);
                hasher.update(current_hash);
            }
            
            current_hash = hasher.finalize().into();
            directions >>= 1;
        }
        
        current_hash.ct_eq(&root_hash).into()
    }
}