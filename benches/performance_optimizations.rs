//! Performance Benchmarks for Optimization Analysis
//!
//! This file contains benchmarks to measure the performance improvements
//! from the lock-free data structures, connection pooling, and other optimizations.

use bitcraps::memory_pool::{GameMemoryPools, MemoryPool};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

// Create a runtime for async benchmarks
fn create_runtime() -> Runtime {
    Runtime::new().unwrap()
}

/// Benchmark lock-free vs traditional locks for concurrent access
fn bench_concurrent_access(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("concurrent_access");
    group.throughput(Throughput::Elements(1000));

    // Traditional RwLock-based HashMap
    let rwlock_map: Arc<RwLock<HashMap<u32, String>>> = Arc::new(RwLock::new(HashMap::new()));

    // Lock-free DashMap
    let dashmap: Arc<DashMap<u32, String>> = Arc::new(DashMap::new());

    // Pre-populate both maps
    rt.block_on(async {
        let mut rwlock_guard = rwlock_map.write().await;
        for i in 0..1000 {
            rwlock_guard.insert(i, format!("value_{}", i));
        }
    });

    for i in 0..1000 {
        dashmap.insert(i, format!("value_{}", i));
    }

    // Benchmark RwLock reads
    group.bench_function("rwlock_reads", |b| {
        let map_clone = rwlock_map.clone();
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let _guard = map_clone.read().await;
                let _value = _guard.get(&(i % 1000));
            }
        });
    });

    // Benchmark DashMap reads
    group.bench_function("dashmap_reads", |b| {
        let map_clone = dashmap.clone();
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let _value = map_clone.get(&(i % 1000));
            }
        });
    });

    // Benchmark concurrent mixed operations
    group.bench_function("rwlock_mixed_ops", |b| {
        let map_clone = rwlock_map.clone();
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = (0..10)
                .map(|i| {
                    let map = map_clone.clone();
                    tokio::spawn(async move {
                        if i % 2 == 0 {
                            let _guard = map.read().await;
                            let _value = _guard.get(&(i % 1000));
                        } else {
                            let mut guard = map.write().await;
                            guard.insert(i + 2000, format!("new_value_{}", i));
                        }
                    })
                })
                .collect();

            for task in tasks {
                let _ = task.await;
            }
        });
    });

    group.bench_function("dashmap_mixed_ops", |b| {
        let map_clone = dashmap.clone();
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = (0..10)
                .map(|i| {
                    let map = map_clone.clone();
                    tokio::spawn(async move {
                        if i % 2 == 0 {
                            let _value = map.get(&(i % 1000));
                        } else {
                            map.insert(i + 2000, format!("new_value_{}", i));
                        }
                    })
                })
                .collect();

            for task in tasks {
                let _ = task.await;
            }
        });
    });

    group.finish();
}

/// Benchmark memory pool vs direct allocation
fn bench_memory_pools(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("memory_allocation");
    group.throughput(Throughput::Elements(1000));

    // Pre-warmed memory pool
    let pool = rt.block_on(async {
        let pool = MemoryPool::<Vec<u8>>::new(100);
        pool.warmup(50).await;
        pool
    });

    // Benchmark direct allocation
    group.bench_function("direct_allocation", |b| {
        b.to_async(&rt).iter(|| async {
            for _i in 0..100 {
                let _vec = Vec::<u8>::with_capacity(1024);
            }
        });
    });

    // Benchmark pool allocation
    group.bench_function("pool_allocation", |b| {
        b.to_async(&rt).iter(|| async {
            for _i in 0..100 {
                let _obj = pool.get().await;
            }
        });
    });

    // Benchmark allocation and usage patterns
    group.bench_function("direct_alloc_with_usage", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let mut vec = Vec::<u8>::with_capacity(1024);
                vec.extend_from_slice(&i.to_be_bytes());
                vec.push(42);
            }
        });
    });

    group.bench_function("pool_alloc_with_usage", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let mut obj = pool.get().await;
                obj.clear();
                obj.extend_from_slice(&i.to_be_bytes());
                obj.push(42);
            }
        });
    });

    group.finish();
}

/// Benchmark event queue throughput with backpressure
fn bench_event_queues(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("event_queues");
    group.throughput(Throughput::Elements(1000));

    // Benchmark unbounded channel
    group.bench_function("unbounded_channel", |b| {
        b.to_async(&rt).iter(|| async {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u32>();

            // Producer task
            let producer = tokio::spawn(async move {
                for i in 0..1000 {
                    let _ = tx.send(i);
                }
            });

            // Consumer task
            let consumer = tokio::spawn(async move {
                let mut count = 0;
                while rx.recv().await.is_some() && count < 1000 {
                    count += 1;
                }
            });

            let _ = tokio::join!(producer, consumer);
        });
    });

    // Benchmark bounded channel with backpressure
    group.bench_function("bounded_channel", |b| {
        b.to_async(&rt).iter(|| async {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<u32>(100);

            // Producer task
            let producer = tokio::spawn(async move {
                for i in 0..1000 {
                    let _ = tx.send(i).await;
                }
            });

            // Consumer task
            let consumer = tokio::spawn(async move {
                let mut count = 0;
                while rx.recv().await.is_some() && count < 1000 {
                    count += 1;
                }
            });

            let _ = tokio::join!(producer, consumer);
        });
    });

    group.finish();
}

/// Benchmark parallel vs sequential message broadcasting
fn bench_message_broadcasting(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("message_broadcasting");

    for participant_count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*participant_count));

        // Sequential broadcasting
        group.bench_with_input(
            BenchmarkId::new("sequential", participant_count),
            participant_count,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    for _i in 0..size {
                        // Simulate sending message
                        tokio::time::sleep(Duration::from_micros(10)).await;
                    }
                });
            },
        );

        // Parallel broadcasting
        group.bench_with_input(
            BenchmarkId::new("parallel", participant_count),
            participant_count,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let tasks: Vec<_> = (0..size)
                        .map(|_| {
                            tokio::spawn(async {
                                // Simulate sending message
                                tokio::time::sleep(Duration::from_micros(10)).await;
                            })
                        })
                        .collect();

                    for task in tasks {
                        let _ = task.await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark STUN server selection strategies
fn bench_stun_selection(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("stun_selection");
    group.throughput(Throughput::Elements(4));

    let servers = vec![
        ("stun1.example.com", Duration::from_millis(50), 0.9),
        ("stun2.example.com", Duration::from_millis(100), 0.8),
        ("stun3.example.com", Duration::from_millis(75), 0.95),
        ("stun4.example.com", Duration::from_millis(200), 0.7),
    ];

    // Sequential STUN requests
    group.bench_function("sequential_stun", |b| {
        b.to_async(&rt).iter(|| async {
            for (name, delay, _success_rate) in &servers {
                // Simulate STUN request
                tokio::time::sleep(*delay / 10).await; // Scaled down for benchmark
                if name.contains("1") {
                    break; // Found first working server
                }
            }
        });
    });

    // Parallel STUN requests with best-server selection
    group.bench_function("parallel_stun", |b| {
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = servers
                .iter()
                .take(3)
                .map(|(name, delay, _)| {
                    let delay = *delay;
                    let name = name.to_string();
                    tokio::spawn(async move {
                        tokio::time::sleep(delay / 10).await; // Scaled down
                        (name, delay)
                    })
                })
                .collect();

            // Wait for first successful response
            for task in tasks {
                if let Ok((name, _delay)) = task.await {
                    if name.contains("1") || name.contains("3") {
                        break; // Got a good server
                    }
                }
            }
        });
    });

    group.finish();
}

/// Benchmark game memory pools comprehensive usage
fn bench_game_memory_pools(c: &mut Criterion) {
    let rt = create_runtime();

    let mut group = c.benchmark_group("game_memory_pools");
    group.throughput(Throughput::Elements(100));

    let pools = rt.block_on(async {
        let pools = GameMemoryPools::new();
        pools.warmup().await;
        pools
    });

    // Benchmark mixed pool usage pattern (typical game scenario)
    group.bench_function("mixed_pool_usage", |b| {
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = (0..20)
                .map(|i| {
                    let pools_ref = &pools;
                    tokio::spawn(async move {
                        match i % 3 {
                            0 => {
                                let mut vec_obj = pools_ref.vec_u8_pool.get().await;
                                vec_obj.extend_from_slice(b"game_data");
                                vec_obj.push(i as u8);
                            }
                            1 => {
                                let mut str_obj = pools_ref.string_pool.get().await;
                                str_obj.clear();
                                str_obj.push_str(&format!("player_{}", i));
                            }
                            2 => {
                                let mut map_obj = pools_ref.hashmap_pool.get().await;
                                map_obj.clear();
                                map_obj.insert("player_id".to_string(), i.to_string());
                                map_obj.insert("score".to_string(), "100".to_string());
                            }
                            _ => unreachable!(),
                        }
                    })
                })
                .collect();

            for task in tasks {
                let _ = task.await;
            }
        });
    });

    // Compare against direct allocation
    group.bench_function("direct_allocation_equivalent", |b| {
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = (0..20)
                .map(|i| {
                    tokio::spawn(async move {
                        match i % 3 {
                            0 => {
                                let mut vec = Vec::with_capacity(1024);
                                vec.extend_from_slice(b"game_data");
                                vec.push(i as u8);
                            }
                            1 => {
                                let mut string = String::with_capacity(256);
                                string.push_str(&format!("player_{}", i));
                            }
                            2 => {
                                let mut map = std::collections::HashMap::with_capacity(16);
                                map.insert("player_id".to_string(), i.to_string());
                                map.insert("score".to_string(), "100".to_string());
                            }
                            _ => unreachable!(),
                        }
                    })
                })
                .collect();

            for task in tasks {
                let _ = task.await;
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_access,
    bench_memory_pools,
    bench_event_queues,
    bench_message_broadcasting,
    bench_stun_selection,
    bench_game_memory_pools
);

criterion_main!(benches);
