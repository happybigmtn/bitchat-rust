use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use bitcraps::optimization::{CpuOptimizer, MessagePool, VoteTracker, CircularBuffer, AutoGarbageCollector};
use bitcraps::protocol::PeerId;
use std::time::Duration;

/// Simplified benchmarks for optimization components that are working
pub fn cpu_optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_optimization");
    let cpu_optimizer = CpuOptimizer::new();
    
    // SIMD hash benchmarks
    let data_sizes = [64, 256, 1024, 4096];
    for size in data_sizes.iter() {
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
        let chunks: Vec<&[u8]> = (0..8).map(|_| {
            let data = vec![0u8; size];
            Box::leak(data.into_boxed_slice()) as &[u8]
        }).collect();
        
        group.bench_with_input(
            BenchmarkId::new("parallel_hash_batch", size),
            size,
            |b, _| {
                b.iter(|| cpu_optimizer.parallel_hash_batch(&chunks))
            }
        );
    }
    
    group.finish();
}

/// Memory optimization benchmarks
pub fn memory_optimization_benchmarks(c: &mut Criterion) {
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
                let _result = gc.get(&key);
            }
        })
    });
    
    group.finish();
}

/// Integration benchmarks testing multiple optimization systems together
pub fn integration_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration_optimization");
    
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
            let _stats = memory_pool.stats();
            let _counts = vote_tracker.get_counts();
            let _buffer_len = circular_buffer.len();
        })
    });
    
    // Hash computation workload
    group.bench_function("hash_computation_workload", |b| {
        b.iter(|| {
            let cpu_optimizer = CpuOptimizer::new();
            let data_chunks: Vec<Vec<u8>> = (0..100)
                .map(|i| vec![i as u8; 1024])
                .collect();
            
            let chunk_refs: Vec<&[u8]> = data_chunks.iter().map(|v| v.as_slice()).collect();
            let _hashes = cpu_optimizer.parallel_hash_batch(&chunk_refs);
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    cpu_optimization_benchmarks,
    memory_optimization_benchmarks,
    integration_benchmarks
);

criterion_main!(benches);