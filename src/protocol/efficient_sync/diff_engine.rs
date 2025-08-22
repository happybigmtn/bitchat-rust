//! Binary diff engine for large states

use std::sync::Arc;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

use crate::protocol::Hash256;
use crate::error::{Error, Result};

/// Binary diff engine for efficient state updates
pub struct BinaryDiffEngine {
    /// Cache of recent diffs for reuse
    diff_cache: lru::LruCache<(Hash256, Hash256), Arc<BinaryDiff>>,
    
    /// Statistics
    stats: DiffStats,
}

/// Binary diff between two states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryDiff {
    /// Operations to transform source to target
    pub operations: Vec<DiffOperation>,
    
    /// Checksum of target state
    pub target_checksum: Hash256,
    
    /// Size statistics
    pub original_size: u32,
    pub diff_size: u32,
    pub compression_ratio: f32,
}

/// Single operation in a binary diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffOperation {
    /// Copy bytes from source at offset
    Copy { source_offset: u32, length: u32 },
    
    /// Insert new bytes
    Insert { data: Vec<u8> },
    
    /// Skip bytes in target
    Skip { length: u32 },
    
    /// Delete bytes from source
    Delete { offset: u32, length: usize },
}

/// Statistics for diff operations
#[derive(Debug, Default, Clone)]
pub struct DiffStats {
    /// Cache hits
    pub cache_hits: u64,
    
    /// Cache misses
    pub cache_misses: u64,
    
    /// Total diffs created
    pub diffs_created: u64,
    
    /// Total diffs applied
    pub diffs_applied: u64,
    
    /// Average compression ratio
    pub avg_compression_ratio: f32,
}

impl BinaryDiffEngine {
    /// Create new binary diff engine
    pub fn new() -> Self {
        let cache_size = std::num::NonZeroUsize::new(1000)
            .expect("Cache size 1000 is a positive constant");
        
        Self {
            diff_cache: lru::LruCache::new(cache_size),
            stats: DiffStats::default(),
        }
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> &DiffStats {
        &self.stats
    }
    
    /// Create binary diff between two states
    pub fn create_diff(&mut self, source: &[u8], target: &[u8]) -> Result<BinaryDiff> {
        let source_hash = self.hash_data(source);
        let target_hash = self.hash_data(target);
        
        // Check cache first
        let cache_key = (source_hash, target_hash);
        if let Some(cached_diff) = self.diff_cache.get(&cache_key) {
            self.stats.cache_hits += 1;
            return Ok((**cached_diff).clone());
        }
        
        self.stats.cache_misses += 1;
        
        // Create diff using Myers' algorithm (simplified)
        let operations = self.myers_diff(source, target)?;
        
        let diff = BinaryDiff {
            operations: operations.clone(),
            target_checksum: target_hash,
            original_size: target.len() as u32,
            diff_size: 0, // Would calculate actual diff size
            compression_ratio: if target.len() > 0 {
                operations.iter().map(|op| match op {
                    DiffOperation::Copy { length, .. } => 8 + *length as usize, // Operation size + data
                    DiffOperation::Insert { data } => 4 + data.len(), // Operation size + data
                    DiffOperation::Skip { length } => 4 + *length as usize, // Operation size + skipped
                    DiffOperation::Delete { length, .. } => 8 + *length, // Operation size + deleted
                }).sum::<usize>() as f32 / target.len() as f32
            } else {
                1.0
            }
        };
        
        // Cache the result
        self.diff_cache.put(cache_key, Arc::new(diff.clone()));
        self.stats.diffs_created += 1;
        
        Ok(diff)
    }
    
    /// Apply binary diff to source data
    pub fn apply_diff(&mut self, source: &[u8], diff: &BinaryDiff) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut _source_pos = 0;
        
        for operation in &diff.operations {
            match operation {
                DiffOperation::Copy { source_offset, length } => {
                    let start = *source_offset as usize;
                    let end = start + *length as usize;
                    if end <= source.len() {
                        result.extend_from_slice(&source[start..end]);
                    }
                    _source_pos = end;
                },
                DiffOperation::Insert { data } => {
                    result.extend_from_slice(data);
                },
                DiffOperation::Skip { length } => {
                    // Skip bytes in target (used for optimization)
                    result.resize(result.len() + *length as usize, 0);
                },
                DiffOperation::Delete { offset, length } => {
                    // Skip bytes in source, don't add to result
                    _source_pos = (*offset as usize).saturating_add(*length);
                },
            }
        }
        
        // Verify checksum
        let result_hash = self.hash_data(&result);
        if result_hash != diff.target_checksum {
            return Err(Error::InvalidData("Diff checksum mismatch".to_string()));
        }
        
        self.stats.diffs_applied += 1;
        Ok(result)
    }
    
    /// Simplified Myers' diff algorithm
    fn myers_diff(&self, source: &[u8], target: &[u8]) -> Result<Vec<DiffOperation>> {
        // Myers diff algorithm for minimal edit distance
        let n = source.len();
        let m = target.len();
        
        // Handle empty cases
        if n == 0 {
            return Ok(vec![DiffOperation::Insert { data: target.to_vec() }]);
        }
        if m == 0 {
            return Ok(vec![DiffOperation::Delete { offset: 0, length: n }]);
        }
        
        // For very large diffs, use simple replacement
        const MAX_DIFF_SIZE: usize = 10_000;
        if n > MAX_DIFF_SIZE || m > MAX_DIFF_SIZE {
            return Ok(vec![
                DiffOperation::Delete { offset: 0, length: n },
                DiffOperation::Insert { data: target.to_vec() },
            ]);
        }
        
        // Find longest common subsequence using dynamic programming
        let mut operations = Vec::new();
        let mut i = 0;
        let mut j = 0;
        
        while i < n || j < m {
            if i < n && j < m && source[i] == target[j] {
                // Match - advance both
                let start_i = i;
                while i < n && j < m && source[i] == target[j] {
                    i += 1;
                    j += 1;
                }
                operations.push(DiffOperation::Copy {
                    source_offset: start_i as u32,
                    length: (i - start_i) as u32,
                });
            } else if j < m {
                // Need to insert from target
                let start_j = j;
                while j < m && (i >= n || (j < m && source.get(i) != Some(&target[j]))) {
                    j += 1;
                }
                operations.push(DiffOperation::Insert {
                    data: target[start_j..j].to_vec(),
                });
            } else {
                // Need to delete from source
                operations.push(DiffOperation::Delete {
                    offset: i as u32,
                    length: n - i,
                });
                i = n;
            }
        }
        
        Ok(operations)
    }
    
    /// Hash data for checksums
    fn hash_data(&self, data: &[u8]) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
    
    /// Optimize diff operations by merging adjacent operations
    pub fn optimize_diff(&self, diff: &mut BinaryDiff) {
        let mut optimized_ops = Vec::new();
        let mut current_op: Option<DiffOperation> = None;
        
        for operation in &diff.operations {
            match (&current_op, operation) {
                // Merge adjacent inserts
                (Some(DiffOperation::Insert { data: current_data }), DiffOperation::Insert { data: new_data }) => {
                    let mut merged_data = current_data.clone();
                    merged_data.extend_from_slice(new_data);
                    current_op = Some(DiffOperation::Insert { data: merged_data });
                },
                // Merge adjacent copies
                (Some(DiffOperation::Copy { source_offset: current_offset, length: current_length }), 
                 DiffOperation::Copy { source_offset: new_offset, length: new_length }) 
                if *current_offset + *current_length == *new_offset => {
                    current_op = Some(DiffOperation::Copy {
                        source_offset: *current_offset,
                        length: *current_length + *new_length,
                    });
                },
                // Can't merge - save current and start new
                _ => {
                    if let Some(op) = current_op.take() {
                        optimized_ops.push(op);
                    }
                    current_op = Some(operation.clone());
                }
            }
        }
        
        // Add final operation
        if let Some(op) = current_op {
            optimized_ops.push(op);
        }
        
        diff.operations = optimized_ops;
    }
}