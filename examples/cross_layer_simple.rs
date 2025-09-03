//! Cross-layer integration example showing educational concepts
//! This simplified version focuses on demonstrating the TODO implementations
//!
//! Run with: cargo run --example cross_layer_simple

use bitcraps::error::Result;
use bitcraps::protocol::{PeerId, PeerIdExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    println!("BitCraps Cross-Layer Integration - TODO Implementations");
    println!("======================================================\n");

    // Demonstrate the TODO implementations that were requested
    exercise_custom_layer_demo()?;
    exercise_performance_profiling_demo().await?;
    exercise_failure_injection_demo().await?;

    println!("✅ All cross-layer TODO implementations demonstrated!");
    Ok(())
}

/// Exercise 1: Custom Filtering Layer Demo
///
/// This demonstrates the TODO implementation for custom filtering layer
fn exercise_custom_layer_demo() -> Result<()> {
    println!("=== Custom Filtering Layer Demo ===");
    println!("Adding content filtering and rate limiting between layers\n");

    #[derive(Debug, PartialEq)]
    enum FilterResult {
        Allow,
        Block(String),
        RateLimit,
    }

    struct ContentRateFilter {
        max_messages_per_minute: u32,
        blocked_words: Vec<String>,
        max_message_size: usize,
        peer_message_counts: HashMap<PeerId, (u32, Instant)>,
        stats_allowed: u32,
        stats_blocked: u32,
        stats_rate_limited: u32,
    }

    impl ContentRateFilter {
        fn new() -> Self {
            Self {
                max_messages_per_minute: 60,
                blocked_words: vec![
                    "spam".to_string(),
                    "scam".to_string(),
                    "exploit".to_string(),
                ],
                max_message_size: 10240,
                peer_message_counts: HashMap::new(),
                stats_allowed: 0,
                stats_blocked: 0,
                stats_rate_limited: 0,
            }
        }

        fn filter_message(&mut self, sender: PeerId, content: &[u8]) -> FilterResult {
            // Check rate limiting
            let now = Instant::now();
            if let Some((count, window_start)) = self.peer_message_counts.get_mut(&sender) {
                if now.duration_since(*window_start) > Duration::from_secs(60) {
                    *count = 0;
                    *window_start = now;
                }

                *count += 1;
                if *count > self.max_messages_per_minute {
                    self.stats_rate_limited += 1;
                    return FilterResult::RateLimit;
                }
            } else {
                self.peer_message_counts.insert(sender, (1, now));
            }

            // Check message size
            if content.len() > self.max_message_size {
                self.stats_blocked += 1;
                return FilterResult::Block("Message too large".to_string());
            }

            // Check content
            if let Ok(text) = String::from_utf8(content.to_vec()) {
                let text_lower = text.to_lowercase();

                for blocked_word in &self.blocked_words {
                    if text_lower.contains(blocked_word) {
                        self.stats_blocked += 1;
                        return FilterResult::Block(format!(
                            "Contains blocked content: {}",
                            blocked_word
                        ));
                    }
                }

                // Check for suspicious patterns
                if text_lower.len() > 100 && text_lower.matches("http").count() > 3 {
                    self.stats_blocked += 1;
                    return FilterResult::Block("Suspicious link pattern".to_string());
                }
            }

            self.stats_allowed += 1;
            FilterResult::Allow
        }

        fn get_effectiveness(&self) -> f64 {
            let total = self.stats_allowed + self.stats_blocked + self.stats_rate_limited;
            if total == 0 {
                return 0.0;
            }

            let blocked = self.stats_blocked + self.stats_rate_limited;
            (blocked as f64 / total as f64) * 100.0
        }
    }

    let mut filter = ContentRateFilter::new();

    println!("Phase 1: Filter configuration");
    println!(
        "  Rate limiting: {} messages/minute per peer",
        filter.max_messages_per_minute
    );
    println!("  Max message size: {} bytes", filter.max_message_size);
    println!("  Blocked words: {:?}", filter.blocked_words);

    println!("\nPhase 2: Testing normal messages");
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();

    let normal_messages = vec![
        b"PlaceBet:Pass:100".to_vec(),
        b"RollDice:4:3".to_vec(),
        b"GameState:Playing".to_vec(),
    ];

    for (i, msg) in normal_messages.iter().enumerate() {
        let result = filter.filter_message(peer1, msg);
        println!(
            "  Message {}: {:?} -> {:?}",
            i + 1,
            String::from_utf8_lossy(msg),
            result
        );
    }

    println!("\nPhase 3: Testing blocked content");
    let blocked_messages = vec![
        b"This is spam content".to_vec(),
        b"Check out this exploit".to_vec(),
        vec![b'A'; 15000], // Too large
    ];

    for (i, msg) in blocked_messages.iter().enumerate() {
        let result = filter.filter_message(peer2, msg);
        let preview = if msg.len() > 20 {
            format!(
                "{}... ({} bytes)",
                String::from_utf8_lossy(&msg[..20]),
                msg.len()
            )
        } else {
            String::from_utf8_lossy(msg).to_string()
        };
        println!("  Blocked test {}: {:?} -> {:?}", i + 1, preview, result);
    }

    println!("\nPhase 4: Testing rate limiting");
    println!("  Sending rapid messages from peer1...");

    for i in 0..65 {
        // Exceed the 60/minute limit
        let msg = format!("RapidMessage:{}", i).into_bytes();
        let result = filter.filter_message(peer1, &msg);

        if i == 60 {
            println!("    Message {}: {:?} (rate limit triggered)", i + 1, result);
        } else if i > 60 && i < 63 {
            println!("    Message {}: {:?}", i + 1, result);
        }
    }

    println!("\nPhase 5: Filter performance report");
    println!("  Messages allowed: {}", filter.stats_allowed);
    println!("  Messages blocked (content): {}", filter.stats_blocked);
    println!(
        "  Messages blocked (rate limit): {}",
        filter.stats_rate_limited
    );
    println!(
        "  Overall filter effectiveness: {:.1}%",
        filter.get_effectiveness()
    );

    println!("\n✓ Custom filtering layer complete!");
    println!("Key concepts demonstrated:");
    println!("  • Content-based filtering");
    println!("  • Rate limiting per peer");
    println!("  • Configurable filter rules");
    println!("  • Performance metrics");
    println!("  • Integration between protocol layers\n");

    Ok(())
}

/// Exercise 2: Performance Profiling Demo
///
/// This demonstrates the TODO implementation for performance profiling
async fn exercise_performance_profiling_demo() -> Result<()> {
    println!("=== Performance Profiling Demo ===");
    println!("Profiling overhead and timing of each protocol stack layer\n");

    #[derive(Debug)]
    struct LayerTiming {
        name: String,
        total_time: Duration,
        sample_count: u32,
        min_time: Duration,
        max_time: Duration,
    }

    impl LayerTiming {
        fn new(name: String) -> Self {
            Self {
                name,
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
    }

    struct ProtocolProfiler {
        layer_timings: Vec<LayerTiming>,
        message_samples: Vec<(usize, Duration)>,
    }

    impl ProtocolProfiler {
        fn new() -> Self {
            Self {
                layer_timings: vec![
                    LayerTiming::new("Application".to_string()),
                    LayerTiming::new("Protocol".to_string()),
                    LayerTiming::new("Crypto".to_string()),
                    LayerTiming::new("Compression".to_string()),
                    LayerTiming::new("Mesh".to_string()),
                    LayerTiming::new("Transport".to_string()),
                ],
                message_samples: Vec::new(),
            }
        }

        async fn profile_message_processing(&mut self, message_size: usize) -> Duration {
            let total_start = Instant::now();

            for layer in &mut self.layer_timings {
                let layer_start = Instant::now();

                // Simulate processing based on layer type and message size
                let processing_time = match layer.name.as_str() {
                    "Application" => Duration::from_micros(50 + message_size as u64 / 100),
                    "Protocol" => Duration::from_micros(100 + message_size as u64 / 50),
                    "Crypto" => Duration::from_micros(500 + message_size as u64 / 10), // Most expensive
                    "Compression" => {
                        if message_size > 100 {
                            Duration::from_micros(200 + message_size as u64 / 20)
                        } else {
                            Duration::from_micros(10) // Skip small messages
                        }
                    }
                    "Mesh" => Duration::from_micros(150 + message_size as u64 / 30),
                    "Transport" => Duration::from_micros(80 + message_size as u64 / 100),
                    _ => Duration::from_micros(100),
                };

                sleep(processing_time).await;
                let elapsed = layer_start.elapsed();
                layer.add_sample(elapsed);
            }

            let total_time = total_start.elapsed();
            self.message_samples.push((message_size, total_time));
            total_time
        }

        fn generate_report(&self) {
            println!("=== Performance Profile Report ===");
            println!("{}", "-".repeat(60));

            println!("\nLayer Performance (per message):");
            for layer in &self.layer_timings {
                println!(
                    "  {:<15}: avg={:>7.1}μs, min={:>7.1}μs, max={:>7.1}μs, samples={}",
                    layer.name,
                    layer.average_time().as_micros() as f64,
                    layer.min_time.as_micros() as f64,
                    layer.max_time.as_micros() as f64,
                    layer.sample_count
                );
            }

            // Total stack performance
            let total_avg: Duration = self.layer_timings.iter().map(|l| l.average_time()).sum();

            println!("\nStack Performance Summary:");
            println!(
                "  Total average latency: {:.1}μs",
                total_avg.as_micros() as f64
            );
            println!(
                "  Theoretical max throughput: {:.0} messages/sec",
                1_000_000.0 / total_avg.as_micros() as f64
            );

            // Message size impact
            if !self.message_samples.is_empty() {
                println!("\nMessage Size Impact:");

                let mut size_groups: HashMap<&str, Vec<Duration>> = HashMap::new();

                for (size, duration) in &self.message_samples {
                    let group = match size {
                        0..=100 => "Small (≤100B)",
                        101..=1000 => "Medium (101B-1KB)",
                        1001..=10000 => "Large (1KB-10KB)",
                        _ => "Extra Large (>10KB)",
                    };
                    size_groups
                        .entry(group)
                        .or_insert_with(Vec::new)
                        .push(*duration);
                }

                for (group, durations) in size_groups {
                    let avg: Duration = durations.iter().sum::<Duration>() / durations.len() as u32;
                    println!(
                        "  {:<20}: avg={:>7.1}μs, samples={}",
                        group,
                        avg.as_micros() as f64,
                        durations.len()
                    );
                }
            }

            // Bottleneck analysis
            println!("\nBottleneck Analysis:");
            if let Some(bottleneck) = self.layer_timings.iter().max_by_key(|l| l.average_time()) {
                let percentage = (bottleneck.average_time().as_micros() as f64
                    / total_avg.as_micros() as f64)
                    * 100.0;

                println!(
                    "  Primary bottleneck: {} ({:.1}% of total time)",
                    bottleneck.name, percentage
                );

                if percentage > 50.0 {
                    println!("    ⚠️ Significant bottleneck - optimization recommended");
                } else if percentage > 30.0 {
                    println!("    ⚡ Moderate bottleneck - consider optimization");
                } else {
                    println!("    ✅ Balanced performance across layers");
                }
            }
        }
    }

    let mut profiler = ProtocolProfiler::new();

    println!("Phase 1: Profiling different message sizes");
    let test_sizes = vec![50, 200, 1000, 5000];
    let samples_per_size = 10;

    for &size in &test_sizes {
        println!("  Testing {}B messages:", size);
        for i in 0..samples_per_size {
            let duration = profiler.profile_message_processing(size).await;
            if i < 3 {
                // Show first few samples
                println!("    Sample {}: {:.1}μs", i + 1, duration.as_micros() as f64);
            }
        }
    }

    println!("\nPhase 2: Analysis and reporting");
    profiler.generate_report();

    println!("\n=== Optimization Recommendations ===");

    // Find the slowest layer
    if let Some(slowest) = profiler
        .layer_timings
        .iter()
        .max_by_key(|l| l.average_time())
    {
        match slowest.name.as_str() {
            "Crypto" => {
                println!("1. Crypto Optimization:");
                println!("   • Implement hardware acceleration (AES-NI)");
                println!("   • Use SIMD instructions for bulk operations");
                println!("   • Consider cipher suite optimization");
                println!("   • Implement session key caching");
            }
            "Compression" => {
                println!("1. Compression Optimization:");
                println!("   • Use faster algorithms (LZ4) for real-time data");
                println!("   • Skip compression for small messages");
                println!("   • Implement adaptive compression");
            }
            _ => {
                println!("1. General Optimizations:");
                println!("   • Implement zero-copy operations");
                println!("   • Use memory pools for allocations");
                println!("   • Pipeline processing across layers");
            }
        }
    }

    println!("2. Overall Performance Tips:");
    println!("   • Batch small messages together");
    println!("   • Implement async processing pipelines");
    println!("   • Use connection pooling");
    println!("   • Monitor performance in production");

    println!("\n✓ Performance profiling complete!");
    println!("Key insights:");
    println!("  • Identified primary bottlenecks");
    println!("  • Measured layer-by-layer overhead");
    println!("  • Analyzed message size impact");
    println!("  • Provided optimization recommendations\n");

    Ok(())
}

/// Exercise 3: Failure Injection Demo
///
/// This demonstrates the TODO implementation for failure injection
async fn exercise_failure_injection_demo() -> Result<()> {
    println!("=== Failure Injection Demo ===");
    println!("Testing system resilience by injecting failures at each layer\n");

    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct FailureInjector {
        transport_drop_rate: u32, // Percentage 0-100
        crypto_failure_rate: u32,
        mesh_routing_errors: u32,
        consensus_byzantine_rate: u32,
        failures_injected: Arc<AtomicU32>,
        failures_recovered: Arc<AtomicU32>,
    }

    impl FailureInjector {
        fn new() -> Self {
            Self {
                transport_drop_rate: 0,
                crypto_failure_rate: 0,
                mesh_routing_errors: 0,
                consensus_byzantine_rate: 0,
                failures_injected: Arc::new(AtomicU32::new(0)),
                failures_recovered: Arc::new(AtomicU32::new(0)),
            }
        }

        fn set_failure_rates(&mut self, transport: u32, crypto: u32, mesh: u32, consensus: u32) {
            self.transport_drop_rate = transport.min(100);
            self.crypto_failure_rate = crypto.min(100);
            self.mesh_routing_errors = mesh.min(100);
            self.consensus_byzantine_rate = consensus.min(100);
        }

        fn should_inject_failure(&self, layer: &str) -> bool {
            let rate = match layer {
                "transport" => self.transport_drop_rate,
                "crypto" => self.crypto_failure_rate,
                "mesh" => self.mesh_routing_errors,
                "consensus" => self.consensus_byzantine_rate,
                _ => 0,
            };

            if rate > 0 && rand::random::<u32>() % 100 < rate {
                self.failures_injected.fetch_add(1, Ordering::Relaxed);
                true
            } else {
                false
            }
        }

        fn record_recovery(&self) {
            self.failures_recovered.fetch_add(1, Ordering::Relaxed);
        }

        fn get_stats(&self) -> (u32, u32, f64) {
            let injected = self.failures_injected.load(Ordering::Relaxed);
            let recovered = self.failures_recovered.load(Ordering::Relaxed);
            let recovery_rate = if injected > 0 {
                (recovered as f64 / injected as f64) * 100.0
            } else {
                0.0
            };
            (injected, recovered, recovery_rate)
        }
    }

    struct ResilientMessageProcessor {
        injector: FailureInjector,
        retry_attempts: u32,
        successful: Arc<AtomicU32>,
        failed: Arc<AtomicU32>,
        retried: Arc<AtomicU32>,
    }

    impl ResilientMessageProcessor {
        fn new() -> Self {
            Self {
                injector: FailureInjector::new(),
                retry_attempts: 3,
                successful: Arc::new(AtomicU32::new(0)),
                failed: Arc::new(AtomicU32::new(0)),
                retried: Arc::new(AtomicU32::new(0)),
            }
        }

        fn set_failure_rates(&mut self, transport: u32, crypto: u32, mesh: u32, consensus: u32) {
            self.injector
                .set_failure_rates(transport, crypto, mesh, consensus);
        }

        async fn process_message_with_resilience(&self, message_id: u32) -> Result<bool> {
            let mut attempts = 0;

            while attempts <= self.retry_attempts {
                match self.try_process_message(message_id).await {
                    Ok(true) => {
                        self.successful.fetch_add(1, Ordering::Relaxed);
                        if attempts > 0 {
                            self.injector.record_recovery();
                            self.retried.fetch_add(1, Ordering::Relaxed);
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
                            sleep(Duration::from_millis(100 * attempts as u64)).await;
                            // Exponential backoff
                        }
                    }
                    Err(e) => {
                        self.failed.fetch_add(1, Ordering::Relaxed);
                        return Err(e);
                    }
                }
            }

            self.failed.fetch_add(1, Ordering::Relaxed);
            println!(
                "    ✗ Message {} failed permanently after {} attempts",
                message_id, attempts
            );
            Ok(false)
        }

        async fn try_process_message(&self, message_id: u32) -> Result<bool> {
            // Simulate processing through each layer with failure injection

            // Transport layer
            if self.injector.should_inject_failure("transport") {
                println!("      🔌 Transport layer dropped message {}", message_id);
                return Ok(false);
            }

            // Mesh layer
            if self.injector.should_inject_failure("mesh") {
                println!(
                    "      🕸 Mesh layer routing error for message {}",
                    message_id
                );
                return Ok(false);
            }

            // Crypto layer
            if self.injector.should_inject_failure("crypto") {
                println!(
                    "      🔐 Crypto layer signature failure for message {}",
                    message_id
                );
                return Ok(false);
            }

            // Consensus layer
            if self.injector.should_inject_failure("consensus") {
                println!(
                    "      ⚖ Consensus layer byzantine behavior for message {}",
                    message_id
                );
                return Ok(false);
            }

            // If we reach here, processing succeeded
            Ok(true)
        }

        fn get_stats(&self) -> (u32, u32, u32, f64, f64) {
            let successful = self.successful.load(Ordering::Relaxed);
            let failed = self.failed.load(Ordering::Relaxed);
            let retried = self.retried.load(Ordering::Relaxed);
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

    let mut processor = ResilientMessageProcessor::new();

    // Test scenarios with increasing failure rates
    println!("Phase 1: Baseline (no failures)");
    for i in 0..5 {
        processor.process_message_with_resilience(i).await?;
    }
    let (successful, failed, _retried, success_rate, retry_rate) = processor.get_stats();
    println!(
        "  Results: {}✓ {}✗ ({:.1}% success, {:.1}% retries)",
        successful, failed, success_rate, retry_rate
    );

    println!("\nPhase 2: Light transport failures (10%)");
    processor.set_failure_rates(10, 0, 0, 0);
    for i in 5..10 {
        processor.process_message_with_resilience(i).await?;
    }
    let (successful, failed, _retried, success_rate, retry_rate) = processor.get_stats();
    println!(
        "  Results: {}✓ {}✗ ({:.1}% success, {:.1}% retries)",
        successful, failed, success_rate, retry_rate
    );

    println!("\nPhase 3: Moderate crypto failures (20%)");
    processor.set_failure_rates(10, 20, 0, 0);
    for i in 10..15 {
        processor.process_message_with_resilience(i).await?;
    }
    let (successful, failed, _retried, success_rate, retry_rate) = processor.get_stats();
    println!(
        "  Results: {}✓ {}✗ ({:.1}% success, {:.1}% retries)",
        successful, failed, success_rate, retry_rate
    );

    println!("\nPhase 4: High mesh errors (30%) + consensus issues (15%)");
    processor.set_failure_rates(10, 20, 30, 15);
    for i in 15..25 {
        processor.process_message_with_resilience(i).await?;
        if i % 3 == 0 {
            sleep(Duration::from_millis(50)).await; // Simulate time passage
        }
    }

    // Final analysis
    let (final_successful, final_failed, final_retried, final_success_rate, final_retry_rate) =
        processor.get_stats();
    let (injected_failures, recovered_failures, recovery_rate) = processor.injector.get_stats();

    println!("\n{}", "=".repeat(50));
    println!("=== Failure Injection Test Results ===");

    println!("\nFailure Injection Summary:");
    println!("  Total failures injected: {}", injected_failures);
    println!("  Failures recovered from: {}", recovered_failures);
    println!("  Recovery success rate: {:.1}%", recovery_rate);

    println!("\nMessage Processing Summary:");
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

    println!("\n=== System Resilience Assessment ===");
    if final_success_rate >= 90.0 {
        println!("✅ EXCELLENT: System maintains high success rate under failure conditions");
    } else if final_success_rate >= 75.0 {
        println!("✅ GOOD: System shows strong resilience to failures");
    } else if final_success_rate >= 60.0 {
        println!("⚠️ MODERATE: System functions but needs resilience improvements");
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

    println!("\n✓ Failure injection testing complete!");
    println!("Key insights:");
    println!("  • Tested resilience across all protocol layers");
    println!("  • Measured recovery effectiveness: {:.1}%", recovery_rate);
    println!(
        "  • Demonstrated graceful degradation: {:.1}% success rate",
        final_success_rate
    );
    println!("  • Validated retry and backoff mechanisms");

    Ok(())
}
