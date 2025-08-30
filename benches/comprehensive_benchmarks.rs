//! Comprehensive performance benchmarks for BitCraps
//!
//! These benchmarks measure the performance of critical system components
//! to ensure they meet performance requirements for mobile gaming.

use bitcraps::{
    crypto::{BitchatIdentity, BitchatKeypair, GameCrypto},
    gaming::{random_peer_id, AntiCheatDetector, CrapsBet, GameSessionManager},
    mesh::MeshService,
    protocol::compression::AdaptiveCompression,
    protocol::{BetType, BitchatPacket, GameId, PeerId},
    token::{Account, TokenLedger},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark cryptographic operations
fn benchmark_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");
    group.sample_size(100);

    // Benchmark keypair generation
    group.bench_function("keypair_generation", |b| {
        b.iter(|| {
            let _keypair = BitchatKeypair::generate();
        })
    });

    // Benchmark signing
    group.bench_function("message_signing", |b| {
        let keypair = BitchatKeypair::generate();
        let message = b"benchmark message";

        b.iter(|| {
            let _signature = keypair.sign(black_box(message));
        })
    });

    // Benchmark signature verification
    group.bench_function("signature_verification", |b| {
        let keypair = BitchatKeypair::generate();
        let message = b"benchmark message";
        let signature = keypair.sign(message);

        b.iter(|| {
            let _result = keypair.verify(black_box(message), black_box(&signature));
        })
    });

    // Benchmark proof-of-work generation
    group.bench_function("proof_of_work", |b| {
        let keypair = BitchatKeypair::generate();

        b.iter(|| {
            let _identity = BitchatIdentity::from_keypair_with_pow(
                black_box(keypair.clone()),
                black_box(8), // Lower difficulty for benchmarks
            );
        })
    });

    group.finish();
}

/// Benchmark packet serialization and deserialization
fn benchmark_packet_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_operations");

    let source = [1u8; 32];
    let target = [2u8; 32];
    let game_id = GameId::new();

    group.bench_function("packet_creation", |b| {
        b.iter(|| {
            let _packet = BitchatPacket::new_ping(black_box(source), black_box(target));
        })
    });

    group.bench_function("packet_serialization", |b| {
        let packet = BitchatPacket::new_game_create(source, game_id, vec![source, target]);

        b.iter(|| {
            let _bytes = bincode::serialize(black_box(&packet)).unwrap();
        })
    });

    group.bench_function("packet_deserialization", |b| {
        let packet = BitchatPacket::new_ping(source, target);
        let bytes = bincode::serialize(&packet).unwrap();

        b.iter(|| {
            let _packet: BitchatPacket = bincode::deserialize(black_box(&bytes)).unwrap();
        })
    });

    group.finish();
}

/// Benchmark compression operations
fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Prepare test data
    let text_data = "BitCraps gaming data ".repeat(100).into_bytes();
    let binary_data = (0..1000).map(|i| (i % 256) as u8).collect::<Vec<u8>>();

    for (name, data) in [("text", &text_data), ("binary", &binary_data)] {
        group.bench_with_input(BenchmarkId::new("compress", name), data, |b, data| {
            let mut compressor = AdaptiveCompression::new();
            b.iter(|| {
                let _compressed = compressor.compress_adaptive(black_box(data)).unwrap();
            })
        });

        group.bench_with_input(BenchmarkId::new("decompress", name), data, |b, data| {
            let mut compressor = AdaptiveCompression::new();
            let compressed = compressor.compress_adaptive(data).unwrap();

            b.iter(|| {
                let _decompressed = compressor.decompress(black_box(&compressed)).unwrap();
            })
        });
    }

    group.finish();
}

/// Benchmark gaming operations
fn benchmark_gaming_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("gaming_operations");
    let rt = Runtime::new().unwrap();

    group.bench_function("session_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let manager = GameSessionManager::new(Default::default());
                let creator = random_peer_id();
                let _session_id = manager.create_session(black_box(creator)).await.unwrap();
            })
        })
    });

    group.bench_function("anti_cheat_validation", |b| {
        let detector = AntiCheatDetector::new();
        let player = random_peer_id();

        b.iter(|| {
            rt.block_on(async {
                let bet = CrapsBet {
                    player: black_box(player),
                    bet_type: BetType::Pass,
                    amount: 100,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                let _result = detector
                    .validate_bet(black_box(&bet), black_box(&player))
                    .await;
            })
        })
    });

    group.finish();
}

/// Benchmark token operations
fn benchmark_token_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_operations");

    group.bench_function("account_creation", |b| {
        let mut ledger = TokenLedger::new();

        b.iter(|| {
            let peer_id = random_peer_id();
            let account = Account::new(black_box(peer_id), 1000);
            let _result = ledger.create_account(black_box(account));
        })
    });

    group.bench_function("balance_query", |b| {
        let mut ledger = TokenLedger::new();
        let peer_id = random_peer_id();
        ledger.create_account(Account::new(peer_id, 1000)).unwrap();

        b.iter(|| {
            let _balance = ledger.get_balance(black_box(&peer_id));
        })
    });

    group.bench_function("token_transfer", |b| {
        let mut ledger = TokenLedger::new();
        let sender = random_peer_id();
        let receiver = random_peer_id();

        ledger.create_account(Account::new(sender, 10000)).unwrap();
        ledger.create_account(Account::new(receiver, 0)).unwrap();

        b.iter(|| {
            let _result = ledger.transfer(black_box(sender), black_box(receiver), black_box(100));
        })
    });

    group.finish();
}

/// Benchmark mesh networking operations
fn benchmark_mesh_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_operations");
    group.sample_size(10); // Mesh operations are slower

    let rt = Runtime::new().unwrap();

    group.bench_function("mesh_service_creation", |b| {
        b.iter(|| {
            let peer_id = random_peer_id();
            let _mesh = MeshService::new(black_box(peer_id), Default::default());
        })
    });

    // Note: More complex mesh benchmarks would require actual network setup
    // which is challenging in a benchmark environment

    group.finish();
}

/// Benchmark data structure operations
fn benchmark_data_structures(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_structures");

    // Benchmark HashMap operations with PeerId keys
    group.bench_function("hashmap_insert_peer_id", |b| {
        use std::collections::HashMap;
        let mut map = HashMap::new();

        b.iter(|| {
            let peer_id = random_peer_id();
            map.insert(black_box(peer_id), black_box(100u64));
        })
    });

    group.bench_function("hashmap_lookup_peer_id", |b| {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        let peer_ids: Vec<_> = (0..1000).map(|_| random_peer_id()).collect();

        for peer_id in &peer_ids {
            map.insert(*peer_id, 100u64);
        }

        b.iter(|| {
            let peer_id = &peer_ids[black_box(0)];
            let _value = map.get(black_box(peer_id));
        })
    });

    // Benchmark Vec operations
    group.bench_function("vec_operations", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..100 {
                vec.push(black_box(i));
            }
            vec.sort();
            let _sum: i32 = vec.iter().sum();
        })
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    group.bench_function("frequent_allocation", |b| {
        b.iter(|| {
            let _data: Vec<Vec<u8>> = (0..100).map(|_| vec![0u8; black_box(64)]).collect();
        })
    });

    group.bench_function("large_allocation", |b| {
        b.iter(|| {
            let _data = vec![0u8; black_box(10000)];
        })
    });

    group.bench_function("arc_clone_operations", |b| {
        use std::sync::Arc;
        let data = Arc::new(vec![0u8; 1000]);

        b.iter(|| {
            let _clones: Vec<_> = (0..100).map(|_| Arc::clone(black_box(&data))).collect();
        })
    });

    group.finish();
}

/// Benchmark concurrent operations
fn benchmark_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency");
    let rt = Runtime::new().unwrap();

    group.bench_function("async_spawn_join", |b| {
        b.iter(|| {
            rt.block_on(async {
                let handles: Vec<_> = (0..10)
                    .map(|i| {
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_micros(black_box(i * 10))).await;
                            i * 2
                        })
                    })
                    .collect();

                let _results: Vec<_> = futures::future::join_all(handles)
                    .await
                    .into_iter()
                    .map(|r| r.unwrap())
                    .collect();
            })
        })
    });

    group.bench_function("mutex_operations", |b| {
        use std::sync::{Arc, Mutex};

        let counter = Arc::new(Mutex::new(0i32));

        b.iter(|| {
            rt.block_on(async {
                let handles: Vec<_> = (0..10)
                    .map(|_| {
                        let counter = Arc::clone(&counter);
                        tokio::spawn(async move {
                            let mut num = counter.lock().unwrap();
                            *num += black_box(1);
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.await.unwrap();
                }
            })
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_crypto_operations,
    benchmark_packet_operations,
    benchmark_compression,
    benchmark_gaming_operations,
    benchmark_token_operations,
    benchmark_mesh_operations,
    benchmark_data_structures,
    benchmark_memory_patterns,
    benchmark_concurrency
);

criterion_main!(benches);
