//! Real Performance Benchmarks for BitCraps
//!
//! This module contains actual performance benchmarks that measure:
//! - Consensus throughput and latency
//! - Cryptographic operations performance
//! - Network message processing
//! - Memory usage patterns
//! - Concurrent user scaling

use bitcraps::crypto::{BitchatIdentity, GameCrypto};
use bitcraps::mesh::MeshService;
use bitcraps::protocol::consensus::byzantine_engine::{
    ByzantineConfig, ByzantineConsensusEngine, ProposalData,
};
use bitcraps::protocol::{Bet, BetType, BitchatPacket, DiceRoll, GameId, PeerId};
use bitcraps::transport::{TransportAddress, TransportCoordinator};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark consensus operations
fn bench_consensus_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("consensus");

    // Setup consensus engine
    let config = ByzantineConfig::default();
    let crypto = Arc::new(GameCrypto::new());
    let node_id = [1u8; 32];

    group.bench_function("byzantine_consensus_round", |b| {
        b.to_async(&rt).iter(|| async {
            let engine = ByzantineConsensusEngine::new(config.clone(), crypto.clone(), node_id);

            // Add participants
            for i in 0..4 {
                engine.add_participant([i as u8; 32]).await.unwrap();
            }

            // Start round
            let round = engine.start_round().await.unwrap();

            // Submit proposal
            let data = ProposalData::DiceRoll(DiceRoll {
                die1: 3,
                die2: 4,
                timestamp: 0,
            });

            let hash = engine.submit_proposal(data).await.unwrap();
            black_box((round, hash))
        });
    });

    // Benchmark vote processing
    group.bench_function("vote_verification", |b| {
        b.iter(|| {
            let crypto = GameCrypto::new();
            let signer = [1u8; 32];
            let message = b"test vote data";

            // Create signature
            let identity = BitchatIdentity::generate_with_pow(0);
            let sig = identity.sign(message);

            // Verify signature
            let result = crypto.verify_signature(&signer, message, &sig.signature);
            black_box(result)
        });
    });

    // Benchmark proposal validation with varying participant counts
    for participants in [4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::new("consensus_with_participants", participants),
            participants,
            |b, &participant_count| {
                b.to_async(&rt).iter(|| async {
                    let engine =
                        ByzantineConsensusEngine::new(config.clone(), crypto.clone(), node_id);

                    // Add participants
                    for i in 0..participant_count {
                        engine.add_participant([i as u8; 32]).await.unwrap();
                    }

                    // Measure round completion time
                    let round = engine.start_round().await.unwrap();
                    black_box(round)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cryptographic operations
fn bench_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cryptography");

    // Key generation
    group.bench_function("keypair_generation", |b| {
        b.iter(|| {
            let identity = BitchatIdentity::generate_with_pow(0);
            black_box(identity)
        });
    });

    // Proof of work with varying difficulty
    for difficulty in [0, 8, 16, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("proof_of_work", difficulty),
            difficulty,
            |b, &diff| {
                b.iter(|| {
                    let identity = BitchatIdentity::generate_with_pow(diff);
                    black_box(identity)
                });
            },
        );
    }

    // Signature operations
    group.bench_function("sign_message", |b| {
        let identity = BitchatIdentity::generate_with_pow(0);
        let message = b"benchmark message data";

        b.iter(|| {
            let signature = identity.sign(message);
            black_box(signature)
        });
    });

    group.bench_function("verify_signature", |b| {
        let identity = BitchatIdentity::generate_with_pow(0);
        let message = b"benchmark message data";
        let signature = identity.sign(message);
        let crypto = GameCrypto::new();

        b.iter(|| {
            let valid =
                crypto.verify_signature(identity.public_key_bytes(), message, &signature.signature);
            black_box(valid)
        });
    });

    // Batch signature verification
    group.bench_function("batch_verify_100_signatures", |b| {
        let crypto = GameCrypto::new();
        let signatures: Vec<_> = (0..100)
            .map(|i| {
                let identity = BitchatIdentity::generate_with_pow(0);
                let message = format!("message {}", i);
                let sig = identity.sign(message.as_bytes());
                (identity, message, sig)
            })
            .collect();

        b.iter(|| {
            let mut valid_count = 0;
            for (identity, message, sig) in &signatures {
                if crypto.verify_signature(
                    identity.public_key_bytes(),
                    message.as_bytes(),
                    &sig.signature,
                ) {
                    valid_count += 1;
                }
            }
            black_box(valid_count)
        });
    });

    group.finish();
}

/// Benchmark network message processing
fn bench_network_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("network");

    // Packet serialization
    group.bench_function("packet_serialization", |b| {
        let packet = BitchatPacket::new(0x01, 0x10, 0x00, vec![1, 2, 3, 4, 5]);

        b.iter(|| {
            let serialized = bincode::serialize(&packet).unwrap();
            black_box(serialized)
        });
    });

    // Packet deserialization
    group.bench_function("packet_deserialization", |b| {
        let packet = BitchatPacket::new(0x01, 0x10, 0x00, vec![1, 2, 3, 4, 5]);
        let serialized = bincode::serialize(&packet).unwrap();

        b.iter(|| {
            let deserialized: BitchatPacket = bincode::deserialize(&serialized).unwrap();
            black_box(deserialized)
        });
    });

    // Message routing with varying peer counts
    for peer_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*peer_count as u64));
        group.bench_with_input(
            BenchmarkId::new("message_routing", peer_count),
            peer_count,
            |b, &peers| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    let coordinator = Arc::new(TransportCoordinator::new());

                    // Simulate routing to multiple peers
                    let mut handles = Vec::new();
                    for i in 0..peers {
                        let coord = coordinator.clone();
                        let handle = tokio::spawn(async move {
                            let addr = TransportAddress::Bluetooth(format!("peer_{}", i));
                            coord.check_connection_limits(&addr).await.ok();
                        });
                        handles.push(handle);
                    }

                    // Wait for all routing operations
                    for handle in handles {
                        handle.await.unwrap();
                    }

                    black_box(peers)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent user operations
fn bench_concurrent_users(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_users");

    // Test with different user counts to find actual capacity
    for user_count in [10, 50, 100, 200, 500].iter() {
        group.throughput(Throughput::Elements(*user_count as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_game_sessions", user_count),
            user_count,
            |b, &users| {
                let rt = Runtime::new().unwrap();

                b.to_async(&rt).iter(|| async {
                    use bitcraps::gaming::{GameSessionConfig, GameSessionManager};

                    let manager = Arc::new(GameSessionManager::new());
                    let mut handles = Vec::new();

                    for i in 0..users {
                        let mgr = manager.clone();
                        let handle = tokio::spawn(async move {
                            let player_id = [i as u8; 32];
                            let game_id = [0u8; 16];
                            let config = GameSessionConfig {
                                min_players: 2,
                                max_players: 8,
                                min_bet: 10,
                                max_bet: 1000,
                                timeout: Duration::from_secs(300),
                            };

                            // Create or join session
                            let session = mgr
                                .create_or_join_session(game_id, player_id, config)
                                .await
                                .unwrap();

                            black_box(session)
                        });
                        handles.push(handle);
                    }

                    // Wait for all users
                    for handle in handles {
                        handle.await.ok();
                    }

                    // Get final stats
                    let stats = manager.get_statistics().await;
                    black_box(stats.active_sessions)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Measure memory allocation for game state
    group.bench_function("game_state_allocation", |b| {
        b.iter(|| {
            use bitcraps::protocol::craps::CrapsGame;

            let game_id = [0u8; 16];
            let shooter = [1u8; 32];
            let game = CrapsGame::new(game_id, shooter);
            black_box(game)
        });
    });

    // Measure bet storage efficiency
    group.bench_function("bet_storage_1000", |b| {
        b.iter(|| {
            let mut bets = Vec::with_capacity(1000);
            for i in 0..1000 {
                let bet = Bet::new([i as u8; 32], [0u8; 16], BetType::Pass, 100);
                bets.push(bet);
            }
            black_box(bets)
        });
    });

    // Cache performance
    group.bench_function("lru_cache_operations", |b| {
        use lru::LruCache;
        use std::num::NonZeroUsize;

        let mut cache = LruCache::new(NonZeroUsize::new(1000).unwrap());

        b.iter(|| {
            // Mixed read/write operations
            for i in 0..100 {
                cache.put(i, i * 2);
                if i > 10 {
                    cache.get(&(i - 10));
                }
            }
            black_box(cache.len())
        });
    });

    group.finish();
}

/// Benchmark compression efficiency
fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Test different data types
    let test_data = vec![
        ("small_packet", vec![1u8; 100]),
        ("medium_packet", vec![2u8; 1000]),
        ("large_packet", vec![3u8; 10000]),
        ("random_data", (0..1000).map(|i| i as u8).collect()),
    ];

    for (name, data) in test_data {
        group.bench_with_input(BenchmarkId::new("lz4_compress", name), &data, |b, input| {
            b.iter(|| {
                let compressed = lz4_flex::compress_prepend_size(input);
                black_box(compressed)
            });
        });

        let compressed = lz4_flex::compress_prepend_size(&data);
        group.bench_with_input(
            BenchmarkId::new("lz4_decompress", name),
            &compressed,
            |b, input| {
                b.iter(|| {
                    let decompressed = lz4_flex::decompress_size_prepended(input).unwrap();
                    black_box(decompressed)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_consensus_operations,
    bench_crypto_operations,
    bench_network_operations,
    bench_concurrent_users,
    bench_memory_operations,
    bench_compression
);

criterion_main!(benches);
