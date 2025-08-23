//! High-performance caching subsystem for BitCraps

pub mod multi_tier;

pub use multi_tier::{MultiTierCache, CacheStats, CacheEntry};