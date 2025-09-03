//! Production-Grade Performance Benchmarks
//!
//! Comprehensive benchmarking suite for measuring performance under
//! realistic production workloads and conditions.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;

use bitcraps::protocol::consensus::engine::ConsensusEngine;
use bitcraps::protocol::{ConsensusMessage, GameId, PeerId, ConsensusPayload};
use bitcraps::gaming::{GameProposal, DiceRoll};
use bitcraps::crypto::{GameCrypto, SecureRng};
use bitcraps::utils::correlation::{CorrelationManager, CorrelationConfig, RequestContext};
use bitcraps::security::{SecurityManager, SecurityConfig};
use bitcraps::transport::nat_traversal::TurnRelay;

/// Benchmark consensus message processing throughput
fn benchmark_consensus_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_throughput");
    
    for message_count in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Elements(*message_count as u64));
        
        group.bench_with_input(
            BenchmarkId::new("process_messages", message_count),
            message_count,
            |b, &message_count| {
                b.to_async(&rt).iter(|| async {
                    let mut engine = ConsensusEngine::new();
                    let game_id = black_box([1u8; 16]);
                    let peer_id = black_box([2u8; 32]);
                    
                    for i in 0..message_count {
                        let message = ConsensusMessage::new(
                            game_id,
                            peer_id,
                            ConsensusPayload::GameProposal(GameProposal {
                                operation: format!("op_{}", i),
                                participants: vec![peer_id],
                                data: vec![i as u8],
                                timestamp: 1000 + i as u64,
                            }),
                            1000 + i as u64,
                        );
                        
                        black_box(engine.process_message(message).unwrap());
                    }
                    
                    black_box(engine.get_state())
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark cryptographic operations performance
fn benchmark_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");
    
    // Dice roll generation
    group.bench_function("dice_roll_generation", |b| {
        b.iter(|| {
            black_box(GameCrypto::generate_secure_dice_roll())
        });
    });
    
    // Hash operations
    let test_data = vec![42u8; 1024];
    group.bench_function("hash_1kb", |b| {
        b.iter(|| {
            black_box(GameCrypto::hash(black_box(&test_data)))
        });
    });
    
    // HMAC operations
    let key = [0u8; 32];
    group.bench_function("hmac_1kb", |b| {
        b.iter(|| {
            black_box(GameCrypto::create_hmac(black_box(&key), black_box(&test_data)))
        });
    });
    
    // Commitment schemes
    let secret = [42u8; 32];
    group.bench_function("commitment_create", |b| {
        b.iter(|| {
            black_box(GameCrypto::commit_randomness(black_box(&secret)))
        });
    });
    
    let commitment = GameCrypto::commit_randomness(&secret);
    group.bench_function("commitment_verify", |b| {
        b.iter(|| {
            black_box(GameCrypto::verify_commitment(black_box(&commitment), black_box(&secret)))
        });
    });
    
    // Randomness combination
    let sources = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
    group.bench_function("randomness_combination", |b| {
        b.iter(|| {
            black_box(GameCrypto::combine_randomness(black_box(&sources)))
        });
    });
    
    group.finish();
}

/// Benchmark security validation performance
fn benchmark_security_validation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("security_validation");
    
    group.bench_function("game_join_validation", |b| {
        let security_manager = SecurityManager::new(SecurityConfig::default());
        
        b.to_async(&rt).iter(|| async {
            let game_id = black_box([1u8; 16]);
            let player_id = black_box([2u8; 32]);
            let buy_in = black_box(1000u64);
            let timestamp = black_box(1000000u64);
            let client_ip = black_box(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
            
            // Note: This will fail rate limiting after first few calls, but measures validation performance
            let _ = black_box(security_manager.validate_game_join_request(
                &game_id, &player_id, buy_in, timestamp, client_ip
            ));
        });
    });
    
    group.bench_function("dice_roll_validation", |b| {
        let security_manager = SecurityManager::new(SecurityConfig::default());
        
        b.to_async(&rt).iter(|| async {
            let game_id = black_box([1u8; 16]);
            let player_id = black_box([2u8; 32]);
            let entropy = black_box([42u8; 32]);
            let commitment = black_box(GameCrypto::commit_randomness(&entropy));
            let timestamp = black_box(1000000u64);
            let client_ip = black_box(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 2)));
            
            let _ = black_box(security_manager.validate_dice_roll_commit(
                &game_id, &player_id, &entropy, &commitment, timestamp, client_ip
            ));
        });
    });
    
    group.finish();
}

/// Benchmark correlation ID management performance
fn benchmark_correlation_management(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("correlation_management");
    
    for request_count in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*request_count as u64));
        
        group.bench_with_input(
            BenchmarkId::new("request_tracking", request_count),
            request_count,
            |b, &request_count| {
                b.to_async(&rt).iter(|| async {
                    let manager = CorrelationManager::new(CorrelationConfig::default());
                    
                    let mut contexts = Vec::new();
                    
                    // Start tracking requests
                    for i in 0..request_count {
                        let context = RequestContext::new()
                            .with_operation(format!("bench_op_{}", i))
                            .with_source("benchmark".to_string());
                        
                        black_box(manager.start_request(context.clone()).await.unwrap());
                        contexts.push(context);
                    }
                    
                    // Complete half the requests
                    for context in contexts.iter().take(request_count / 2) {
                        black_box(manager.complete_request(&context.correlation_id).await.unwrap());
                    }
                    
                    // Fail the other half
                    for context in contexts.iter().skip(request_count / 2) {
                        black_box(manager.fail_request(
                            &context.correlation_id, 
                            "benchmark failure".to_string()
                        ).await.unwrap());
                    }
                    
                    black_box(manager.get_statistics().await)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark TURN relay performance
fn benchmark_turn_relay(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("turn_relay");
    
    group.bench_function("allocation", |b| {
        b.to_async(&rt).iter(|| async {
            let relay = TurnRelay::new("benchmark-realm".to_string());
            let client_id = format!("client_{}", rand::random::<u32>());
            
            black_box(relay.allocate_relay_address(client_id).await)
        });
    });
    
    for data_size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*data_size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("data_relay", data_size),
            data_size,
            |b, &data_size| {
                b.to_async(&rt).iter(|| async {
                    let relay = TurnRelay::new("benchmark-realm".to_string());
                    let data = vec![42u8; data_size];
                    
                    black_box(relay.relay_data(
                        "client_a".to_string(),
                        "client_b".to_string(),
                        black_box(data)
                    ).await)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent consensus operations
fn benchmark_concurrent_consensus(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_consensus");
    
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_engines", thread_count),
            thread_count,
            |b, &thread_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for thread_id in 0..*thread_count {
                        let handle = tokio::spawn(async move {
                            let mut engine = ConsensusEngine::new();
                            let game_id = [thread_id as u8; 16];
                            let peer_id = [thread_id as u8; 32];
                            
                            // Process 100 messages per thread
                            for i in 0..100 {
                                let message = ConsensusMessage::new(
                                    game_id,
                                    peer_id,
                                    ConsensusPayload::DiceRoll(DiceRoll {
                                        die1: (i % 6) + 1,
                                        die2: ((i + 1) % 6) + 1,
                                    }),
                                    1000 + i as u64,
                                );
                                
                                let _ = engine.process_message(message);
                            }
                            
                            engine.get_state().get_hash()
                        });
                        
                        handles.push(handle);
                    }
                    
                    // Wait for all threads to complete
                    let mut results = Vec::new();
                    for handle in handles {
                        results.push(handle.await.unwrap());
                    }
                    
                    black_box(results)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    
    // Benchmark message creation and serialization
    group.bench_function("message_creation", |b| {
        b.iter(|| {
            let game_id = black_box([1u8; 16]);
            let peer_id = black_box([2u8; 32]);
            let timestamp = black_box(1000u64);
            
            let message = black_box(ConsensusMessage::new(
                game_id,
                peer_id,
                ConsensusPayload::GameProposal(GameProposal {
                    operation: "memory_test".to_string(),
                    participants: vec![peer_id],
                    data: vec![1, 2, 3, 4, 5],
                    timestamp,
                }),
                timestamp,
            ));
            
            // Simulate serialization overhead
            black_box(bincode::serialize(&message).unwrap())
        });
    });
    
    // Benchmark large data structures
    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("large_proposal", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let game_id = black_box([1u8; 16]);
                    let peer_id = black_box([2u8; 32]);
                    let large_data = vec![42u8; size];
                    
                    let message = black_box(ConsensusMessage::new(
                        game_id,
                        peer_id,
                        ConsensusPayload::GameProposal(GameProposal {
                            operation: "large_data_test".to_string(),
                            participants: vec![peer_id],
                            data: large_data,
                            timestamp: 1000,
                        }),
                        1000,
                    ));
                    
                    black_box(bincode::serialize(&message).unwrap())
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark end-to-end game scenarios
fn benchmark_game_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("game_scenarios");
    
    group.bench_function("complete_game_flow", |b| {
        b.to_async(&rt).iter(|| async {
            let mut engine = ConsensusEngine::new();
            let game_id = black_box([1u8; 16]);
            let players = vec![
                [1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]
            ];
            
            // Game creation
            let create_msg = ConsensusMessage::new(
                game_id,
                players[0],
                ConsensusPayload::GameProposal(GameProposal {
                    operation: "create_game".to_string(),
                    participants: players.clone(),
                    data: vec![],
                    timestamp: 1000,
                }),
                1000,
            );
            black_box(engine.process_message(create_msg).unwrap());
            
            // Players place bets
            for (i, &player) in players.iter().enumerate() {
                let bet_msg = ConsensusMessage::new(
                    game_id,
                    player,
                    ConsensusPayload::BetPlacement { amount: 100 * (i as u64 + 1) },
                    1001 + i as u64,
                );
                black_box(engine.process_message(bet_msg).unwrap());
            }
            
            // Dice rolls
            for (i, &player) in players.iter().enumerate() {
                let roll = DiceRoll {
                    die1: ((i % 6) + 1) as u8,
                    die2: (((i + 1) % 6) + 1) as u8,
                };
                let roll_msg = ConsensusMessage::new(
                    game_id,
                    player,
                    ConsensusPayload::DiceRoll(roll),
                    1010 + i as u64,
                );
                black_box(engine.process_message(roll_msg).unwrap());
            }
            
            black_box(engine.get_state())
        });
    });
    
    group.finish();
}

/// Custom benchmark configuration
fn custom_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(100)
        .warm_up_time(Duration::from_secs(2))
        .with_plots() // Generate performance plots
}

criterion_group!(
    name = benches;
    config = custom_criterion();
    targets = 
        benchmark_consensus_throughput,
        benchmark_crypto_operations,
        benchmark_security_validation,
        benchmark_correlation_management,
        benchmark_turn_relay,
        benchmark_concurrent_consensus,
        benchmark_memory_patterns,
        benchmark_game_scenarios
);

criterion_main!(benches);