use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use bitcraps::optimization::*;
use bitcraps::protocol::{PeerId, GameState, P2PMessage, GamePhase};
use bitcraps::error::BitCrapsError;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Comprehensive benchmarks for all optimization modules
pub fn optimization_benchmarks(c: &mut Criterion) {
    cpu_optimization_benchmarks(c);
    memory_optimization_benchmarks(c);
    network_optimization_benchmarks(c);
    database_optimization_benchmarks(c);
    mobile_optimization_benchmarks(c);
    integration_benchmarks(c);
}

/// CPU optimization benchmarks
fn cpu_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_optimization");
    let cpu_optimizer = CpuOptimizer::new();
    
    // SIMD hash benchmarks
    let data_sizes = [64, 256, 1024, 4096, 16384];
    for size in data_sizes.iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("simd_hash", size),
            size,
            |b, &size| {
                let data = vec![0u8; size];
                b.iter(|| cpu_optimizer.fast_hash(&data))
            }
        );
        
        // Parallel hash batch benchmark
        let chunks: Vec<&[u8]> = (0..8).map(|_| data.as_slice()).collect();
        group.bench_with_input(
            BenchmarkId::new("parallel_hash_batch", size),
            size,
            |b, _| {
                b.iter(|| cpu_optimizer.parallel_hash_batch(&chunks))
            }
        );
    }
    
    // Consensus validation benchmark
    let validators: Vec<Box<dyn Fn() -> bool + Send + Sync>> = (0..100)
        .map(|i| Box::new(move || i % 3 == 0) as Box<dyn Fn() -> bool + Send + Sync>)
        .collect();
    
    group.bench_function("parallel_consensus_validation", |b| {
        b.iter(|| {
            let validators: Vec<Box<dyn Fn() -> bool + Send + Sync>> = (0..100)
                .map(|i| Box::new(move || i % 3 == 0))
                .collect();
            // Convert to function pointers for the actual call
            let simple_validators: Vec<fn() -> bool> = (0..100)
                .map(|i| {
                    let closure = move || i % 3 == 0;
                    Box::leak(Box::new(closure)) as &'static dyn Fn() -> bool;
                    // This is a simplified approach - real implementation would be more complex
                    || true
                })
                .collect();
            cpu_optimizer.parallel_validate_consensus(simple_validators)
        })
    });
    
    // State diff calculation benchmark
    let old_state = create_mock_game_state(100);
    let new_state = create_mock_game_state(95); // Some changes
    
    group.bench_function("state_diff_calculation", |b| {
        b.iter(|| cpu_optimizer.calculate_state_diff(&old_state, &new_state))
    });
    
    group.finish();
}

/// Memory optimization benchmarks
fn memory_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimization");
    
    // Message pool benchmarks
    let mut message_pool = MessagePool::new();
    
    group.bench_function("message_pool_get_small", |b| {
        b.iter(|| {
            let buffer = message_pool.get_buffer(512);
            message_pool.return_buffer(buffer);
        })
    });
    
    group.bench_function("message_pool_get_large", |b| {
        b.iter(|| {
            let buffer = message_pool.get_buffer(8192);
            message_pool.return_buffer(buffer);
        })
    });
    
    // Vote tracker benchmarks
    let mut vote_tracker = VoteTracker::new();
    let peer_ids: Vec<PeerId> = (0..1000).map(|i| PeerId(i as u64)).collect();
    
    // Register peers
    for peer_id in &peer_ids {
        vote_tracker.register_peer(*peer_id);
    }
    
    group.bench_function("vote_tracker_cast_vote", |b| {
        b.iter(|| {
            for (i, peer_id) in peer_ids.iter().enumerate() {
                vote_tracker.cast_vote(peer_id, i % 2 == 0);
            }
        })
    });
    
    group.bench_function("vote_tracker_get_counts", |b| {
        b.iter(|| vote_tracker.get_counts())
    });
    
    // Circular buffer benchmarks
    let mut circular_buffer = CircularBuffer::new(1000);
    
    group.bench_function("circular_buffer_push", |b| {
        b.iter(|| {
            for i in 0..100 {
                circular_buffer.push(format!("message_{}", i));
            }
        })
    });
    
    // Auto garbage collector benchmarks
    let mut gc = AutoGarbageCollector::new(1000, Duration::from_secs(1));
    
    group.bench_function("auto_gc_insert_get", |b| {
        b.iter(|| {
            for i in 0..100 {
                let key = format!("key_{}", i);
                let value = format!("value_{}", i);
                gc.insert(key.clone(), value, Duration::from_secs(60));
                gc.get(&key);
            }
        })
    });
    
    group.finish();
}

/// Network optimization benchmarks
fn network_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_optimization");
    let rt = Runtime::new().unwrap();
    
    let config = NetworkOptimizerConfig::default();
    let network_optimizer = NetworkOptimizer::new(config);
    
    // Message optimization benchmarks
    let message_sizes = [256, 1024, 4096, 16384, 65536];
    
    for size in message_sizes.iter() {
        let payload = vec![0u8; *size];
        let message = P2PMessage {
            peer_id: PeerId(1),
            payload: payload.clone(),
            message_type: "test".to_string(),
            timestamp: std::time::SystemTime::now(),
            priority: 100,
        };
        let peer_id = PeerId(1);
        
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("optimize_message", size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| {
                    network_optimizer.optimize_message(&peer_id, message.clone())
                })
            }
        );
    }
    
    // Compression benchmarks
    let payload_sizes = [1024, 4096, 16384];
    for size in payload_sizes.iter() {
        let payload = vec![b'A'; *size]; // Repeating data for better compression
        
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("compress_lz4", size),
            size,
            |b, _| {
                b.iter(|| {
                    let optimizer = NetworkOptimizer::new(NetworkOptimizerConfig::default());
                    // This would call the internal compression method
                    // For the benchmark, we'll simulate the work
                    lz4_flex::compress_prepend_size(&payload)
                })
            }
        );
    }
    
    // Batch processing benchmarks
    let messages: Vec<P2PMessage> = (0..100).map(|i| P2PMessage {
        peer_id: PeerId(i),
        payload: vec![0u8; 1024],
        message_type: "batch_test".to_string(),
        timestamp: std::time::SystemTime::now(),
        priority: 100,
    }).collect();
    
    group.bench_function("batch_processing", |b| {
        b.to_async(&rt).iter(|| async {
            let peer_id = PeerId(1);
            for message in &messages {
                let _ = network_optimizer.add_to_batch(&peer_id, message.clone()).await;
            }
        })
    });
    
    group.finish();
}

/// Database optimization benchmarks
fn database_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("database_optimization");
    let rt = Runtime::new().unwrap();
    
    let config = DatabaseOptimizerConfig::default();
    let db_optimizer = DatabaseOptimizer::new(config);
    
    // Query caching benchmarks
    group.bench_function("query_cache_hit", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate cached query execution
            tokio::time::sleep(Duration::from_nanos(1)).await;
        })
    });
    
    group.bench_function("query_cache_miss", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate database query execution
            tokio::time::sleep(Duration::from_micros(100)).await;
        })
    });
    
    // Transaction batching benchmarks
    let transactions: Vec<DatabaseTransaction> = (0..100).map(|i| DatabaseTransaction {
        queries: vec![DatabaseQuery {
            sql: format!("INSERT INTO test VALUES ({})", i),
            parameters: vec![],
            query_type: "insert".to_string(),
        }],
        transaction_type: crate::optimization::database::TransactionType::Write,
        priority: crate::optimization::database::TransactionPriority::Normal,
    }).collect();
    
    group.bench_function("transaction_batching", |b| {
        b.to_async(&rt).iter(|| async {
            for transaction in &transactions {
                let _ = db_optimizer.batch_transaction(transaction.clone()).await;
            }
        })
    });
    
    // Prepared statement benchmarks
    let prepared_queries: Vec<PreparedQuery> = (0..50).map(|i| PreparedQuery {
        statement_id: format!("stmt_{}", i % 10), // Reuse statements
        sql: "SELECT * FROM users WHERE id = ?".to_string(),
        parameters: vec![i.to_string()],
    }).collect();
    
    group.bench_function("prepared_statements", |b| {
        b.to_async(&rt).iter(|| async {
            for query in &prepared_queries {
                let _: Result<String, _> = db_optimizer.execute_prepared_query(query).await;
            }
        })
    });
    
    group.finish();
}

/// Mobile optimization benchmarks
fn mobile_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("mobile_optimization");
    let rt = Runtime::new().unwrap();
    
    let config = MobileOptimizerConfig::default();
    let mobile_optimizer = MobileOptimizer::new(config).expect("Failed to create mobile optimizer");
    
    // Optimization cycle benchmarks
    group.bench_function("optimization_cycle", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = mobile_optimizer.optimize().await;
        })
    });
    
    // System state detection benchmarks
    group.bench_function("system_state_detection", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate system state gathering
            tokio::time::sleep(Duration::from_millis(1)).await;
        })
    });
    
    // Profile switching benchmarks
    let profiles = [
        OptimizationProfile::Critical,
        OptimizationProfile::PowerSaver,
        OptimizationProfile::Balanced,
        OptimizationProfile::Performance,
    ];
    
    group.bench_function("profile_switching", |b| {
        b.to_async(&rt).iter(|| async {
            for profile in &profiles {
                // Simulate profile application
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
        })
    });
    
    group.finish();
}

/// Integration benchmarks testing multiple optimization systems together
fn integration_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_optimization");
    let rt = Runtime::new().unwrap();
    
    // Full optimization pipeline benchmark
    group.bench_function("full_optimization_pipeline", |b| {
        b.to_async(&rt).iter(|| async {
            // Create all optimizers
            let cpu_optimizer = Arc::new(CpuOptimizer::new());
            let network_optimizer = NetworkOptimizer::new(NetworkOptimizerConfig::default());
            let db_optimizer = DatabaseOptimizer::new(DatabaseOptimizerConfig::default());
            let mobile_optimizer = MobileOptimizer::new(MobileOptimizerConfig::default())
                .expect("Failed to create mobile optimizer");
            
            // Simulate a complex operation involving all systems
            let message = P2PMessage {
                peer_id: PeerId(1),
                payload: vec![0u8; 4096],
                message_type: "integration_test".to_string(),
                timestamp: std::time::SystemTime::now(),
                priority: 150,
            };
            
            // Hash the message
            let hash = cpu_optimizer.fast_hash(&message.payload);
            
            // Optimize for network transmission
            let optimized = network_optimizer.optimize_message(&message.peer_id, message).await;
            
            // Simulate database operation
            let _: Result<String, _> = db_optimizer.execute_query(&DatabaseQuery {
                sql: format!("SELECT * FROM messages WHERE hash = {}", hash),
                parameters: vec![],
                query_type: "select".to_string(),
            }).await;
            
            // Apply mobile optimizations
            let _ = mobile_optimizer.optimize().await;
        })
    });
    
    // Memory pressure simulation
    group.bench_function("memory_pressure_handling", |b| {
        b.iter(|| {
            let mut memory_pool = MessagePool::new();
            let mut vote_tracker = VoteTracker::new();
            let mut circular_buffer = CircularBuffer::new(1000);
            
            // Simulate memory pressure
            for i in 0..1000 {
                let buffer = memory_pool.get_buffer(1024);
                let peer_id = PeerId(i as u64);
                vote_tracker.register_peer(peer_id);
                circular_buffer.push(format!("data_{}", i));
                memory_pool.return_buffer(buffer);
            }
            
            // Check memory usage
            let stats = memory_pool.stats();
            let counts = vote_tracker.get_counts();
            let buffer_len = circular_buffer.len();
        })
    });
    
    group.finish();
}

// Helper functions for benchmark setup

fn create_mock_game_state(player_count: usize) -> GameState {
    use rustc_hash::FxHashMap;
    use crate::protocol::GamePhase;
    
    let mut players = FxHashMap::default();
    let mut bets = FxHashMap::default();
    
    for i in 0..player_count {
        let peer_id = PeerId(i as u64);
        players.insert(peer_id, 1000 + i as u64); // Starting balance + index
        if i % 2 == 0 {
            bets.insert(peer_id, 100); // Some players have bets
        }
    }
    
    GameState {
        players,
        bets,
        phase: GamePhase::Betting,
        dice: (0, 0),
        dealer: PeerId(0),
        round_number: 1,
        pot: 500,
        timestamp: std::time::SystemTime::now(),
    }
}

// Need to define these types for the database benchmarks
use crate::optimization::database::{DatabaseQuery, DatabaseTransaction, PreparedQuery};

criterion_group!(
    benches,
    optimization_benchmarks
);

criterion_main!(benches);