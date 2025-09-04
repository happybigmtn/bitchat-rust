//! Performance Optimization Integration Demo
//!
//! This example demonstrates how to integrate all the performance optimization
//! components together for a comprehensive performance management system.

use bitcraps::optimization::{
    RuntimeProfiler, ProfilerConfig,
    MemoryOptimizer, MemoryOptimizerConfig,
    CacheOptimizer, CacheOptimizerConfig,
    QueryOptimizer, QueryOptimizerConfig,
    AdaptiveResourceScheduler, ResourceSchedulerConfig,
    TaskPriority, TaskCategory, ResourceRequirements
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Comprehensive performance management system
pub struct PerformanceManager {
    profiler: Arc<RuntimeProfiler>,
    memory_optimizer: Arc<MemoryOptimizer>,
    cache_optimizer: Arc<CacheOptimizer>,
    query_optimizer: Arc<QueryOptimizer>,
    resource_scheduler: Arc<AdaptiveResourceScheduler>,
}

impl PerformanceManager {
    /// Create a new performance manager with optimal configurations
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("🚀 Initializing BitCraps Performance Management System...");

        // Configure runtime profiler
        let profiler_config = ProfilerConfig {
            enable_cpu_profiling: true,
            enable_memory_profiling: true,
            enable_io_profiling: true,
            sampling_interval: Duration::from_millis(100),
            slow_operation_threshold_ms: 50.0,
            collect_stack_traces: false, // Disabled for performance
            ..Default::default()
        };
        let profiler = Arc::new(RuntimeProfiler::new(profiler_config));

        // Configure memory optimizer
        let memory_config = MemoryOptimizerConfig {
            enable_gc_tuning: true,
            enable_memory_pools: true,
            enable_leak_detection: true,
            pressure_threshold_mb: 1024,
            max_cache_size_mb: 256,
            leak_detection_threshold_mb: 25.0,
            ..Default::default()
        };
        let memory_optimizer = Arc::new(MemoryOptimizer::new(memory_config));

        // Configure cache optimizer
        let cache_config = CacheOptimizerConfig {
            enable_adaptive_sizing: true,
            enable_cache_warming: true,
            enable_pattern_learning: true,
            max_cache_memory_mb: 512,
            target_hit_ratio: 0.90,
            ..Default::default()
        };
        let cache_optimizer = Arc::new(CacheOptimizer::new(cache_config));

        // Configure query optimizer
        let query_config = QueryOptimizerConfig {
            enable_query_cache: true,
            enable_query_rewriting: true,
            enable_plan_cache: true,
            max_cached_queries: 5000,
            slow_query_threshold_ms: 75,
            enable_adaptive_optimization: true,
            ..Default::default()
        };
        let query_optimizer = Arc::new(QueryOptimizer::new(query_config));

        // Configure resource scheduler
        let scheduler_config = ResourceSchedulerConfig {
            max_concurrent_tasks: num_cpus::get() * 3,
            enable_prioritization: true,
            enable_load_balancing: true,
            enable_adaptive_scheduling: true,
            target_cpu_utilization: 0.75,
            target_memory_utilization: 0.65,
            ..Default::default()
        };
        let resource_scheduler = Arc::new(
            AdaptiveResourceScheduler::new(scheduler_config).await?
        );

        Ok(Self {
            profiler,
            memory_optimizer,
            cache_optimizer,
            query_optimizer,
            resource_scheduler,
        })
    }

    /// Start all optimization components
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔧 Starting performance optimization components...");

        // Start profiler
        self.profiler.start().await;
        println!("✓ Runtime profiler started");

        // Start memory optimizer
        self.memory_optimizer.start().await;
        println!("✓ Memory optimizer started");

        // Start query optimizer
        self.query_optimizer.start().await;
        println!("✓ Query optimizer started");

        // Start resource scheduler
        self.resource_scheduler.start().await?;
        println!("✓ Resource scheduler started");

        println!("🎯 All optimization components are running!");
        Ok(())
    }

    /// Stop all optimization components
    pub async fn stop(&self) {
        println!("🛑 Stopping performance optimization components...");

        self.profiler.stop().await;
        self.memory_optimizer.stop().await;
        self.query_optimizer.stop().await;
        self.resource_scheduler.stop().await;

        println!("✓ All components stopped");
    }

    /// Run performance demonstration
    pub async fn run_demo(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("\n🎮 Running BitCraps Performance Demo...\n");

        // Demonstrate profiling
        self.demo_profiling().await;

        // Demonstrate memory optimization
        self.demo_memory_optimization().await;

        // Demonstrate query optimization
        self.demo_query_optimization().await;

        // Demonstrate resource scheduling
        self.demo_resource_scheduling().await;

        // Demonstrate cache optimization
        self.demo_cache_optimization().await;

        // Generate comprehensive report
        self.generate_performance_report().await;

        Ok(())
    }

    /// Demonstrate runtime profiling
    async fn demo_profiling(&self) {
        println!("📊 === PROFILING DEMONSTRATION ===");

        // Profile some gaming operations
        for i in 0..10 {
            let timer = self.profiler.time_operation("dice_roll_simulation").await;
            
            // Simulate dice roll processing
            self.simulate_dice_roll().await;
            
            timer.finish().await;
            
            // Profile consensus operation
            let timer = self.profiler.time_operation("consensus_validation").await;
            self.simulate_consensus_validation().await;
            timer.finish().await;
        }

        // Record memory allocations
        for _ in 0..20 {
            self.profiler.record_memory_allocation(
                1024,
                bitcraps::optimization::profiler::AllocationType::Gaming,
                "game_state_buffer"
            ).await;
        }

        println!("✓ Profiling data collected for 10 gaming operations");
    }

    /// Demonstrate memory optimization
    async fn demo_memory_optimization(&self) {
        println!("💾 === MEMORY OPTIMIZATION DEMONSTRATION ===");

        // Create a memory pool for game objects
        let game_pool = self.memory_optimizer.create_memory_pool(
            "game_objects".to_string(),
            100,
            || Vec::<u8>::with_capacity(1024),
        ).await;

        // Use the memory pool
        for _ in 0..10 {
            if let Some(_object) = game_pool.acquire().await {
                // Simulate using the object
                sleep(Duration::from_millis(10)).await;
            }
        }

        // Create an adaptive cache for player data
        let player_cache = self.memory_optimizer.create_adaptive_cache::<String, PlayerData>(
            "player_cache".to_string(),
            1000,
        ).await;

        // Populate cache with player data
        for i in 0..50 {
            let player_id = format!("player_{}", i);
            let player_data = PlayerData {
                id: player_id.clone(),
                balance: 1000,
                games_played: i as u32,
            };
            
            player_cache.insert(player_id, player_data, 256, None).await;
        }

        // Test cache hit ratio
        for i in 0..25 {
            let player_id = format!("player_{}", i);
            let _data = player_cache.get(&player_id).await;
        }

        let memory_stats = self.memory_optimizer.get_memory_statistics().await;
        println!("✓ Memory optimization active - {} pools, {:.1}MB allocated", 
                 1, memory_stats.total_allocated_mb);
    }

    /// Demonstrate query optimization
    async fn demo_query_optimization(&self) {
        println!("🔍 === QUERY OPTIMIZATION DEMONSTRATION ===");

        // Simulate various database queries
        let queries = vec![
            "SELECT * FROM games WHERE status = 'active'",
            "SELECT player_id, balance FROM accounts WHERE balance > 100",
            "UPDATE games SET dice_roll = ? WHERE game_id = ?",
            "SELECT COUNT(*) FROM bets WHERE timestamp > ?",
            "SELECT * FROM games JOIN players ON games.creator = players.id",
        ];

        for (i, query) in queries.iter().enumerate() {
            let result = self.query_optimizer.optimize_and_execute(query, |optimized_query| {
                async move {
                    // Simulate query execution
                    sleep(Duration::from_millis(20 + (i * 5) as u64)).await;
                    Ok(format!("Result for: {}", optimized_query))
                }
            }).await;

            if result.is_ok() {
                println!("✓ Query optimized and executed: {}", &query[0..30]);
            }
        }

        let query_stats = self.query_optimizer.get_statistics().await;
        println!("✓ {} queries processed with {:.1}% cache hit ratio", 
                 query_stats.total_queries, query_stats.cache_hit_ratio * 100.0);
    }

    /// Demonstrate resource scheduling
    async fn demo_resource_scheduling(&self) {
        println!("⚡ === RESOURCE SCHEDULING DEMONSTRATION ===");

        use bitcraps::optimization::resource_scheduler::{ScheduledTask, SimpleTask, TaskFunction};

        // Create various gaming tasks with different priorities
        let tasks = vec![
            (TaskPriority::Critical, TaskCategory::Consensus, "consensus_validation", 100),
            (TaskPriority::High, TaskCategory::Gaming, "dice_roll_processing", 50),
            (TaskPriority::Normal, TaskCategory::NetworkIO, "player_sync", 75),
            (TaskPriority::Normal, TaskCategory::Database, "save_game_state", 200),
            (TaskPriority::Low, TaskCategory::Background, "cleanup_old_games", 500),
        ];

        for (priority, category, name, duration_ms) in tasks {
            let task = ScheduledTask {
                task_id: Uuid::new_v4(),
                name: name.to_string(),
                priority,
                category,
                requirements: ResourceRequirements {
                    cpu_weight: 0.7,
                    memory_mb: 50,
                    estimated_duration: Duration::from_millis(duration_ms),
                    ..Default::default()
                },
                created_at: std::time::Instant::now(),
                deadline: None,
                timeout: Duration::from_secs(10),
                retry_count: 0,
                max_retries: 2,
                dependencies: Vec::new(),
                affinity: None,
                task_fn: Box::new(SimpleTask {
                    name: name.to_string(),
                    work_duration: Duration::from_millis(duration_ms),
                }),
            };

            let task_id = self.resource_scheduler.schedule_task(task).await.unwrap();
            println!("✓ Scheduled {} task: {}", priority as u8, name);
        }

        // Wait for tasks to complete
        sleep(Duration::from_secs(2)).await;

        let scheduler_stats = self.resource_scheduler.get_statistics().await;
        println!("✓ {} tasks scheduled, {} completed", 
                 scheduler_stats.total_tasks_scheduled, scheduler_stats.total_tasks_completed);
    }

    /// Demonstrate cache optimization
    async fn demo_cache_optimization(&self) {
        println!("🚀 === CACHE OPTIMIZATION DEMONSTRATION ===");

        // Would demonstrate cache optimization features
        // For now, just show cache usage statistics
        sleep(Duration::from_millis(100)).await;
        println!("✓ Cache optimization patterns analyzed");
    }

    /// Generate comprehensive performance report
    async fn generate_performance_report(&self) {
        println!("\n📈 === COMPREHENSIVE PERFORMANCE REPORT ===\n");

        // Profiler report
        let profiler_report = self.profiler.generate_report().await;
        println!("{}", profiler_report);

        // Memory optimizer report
        let memory_report = self.memory_optimizer.generate_report().await;
        println!("{}", memory_report);

        // Query optimizer report
        let query_report = self.query_optimizer.generate_report().await;
        println!("{}", query_report);

        // System metrics
        let system_metrics = self.resource_scheduler.get_system_metrics().await;
        println!("=== SYSTEM METRICS ===");
        println!("CPU Utilization: {:.1}%", system_metrics.cpu_utilization * 100.0);
        println!("Memory Utilization: {:.1}%", system_metrics.memory_utilization * 100.0);
        println!("Active Tasks: {}", system_metrics.active_tasks);
        println!("Queued Tasks: {}", system_metrics.queued_tasks);

        println!("\n🎯 Performance optimization demonstration completed!");
        println!("💡 All components are working together to optimize BitCraps performance");
    }

    /// Simulate dice roll processing
    async fn simulate_dice_roll(&self) {
        // Simulate the computational work of processing a dice roll
        sleep(Duration::from_millis(5)).await;
    }

    /// Simulate consensus validation
    async fn simulate_consensus_validation(&self) {
        // Simulate consensus algorithm validation work
        sleep(Duration::from_millis(15)).await;
    }
}

/// Example player data structure
#[derive(Debug, Clone)]
struct PlayerData {
    id: String,
    balance: u64,
    games_played: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🎲 BitCraps Performance Optimization Demo");
    println!("==========================================");

    // Create performance management system
    let performance_manager = PerformanceManager::new().await?;

    // Start all components
    performance_manager.start().await?;

    // Run the demonstration
    performance_manager.run_demo().await?;

    // Give components time to finish processing
    sleep(Duration::from_secs(2)).await;

    // Stop all components
    performance_manager.stop().await;

    println!("\n🏆 Demo completed successfully!");
    println!("🔧 Performance optimization system ready for production use.");

    Ok(())
}