#[cfg(feature = "optimization")]
pub mod cpu;
#[cfg(feature = "optimization")]
pub mod database;
#[cfg(feature = "optimization")]
pub mod memory;
#[cfg(feature = "optimization")]
pub mod mobile;
#[cfg(feature = "optimization")]
pub mod network;

// New comprehensive optimization modules
#[cfg(feature = "optimization")]
pub mod profiler;
#[cfg(feature = "optimization")]
pub mod memory_optimizer;
#[cfg(feature = "optimization")]
pub mod cache_optimizer;
#[cfg(feature = "optimization")]
pub mod connection_pool_optimizer;
#[cfg(feature = "optimization")]
pub mod query_optimizer;
#[cfg(feature = "optimization")]
pub mod resource_scheduler;

#[cfg(feature = "optimization")]
pub use cpu::{CpuOptimizer, CpuOptimizerStats, OptimizedCache, SimdFeatures};
#[cfg(feature = "optimization")]
pub use database::{
    DatabaseOptimizer, DatabaseOptimizerConfig, DatabasePerformanceStats, OptimizationSuggestion,
};
#[cfg(feature = "optimization")]
pub use memory::{AutoGarbageCollector, CircularBuffer, MessagePool, MmapStorage, VoteTracker};
#[cfg(feature = "optimization")]
pub use mobile::{MobileMetrics, MobileOptimizer, MobileOptimizerConfig, OptimizationProfile};
#[cfg(feature = "optimization")]
pub use network::{
    CompressionType, NetworkOptimizer, NetworkOptimizerConfig, NetworkOptimizerStats,
};

// Re-export new optimization components
#[cfg(feature = "optimization")]
pub use profiler::{RuntimeProfiler, ProfilerConfig, ProfilerStatistics, OperationTimer};
#[cfg(feature = "optimization")]
pub use memory_optimizer::{MemoryOptimizer, MemoryOptimizerConfig, MemoryStatistics, MemoryPressure};
#[cfg(feature = "optimization")]
pub use cache_optimizer::{IntelligentCache, CacheOptimizer, CacheOptimizerConfig, CacheMetrics};
#[cfg(feature = "optimization")]
pub use connection_pool_optimizer::{AdaptiveConnectionPool, ConnectionPoolConfig, PoolStatistics};
#[cfg(feature = "optimization")]
pub use query_optimizer::{QueryOptimizer, QueryOptimizerConfig, OptimizerStatistics, QueryMetadata};
#[cfg(feature = "optimization")]
pub use resource_scheduler::{
    AdaptiveResourceScheduler, ResourceSchedulerConfig, SchedulerStatistics, 
    TaskPriority, TaskCategory, ResourceRequirements
};
