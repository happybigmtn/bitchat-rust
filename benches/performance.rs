//! Performance benchmarks for BitCraps

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use bitcraps::*;
use std::time::Duration;

fn benchmark_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto");
    
    // Benchmark key generation
    group.bench_function("keypair_generation", |b| {
        b.iter(|| {
            BitchatKeypair::generate()
        });
    });
    
    // Benchmark signature creation and verification
    let keypair = BitchatKeypair::generate();
    let message = b"Test message for benchmarking signatures";
    
    group.bench_function("sign_message", |b| {
        b.iter(|| {
            keypair.sign(black_box(message))
        });
    });
    
    let signature = keypair.sign(message);
    let public_key = keypair.public_key();
    
    group.bench_function("verify_signature", |b| {
        b.iter(|| {
            public_key.verify(black_box(message), black_box(&signature))
        });
    });
    
    // Benchmark Proof of Work
    for difficulty in [8, 12, 16, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("proof_of_work", difficulty),
            difficulty,
            |b, &difficulty| {
                b.iter(|| {
                    ProofOfWork::mine(black_box(message), black_box(difficulty))
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_packet_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    group.throughput(Throughput::Bytes(1024));
    
    // Create various packet types
    let ping_packet = BitchatPacket::ping([1u8; 32]);
    let game_create = BitchatPacket::game_create([2u8; 32], 100);
    let dice_roll = BitchatPacket::dice_roll([3u8; 32], [4u8; 32], 7, 11);
    
    // Benchmark serialization
    group.bench_function("serialize_ping", |b| {
        b.iter(|| {
            ping_packet.serialize()
        });
    });
    
    group.bench_function("serialize_game_create", |b| {
        b.iter(|| {
            game_create.serialize()
        });
    });
    
    group.bench_function("serialize_dice_roll", |b| {
        b.iter(|| {
            dice_roll.serialize()
        });
    });
    
    // Benchmark deserialization
    let ping_bytes = ping_packet.serialize().unwrap();
    let game_bytes = game_create.serialize().unwrap();
    let dice_bytes = dice_roll.serialize().unwrap();
    
    group.bench_function("deserialize_ping", |b| {
        b.iter(|| {
            BitchatPacket::deserialize(black_box(&ping_bytes))
        });
    });
    
    group.bench_function("deserialize_game_create", |b| {
        b.iter(|| {
            BitchatPacket::deserialize(black_box(&game_bytes))
        });
    });
    
    group.bench_function("deserialize_dice_roll", |b| {
        b.iter(|| {
            BitchatPacket::deserialize(black_box(&dice_bytes))
        });
    });
    
    group.finish();
}

fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");
    
    use bitcraps::protocol::compression::{MessageCompressor, CompressionAlgorithm};
    
    let compressor = MessageCompressor::new();
    
    // Different data types for compression
    let json_data = r#"{"game_id":"abc123","players":[{"id":"player1","balance":1000},{"id":"player2","balance":2000}],"bets":[{"type":"pass","amount":50},{"type":"dont_pass","amount":100}]}"#.as_bytes();
    let binary_data = vec![0xFF; 1024];
    let text_data = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(20).into_bytes();
    
    for (name, data) in [("json", json_data), ("binary", &binary_data), ("text", &text_data)].iter() {
        group.throughput(Throughput::Bytes(data.len() as u64));
        
        group.bench_with_input(
            BenchmarkId::new("compress_lz4", name),
            data,
            |b, data| {
                b.iter(|| {
                    compressor.compress(black_box(data), CompressionAlgorithm::Lz4)
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("compress_zlib", name),
            data,
            |b, data| {
                b.iter(|| {
                    compressor.compress(black_box(data), CompressionAlgorithm::Zlib)
                });
            },
        );
        
        let compressed_lz4 = compressor.compress(data, CompressionAlgorithm::Lz4).unwrap();
        let compressed_zlib = compressor.compress(data, CompressionAlgorithm::Zlib).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("decompress_lz4", name),
            &compressed_lz4,
            |b, compressed| {
                b.iter(|| {
                    compressor.decompress(black_box(compressed))
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("decompress_zlib", name),
            &compressed_zlib,
            |b, compressed| {
                b.iter(|| {
                    compressor.decompress(black_box(compressed))
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache");
    
    use bitcraps::cache::{MultiTierCache, CacheEntry};
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let cache: MultiTierCache<String, Vec<u8>> = 
        MultiTierCache::new(temp_dir.path().to_path_buf()).unwrap();
    
    // Pre-populate cache
    for i in 0..1000 {
        let key = format!("key_{}", i);
        let value = vec![i as u8; 100];
        cache.insert(key, value).unwrap();
    }
    
    group.bench_function("cache_hit_l1", |b| {
        b.iter(|| {
            cache.get(black_box(&"key_500".to_string()))
        });
    });
    
    // Clear L1 to test L2
    cache.clear_all();
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = vec![i as u8; 100];
        cache.insert(key, value).unwrap();
    }
    
    group.bench_function("cache_hit_l2", |b| {
        b.iter(|| {
            cache.get(black_box(&"key_50".to_string()))
        });
    });
    
    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            cache.get(black_box(&"nonexistent_key".to_string()))
        });
    });
    
    group.bench_function("cache_insert", |b| {
        let mut counter = 0;
        b.iter(|| {
            let key = format!("bench_key_{}", counter);
            let value = vec![counter as u8; 100];
            cache.insert(black_box(key), black_box(value)).unwrap();
            counter += 1;
        });
    });
    
    group.finish();
}

fn benchmark_consensus_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("consensus");
    
    use bitcraps::protocol::consensus::{ConsensusEngine, GameConsensusState, ConsensusVote};
    use tokio::runtime::Runtime;
    
    let rt = Runtime::new().unwrap();
    let engine = ConsensusEngine::new([1u8; 32]);
    
    // Initialize with a game state
    let game_id = [2u8; 32];
    rt.block_on(engine.initialize_game(game_id, vec![[3u8; 32], [4u8; 32]]));
    
    group.bench_function("submit_vote", |b| {
        let vote = ConsensusVote {
            voter: [3u8; 32],
            game_id,
            round: 1,
            vote_type: bitcraps::protocol::consensus::VoteType::DiceRoll(7),
            signature: [0u8; 64],
            timestamp: 0,
        };
        
        b.to_async(&rt).iter(|| async {
            engine.submit_vote(black_box(vote.clone())).await
        });
    });
    
    group.bench_function("get_state", |b| {
        b.to_async(&rt).iter(|| async {
            engine.get_state(black_box(&game_id)).await
        });
    });
    
    group.bench_function("finalize_round", |b| {
        b.to_async(&rt).iter(|| async {
            engine.finalize_round(black_box(game_id), black_box(1)).await
        });
    });
    
    group.finish();
}

fn benchmark_mesh_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh");
    
    use bitcraps::mesh::{MeshService, MeshPeer};
    use tokio::runtime::Runtime;
    
    let rt = Runtime::new().unwrap();
    let keypair = BitchatKeypair::generate();
    let mesh = rt.block_on(MeshService::new(keypair));
    
    // Add some peers
    for i in 0..50 {
        let peer = MeshPeer {
            id: [i as u8; 32],
            address: format!("192.168.1.{}", i),
            public_key: [i as u8; 32],
            last_seen: std::time::Instant::now(),
            relay_score: 100,
            is_relay: i % 5 == 0,
        };
        rt.block_on(mesh.add_peer(peer));
    }
    
    let packet = BitchatPacket::ping([1u8; 32]);
    let serialized = packet.serialize().unwrap();
    
    group.bench_function("route_packet", |b| {
        b.to_async(&rt).iter(|| async {
            mesh.route_packet(black_box(&serialized), black_box([25u8; 32])).await
        });
    });
    
    group.bench_function("find_best_route", |b| {
        b.iter(|| {
            mesh.find_best_route(black_box([10u8; 32]), black_box([40u8; 32]))
        });
    });
    
    group.bench_function("broadcast_packet", |b| {
        b.to_async(&rt).iter(|| async {
            mesh.broadcast(black_box(&serialized)).await
        });
    });
    
    group.finish();
}

fn benchmark_token_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("token");
    
    use bitcraps::token::{TokenLedger, ProofOfRelay};
    
    let mut ledger = TokenLedger::new();
    
    // Initialize some accounts
    for i in 0..100 {
        let account_id = [i as u8; 32];
        ledger.create_account(account_id);
        ledger.mint(account_id, 1000);
    }
    
    group.bench_function("transfer", |b| {
        let from = [1u8; 32];
        let to = [2u8; 32];
        b.iter(|| {
            ledger.transfer(black_box(from), black_box(to), black_box(10))
        });
    });
    
    group.bench_function("get_balance", |b| {
        let account = [50u8; 32];
        b.iter(|| {
            ledger.get_balance(black_box(&account))
        });
    });
    
    let proof = ProofOfRelay {
        relayer: [10u8; 32],
        packet_hash: [0xFF; 32],
        timestamp: 0,
        difficulty: 16,
        nonce: 12345,
    };
    
    group.bench_function("validate_proof_of_relay", |b| {
        b.iter(|| {
            proof.validate(black_box(16))
        });
    });
    
    group.bench_function("mint_relay_reward", |b| {
        b.iter(|| {
            ledger.mint(black_box([10u8; 32]), black_box(1))
        });
    });
    
    group.finish();
}

fn benchmark_simd_crypto(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_crypto");
    
    use bitcraps::crypto::simd_acceleration::{SimdCrypto, SimdHash};
    
    let crypto = SimdCrypto::new();
    let hasher = SimdHash::new();
    
    // Benchmark batch signature verification
    let mut signatures = Vec::new();
    let mut messages = Vec::new();
    let mut public_keys = Vec::new();
    
    for i in 0..32 {
        let keypair = BitchatKeypair::generate();
        let message = format!("Message {}", i).into_bytes();
        let signature = keypair.sign(&message);
        
        signatures.push(signature);
        messages.push(message);
        public_keys.push(keypair.public_key());
    }
    
    for batch_size in [4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_verify", batch_size),
            batch_size,
            |b, &size| {
                let sigs = &signatures[..size];
                let msgs = &messages[..size];
                let pks = &public_keys[..size];
                
                b.iter(|| {
                    crypto.batch_verify(black_box(sigs), black_box(msgs), black_box(pks))
                });
            },
        );
    }
    
    // Benchmark SIMD hashing
    let data_1kb = vec![0xFF; 1024];
    let data_4kb = vec![0xFF; 4096];
    let data_16kb = vec![0xFF; 16384];
    
    for (name, data) in [("1KB", &data_1kb), ("4KB", &data_4kb), ("16KB", &data_16kb)].iter() {
        group.throughput(Throughput::Bytes(data.len() as u64));
        
        group.bench_with_input(
            BenchmarkId::new("simd_hash", name),
            data,
            |b, data| {
                b.iter(|| {
                    hasher.hash_data(black_box(data))
                });
            },
        );
    }
    
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = 
        benchmark_crypto_operations,
        benchmark_packet_serialization,
        benchmark_compression,
        benchmark_cache_operations,
        benchmark_consensus_engine,
        benchmark_mesh_routing,
        benchmark_token_operations,
        benchmark_simd_crypto
}

criterion_main!(benches);