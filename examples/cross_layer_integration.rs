//! Cross-layer integration example showing data flow through all layers
//!
//! Run with: cargo run --example cross_layer_integration

use bitcraps::crypto::{decrypt_message, encrypt_message, Identity};
use bitcraps::error::Result;
use bitcraps::mesh::{MeshPacket, MeshService};
use bitcraps::protocol::{BitchatPacket, PeerId};
use bitcraps::transport::{BluetoothTransport, Transport, TransportPacket};
use std::sync::Arc;
use tokio;
use uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("BitCraps Cross-Layer Integration");
    println!("=================================\n");

    // Show how data flows through each layer of the stack
    demonstrate_layer_flow().await?;

    // Show how layers interact during error conditions
    demonstrate_error_handling().await?;

    // Show optimization techniques across layers
    demonstrate_cross_layer_optimization().await?;

    Ok(())
}

/// Demonstrates data flow from application to network and back
async fn demonstrate_layer_flow() -> Result<()> {
    println!("Data Flow Through Layers");
    println!("------------------------\n");

    let sender = Identity::generate()?;
    let receiver = Identity::generate()?;

    // Application Layer: Create game action
    let game_action = b"PlaceBet:Pass:100";
    println!("1. APPLICATION LAYER");
    println!("   Action: {:?}", String::from_utf8_lossy(game_action));
    println!();

    // Protocol Layer: Wrap in protocol message
    let protocol_msg =
        BitchatPacket::create_message(sender.peer_id(), receiver.peer_id(), game_action.to_vec());
    println!("2. PROTOCOL LAYER");
    println!("   Packet ID: {:?}", protocol_msg.id);
    println!("   From: {:?}", protocol_msg.sender);
    println!("   To: {:?}", protocol_msg.recipient);
    println!();

    // Crypto Layer: Encrypt the message
    let encrypted = encrypt_message(&protocol_msg.payload, &sender, &receiver.public_key)?;
    println!("3. CRYPTO LAYER");
    println!("   Original size: {} bytes", protocol_msg.payload.len());
    println!("   Encrypted size: {} bytes", encrypted.len());
    println!(
        "   Overhead: {} bytes",
        encrypted.len() - protocol_msg.payload.len()
    );
    println!();

    // Compression Layer: Compress if beneficial
    let compressed = if encrypted.len() > 100 {
        let compressed = lz4_flex::compress_prepend_size(&encrypted);
        println!("4. COMPRESSION LAYER");
        println!("   Compressed size: {} bytes", compressed.len());
        println!(
            "   Compression ratio: {:.1}%",
            (compressed.len() as f64 / encrypted.len() as f64) * 100.0
        );
        println!();
        compressed
    } else {
        println!("4. COMPRESSION LAYER");
        println!("   Skipped (message too small)");
        println!();
        encrypted.clone()
    };

    // Mesh Layer: Add routing information
    let mesh_packet = MeshPacket {
        id: uuid::Uuid::new_v4(),
        source: sender.peer_id(),
        destination: receiver.peer_id(),
        ttl: 5,
        payload: compressed,
        route_history: vec![sender.peer_id()],
    };
    println!("5. MESH LAYER");
    println!("   TTL: {}", mesh_packet.ttl);
    println!("   Route: {:?}", mesh_packet.route_history);
    println!();

    // Transport Layer: Prepare for transmission
    let transport_packet = TransportPacket {
        header: vec![0x01, 0x00], // Version and flags
        payload: bincode::serialize(&mesh_packet)?,
    };
    println!("6. TRANSPORT LAYER");
    println!("   Header: {:?}", transport_packet.header);
    println!(
        "   Total size: {} bytes",
        transport_packet.header.len() + transport_packet.payload.len()
    );
    println!();

    // Physical Layer: Bluetooth transmission
    println!("7. PHYSICAL LAYER (Bluetooth)");
    println!("   MTU: 512 bytes");
    println!(
        "   Fragmentation: {}",
        if transport_packet.payload.len() > 512 {
            "Required"
        } else {
            "Not needed"
        }
    );
    println!();

    // Now show the reverse path
    println!("REVERSE PATH (Receiver Processing)");
    println!("-----------------------------------\n");

    // Transport receives
    println!("7→6. TRANSPORT RECEIVES");
    println!("     Reassemble fragments if needed");
    println!();

    // Mesh processes
    println!("6→5. MESH ROUTING");
    println!("     Check if we're destination");
    println!("     Update route history");
    println!("     Decrement TTL");
    println!();

    // Decompress
    println!("5→4. DECOMPRESSION");
    if compressed != encrypted {
        let decompressed = lz4_flex::decompress_size_prepended(&compressed)?;
        println!("     Decompressed to {} bytes", decompressed.len());
    } else {
        println!("     No decompression needed");
    }
    println!();

    // Decrypt
    println!("4→3. DECRYPTION");
    println!("     Verify sender signature");
    println!("     Decrypt with session key");
    println!();

    // Protocol processing
    println!("3→2. PROTOCOL VALIDATION");
    println!("     Check message integrity");
    println!("     Validate sender permissions");
    println!();

    // Application receives
    println!("2→1. APPLICATION PROCESSES");
    println!("     Execute game action");
    println!("     Update game state");
    println!("     Broadcast result to peers");

    Ok(())
}

/// Demonstrates how layers handle errors
async fn demonstrate_error_handling() -> Result<()> {
    println!("\n\nError Handling Across Layers");
    println!("-----------------------------\n");

    println!("Scenario 1: Crypto Validation Failure");
    println!("  → Crypto layer detects invalid signature");
    println!("  → Returns error to mesh layer");
    println!("  → Mesh layer logs and drops packet");
    println!("  → Updates sender reputation (potential attack)");
    println!();

    println!("Scenario 2: Network Partition");
    println!("  → Transport layer loses connectivity");
    println!("  → Mesh layer detects missing heartbeats");
    println!("  → Consensus layer pauses proposals");
    println!("  → Application layer queues actions");
    println!("  → On reconnect: State sync protocol activates");
    println!();

    println!("Scenario 3: Byzantine Node Detection");
    println!("  → Consensus layer detects conflicting votes");
    println!("  → Protocol layer validates evidence");
    println!("  → Mesh layer isolates malicious node");
    println!("  → Transport layer blocks connections");
    println!("  → Crypto layer revokes trust");

    Ok(())
}

/// Demonstrates cross-layer optimizations
async fn demonstrate_cross_layer_optimization() -> Result<()> {
    println!("\n\nCross-Layer Optimizations");
    println!("-------------------------\n");

    println!("1. MTU-Aware Compression");
    println!("   Transport reports MTU to compression layer");
    println!("   Compression targets output < MTU to avoid fragmentation");
    println!();

    println!("2. Topology-Aware Routing");
    println!("   Transport reports link quality to mesh");
    println!("   Mesh adjusts routing preferences");
    println!("   Consensus adjusts timeout based on topology");
    println!();

    println!("3. Battery-Aware Scheduling");
    println!("   Application reports battery level");
    println!("   Transport reduces scan frequency");
    println!("   Mesh increases heartbeat interval");
    println!("   Consensus batches proposals");
    println!();

    println!("4. Security-Performance Tradeoff");
    println!("   High security: Full encryption + signatures");
    println!("   Medium: Encryption only for sensitive data");
    println!("   Low: MAC authentication only");
    println!("   Decision based on threat level and performance needs");

    Ok(())
}

/// Exercise 1: Implement Custom Layer
///
/// Add a new layer between Protocol and Crypto that implements
/// content-based filtering (e.g., profanity filter, rate limiting).
#[allow(dead_code)]
fn exercise_custom_layer() -> Result<()> {
    println!("\n\n=== Exercise: Custom Filtering Layer ===");
    println!("Adding content filtering and rate limiting between Protocol and Crypto layers\n");

    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    // Define the filtering layer trait
    trait FilterLayer {
        fn filter_message(&mut self, sender: PeerId, content: &[u8]) -> FilterResult;
        fn update_peer_stats(&mut self, peer: PeerId, result: FilterResult);
        fn is_peer_rate_limited(&self, peer: PeerId) -> bool;
        fn get_filter_stats(&self) -> FilterStats;
    }

    #[derive(Debug, Clone, PartialEq)]
    enum FilterResult {
        Allow,
        Block(String), // Block with reason
        RateLimit,
        Quarantine, // Temporary isolation
    }

    #[derive(Debug)]
    struct FilterStats {
        total_messages: u64,
        allowed: u64,
        blocked_content: u64,
        blocked_rate_limit: u64,
        quarantined: u64,
    }

    // Peer rate limiting state
    #[derive(Debug, Clone)]
    struct PeerRateState {
        message_count: u32,
        window_start: Instant,
        violations: u32,
        last_violation: Option<Instant>,
    }

    impl PeerRateState {
        fn new() -> Self {
            Self {
                message_count: 0,
                window_start: Instant::now(),
                violations: 0,
                last_violation: None,
            }
        }

        fn reset_window(&mut self) {
            self.message_count = 0;
            self.window_start = Instant::now();
        }

        fn add_violation(&mut self) {
            self.violations += 1;
            self.last_violation = Some(Instant::now());
        }

        fn is_quarantined(&self) -> bool {
            self.violations >= 5
                && self
                    .last_violation
                    .map(|t| t.elapsed() < Duration::from_secs(300))
                    .unwrap_or(false)
        }
    }

    // Implementation of content and rate filtering
    struct ContentRateFilter {
        // Rate limiting: max messages per time window
        max_messages_per_minute: u32,
        window_duration: Duration,
        peer_states: HashMap<PeerId, PeerRateState>,

        // Content filtering
        blocked_words: Vec<String>,
        max_message_size: usize,

        // Statistics
        stats: FilterStats,
    }

    impl ContentRateFilter {
        fn new() -> Self {
            Self {
                max_messages_per_minute: 60, // 1 message per second average
                window_duration: Duration::from_secs(60),
                peer_states: HashMap::new(),

                // Demo content filters
                blocked_words: vec![
                    "spam".to_string(),
                    "scam".to_string(),
                    "malware".to_string(),
                    "exploit".to_string(),
                ],
                max_message_size: 10240, // 10KB max

                stats: FilterStats {
                    total_messages: 0,
                    allowed: 0,
                    blocked_content: 0,
                    blocked_rate_limit: 0,
                    quarantined: 0,
                },
            }
        }

        fn check_content(&self, content: &[u8]) -> FilterResult {
            // Size check
            if content.len() > self.max_message_size {
                return FilterResult::Block("Message too large".to_string());
            }

            // Convert to string for content analysis (if possible)
            if let Ok(text) = String::from_utf8(content.to_vec()) {
                let text_lower = text.to_lowercase();

                // Check for blocked words
                for blocked_word in &self.blocked_words {
                    if text_lower.contains(blocked_word) {
                        return FilterResult::Block(format!(
                            "Contains blocked content: {}",
                            blocked_word
                        ));
                    }
                }

                // Check for suspicious patterns
                if text_lower.len() > 100 && text_lower.matches("http").count() > 3 {
                    return FilterResult::Block("Suspicious link pattern".to_string());
                }

                // Check for repetitive content (potential spam)
                let words: Vec<&str> = text_lower.split_whitespace().collect();
                if words.len() > 10 {
                    let mut word_counts = HashMap::new();
                    for word in words {
                        *word_counts.entry(word).or_insert(0) += 1;
                    }

                    if word_counts
                        .values()
                        .any(|&count| count > word_counts.len() / 3)
                    {
                        return FilterResult::Block("Repetitive content detected".to_string());
                    }
                }
            }

            FilterResult::Allow
        }

        fn check_rate_limit(&mut self, peer: PeerId) -> FilterResult {
            let now = Instant::now();

            let state = self
                .peer_states
                .entry(peer)
                .or_insert_with(PeerRateState::new);

            // Check if peer is quarantined
            if state.is_quarantined() {
                return FilterResult::Quarantine;
            }

            // Reset window if expired
            if now.duration_since(state.window_start) >= self.window_duration {
                state.reset_window();
            }

            state.message_count += 1;

            if state.message_count > self.max_messages_per_minute {
                state.add_violation();
                return FilterResult::RateLimit;
            }

            FilterResult::Allow
        }
    }

    impl FilterLayer for ContentRateFilter {
        fn filter_message(&mut self, sender: PeerId, content: &[u8]) -> FilterResult {
            self.stats.total_messages += 1;

            // First check rate limiting
            let rate_result = self.check_rate_limit(sender);
            if rate_result != FilterResult::Allow {
                match rate_result {
                    FilterResult::RateLimit => self.stats.blocked_rate_limit += 1,
                    FilterResult::Quarantine => self.stats.quarantined += 1,
                    _ => {}
                }
                return rate_result;
            }

            // Then check content
            let content_result = self.check_content(content);
            match content_result {
                FilterResult::Allow => self.stats.allowed += 1,
                FilterResult::Block(_) => self.stats.blocked_content += 1,
                _ => {}
            }

            content_result
        }

        fn update_peer_stats(&mut self, peer: PeerId, result: FilterResult) {
            // Additional peer-specific statistics could be updated here
            if let FilterResult::Block(_) = result {
                if let Some(state) = self.peer_states.get_mut(&peer) {
                    state.add_violation();
                }
            }
        }

        fn is_peer_rate_limited(&self, peer: PeerId) -> bool {
            self.peer_states
                .get(&peer)
                .map(|state| {
                    state.is_quarantined() || state.message_count > self.max_messages_per_minute
                })
                .unwrap_or(false)
        }

        fn get_filter_stats(&self) -> FilterStats {
            FilterStats {
                total_messages: self.stats.total_messages,
                allowed: self.stats.allowed,
                blocked_content: self.stats.blocked_content,
                blocked_rate_limit: self.stats.blocked_rate_limit,
                quarantined: self.stats.quarantined,
            }
        }
    }

    // Integration with the existing protocol stack
    struct ProtocolWithFilter<F: FilterLayer> {
        filter: F,
        identity: Identity,
    }

    impl<F: FilterLayer> ProtocolWithFilter<F> {
        fn new(filter: F, identity: Identity) -> Self {
            Self { filter, identity }
        }

        fn process_incoming_message(
            &mut self,
            sender: PeerId,
            raw_message: Vec<u8>,
        ) -> Result<Option<Vec<u8>>> {
            println!(
                "    Processing incoming message from {:?}",
                hex::encode(&sender[..6])
            );

            // Apply filtering layer
            let filter_result = self.filter.filter_message(sender, &raw_message);

            match filter_result {
                FilterResult::Allow => {
                    println!("      ✓ Message allowed through filter");
                    // Continue with normal protocol processing
                    Ok(Some(raw_message))
                }
                FilterResult::Block(reason) => {
                    println!("      ✗ Message blocked: {}", reason);
                    self.filter.update_peer_stats(sender, filter_result);
                    Ok(None)
                }
                FilterResult::RateLimit => {
                    println!("      ⏱ Message rate limited");
                    self.filter.update_peer_stats(sender, filter_result);
                    Ok(None)
                }
                FilterResult::Quarantine => {
                    println!("      🚫 Message from quarantined peer");
                    self.filter.update_peer_stats(sender, filter_result);
                    Ok(None)
                }
            }
        }

        fn get_filter_effectiveness(&self) -> f64 {
            let stats = self.filter.get_filter_stats();
            if stats.total_messages == 0 {
                return 0.0;
            }

            let blocked = stats.blocked_content + stats.blocked_rate_limit + stats.quarantined;
            (blocked as f64 / stats.total_messages as f64) * 100.0
        }
    }

    // Demonstration
    println!("Creating custom filtering layer...");

    let filter = ContentRateFilter::new();
    let identity = Identity::generate()?;
    let mut protocol_with_filter = ProtocolWithFilter::new(filter, identity);

    println!("✓ Filter layer created with:");
    println!("  • Rate limiting: 60 messages/minute per peer");
    println!("  • Content filtering: blocked words, size limits");
    println!("  • Pattern detection: spam and malicious content");
    println!("  • Quarantine system: temporary isolation for violators");

    // Test messages
    let test_peer1 = PeerId::random();
    let test_peer2 = PeerId::random();
    let test_peer3 = PeerId::random();

    println!("\nTesting filter with various message types:");
    println!("{}", "-".repeat(45));

    // Test 1: Normal messages (should pass)
    println!("\nTest 1: Normal gaming messages");
    let normal_messages = vec![
        b"PlaceBet:Pass:100".to_vec(),
        b"RollDice:4:3".to_vec(),
        b"GameState:Playing".to_vec(),
    ];

    for msg in normal_messages {
        protocol_with_filter.process_incoming_message(test_peer1, msg)?;
    }

    // Test 2: Blocked content (should be filtered)
    println!("\nTest 2: Blocked content");
    let blocked_messages = vec![
        b"This is spam content for testing".to_vec(),
        b"Check out this exploit technique".to_vec(),
        b"Download malware from this link".to_vec(),
    ];

    for msg in blocked_messages {
        protocol_with_filter.process_incoming_message(test_peer2, msg)?;
    }

    // Test 3: Rate limiting (simulate rapid messages)
    println!("\nTest 3: Rate limiting simulation");
    for i in 0..65 {
        // Exceed the 60/minute limit
        let msg = format!("RapidMessage:{}", i).into_bytes();
        protocol_with_filter.process_incoming_message(test_peer3, msg)?;

        if i == 60 {
            println!("      (Rate limit threshold reached...)");
        }
    }

    // Test 4: Large message filtering
    println!("\nTest 4: Oversized message");
    let large_msg = vec![b'A'; 20000]; // 20KB message (exceeds 10KB limit)
    protocol_with_filter.process_incoming_message(test_peer1, large_msg)?;

    // Test 5: Repetitive content detection
    println!("\nTest 5: Repetitive/spam content");
    let spam_msg = "buy buy buy now now now cheap cheap cheap deal deal deal"
        .repeat(10)
        .into_bytes();
    protocol_with_filter.process_incoming_message(test_peer2, spam_msg)?;

    // Display filter statistics
    println!("\n=== Filter Performance Report ===");
    let stats = protocol_with_filter.filter.get_filter_stats();
    let effectiveness = protocol_with_filter.get_filter_effectiveness();

    println!("Total messages processed: {}", stats.total_messages);
    println!(
        "Messages allowed: {} ({:.1}%)",
        stats.allowed,
        (stats.allowed as f64 / stats.total_messages as f64) * 100.0
    );
    println!(
        "Blocked (content): {} ({:.1}%)",
        stats.blocked_content,
        (stats.blocked_content as f64 / stats.total_messages as f64) * 100.0
    );
    println!(
        "Blocked (rate limit): {} ({:.1}%)",
        stats.blocked_rate_limit,
        (stats.blocked_rate_limit as f64 / stats.total_messages as f64) * 100.0
    );
    println!(
        "Quarantined: {} ({:.1}%)",
        stats.quarantined,
        (stats.quarantined as f64 / stats.total_messages as f64) * 100.0
    );
    println!("\nOverall filter effectiveness: {:.1}%", effectiveness);

    // Integration recommendations
    println!("\n=== Integration with Protocol Stack ===");
    println!("Recommended integration points:");
    println!("  1. Between Transport and Mesh layers (network-level filtering)");
    println!("  2. Between Mesh and Protocol layers (application-level filtering)");
    println!("  3. Between Protocol and Crypto layers (content validation)");

    println!("\nPerformance considerations:");
    println!("  • Add async processing for content analysis");
    println!("  • Use bloom filters for large blocked word lists");
    println!("  • Implement LRU cache for peer rate states");
    println!("  • Add machine learning for adaptive spam detection");

    println!("\nSecurity benefits:");
    println!("  ✓ DoS protection via rate limiting");
    println!("  ✓ Content-based attack prevention");
    println!("  ✓ Malicious peer identification and isolation");
    println!("  ✓ Network resource conservation");

    println!("\n✓ Custom filtering layer exercise complete!\n");
    Ok(())
}

/// Exercise 2: Performance Profiling
///
/// Profile the overhead of each layer and identify
/// optimization opportunities.
#[allow(dead_code)]
async fn exercise_performance_profiling() -> Result<()> {
    println!("\n\n=== Exercise: Performance Profiling ===");
    println!("Profiling overhead and timing of each protocol stack layer\n");

    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    // Performance measurement structure
    #[derive(Debug, Clone)]
    struct LayerTiming {
        total_time: Duration,
        sample_count: u32,
        min_time: Duration,
        max_time: Duration,
    }

    impl LayerTiming {
        fn new() -> Self {
            Self {
                total_time: Duration::ZERO,
                sample_count: 0,
                min_time: Duration::from_secs(999),
                max_time: Duration::ZERO,
            }
        }

        fn add_sample(&mut self, duration: Duration) {
            self.total_time += duration;
            self.sample_count += 1;
            if duration < self.min_time {
                self.min_time = duration;
            }
            if duration > self.max_time {
                self.max_time = duration;
            }
        }

        fn average_time(&self) -> Duration {
            if self.sample_count > 0 {
                self.total_time / self.sample_count
            } else {
                Duration::ZERO
            }
        }

        fn throughput_per_sec(&self) -> f64 {
            if self.total_time.as_secs_f64() > 0.0 {
                self.sample_count as f64 / self.total_time.as_secs_f64()
            } else {
                0.0
            }
        }
    }

    // Profiler for the protocol stack
    struct ProtocolStackProfiler {
        layer_timings: HashMap<String, LayerTiming>,
        message_size_samples: Vec<(usize, Duration)>, // (size, total_time)
    }

    impl ProtocolStackProfiler {
        fn new() -> Self {
            Self {
                layer_timings: HashMap::new(),
                message_size_samples: Vec::new(),
            }
        }

        fn start_layer_timing(&self, layer_name: &str) -> (String, Instant) {
            (layer_name.to_string(), Instant::now())
        }

        fn end_layer_timing(&mut self, layer_name: String, start_time: Instant) {
            let duration = start_time.elapsed();
            self.layer_timings
                .entry(layer_name)
                .or_insert_with(LayerTiming::new)
                .add_sample(duration);
        }

        fn profile_complete_message(&mut self, message_size: usize, total_time: Duration) {
            self.message_size_samples.push((message_size, total_time));
        }

        fn generate_report(&self) {
            println!("=== Performance Profile Report ===");
            println!("{}", "-".repeat(50));

            // Layer performance summary
            println!("Layer Performance (per message):");
            let mut layers: Vec<_> = self.layer_timings.iter().collect();
            layers.sort_by_key(|(_, timing)| timing.average_time());

            for (layer_name, timing) in layers {
                println!(
                    "  {:<20}: avg={:>6.2}μs, min={:>6.2}μs, max={:>6.2}μs, samples={}",
                    layer_name,
                    timing.average_time().as_micros(),
                    timing.min_time.as_micros(),
                    timing.max_time.as_micros(),
                    timing.sample_count
                );
            }

            // Calculate total stack overhead
            let total_avg_time: Duration = self
                .layer_timings
                .values()
                .map(|timing| timing.average_time())
                .sum();

            println!("\nStack Performance Summary:");
            println!(
                "  Total average latency: {:.2}μs",
                total_avg_time.as_micros()
            );
            println!(
                "  Theoretical max throughput: {:.0} messages/sec",
                1_000_000.0 / total_avg_time.as_micros() as f64
            );

            // Message size analysis
            if !self.message_size_samples.is_empty() {
                println!("\nMessage Size Impact Analysis:");

                // Group by size ranges
                let mut size_buckets: HashMap<String, Vec<Duration>> = HashMap::new();
                for (size, duration) in &self.message_size_samples {
                    let bucket = match size {
                        0..=100 => "Small (≤100B)",
                        101..=1000 => "Medium (101B-1KB)",
                        1001..=10000 => "Large (1KB-10KB)",
                        _ => "Extra Large (>10KB)",
                    };
                    size_buckets
                        .entry(bucket.to_string())
                        .or_insert_with(Vec::new)
                        .push(*duration);
                }

                for (bucket, durations) in size_buckets {
                    let avg_duration: Duration =
                        durations.iter().sum::<Duration>() / durations.len() as u32;
                    let throughput =
                        durations.len() as f64 / durations.iter().sum::<Duration>().as_secs_f64();
                    println!(
                        "  {:<20}: avg={:>6.2}μs, throughput={:>6.1}/sec, samples={}",
                        bucket,
                        avg_duration.as_micros(),
                        throughput,
                        durations.len()
                    );
                }
            }

            // Bottleneck identification
            println!("\nBottleneck Analysis:");
            let slowest_layer = self
                .layer_timings
                .iter()
                .max_by_key(|(_, timing)| timing.average_time())
                .map(|(name, timing)| (name.clone(), timing.average_time()));

            if let Some((layer_name, duration)) = slowest_layer {
                let percentage =
                    (duration.as_micros() as f64 / total_avg_time.as_micros() as f64) * 100.0;
                println!(
                    "  Primary bottleneck: {} ({:.1}% of total time)",
                    layer_name, percentage
                );

                if percentage > 50.0 {
                    println!("    ⚠ Significant bottleneck - optimization recommended");
                } else if percentage > 30.0 {
                    println!("    ⚡ Moderate bottleneck - consider optimization");
                } else {
                    println!("    ✓ Balanced performance across layers");
                }
            }
        }
    }

    // Simulate each layer's processing with timing
    async fn simulate_layer_processing(
        profiler: &mut ProtocolStackProfiler,
        layer_name: &str,
        message_data: &[u8],
        processing_complexity: u32,
    ) -> Vec<u8> {
        let (name, start_time) = profiler.start_layer_timing(layer_name);

        // Simulate processing work based on complexity and message size
        let work_units = processing_complexity + (message_data.len() / 100) as u32;

        // Simulate different types of work
        let result = match layer_name {
            "Application" => {
                // Light processing - mostly data preparation
                for _ in 0..work_units {
                    std::hint::black_box(message_data.len() * 2);
                }
                message_data.to_vec()
            }
            "Protocol" => {
                // Medium processing - serialization and validation
                let mut result = message_data.to_vec();
                for _ in 0..work_units * 2 {
                    result = bincode::serialize(&result).unwrap_or_else(|_| result);
                    if result.len() > message_data.len() * 2 {
                        result = message_data.to_vec();
                    }
                }
                result
            }
            "Crypto" => {
                // Heavy processing - encryption operations
                let mut result = message_data.to_vec();
                for i in 0..work_units * 5 {
                    // Simulate crypto operations with XOR (not secure, just for timing)
                    for byte in &mut result {
                        *byte ^= (i as u8).wrapping_add(0x5A);
                    }
                }
                result
            }
            "Compression" => {
                // Variable processing - depends on data compressibility
                if message_data.len() > 100 {
                    // Simulate compression
                    let compressed = lz4_flex::compress_prepend_size(message_data);
                    // Add some CPU work to simulate compression algorithm
                    for _ in 0..work_units * 3 {
                        std::hint::black_box(compressed.len());
                    }
                    compressed
                } else {
                    message_data.to_vec()
                }
            }
            "Mesh" => {
                // Medium processing - routing table lookups and packet wrapping
                let mut result = message_data.to_vec();
                result.extend_from_slice(&[0u8; 32]); // Add routing header
                for _ in 0..work_units * 2 {
                    std::hint::black_box(result.len());
                }
                result
            }
            "Transport" => {
                // Light processing - simple framing
                let mut result = Vec::with_capacity(message_data.len() + 8);
                result.extend_from_slice(&(message_data.len() as u32).to_be_bytes());
                result.extend_from_slice(&[0u8; 4]); // Flags
                result.extend_from_slice(message_data);
                for _ in 0..work_units {
                    std::hint::black_box(result.len());
                }
                result
            }
            _ => message_data.to_vec(),
        };

        profiler.end_layer_timing(name, start_time);
        result
    }

    // Test with different message sizes
    let test_sizes = vec![50, 200, 1000, 5000, 15000]; // bytes
    let samples_per_size = 20;

    println!("Starting performance profiling...");
    println!(
        "Test configuration: {} message sizes, {} samples each",
        test_sizes.len(),
        samples_per_size
    );

    let mut profiler = ProtocolStackProfiler::new();

    for &size in &test_sizes {
        println!("\nTesting with {}B messages:", size);

        for sample in 0..samples_per_size {
            // Create test message
            let test_message = vec![((sample + size) % 256) as u8; size];
            let message_start = Instant::now();

            print!("  Sample {}: ", sample + 1);

            // Process through each layer in order
            let mut current_data = test_message.clone();

            // 1. Application Layer
            current_data =
                simulate_layer_processing(&mut profiler, "Application", &current_data, 1).await;
            print!("A ");

            // 2. Protocol Layer
            current_data =
                simulate_layer_processing(&mut profiler, "Protocol", &current_data, 3).await;
            print!("P ");

            // 3. Crypto Layer
            current_data =
                simulate_layer_processing(&mut profiler, "Crypto", &current_data, 8).await;
            print!("C ");

            // 4. Compression Layer
            current_data =
                simulate_layer_processing(&mut profiler, "Compression", &current_data, 5).await;
            print!("Z ");

            // 5. Mesh Layer
            current_data = simulate_layer_processing(&mut profiler, "Mesh", &current_data, 4).await;
            print!("M ");

            // 6. Transport Layer
            current_data =
                simulate_layer_processing(&mut profiler, "Transport", &current_data, 2).await;
            print!("T ");

            let total_time = message_start.elapsed();
            profiler.profile_complete_message(size, total_time);

            println!("({:.1}μs)", total_time.as_micros());

            // Small delay between samples to avoid overwhelming the system
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    }

    println!("\n{}", "=".repeat(60));
    profiler.generate_report();

    // Optimization recommendations
    println!("\n=== Optimization Recommendations ===");

    let crypto_timing = profiler.layer_timings.get("Crypto");
    let total_avg: Duration = profiler
        .layer_timings
        .values()
        .map(|t| t.average_time())
        .sum();

    if let Some(crypto) = crypto_timing {
        let crypto_percentage =
            (crypto.average_time().as_micros() as f64 / total_avg.as_micros() as f64) * 100.0;
        if crypto_percentage > 40.0 {
            println!(
                "1. Crypto Optimization ({}% of processing time):",
                crypto_percentage as u32
            );
            println!("   • Implement hardware acceleration (AES-NI, ARM crypto extensions)");
            println!("   • Use SIMD instructions for bulk operations");
            println!("   • Consider cipher suite optimization");
            println!("   • Implement session key caching");
        }
    }

    let compression_timing = profiler.layer_timings.get("Compression");
    if let Some(compression) = compression_timing {
        let comp_percentage =
            (compression.average_time().as_micros() as f64 / total_avg.as_micros() as f64) * 100.0;
        if comp_percentage > 25.0 {
            println!(
                "2. Compression Optimization ({}% of processing time):",
                comp_percentage as u32
            );
            println!("   • Implement adaptive compression based on content type");
            println!("   • Use faster algorithms (LZ4, Zstandard) for real-time data");
            println!("   • Pre-compress static content");
            println!("   • Skip compression for small messages");
        }
    }

    println!("3. General Optimizations:");
    println!("   • Implement zero-copy operations where possible");
    println!("   • Use memory pools for frequent allocations");
    println!("   • Pipeline processing across layers");
    println!("   • Batch small messages for better throughput");

    let max_throughput = profiler
        .layer_timings
        .values()
        .map(|t| t.throughput_per_sec())
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    println!("\n=== Performance Summary ===");
    println!(
        "Current theoretical max throughput: {:.0} messages/sec",
        max_throughput
    );
    println!("Average end-to-end latency: {:.2}μs", total_avg.as_micros());

    if max_throughput > 10000.0 {
        println!("✓ Excellent performance for gaming applications");
    } else if max_throughput > 1000.0 {
        println!("⚡ Good performance - some optimization opportunities");
    } else {
        println!("⚠ Performance optimization recommended for production");
    }

    println!("\n✓ Performance profiling exercise complete!\n");
    Ok(())
}

/// Exercise 3: Failure Injection
///
/// Inject failures at each layer and verify the system
/// handles them gracefully.
#[allow(dead_code)]
async fn exercise_failure_injection() -> Result<()> {
    println!("\n\n=== Exercise: Failure Injection Testing ===");
    println!("Testing system resilience by injecting failures at each layer\n");

    use rand::Rng;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    // Failure injection controller
    struct FailureInjector {
        transport_drop_rate: Arc<AtomicU32>,      // Percentage (0-100)
        crypto_corruption_rate: Arc<AtomicU32>,   // Percentage (0-100)
        mesh_routing_errors: Arc<AtomicU32>,      // Percentage (0-100)
        consensus_byzantine_rate: Arc<AtomicU32>, // Percentage (0-100)
        enabled: Arc<AtomicBool>,

        // Statistics
        injected_failures: Arc<AtomicU32>,
        recovered_failures: Arc<AtomicU32>,
    }

    impl FailureInjector {
        fn new() -> Self {
            Self {
                transport_drop_rate: Arc::new(AtomicU32::new(0)),
                crypto_corruption_rate: Arc::new(AtomicU32::new(0)),
                mesh_routing_errors: Arc::new(AtomicU32::new(0)),
                consensus_byzantine_rate: Arc::new(AtomicU32::new(0)),
                enabled: Arc::new(AtomicBool::new(true)),
                injected_failures: Arc::new(AtomicU32::new(0)),
                recovered_failures: Arc::new(AtomicU32::new(0)),
            }
        }

        fn set_transport_drop_rate(&self, rate: u32) {
            self.transport_drop_rate
                .store(rate.min(100), Ordering::Relaxed);
        }

        fn set_crypto_corruption_rate(&self, rate: u32) {
            self.crypto_corruption_rate
                .store(rate.min(100), Ordering::Relaxed);
        }

        fn set_mesh_routing_errors(&self, rate: u32) {
            self.mesh_routing_errors
                .store(rate.min(100), Ordering::Relaxed);
        }

        fn set_consensus_byzantine_rate(&self, rate: u32) {
            self.consensus_byzantine_rate
                .store(rate.min(100), Ordering::Relaxed);
        }

        fn should_inject_transport_failure(&self) -> bool {
            if !self.enabled.load(Ordering::Relaxed) {
                return false;
            }
            let rate = self.transport_drop_rate.load(Ordering::Relaxed);
            let should_fail = rand::thread_rng().gen_range(0..100) < rate;
            if should_fail {
                self.injected_failures.fetch_add(1, Ordering::Relaxed);
            }
            should_fail
        }

        fn should_inject_crypto_failure(&self) -> bool {
            if !self.enabled.load(Ordering::Relaxed) {
                return false;
            }
            let rate = self.crypto_corruption_rate.load(Ordering::Relaxed);
            let should_fail = rand::thread_rng().gen_range(0..100) < rate;
            if should_fail {
                self.injected_failures.fetch_add(1, Ordering::Relaxed);
            }
            should_fail
        }

        fn should_inject_mesh_failure(&self) -> bool {
            if !self.enabled.load(Ordering::Relaxed) {
                return false;
            }
            let rate = self.mesh_routing_errors.load(Ordering::Relaxed);
            let should_fail = rand::thread_rng().gen_range(0..100) < rate;
            if should_fail {
                self.injected_failures.fetch_add(1, Ordering::Relaxed);
            }
            should_fail
        }

        fn should_inject_consensus_failure(&self) -> bool {
            if !self.enabled.load(Ordering::Relaxed) {
                return false;
            }
            let rate = self.consensus_byzantine_rate.load(Ordering::Relaxed);
            let should_fail = rand::thread_rng().gen_range(0..100) < rate;
            if should_fail {
                self.injected_failures.fetch_add(1, Ordering::Relaxed);
            }
            should_fail
        }

        fn record_recovery(&self) {
            self.recovered_failures.fetch_add(1, Ordering::Relaxed);
        }

        fn get_stats(&self) -> (u32, u32, f64) {
            let injected = self.injected_failures.load(Ordering::Relaxed);
            let recovered = self.recovered_failures.load(Ordering::Relaxed);
            let recovery_rate = if injected > 0 {
                (recovered as f64 / injected as f64) * 100.0
            } else {
                0.0
            };
            (injected, recovered, recovery_rate)
        }
    }

    // Resilient message processor that handles failures
    struct ResilientMessageProcessor {
        injector: Arc<FailureInjector>,
        retry_attempts: u32,
        retry_delay: Duration,

        // Success/failure tracking
        successful_messages: Arc<AtomicU32>,
        failed_messages: Arc<AtomicU32>,
        retried_messages: Arc<AtomicU32>,
    }

    impl ResilientMessageProcessor {
        fn new(injector: Arc<FailureInjector>) -> Self {
            Self {
                injector,
                retry_attempts: 3,
                retry_delay: Duration::from_millis(100),
                successful_messages: Arc::new(AtomicU32::new(0)),
                failed_messages: Arc::new(AtomicU32::new(0)),
                retried_messages: Arc::new(AtomicU32::new(0)),
            }
        }

        async fn process_message_with_resilience(
            &self,
            message_id: u32,
            message_data: Vec<u8>,
        ) -> Result<bool> {
            let mut attempts = 0;

            while attempts <= self.retry_attempts {
                match self.try_process_message(message_id, &message_data).await {
                    Ok(true) => {
                        self.successful_messages.fetch_add(1, Ordering::Relaxed);
                        if attempts > 0 {
                            self.injector.record_recovery();
                            self.retried_messages.fetch_add(1, Ordering::Relaxed);
                            println!(
                                "    ✓ Message {} succeeded after {} retries",
                                message_id, attempts
                            );
                        }
                        return Ok(true);
                    }
                    Ok(false) => {
                        attempts += 1;
                        if attempts <= self.retry_attempts {
                            println!(
                                "    ⚠ Message {} failed, retrying... (attempt {})",
                                message_id, attempts
                            );
                            sleep(self.retry_delay * attempts).await; // Exponential backoff
                        }
                    }
                    Err(e) => {
                        println!("    ✗ Message {} fatal error: {}", message_id, e);
                        self.failed_messages.fetch_add(1, Ordering::Relaxed);
                        return Err(e);
                    }
                }
            }

            self.failed_messages.fetch_add(1, Ordering::Relaxed);
            println!(
                "    ✗ Message {} failed permanently after {} attempts",
                message_id, attempts
            );
            Ok(false)
        }

        async fn try_process_message(&self, message_id: u32, data: &[u8]) -> Result<bool> {
            // Transport Layer - simulate packet drops
            if self.injector.should_inject_transport_failure() {
                println!("      🔌 Transport layer dropped message {}", message_id);
                return Ok(false);
            }

            // Mesh Layer - simulate routing failures
            if self.injector.should_inject_mesh_failure() {
                println!(
                    "      🕸 Mesh layer routing error for message {}",
                    message_id
                );
                return Ok(false);
            }

            // Crypto Layer - simulate signature corruption
            if self.injector.should_inject_crypto_failure() {
                println!(
                    "      🔐 Crypto layer signature corruption for message {}",
                    message_id
                );
                return Ok(false);
            }

            // Consensus Layer - simulate byzantine behavior
            if self.injector.should_inject_consensus_failure() {
                println!(
                    "      ⚖ Consensus layer byzantine behavior for message {}",
                    message_id
                );
                return Ok(false);
            }

            // If we get here, message processed successfully
            Ok(true)
        }

        fn get_processing_stats(&self) -> (u32, u32, u32, f64, f64) {
            let successful = self.successful_messages.load(Ordering::Relaxed);
            let failed = self.failed_messages.load(Ordering::Relaxed);
            let retried = self.retried_messages.load(Ordering::Relaxed);
            let total = successful + failed;

            let success_rate = if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let retry_rate = if successful > 0 {
                (retried as f64 / successful as f64) * 100.0
            } else {
                0.0
            };

            (successful, failed, retried, success_rate, retry_rate)
        }
    }

    // Test scenarios with increasing failure rates
    let injector = Arc::new(FailureInjector::new());
    let processor = ResilientMessageProcessor::new(injector.clone());

    println!("Testing system resilience with progressive failure injection...");

    // Scenario 1: Baseline (no failures)
    println!("\n📊 Scenario 1: Baseline (no failures)");
    println!("{}", "-".repeat(40));

    for i in 0..10 {
        let message = format!("baseline_message_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;
    }

    let (successful, failed, retried, success_rate, retry_rate) = processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        successful, failed, success_rate, retry_rate
    );

    // Scenario 2: Light transport failures (10%)
    println!("\n📊 Scenario 2: Light transport failures (10%)");
    println!("{}", "-".repeat(40));

    injector.set_transport_drop_rate(10);

    for i in 10..25 {
        let message = format!("transport_test_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;
    }

    let (successful, failed, retried, success_rate, retry_rate) = processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        successful, failed, success_rate, retry_rate
    );

    // Scenario 3: Moderate crypto failures (20%)
    println!("\n📊 Scenario 3: Moderate crypto failures (20%)");
    println!("{}", "-".repeat(40));

    injector.set_crypto_corruption_rate(20);

    for i in 25..40 {
        let message = format!("crypto_test_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;
    }

    let (successful, failed, retried, success_rate, retry_rate) = processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        successful, failed, success_rate, retry_rate
    );

    // Scenario 4: High mesh routing errors (30%)
    println!("\n📊 Scenario 4: High mesh routing errors (30%)");
    println!("{}", "-".repeat(40));

    injector.set_mesh_routing_errors(30);

    for i in 40..55 {
        let message = format!("mesh_test_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;
    }

    let (successful, failed, retried, success_rate, retry_rate) = processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        successful, failed, success_rate, retry_rate
    );

    // Scenario 5: Byzantine consensus failures (15%)
    println!("\n📊 Scenario 5: Byzantine consensus failures (15%)");
    println!("{}", "-".repeat(40));

    injector.set_consensus_byzantine_rate(15);

    for i in 55..70 {
        let message = format!("consensus_test_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;
    }

    let (successful, failed, retried, success_rate, retry_rate) = processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        successful, failed, success_rate, retry_rate
    );

    // Scenario 6: Extreme stress test (all failures at high rates)
    println!("\n📊 Scenario 6: Extreme stress test (all failures active)");
    println!("{}", "-".repeat(40));

    injector.set_transport_drop_rate(25);
    injector.set_crypto_corruption_rate(20);
    injector.set_mesh_routing_errors(30);
    injector.set_consensus_byzantine_rate(25);

    println!("Active failures: 25% transport, 20% crypto, 30% mesh, 25% consensus");

    for i in 70..90 {
        let message = format!("stress_test_{}", i).into_bytes();
        processor
            .process_message_with_resilience(i, message)
            .await?;

        // Add small delay to make output readable
        if i % 5 == 0 {
            sleep(Duration::from_millis(50)).await;
        }
    }

    let (final_successful, final_failed, final_retried, final_success_rate, final_retry_rate) =
        processor.get_processing_stats();
    println!(
        "Results: {}✓ {}✗ ({:.1}% success, {:.1}% required retries)",
        final_successful, final_failed, final_success_rate, final_retry_rate
    );

    // Final analysis
    println!("\n{}", "=".repeat(60));
    println!("=== Failure Injection Test Results ===");

    let (injected_failures, recovered_failures, recovery_rate) = injector.get_stats();

    println!("\nFailure Injection Summary:");
    println!("  Total failures injected: {}", injected_failures);
    println!("  Failures recovered from: {}", recovered_failures);
    println!("  Recovery success rate: {:.1}%", recovery_rate);

    println!("\nMessage Processing Summary:");
    println!("  Total messages sent: {}", final_successful + final_failed);
    println!(
        "  Successfully processed: {} ({:.1}%)",
        final_successful, final_success_rate
    );
    println!(
        "  Permanently failed: {} ({:.1}%)",
        final_failed,
        100.0 - final_success_rate
    );
    println!(
        "  Required retries: {} ({:.1}% of successful)",
        final_retried, final_retry_rate
    );

    // System resilience assessment
    println!("\n=== System Resilience Assessment ===");

    if final_success_rate >= 90.0 {
        println!("✅ EXCELLENT: System maintains high success rate under extreme conditions");
    } else if final_success_rate >= 75.0 {
        println!("✅ GOOD: System shows strong resilience to failures");
    } else if final_success_rate >= 60.0 {
        println!("⚠️ MODERATE: System functions but may need resilience improvements");
    } else {
        println!("❌ POOR: System struggling under failure conditions");
    }

    if recovery_rate >= 80.0 {
        println!("✅ EXCELLENT: Recovery mechanisms working effectively");
    } else if recovery_rate >= 60.0 {
        println!("⚠️ MODERATE: Recovery mechanisms need improvement");
    } else {
        println!("❌ POOR: Recovery mechanisms ineffective");
    }

    println!("\nKey Resilience Features Demonstrated:");
    println!("  ✓ Automatic retry with exponential backoff");
    println!("  ✓ Layer-specific failure detection and handling");
    println!("  ✓ Graceful degradation under high failure rates");
    println!("  ✓ Recovery tracking and metrics");

    println!("\nRecommended Improvements:");
    if recovery_rate < 70.0 {
        println!("  • Implement circuit breakers for failing components");
        println!("  • Add alternative routing for mesh failures");
        println!("  • Implement redundant crypto validation");
    }
    if final_success_rate < 80.0 {
        println!("  • Increase retry attempts for critical messages");
        println!("  • Implement message priority queuing");
        println!("  • Add failure prediction based on patterns");
    }
    println!("  • Monitor failure rates in production");
    println!("  • Implement automated failure injection in testing");

    println!("\n✓ Failure injection testing exercise complete!\n");
    Ok(())
}
#![cfg(feature = "legacy-examples")]
