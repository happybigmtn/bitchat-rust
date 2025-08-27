# Chapter 78: System Monitoring Infrastructure - Production Observability

*You can't manage what you can't measure. In distributed systems, comprehensive monitoring isn't optional - it's essential for survival. Let's explore BitCraps' production-grade monitoring infrastructure.*

## The Observability Challenge

Imagine running a distributed casino across thousands of devices. How do you know if:
- A node is struggling with high CPU usage?
- The network is experiencing unusual latency?
- Battery drain is exceeding targets on mobile devices?
- Consensus is taking longer than expected?

Our monitoring system in `/src/monitoring/metrics.rs` provides complete observability.

## Hierarchical Metrics Architecture

We organize metrics into logical categories:

```rust
pub struct MetricsCollector {
    pub network: NetworkMetrics,      // Connection health
    pub consensus: ConsensusMetrics,   // Agreement performance  
    pub gaming: GamingMetrics,        // Game statistics
    pub performance: PerformanceMetrics, // Latency tracking
    pub resources: ResourceMetrics,    // System resources
    pub errors: ErrorMetrics,         // Error tracking
    start_time: Instant,              // For uptime calculation
}
```

Each category tracks specific aspects of system health.

## Real System Monitoring Integration

Unlike many systems that use simulated metrics, we collect real data:

```rust
pub fn update_from_system_monitor(&self) {
    if let Ok(system_metrics) = global_system_monitor().collect_metrics() {
        self.resources.update_from_system_metrics(&system_metrics);
        
        log::debug!("Updated metrics: CPU {}%, Memory {} MB, Battery: {:?}%", 
            system_metrics.cpu_usage_percent,
            system_metrics.used_memory_bytes / 1024 / 1024,
            system_metrics.battery_level);
    }
}
```

This connects to our platform-specific monitoring (using `sysinfo` crate):
- CPU usage via `/proc/stat` on Linux
- Memory from `/proc/meminfo`
- Battery from `/sys/class/power_supply`
- Temperature from thermal zones

## Prometheus Export Format

We export metrics in the industry-standard Prometheus format:

```rust
pub fn export_prometheus(&self) -> String {
    let mut output = String::new();
    
    // Network metrics with proper annotations
    output.push_str(&format!(
        "# HELP bitcraps_network_messages_sent Total messages sent\n\
         # TYPE bitcraps_network_messages_sent counter\n\
         bitcraps_network_messages_sent {}\n",
        self.network.messages_sent.load(Ordering::Relaxed)
    ));
    
    // Consensus performance
    output.push_str(&format!(
        "# HELP bitcraps_consensus_latency_ms Average consensus latency\n\
         # TYPE bitcraps_consensus_latency_ms gauge\n\
         bitcraps_consensus_latency_ms {}\n",
        self.consensus.average_latency_ms()
    ));
    
    // Mobile-specific metrics
    if let Some(battery_level) = self.resources.get_battery_level() {
        output.push_str(&format!(
            "# HELP bitcraps_battery_level Battery percentage\n\
             # TYPE bitcraps_battery_level gauge\n\
             bitcraps_battery_level {}\n",
            battery_level
        ));
    }
}
```

### Metric Types Explained

**Counter**: Always increases (messages sent, errors)
```
bitcraps_network_messages_sent 142857
```

**Gauge**: Can go up or down (active connections, CPU usage)
```
bitcraps_network_active_connections 42
```

**Histogram**: Distribution of values (latency percentiles)
```
bitcraps_consensus_latency_ms_bucket{le="10"} 100
bitcraps_consensus_latency_ms_bucket{le="50"} 450
bitcraps_consensus_latency_ms_bucket{le="100"} 490
```

## Atomic Operations for Thread Safety

All metrics use lock-free atomic operations:

```rust
pub struct NetworkMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicUsize,
}

impl NetworkMetrics {
    pub fn record_message_sent(&self, size: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
    }
}
```

Why atomics? In a multi-threaded environment, multiple threads update metrics concurrently. Atomics ensure:
- No data races
- No locks (better performance)
- Accurate counting

## Latency Tracking with Percentiles

We track latency distributions, not just averages:

```rust
pub struct LatencyHistogram {
    buckets: RwLock<Vec<u64>>,
    boundaries: Vec<u64>, // [1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s]
}

impl LatencyHistogram {
    pub fn record(&self, latency_ms: u64) {
        let mut buckets = self.buckets.write();
        
        // Find appropriate bucket
        for (i, &boundary) in self.boundaries.iter().enumerate() {
            if latency_ms <= boundary {
                buckets[i] += 1;
                break;
            }
        }
    }
    
    pub fn percentile(&self, p: f64) -> u64 {
        let buckets = self.buckets.read();
        let total: u64 = buckets.iter().sum();
        let target = (total as f64 * p / 100.0) as u64;
        
        let mut count = 0;
        for (i, &bucket_count) in buckets.iter().enumerate() {
            count += bucket_count;
            if count >= target {
                return self.boundaries[i];
            }
        }
        
        self.boundaries.last().copied().unwrap_or(0)
    }
}
```

This tells us:
- p50 (median): Typical performance
- p95: Most requests
- p99: Outliers that affect user experience

## Resource Monitoring

Critical for mobile devices and resource-constrained environments:

```rust
pub struct ResourceMetrics {
    pub memory_usage_bytes: AtomicU64,
    pub cpu_usage_percent: AtomicU8,
    pub disk_usage_bytes: AtomicU64,
    pub battery_level: RwLock<Option<u8>>,
    pub battery_charging: AtomicBool,
    pub temperature_celsius: RwLock<Option<f32>>,
    pub thermal_throttling: AtomicBool,
}

impl ResourceMetrics {
    pub fn check_health_thresholds(&self) -> HealthStatus {
        let memory = self.memory_usage_bytes.load(Ordering::Relaxed);
        let cpu = self.cpu_usage_percent.load(Ordering::Relaxed);
        
        if memory > 200_000_000 { // 200MB
            return HealthStatus::Critical("Memory usage exceeds 200MB");
        }
        
        if cpu > 80 {
            return HealthStatus::Warning("CPU usage above 80%");
        }
        
        if self.thermal_throttling.load(Ordering::Relaxed) {
            return HealthStatus::Warning("Thermal throttling active");
        }
        
        HealthStatus::Healthy
    }
}
```

## Error Tracking and Classification

Errors are categorized for better debugging:

```rust
pub struct ErrorMetrics {
    pub total_errors: AtomicU64,
    pub network_errors: AtomicU64,
    pub consensus_errors: AtomicU64,
    pub validation_errors: AtomicU64,
    pub recent_errors: RwLock<VecDeque<ErrorEvent>>,
}

pub struct ErrorEvent {
    pub timestamp: SystemTime,
    pub category: ErrorCategory,
    pub message: String,
    pub severity: ErrorSeverity,
}

impl ErrorMetrics {
    pub fn record_error(&self, category: ErrorCategory, message: String) {
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        
        match category {
            ErrorCategory::Network => {
                self.network_errors.fetch_add(1, Ordering::Relaxed);
            }
            ErrorCategory::Consensus => {
                self.consensus_errors.fetch_add(1, Ordering::Relaxed);
            }
            // ...
        }
        
        // Keep last 100 errors for debugging
        let mut recent = self.recent_errors.write();
        if recent.len() >= 100 {
            recent.pop_front();
        }
        recent.push_back(ErrorEvent {
            timestamp: SystemTime::now(),
            category,
            message,
            severity: ErrorSeverity::from_category(category),
        });
    }
}
```

## Continuous Background Monitoring

The system automatically collects metrics:

```rust
pub fn start_system_monitoring() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            // Collect system metrics
            METRICS.update_from_system_monitor();
            
            // Check health thresholds
            let health = METRICS.resources.check_health_thresholds();
            
            match health {
                HealthStatus::Critical(msg) => {
                    log::error!("CRITICAL: {}", msg);
                    // Trigger alerts
                }
                HealthStatus::Warning(msg) => {
                    log::warn!("WARNING: {}", msg);
                }
                HealthStatus::Healthy => {}
            }
            
            // Export to monitoring backend
            if let Err(e) = export_to_backend().await {
                log::warn!("Failed to export metrics: {}", e);
            }
        }
    })
}
```

## Gaming-Specific Metrics

We track game-specific performance:

```rust
pub struct GamingMetrics {
    pub total_games: AtomicU64,
    pub active_games: AtomicUsize,
    pub total_bets: AtomicU64,
    pub total_volume: AtomicU64,
    pub house_edge_actual: RwLock<f64>,
    pub player_stats: RwLock<HashMap<PeerId, PlayerStats>>,
}

pub struct PlayerStats {
    pub games_played: u64,
    pub total_wagered: u64,
    pub total_won: u64,
    pub win_rate: f64,
    pub suspicious_activity_score: f64,
}
```

This helps detect:
- Unusual betting patterns
- Statistical anomalies
- Potential cheating

## Dashboard Integration

Metrics feed into real-time dashboards:

```rust
pub async fn serve_metrics_endpoint() {
    let metrics_route = warp::path!("metrics")
        .map(|| {
            let prometheus_data = METRICS.export_prometheus();
            warp::reply::with_header(
                prometheus_data,
                "Content-Type", 
                "text/plain; version=0.0.4"
            )
        });
    
    warp::serve(metrics_route)
        .run(([0, 0, 0, 0], 9090))
        .await;
}
```

Grafana queries this endpoint:
```promql
# Alert if consensus is slow
alert: SlowConsensus
expr: bitcraps_consensus_latency_ms > 1000
for: 5m

# Calculate bet volume rate
rate(bitcraps_bets_total[5m])

# Memory usage trend
deriv(bitcraps_memory_usage_bytes[1h])
```

## Distributed Tracing

For debugging complex issues across nodes:

```rust
pub struct TraceContext {
    pub trace_id: Uuid,
    pub span_id: Uuid,
    pub parent_span: Option<Uuid>,
    pub start_time: Instant,
    pub tags: HashMap<String, String>,
}

impl TraceContext {
    pub fn child_span(&self, operation: &str) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id: Uuid::new_v4(),
            parent_span: Some(self.span_id),
            start_time: Instant::now(),
            tags: hashmap!{
                "operation".to_string() => operation.to_string()
            },
        }
    }
}

// Usage in consensus
let span = trace.child_span("consensus.propose");
let proposal = create_proposal();
span.record_duration();
```

## Alert Rules

Automatic alerting for critical conditions:

```yaml
groups:
  - name: bitcraps_alerts
    rules:
      - alert: HighMemoryUsage
        expr: bitcraps_memory_usage_bytes > 150000000
        for: 5m
        annotations:
          summary: "Memory usage exceeds 150MB"
          
      - alert: ConsensusStalled
        expr: rate(bitcraps_consensus_proposals_accepted[5m]) == 0
        for: 10m
        annotations:
          summary: "No consensus progress in 10 minutes"
          
      - alert: BatteryDrainHigh
        expr: deriv(bitcraps_battery_level[1h]) < -5
        annotations:
          summary: "Battery draining faster than 5% per hour"
```

## Performance Impact

Monitoring itself must be lightweight:

```rust
#[cfg(feature = "metrics")]
macro_rules! record_metric {
    ($metric:expr) => {
        $metric  // Only compiled in when metrics enabled
    };
}

#[cfg(not(feature = "metrics"))]
macro_rules! record_metric {
    ($metric:expr) => {
        ()  // No-op when disabled
    };
}

// Usage
record_metric!(METRICS.network.record_message_sent(msg.len()));
```

## Exercise: Custom Metric Implementation

Add a new metric for tracking dice roll fairness:

```rust
pub struct FairnessMetrics {
    roll_distribution: RwLock<[u64; 11]>, // 2-12
}

impl FairnessMetrics {
    pub fn record_roll(&self, total: u8) {
        // TODO: Update distribution
        // TODO: Calculate chi-square statistic
        // TODO: Export as Prometheus histogram
    }
    
    pub fn is_fair(&self) -> bool {
        // TODO: Statistical test for fairness
    }
}
```

## Key Takeaways

1. **Hierarchical Organization**: Metrics grouped by subsystem
2. **Real Data**: Actual system monitoring, not simulated
3. **Lock-Free Updates**: Atomic operations for performance
4. **Standard Format**: Prometheus-compatible export
5. **Percentile Tracking**: Beyond simple averages
6. **Error Classification**: Detailed error tracking
7. **Health Thresholds**: Automatic alerting
8. **Distributed Tracing**: Cross-node debugging
9. **Minimal Overhead**: Conditional compilation

Great monitoring is invisible when everything works, invaluable when it doesn't. Our comprehensive system ensures we know exactly what's happening across the entire distributed casino.

Next, we'll explore how the system recovers when things inevitably go wrong.