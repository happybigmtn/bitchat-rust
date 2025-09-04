pub mod cpu;
pub mod database;
pub mod memory;
pub mod mobile;
pub mod network;

// New comprehensive optimization modules
pub mod profiler;
pub mod memory_optimizer;
pub mod cache_optimizer;
pub mod connection_pool_optimizer;
pub mod query_optimizer;
pub mod resource_scheduler;

pub use cpu::{CpuOptimizer, CpuOptimizerStats, OptimizedCache, SimdFeatures};
pub use database::{
    DatabaseOptimizer, DatabaseOptimizerConfig, DatabasePerformanceStats, OptimizationSuggestion,
};
pub use memory::{AutoGarbageCollector, CircularBuffer, MessagePool, MmapStorage, VoteTracker};
pub use mobile::{MobileMetrics, MobileOptimizer, MobileOptimizerConfig, OptimizationProfile};
pub use network::{
    CompressionType, NetworkOptimizer, NetworkOptimizerConfig, NetworkOptimizerStats,
};

// Re-export new optimization components
pub use profiler::{RuntimeProfiler, ProfilerConfig, ProfilerStatistics, OperationTimer};
pub use memory_optimizer::{MemoryOptimizer, MemoryOptimizerConfig, MemoryStatistics, MemoryPressure};
pub use cache_optimizer::{IntelligentCache, CacheOptimizer, CacheOptimizerConfig, CacheMetrics};
pub use connection_pool_optimizer::{AdaptiveConnectionPool, ConnectionPoolConfig, PoolStatistics};
pub use query_optimizer::{QueryOptimizer, QueryOptimizerConfig, OptimizerStatistics, QueryMetadata};
pub use resource_scheduler::{
    AdaptiveResourceScheduler, ResourceSchedulerConfig, SchedulerStatistics, 
    TaskPriority, TaskCategory, ResourceRequirements
};
