use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bitchat::protocol::*;

fn message_serialization_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");
    
    for size in [100, 1000, 10000, 100000].iter() {
        let data = vec![0u8; *size];
        let message = Message::new(MessageType::Chat, data, PeerId::random());
        
        group.bench_with_input(
            BenchmarkId::new("serialize", size),
            size,
            |b, _| {
                b.iter(|| {
                    black_box(message.serialize().unwrap())
                })
            },
        );
        
        let serialized = message.serialize().unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            size,
            |b, _| {
                b.iter(|| {
                    black_box(Message::deserialize(&serialized).unwrap())
                })
            },
        );
    }
    
    group.finish();
}

fn crypto_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");
    
    let keypair = Encryption::generate_keypair();
    let data = vec![0u8; 1024];
    
    group.bench_function("key_generation", |b| {
        b.iter(|| {
            black_box(Encryption::generate_keypair())
        })
    });
    
    group.bench_function("encryption", |b| {
        b.iter(|| {
            black_box(Encryption::encrypt(&data, &keypair.public_key).unwrap())
        })
    });
    
    let encrypted = Encryption::encrypt(&data, &keypair.public_key).unwrap();
    group.bench_function("decryption", |b| {
        b.iter(|| {
            black_box(Encryption::decrypt(&encrypted, &keypair.private_key).unwrap())
        })
    });
    
    group.finish();
}

criterion_group!(benches, message_serialization_benchmark, crypto_benchmark);
criterion_main!(benches);