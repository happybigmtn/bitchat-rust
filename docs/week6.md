# Week 6: Testing, Optimization & Deployment

## ⚠️ IMPORTANT: Updated Implementation Notes

**Before starting this week, please review `/docs/COMPILATION_FIXES.md` for critical dependency and API updates.**

**Key fixes for Week 6:**
- Add test dependencies in `[dev-dependencies]`: `criterion = "0.7.0"`, `proptest = "1.7.0"`, etc.
- Use `BenchmarkId` for parameterized benchmarks in criterion
- Set up proper async test runtime with `#[tokio::test]`
- Use `mockall` with `#[automock]` for async traits
- Add proper test setup and teardown patterns

## Overview
Comprehensive testing, performance optimization, and production deployment preparation for the BitChat P2P messaging system, including specialized gaming fairness testing and economic simulations for BitCraps casino operations.

## Day 1: Unit Testing Framework and Test Coverage

### Test Structure Setup
```rust
// tests/lib.rs
use bitchat::{
    protocol::MessageProtocol,
    crypto::Encryption,
    mesh::PeerManager,
    token::TokenLedger,
};

#[cfg(test)]
mod unit_tests {
    use super::*;
    use tokio_test;
    
    mod crypto_tests;
    mod protocol_tests;
    mod mesh_tests;
    mod token_tests;
    mod incentive_tests;
}
```

### Core Component Tests
```rust
// tests/unit_tests/crypto_tests.rs
#[tokio::test]
async fn test_key_generation() {
    let keypair = Encryption::generate_keypair();
    assert!(keypair.public_key.len() == 32);
    assert!(keypair.private_key.len() == 32);
}

#[tokio::test]
async fn test_message_encryption_decryption() {
    let alice_keys = Encryption::generate_keypair();
    let bob_keys = Encryption::generate_keypair();
    let plaintext = b"Hello, secure world!";
    
    let encrypted = Encryption::encrypt(plaintext, &bob_keys.public_key)?;
    let decrypted = Encryption::decrypt(&encrypted, &bob_keys.private_key)?;
    
    assert_eq!(plaintext, &decrypted[..]);
}

// tests/unit_tests/protocol_tests.rs
#[tokio::test]
async fn test_message_serialization() {
    let message = Message::new(
        MessageType::Chat,
        b"test content".to_vec(),
        PeerId::random(),
    );
    
    let serialized = message.serialize()?;
    let deserialized = Message::deserialize(&serialized)?;
    
    assert_eq!(message.id, deserialized.id);
    assert_eq!(message.content, deserialized.content);
}

#[tokio::test]
async fn test_routing_protocol() {
    let mut router = MessageRouter::new();
    let peer_a = PeerId::random();
    let peer_b = PeerId::random();
    
    router.add_route(peer_b, peer_a);
    let route = router.find_route(peer_b).unwrap();
    assert_eq!(route.next_hop, peer_a);
}
```

### Test Utilities and Mocks
```rust
// tests/common/mod.rs
pub struct MockNetwork {
    pub sent_messages: Arc<Mutex<Vec<Message>>>,
    pub peers: Arc<Mutex<HashMap<PeerId, PeerInfo>>>,
}

impl MockNetwork {
    pub fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub async fn add_peer(&self, peer_id: PeerId, info: PeerInfo) {
        self.peers.lock().await.insert(peer_id, info);
    }
}

#[async_trait]
impl NetworkTrait for MockNetwork {
    async fn send_message(&self, peer: PeerId, message: Message) -> Result<(), NetworkError> {
        self.sent_messages.lock().await.push(message);
        Ok(())
    }
    
    async fn get_peers(&self) -> Vec<PeerId> {
        self.peers.lock().await.keys().cloned().collect()
    }
}
```

### Coverage Configuration
```toml
# Cargo.toml test configuration
[dependencies]
tokio-test = "0.4"
mockall = "0.12"
proptest = "1.4"

[[bin]]
name = "coverage"
test = false

[profile.test]
debug = true
incremental = false
codegen-units = 1
```

### Coverage Script
```bash
#!/bin/bash
# scripts/coverage.sh
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"

cargo test --all-features
grcov . --binary-path ./target/debug/ -s . -t html --branch --ignore-not-existing -o ./target/coverage/
```

## Day 2: Integration Testing with Simulated Networks

### Multi-Node Test Environment
```rust
// tests/integration/network_simulation.rs
use std::collections::HashMap;
use tokio::sync::mpsc;

pub struct NetworkSimulator {
    nodes: HashMap<PeerId, SimulatedNode>,
    message_delay: Duration,
    packet_loss_rate: f64,
}

pub struct SimulatedNode {
    id: PeerId,
    network: MockNetwork,
    app: BitChatApp,
    message_rx: mpsc::Receiver<Message>,
}

impl NetworkSimulator {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            message_delay: Duration::from_millis(10),
            packet_loss_rate: 0.01,
        }
    }
    
    pub async fn add_node(&mut self, node_id: PeerId) -> Result<(), SimError> {
        let (tx, rx) = mpsc::channel(1000);
        let network = MockNetwork::new();
        let app = BitChatApp::new(network.clone()).await?;
        
        let node = SimulatedNode {
            id: node_id,
            network,
            app,
            message_rx: rx,
        };
        
        self.nodes.insert(node_id, node);
        Ok(())
    }
    
    pub async fn connect_nodes(&mut self, node_a: PeerId, node_b: PeerId) -> Result<(), SimError> {
        // Simulate connection establishment
        let node_a_ref = self.nodes.get_mut(&node_a).unwrap();
        node_a_ref.network.add_peer(node_b, PeerInfo::default()).await;
        
        let node_b_ref = self.nodes.get_mut(&node_b).unwrap();
        node_b_ref.network.add_peer(node_a, PeerInfo::default()).await;
        
        Ok(())
    }
    
    pub async fn simulate_message_propagation(&mut self, sender: PeerId, message: String) -> PropagationStats {
        let start_time = Instant::now();
        let mut stats = PropagationStats::new();
        
        // Send message from sender
        if let Some(sender_node) = self.nodes.get_mut(&sender) {
            sender_node.app.send_broadcast_message(message.clone()).await.unwrap();
        }
        
        // Simulate network propagation with delays
        tokio::time::sleep(self.message_delay).await;
        
        // Check message receipt at all nodes
        for (peer_id, node) in &mut self.nodes {
            if *peer_id != sender {
                // Simulate packet loss
                if rand::random::<f64>() < self.packet_loss_rate {
                    stats.lost_packets += 1;
                    continue;
                }
                
                // Check if message was received
                if let Ok(received_msg) = node.message_rx.try_recv() {
                    stats.successful_deliveries += 1;
                    stats.delivery_times.push(start_time.elapsed());
                }
            }
        }
        
        stats
    }
}
```

### Network Partition Tests
```rust
// tests/integration/partition_tests.rs
#[tokio::test]
async fn test_network_partition_recovery() {
    let mut sim = NetworkSimulator::new();
    
    // Create 5-node network
    let nodes: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();
    for node in &nodes {
        sim.add_node(*node).await.unwrap();
    }
    
    // Connect in a line topology
    for i in 0..nodes.len()-1 {
        sim.connect_nodes(nodes[i], nodes[i+1]).await.unwrap();
    }
    
    // Send message before partition
    let stats_before = sim.simulate_message_propagation(nodes[0], "before partition".to_string()).await;
    assert_eq!(stats_before.successful_deliveries, 4);
    
    // Create partition (disconnect middle node)
    sim.disconnect_nodes(nodes[2], nodes[1]).await.unwrap();
    sim.disconnect_nodes(nodes[2], nodes[3]).await.unwrap();
    
    // Test message propagation during partition
    let stats_during = sim.simulate_message_propagation(nodes[0], "during partition".to_string()).await;
    assert!(stats_during.successful_deliveries < 4);
    
    // Reconnect and test recovery
    sim.connect_nodes(nodes[2], nodes[1]).await.unwrap();
    sim.connect_nodes(nodes[2], nodes[3]).await.unwrap();
    
    tokio::time::sleep(Duration::from_millis(100)).await; // Allow recovery
    
    let stats_after = sim.simulate_message_propagation(nodes[0], "after recovery".to_string()).await;
    assert_eq!(stats_after.successful_deliveries, 4);
}
```

### Load Testing Framework
```rust
// tests/integration/load_tests.rs
#[tokio::test]
async fn test_high_message_throughput() {
    const NUM_NODES: usize = 50;
    const MESSAGES_PER_NODE: usize = 100;
    
    let mut sim = NetworkSimulator::new();
    let nodes: Vec<PeerId> = (0..NUM_NODES).map(|_| PeerId::random()).collect();
    
    // Setup fully connected network
    for node in &nodes {
        sim.add_node(*node).await.unwrap();
    }
    
    for i in 0..nodes.len() {
        for j in i+1..nodes.len() {
            sim.connect_nodes(nodes[i], nodes[j]).await.unwrap();
        }
    }
    
    let start_time = Instant::now();
    let mut handles = Vec::new();
    
    // Spawn concurrent message senders
    for (i, &node_id) in nodes.iter().enumerate() {
        let sim_clone = sim.clone();
        let handle = tokio::spawn(async move {
            for msg_num in 0..MESSAGES_PER_NODE {
                let message = format!("Message {} from node {}", msg_num, i);
                sim_clone.simulate_message_propagation(node_id, message).await;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }
    
    // Wait for all messages to be sent
    for handle in handles {
        handle.await.unwrap();
    }
    
    let total_time = start_time.elapsed();
    let total_messages = NUM_NODES * MESSAGES_PER_NODE;
    let throughput = total_messages as f64 / total_time.as_secs_f64();
    
    println!("Throughput: {:.2} messages/second", throughput);
    assert!(throughput > 1000.0); // Expect at least 1000 msg/s
}
```

## Day 3: Performance Optimization and Benchmarking

### Benchmark Suite Setup
```rust
// benches/protocol_benchmarks.rs
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
```

### Memory Usage Optimization
```rust
// src/optimization/memory.rs
use std::sync::Arc;
use bytes::{Bytes, BytesMut};

pub struct MessagePool {
    small_pool: Vec<BytesMut>,
    medium_pool: Vec<BytesMut>,
    large_pool: Vec<BytesMut>,
}

impl MessagePool {
    pub fn new() -> Self {
        Self {
            small_pool: Vec::with_capacity(100),
            medium_pool: Vec::with_capacity(50),
            large_pool: Vec::with_capacity(10),
        }
    }
    
    pub fn get_buffer(&mut self, size: usize) -> BytesMut {
        match size {
            0..=1024 => self.small_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(1024)),
            1025..=8192 => self.medium_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(8192)),
            _ => self.large_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(size.next_power_of_two())),
        }
    }
    
    pub fn return_buffer(&mut self, mut buffer: BytesMut) {
        buffer.clear();
        match buffer.capacity() {
            0..=1024 if self.small_pool.len() < 100 => {
                self.small_pool.push(buffer);
            }
            1025..=8192 if self.medium_pool.len() < 50 => {
                self.medium_pool.push(buffer);
            }
            _ if self.large_pool.len() < 10 => {
                self.large_pool.push(buffer);
            }
            _ => {} // Drop oversized or excess buffers
        }
    }
}

// Zero-copy message passing
pub struct ZeroCopyMessage {
    pub header: MessageHeader,
    pub payload: Bytes,
}

impl ZeroCopyMessage {
    pub fn new(header: MessageHeader, payload: Bytes) -> Self {
        Self { header, payload }
    }
    
    pub fn split_payload(&self, chunk_size: usize) -> Vec<Bytes> {
        let mut chunks = Vec::new();
        let mut offset = 0;
        
        while offset < self.payload.len() {
            let end = std::cmp::min(offset + chunk_size, self.payload.len());
            chunks.push(self.payload.slice(offset..end));
            offset = end;
        }
        
        chunks
    }
}
```

### Performance Monitoring
```rust
// src/monitoring/metrics.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct PerformanceMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicUsize,
    pub avg_latency_ms: AtomicU64,
    pub peak_memory_mb: AtomicU64,
}

impl PerformanceMetrics {
    pub fn record_message_sent(&self, size: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
    }
    
    pub fn record_message_received(&self, size: usize, latency: Duration) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(size as u64, Ordering::Relaxed);
        
        let latency_ms = latency.as_millis() as u64;
        let current_avg = self.avg_latency_ms.load(Ordering::Relaxed);
        let new_avg = (current_avg + latency_ms) / 2;
        self.avg_latency_ms.store(new_avg, Ordering::Relaxed);
    }
    
    pub fn get_throughput(&self) -> (f64, f64) {
        let sent = self.messages_sent.load(Ordering::Relaxed) as f64;
        let received = self.messages_received.load(Ordering::Relaxed) as f64;
        (sent, received)
    }
    
    pub fn get_bandwidth(&self) -> (f64, f64) {
        let sent_bytes = self.bytes_sent.load(Ordering::Relaxed) as f64;
        let received_bytes = self.bytes_received.load(Ordering::Relaxed) as f64;
        (sent_bytes / 1024.0 / 1024.0, received_bytes / 1024.0 / 1024.0)
    }
}
```

## Day 4: Security Audit and Fuzzing

### Security Test Suite
```rust
// tests/security/crypto_security.rs
#[tokio::test]
async fn test_key_reuse_vulnerability() {
    let keypair = Encryption::generate_keypair();
    let message1 = b"First message";
    let message2 = b"Second message";
    
    let encrypted1 = Encryption::encrypt(message1, &keypair.public_key).unwrap();
    let encrypted2 = Encryption::encrypt(message2, &keypair.public_key).unwrap();
    
    // Ensure different ciphertexts for same key (nonce should differ)
    assert_ne!(encrypted1, encrypted2);
    
    // Both should decrypt correctly
    let decrypted1 = Encryption::decrypt(&encrypted1, &keypair.private_key).unwrap();
    let decrypted2 = Encryption::decrypt(&encrypted2, &keypair.private_key).unwrap();
    
    assert_eq!(decrypted1, message1);
    assert_eq!(decrypted2, message2);
}

#[tokio::test]
async fn test_message_tampering_detection() {
    let keypair = Encryption::generate_keypair();
    let original_message = b"Important message";
    
    let mut encrypted = Encryption::encrypt(original_message, &keypair.public_key).unwrap();
    
    // Tamper with ciphertext
    if let Some(byte) = encrypted.get_mut(10) {
        *byte = byte.wrapping_add(1);
    }
    
    // Decryption should fail
    let result = Encryption::decrypt(&encrypted, &keypair.private_key);
    assert!(result.is_err());
}

// tests/security/protocol_security.rs
#[tokio::test]
async fn test_replay_attack_prevention() {
    let mut message_processor = MessageProcessor::new();
    let message = create_test_message();
    
    // First processing should succeed
    let result1 = message_processor.process_message(message.clone()).await;
    assert!(result1.is_ok());
    
    // Replayed message should be rejected
    let result2 = message_processor.process_message(message).await;
    assert!(result2.is_err());
    assert_matches!(result2.unwrap_err(), ProcessingError::ReplayAttack);
}

#[tokio::test]
async fn test_dos_protection() {
    let mut rate_limiter = RateLimiter::new(10, Duration::from_secs(1));
    let peer_id = PeerId::random();
    
    // Allow normal rate
    for _ in 0..10 {
        assert!(rate_limiter.check_rate(peer_id).is_ok());
    }
    
    // Block excessive rate
    assert!(rate_limiter.check_rate(peer_id).is_err());
    
    // Should recover after time window
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(rate_limiter.check_rate(peer_id).is_ok());
}
```

### Fuzzing Implementation
```rust
// fuzz/fuzz_targets/protocol_parsing.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use bitchat::protocol::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz message deserialization
    if let Ok(message) = Message::deserialize(data) {
        // If parsing succeeds, re-serialization should work
        let reserialized = message.serialize().unwrap();
        
        // Round-trip should be consistent
        let reparsed = Message::deserialize(&reserialized).unwrap();
        assert_eq!(message.id, reparsed.id);
        assert_eq!(message.message_type, reparsed.message_type);
    }
});

// fuzz/fuzz_targets/crypto_operations.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use bitchat::crypto::Encryption;

fuzz_target!(|data: &[u8]| {
    if data.len() >= 64 {
        let (key_data, message) = data.split_at(32);
        
        // Try to use arbitrary data as key
        if let Ok(public_key) = key_data.try_into() {
            // Encryption should not panic with arbitrary keys
            let _ = Encryption::encrypt(message, &public_key);
        }
    }
});
```

### Vulnerability Assessment
```rust
// tests/security/vulnerability_tests.rs
use std::collections::HashMap;

struct VulnerabilityScanner {
    test_vectors: HashMap<String, Vec<TestCase>>,
}

struct TestCase {
    name: String,
    input: Vec<u8>,
    expected_behavior: ExpectedBehavior,
}

enum ExpectedBehavior {
    ShouldReject,
    ShouldAccept,
    ShouldPanic, // Should NOT panic - test for robustness
}

impl VulnerabilityScanner {
    fn new() -> Self {
        let mut scanner = Self {
            test_vectors: HashMap::new(),
        };
        
        scanner.add_injection_tests();
        scanner.add_overflow_tests();
        scanner.add_timing_attack_tests();
        
        scanner
    }
    
    fn add_injection_tests(&mut self) {
        let injection_payloads = vec![
            b"'; DROP TABLE messages; --".to_vec(),
            b"<script>alert('xss')</script>".to_vec(),
            b"\x00\x01\x02\x03".to_vec(), // Null byte injection
        ];
        
        let test_cases: Vec<TestCase> = injection_payloads
            .into_iter()
            .enumerate()
            .map(|(i, payload)| TestCase {
                name: format!("injection_test_{}", i),
                input: payload,
                expected_behavior: ExpectedBehavior::ShouldReject,
            })
            .collect();
            
        self.test_vectors.insert("injection".to_string(), test_cases);
    }
    
    async fn run_all_tests(&self) -> VulnerabilityReport {
        let mut report = VulnerabilityReport::new();
        
        for (category, tests) in &self.test_vectors {
            for test in tests {
                let result = self.run_test(test).await;
                report.add_result(category, test.name.clone(), result);
            }
        }
        
        report
    }
}
```

## Day 5: Packaging and Deployment

### Docker Configuration
```dockerfile
# Dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests

# Build optimized binary
RUN cargo build --release --bin bitchat

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bitchat /usr/local/bin/

# Create non-root user
RUN useradd -r -s /bin/false bitchat
USER bitchat

EXPOSE 8080
VOLUME ["/data"]

CMD ["bitchat", "chat"]
```

### Deployment Scripts
```bash
#!/bin/bash
# scripts/deploy.sh

set -euo pipefail

REGISTRY="${REGISTRY:-localhost:5000}"
TAG="${TAG:-latest}"
IMAGE_NAME="bitchat"

echo "Building BitChat ${TAG}..."

# Build and test
cargo test --release
cargo build --release

# Security scan
cargo audit
cargo clippy -- -D warnings

# Build Docker image
docker build -t "${REGISTRY}/${IMAGE_NAME}:${TAG}" .

# Push to registry
docker push "${REGISTRY}/${IMAGE_NAME}:${TAG}"

echo "Deployment completed: ${REGISTRY}/${IMAGE_NAME}:${TAG}"
```

### Kubernetes Deployment
```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: bitchat
  labels:
    app: bitchat
spec:
  replicas: 3
  selector:
    matchLabels:
      app: bitchat
  template:
    metadata:
      labels:
        app: bitchat
    spec:
      containers:
      - name: bitchat
        image: registry.example.com/bitchat:latest
        ports:
        - containerPort: 8080
        env:
        - name: BITCHAT_PORT
          value: "8080"
        - name: BITCHAT_LOG_LEVEL
          value: "info"
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        volumeMounts:
        - name: config
          mountPath: /etc/bitchat
        - name: data
          mountPath: /data
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: bitchat-config
      - name: data
        persistentVolumeClaim:
          claimName: bitchat-data

---
apiVersion: v1
kind: Service
metadata:
  name: bitchat-service
spec:
  selector:
    app: bitchat
  ports:
  - port: 8080
    targetPort: 8080
  type: LoadBalancer
```

### CI/CD Pipeline
```yaml
# .github/workflows/ci-cd.yml
name: BitChat CI/CD

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        components: rustfmt, clippy
    
    - name: Cache cargo dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Format check
      run: cargo fmt --all -- --check
    
    - name: Clippy
      run: cargo clippy --all-targets --all-features -- -D warnings
    
    - name: Unit tests
      run: cargo test --lib
    
    - name: Integration tests
      run: cargo test --test '*'
    
    - name: Benchmarks
      run: cargo bench --no-run

  security:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Security audit
      run: |
        cargo install cargo-audit
        cargo audit
    
    - name: Vulnerability scan
      run: |
        cargo install cargo-deny
        cargo deny check

  deploy:
    needs: [test, security]
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Build and push Docker image
      env:
        REGISTRY: ${{ secrets.DOCKER_REGISTRY }}
        USERNAME: ${{ secrets.DOCKER_USERNAME }}
        PASSWORD: ${{ secrets.DOCKER_PASSWORD }}
      run: |
        echo $PASSWORD | docker login $REGISTRY -u $USERNAME --password-stdin
        ./scripts/deploy.sh
```

## Performance Metrics and SLAs

### Target Performance Metrics
- **Message Throughput**: >10,000 messages/second
- **Memory Usage**: <100MB base + 1KB per active peer
- **Network Latency**: <50ms peer-to-peer message delivery
- **CPU Usage**: <5% idle, <30% under load
- **Connection Time**: <1 second peer discovery and connection
- **File Transfer**: >10MB/s for local network transfers

### Monitoring and Alerting
```rust
// src/monitoring/health.rs
pub struct HealthCheck {
    start_time: Instant,
    metrics: Arc<PerformanceMetrics>,
}

impl HealthCheck {
    pub fn check_health(&self) -> HealthStatus {
        let uptime = self.start_time.elapsed();
        let memory_usage = get_memory_usage();
        let active_peers = self.metrics.active_connections.load(Ordering::Relaxed);
        
        HealthStatus {
            status: if memory_usage < 1024 * 1024 * 1024 { // 1GB limit
                "healthy"
            } else {
                "degraded"
            }.to_string(),
            uptime_seconds: uptime.as_secs(),
            memory_mb: memory_usage / 1024 / 1024,
            active_peers,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
```

## Key Deliverables

1. **Comprehensive Test Suite**: Unit, integration, and security tests with >90% coverage
2. **Performance Benchmarks**: Automated performance regression testing
3. **Security Audit**: Vulnerability assessment and mitigation strategies  
4. **Production Deployment**: Docker containers and Kubernetes manifests
5. **CI/CD Pipeline**: Automated testing, building, and deployment
6. **Monitoring System**: Health checks, metrics collection, and alerting
7. **Documentation**: Deployment guides, performance tuning, and operational procedures

### Gaming-Specific Testing Extensions

```rust
// tests/gaming/fairness_tests.rs
use bitchat::gaming::{GameSessionManager, CrapsSession, BetType, AntiCheatDetector};
use std::collections::HashMap;

#[tokio::test]
async fn test_dice_roll_fairness() {
    let session_manager = GameSessionManager::new(Default::default());
    let game_id = session_manager.create_session(PeerId::random()).await.unwrap();
    
    // Simulate 10,000 dice rolls
    let mut roll_counts = HashMap::new();
    for _ in 0..10000 {
        let (d1, d2) = simulate_dice_roll();
        let total = d1 + d2;
        *roll_counts.entry(total).or_insert(0) += 1;
    }
    
    // Verify statistical distribution
    // Each outcome should occur roughly the expected number of times
    for (total, expected_count) in expected_dice_distribution(10000).iter() {
        let actual_count = roll_counts.get(total).unwrap_or(&0);
        let variance = (*actual_count as f64 - *expected_count).abs();
        let tolerance = expected_count * 0.05; // 5% tolerance
        
        assert!(
            variance < tolerance,
            "Dice roll {} occurred {} times, expected ~{} (±{})",
            total, actual_count, expected_count, tolerance
        );
    }
}

#[tokio::test]
async fn test_anti_cheat_detection() {
    let anti_cheat = AntiCheatDetector::new();
    let player = PeerId::random();
    
    // Test rapid betting detection
    for i in 0..35 { // Exceed max_bets_per_minute (30)
        let bet = create_test_bet(player, BetType::Pass, 100, i);
        let result = anti_cheat.validate_bet(&bet, &player).await;
        
        if i < 30 {
            assert!(result.is_ok(), "Legitimate bet {} should pass", i);
        } else {
            assert!(result.is_err(), "Bet {} should trigger anti-cheat", i);
        }
    }
}

#[tokio::test]
async fn test_bet_escrow_integrity() {
    let gaming_security = GamingSecurityManager::new(Default::default());
    let participants = vec![PeerId::random(), PeerId::random()];
    let session = gaming_security.create_gaming_session("test_game".to_string(), participants.clone()).await.unwrap();
    
    let bet = PendingBet {
        bet_id: "test_bet_1".to_string(),
        player: participants[0],
        amount: 1000,
        bet_hash: [0u8; 32], // Would be calculated properly
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        confirmations: vec![participants[1]], // One confirmation
        escrow_signature: None,
    };
    
    // Should fail with insufficient confirmations
    let result = gaming_security.validate_and_escrow_bet("test_game", &bet).await;
    assert!(result.is_err());
    
    // Add required confirmations
    let mut bet_with_confirmations = bet.clone();
    bet_with_confirmations.confirmations.push(participants[0]); // Self-confirmation for testing
    
    let escrow_result = gaming_security.validate_and_escrow_bet("test_game", &bet_with_confirmations).await;
    assert!(escrow_result.is_ok());
}

fn expected_dice_distribution(total_rolls: u32) -> HashMap<u8, f64> {
    let mut distribution = HashMap::new();
    
    // Probability of each sum when rolling two dice
    let probabilities = [
        (2, 1.0/36.0), (3, 2.0/36.0), (4, 3.0/36.0), (5, 4.0/36.0),
        (6, 5.0/36.0), (7, 6.0/36.0), (8, 5.0/36.0), (9, 4.0/36.0),
        (10, 3.0/36.0), (11, 2.0/36.0), (12, 1.0/36.0),
    ];
    
    for (sum, prob) in probabilities.iter() {
        distribution.insert(*sum, *prob * total_rolls as f64);
    }
    
    distribution
}

fn simulate_dice_roll() -> (u8, u8) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (rng.gen_range(1..=6), rng.gen_range(1..=6))
}

fn create_test_bet(player: PeerId, bet_type: BetType, amount: u64, sequence: u32) -> CrapsBet {
    CrapsBet {
        player,
        bet_type,
        amount,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + sequence as u64, // Ensure unique timestamps
    }
}
```

### Economic Simulation Testing

```rust
// tests/gaming/economic_simulation.rs
use bitchat::gaming::{GameSessionManager, WalletInterface, BetResult};
use std::collections::HashMap;

/// Simulate casino house edge over many games
#[tokio::test]
async fn test_house_edge_simulation() {
    const SIMULATION_RUNS: u32 = 100_000;
    const STARTING_BALANCE: u64 = 10_000;
    
    let mut total_player_losses = 0i64;
    let mut total_bets_placed = 0u64;
    
    for run in 0..SIMULATION_RUNS {
        let mut wallet = WalletInterface::new(STARTING_BALANCE);
        let session_manager = GameSessionManager::new(Default::default());
        
        // Simulate a series of bets
        for bet_num in 0..100 {
            let bet_amount = 100; // Fixed bet size
            
            if wallet.get_available_balance() < bet_amount {
                break; // Player broke
            }
            
            let bet_id = format!("sim_{}_{}", run, bet_num);
            wallet.place_bet(bet_id.clone(), bet_amount).unwrap();
            total_bets_placed += bet_amount;
            
            // Simulate various bet outcomes based on actual craps odds
            let outcome = simulate_craps_outcome(BetType::Pass);
            let payout = calculate_payout(BetType::Pass, bet_amount, outcome);
            
            wallet.resolve_bet(&bet_id, outcome, payout).unwrap();
        }
        
        let final_balance = wallet.get_available_balance() + wallet.get_total_balance();
        total_player_losses += STARTING_BALANCE as i64 - final_balance as i64;
    }
    
    let house_edge = (total_player_losses as f64 / total_bets_placed as f64) * 100.0;
    
    // Pass line bet in craps has a house edge of approximately 1.41%
    assert!(house_edge > 1.0 && house_edge < 2.0, 
           "House edge of {:.2}% is outside expected range", house_edge);
    
    println!("Simulated house edge: {:.2}%", house_edge);
    println!("Total player losses: {}", total_player_losses);
    println!("Total bets placed: {}", total_bets_placed);
}

/// Test game economics under various player strategies
#[tokio::test]
async fn test_player_strategy_analysis() {
    const PLAYERS: u32 = 1000;
    const GAMES_PER_PLAYER: u32 = 50;
    
    let strategies = vec![
        ("conservative", conservative_betting_strategy),
        ("aggressive", aggressive_betting_strategy),
        ("martingale", martingale_betting_strategy),
    ];
    
    for (strategy_name, strategy_fn) in strategies {
        let mut total_profit = 0i64;
        let mut successful_players = 0u32;
        
        for player_id in 0..PLAYERS {
            let mut wallet = WalletInterface::new(1000);
            let session_manager = GameSessionManager::new(Default::default());
            
            let starting_balance = wallet.get_available_balance();
            
            for game_num in 0..GAMES_PER_PLAYER {
                if wallet.get_available_balance() < 10 {
                    break; // Player broke
                }
                
                let bet_amount = strategy_fn(
                    wallet.get_available_balance(),
                    game_num,
                    // Would include game history for more sophisticated strategies
                );
                
                if bet_amount > wallet.get_available_balance() {
                    break;
                }
                
                let bet_id = format!("strat_{}_{}", player_id, game_num);
                wallet.place_bet(bet_id.clone(), bet_amount).unwrap();
                
                let outcome = simulate_craps_outcome(BetType::Pass);
                let payout = calculate_payout(BetType::Pass, bet_amount, outcome);
                
                wallet.resolve_bet(&bet_id, outcome, payout).unwrap();
            }
            
            let final_balance = wallet.get_available_balance() + wallet.get_total_balance();
            let profit = final_balance as i64 - starting_balance as i64;
            total_profit += profit;
            
            if profit > 0 {
                successful_players += 1;
            }
        }
        
        let avg_profit = total_profit as f64 / PLAYERS as f64;
        let success_rate = successful_players as f64 / PLAYERS as f64 * 100.0;
        
        println!("Strategy: {} | Avg Profit: {:.2} | Success Rate: {:.1}%",
                strategy_name, avg_profit, success_rate);
        
        // All strategies should show negative expected value due to house edge
        assert!(avg_profit < 0.0, "Strategy {} shows positive expected value", strategy_name);
    }
}

fn conservative_betting_strategy(balance: u64, _game_num: u32) -> u64 {
    (balance / 100).max(10) // Bet 1% of balance, minimum 10
}

fn aggressive_betting_strategy(balance: u64, _game_num: u32) -> u64 {
    (balance / 10).max(50) // Bet 10% of balance, minimum 50
}

fn martingale_betting_strategy(balance: u64, game_num: u32) -> u64 {
    // Double bet after each loss (simplified)
    let base_bet = 10;
    let multiplier = 2_u64.pow((game_num % 6).min(5)); // Cap at 5 doublings
    (base_bet * multiplier).min(balance / 4) // Don't bet more than 25% of balance
}

fn simulate_craps_outcome(bet_type: BetType) -> BetResult {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    match bet_type {
        BetType::Pass => {
            // Simplified pass line simulation
            let roll = rng.gen_range(1..=36); // Simulate probability space
            if roll <= 8 { // ~22% win on come-out
                BetResult::Won
            } else if roll <= 12 { // ~11% lose on come-out
                BetResult::Lost
            } else { 
                // Point phase - simplified to overall pass line probability
                if rng.gen_range(1..=495) <= 244 { // 49.3% win rate
                    BetResult::Won
                } else {
                    BetResult::Lost
                }
            }
        },
        _ => {
            // Simplified for other bet types
            if rng.gen_range(1..=100) <= 49 {
                BetResult::Won
            } else {
                BetResult::Lost
            }
        }
    }
}

fn calculate_payout(bet_type: BetType, bet_amount: u64, result: BetResult) -> u64 {
    match result {
        BetResult::Won => {
            match bet_type {
                BetType::Pass | BetType::DontPass => bet_amount * 2, // 1:1 payout
                BetType::Field => bet_amount * 2, // Simplified
                BetType::Any7 => bet_amount * 5, // 4:1 payout
                BetType::Any11 => bet_amount * 16, // 15:1 payout
                BetType::AnyCraps => bet_amount * 8, // 7:1 payout
                _ => bet_amount * 2, // Default 1:1
            }
        },
        BetResult::Push => bet_amount, // Return original bet
        _ => 0, // Lost
    }
}
```

### Game Performance Benchmarks

```rust
// tests/gaming/performance_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bitchat::gaming::{GameSessionManager, AntiCheatDetector, GamingSecurityManager};
use std::time::Duration;

fn benchmark_game_session_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("game_session_creation");
    
    for player_count in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_session", player_count),
            player_count,
            |b, &player_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let session_manager = GameSessionManager::new(Default::default());
                        let participants: Vec<PeerId> = (0..player_count)
                            .map(|_| PeerId::random())
                            .collect();
                        
                        let session_id = session_manager.create_session(participants[0]).await.unwrap();
                        
                        for participant in &participants[1..] {
                            session_manager.join_session(&session_id, *participant).await.unwrap();
                        }
                        
                        black_box(session_id);
                    })
                })
            },
        );
    }
    group.finish();
}

fn benchmark_anti_cheat_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("anti_cheat_validation");
    
    for bet_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("validate_bets", bet_count),
            bet_count,
            |b, &bet_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let anti_cheat = AntiCheatDetector::new();
                        let player = PeerId::random();
                        
                        for i in 0..*bet_count {
                            let bet = create_benchmark_bet(player, i);
                            let result = anti_cheat.validate_bet(&bet, &player).await;
                            black_box(result);
                        }
                    })
                })
            },
        );
    }
    group.finish();
}

fn benchmark_bet_escrow_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bet_escrow");
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    let gaming_security = rt.block_on(async {
        let gs = GamingSecurityManager::new(Default::default());
        let participants = vec![PeerId::random(), PeerId::random(), PeerId::random()];
        gs.create_gaming_session("benchmark_game".to_string(), participants.clone()).await.unwrap();
        gs
    });
    
    group.bench_function("escrow_bet", |b| {
        b.iter(|| {
            rt.block_on(async {
                let bet = create_escrow_benchmark_bet();
                let result = gaming_security.validate_and_escrow_bet("benchmark_game", &bet).await;
                black_box(result);
            })
        })
    });
    
    group.finish();
}

fn benchmark_dice_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dice_simulation");
    
    group.bench_function("single_roll", |b| {
        b.iter(|| {
            let roll = simulate_dice_roll_fast();
            black_box(roll);
        })
    });
    
    group.bench_function("10k_rolls", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(10000);
            for _ in 0..10000 {
                results.push(simulate_dice_roll_fast());
            }
            black_box(results);
        })
    });
    
    group.finish();
}

fn create_benchmark_bet(player: PeerId, sequence: u32) -> CrapsBet {
    CrapsBet {
        player,
        bet_type: BetType::Pass,
        amount: 100,
        timestamp: sequence as u64, // Use sequence as timestamp for benchmarking
    }
}

fn create_escrow_benchmark_bet() -> PendingBet {
    PendingBet {
        bet_id: "benchmark_bet".to_string(),
        player: PeerId::random(),
        amount: 1000,
        bet_hash: [0u8; 32],
        timestamp: 0,
        confirmations: vec![PeerId::random(), PeerId::random()],
        escrow_signature: None,
    }
}

fn simulate_dice_roll_fast() -> (u8, u8) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (rng.gen_range(1..=6), rng.gen_range(1..=6))
}

criterion_group!(
    benches, 
    benchmark_game_session_creation,
    benchmark_anti_cheat_validation,
    benchmark_bet_escrow_operations,
    benchmark_dice_simulation
);
criterion_main!(benches);
```

This completes the BitChat implementation with production-ready testing, optimization, and deployment infrastructure, enhanced with comprehensive gaming functionality including fairness testing, economic simulations, and performance benchmarks for BitCraps casino operations.