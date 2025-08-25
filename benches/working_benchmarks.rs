//! Working performance benchmarks for BitCraps
//! 
//! These benchmarks test the actual implementations that exist in the codebase.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bitcraps::{
    protocol::PeerId,
    crypto::{BitchatIdentity, GameCrypto},
};
use std::time::Duration;

/// Benchmark cryptographic operations
fn benchmark_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");
    group.sample_size(100);
    
    // Benchmark identity generation with proof-of-work
    group.bench_function("identity_generation_pow_8", |b| {
        b.iter(|| {
            let _identity = BitchatIdentity::generate_with_pow(black_box(8));
        })
    });
    
    // Benchmark signature operations
    group.bench_function("message_signing", |b| {
        let identity = BitchatIdentity::generate_with_pow(0);
        let message = b"benchmark message for signing";
        
        b.iter(|| {
            let _signature = identity.sign(black_box(message));
        })
    });
    
    // Benchmark signature verification
    group.bench_function("signature_verification", |b| {
        let crypto = GameCrypto::new();
        let identity = BitchatIdentity::generate_with_pow(0);
        let message = b"benchmark message";
        let signature = identity.sign(message);
        let peer_id: PeerId = [0u8; 32]; // Mock peer ID
        
        b.iter(|| {
            let _result = crypto.verify_signature(
                black_box(&peer_id),
                black_box(message),
                black_box(&signature.signature)
            );
        })
    });
    
    group.finish();
}

/// Benchmark hashing operations
fn benchmark_hash_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_operations");
    
    use sha2::{Sha256, Digest};
    
    group.bench_function("sha256_small", |b| {
        let data = b"small data to hash";
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(data));
            let _result = hasher.finalize();
        })
    });
    
    group.bench_function("sha256_1kb", |b| {
        let data = vec![0u8; 1024];
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&data));
            let _result = hasher.finalize();
        })
    });
    
    group.bench_function("sha256_10kb", |b| {
        let data = vec![0u8; 10240];
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&data));
            let _result = hasher.finalize();
        })
    });
    
    group.finish();
}

/// Benchmark serialization operations
fn benchmark_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    
    use serde::{Serialize, Deserialize};
    
    #[derive(Serialize, Deserialize)]
    struct TestPacket {
        id: u64,
        source: [u8; 32],
        target: [u8; 32],
        payload: Vec<u8>,
    }
    
    let packet = TestPacket {
        id: 12345,
        source: [1u8; 32],
        target: [2u8; 32],
        payload: vec![0u8; 256],
    };
    
    group.bench_function("bincode_serialize", |b| {
        b.iter(|| {
            let _bytes = bincode::serialize(black_box(&packet)).unwrap();
        })
    });
    
    group.bench_function("bincode_deserialize", |b| {
        let bytes = bincode::serialize(&packet).unwrap();
        b.iter(|| {
            let _packet: TestPacket = bincode::deserialize(black_box(&bytes)).unwrap();
        })
    });
    
    group.finish();
}

/// Benchmark memory operations
fn benchmark_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    
    use std::sync::Arc;
    
    group.bench_function("arc_creation", |b| {
        let data = vec![0u8; 1000];
        b.iter(|| {
            let _arc = Arc::new(black_box(data.clone()));
        })
    });
    
    group.bench_function("arc_cloning", |b| {
        let data = Arc::new(vec![0u8; 1000]);
        b.iter(|| {
            let _clone = Arc::clone(black_box(&data));
        })
    });
    
    group.bench_function("vec_allocation_small", |b| {
        b.iter(|| {
            let _vec: Vec<u8> = vec![0u8; black_box(64)];
        })
    });
    
    group.bench_function("vec_allocation_large", |b| {
        b.iter(|| {
            let _vec: Vec<u8> = vec![0u8; black_box(10000)];
        })
    });
    
    group.finish();
}

/// Benchmark concurrent operations
fn benchmark_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency");
    
    use std::sync::{Arc, Mutex, RwLock};
    use std::collections::HashMap;
    
    group.bench_function("mutex_lock_unlock", |b| {
        let mutex = Arc::new(Mutex::new(0u64));
        b.iter(|| {
            let mut guard = mutex.lock().unwrap();
            *guard += black_box(1);
        })
    });
    
    group.bench_function("rwlock_read", |b| {
        let rwlock = Arc::new(RwLock::new(HashMap::<u64, u64>::new()));
        for i in 0..100 {
            rwlock.write().unwrap().insert(i, i * 2);
        }
        
        b.iter(|| {
            let guard = rwlock.read().unwrap();
            let _value = guard.get(black_box(&50));
        })
    });
    
    group.bench_function("rwlock_write", |b| {
        let rwlock = Arc::new(RwLock::new(0u64));
        b.iter(|| {
            let mut guard = rwlock.write().unwrap();
            *guard += black_box(1);
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_crypto_operations,
    benchmark_hash_operations,
    benchmark_serialization,
    benchmark_memory_patterns,
    benchmark_concurrency
);

criterion_main!(benches);