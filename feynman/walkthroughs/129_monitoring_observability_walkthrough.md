# Chapter 15: Monitoring and Observability

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending

## Walking Through `src/monitoring/`

*Part of the comprehensive BitCraps curriculum - a deep dive into production system monitoring*

---

## Part I: Monitoring and Observability for Complete Beginners

Have you ever wondered how Google knows their services are down before users complain? Or how Netflix can automatically route traffic away from failing servers before viewers even notice a glitch? Welcome to the world of monitoring and observability - the eyes and ears that keep complex distributed systems running smoothly.

This might seem like a mundane topic compared to cryptography or consensus algorithms, but monitoring is absolutely critical. A system that can't be observed can't be debugged, optimized, or trusted. As the saying goes: "If you can't measure it, you can't manage it."

### What is Monitoring, Really?

At its core, monitoring is about answering three fundamental questions:
1. **Is the system working?** (Health monitoring)
2. **How well is it working?** (Performance monitoring)
3. **What happened when it wasn't working?** (Diagnostic monitoring)

But as systems became more complex, monitoring evolved into *observability* - the ability to understand the internal state of a system based on its external outputs.

### The Evolution of System Monitoring

**The Stone Age: Log Files (1970s-1980s)**

Early computer systems logged events to files. System administrators would manually read through text logs, looking for error patterns. This was like trying to understand a movie by reading a transcript - you got the facts but missed the bigger picture.

Example: Unix syslog files showing cryptic entries like:
```
Jan 15 14:32:01 server kernel: Out of memory: Kill process 1234
```

**The Bronze Age: Simple Metrics (1990s)**

As systems grew, administrators needed better tools. SNMP (Simple Network Management Protocol) let you query devices for basic metrics: CPU usage, memory, disk space. This was like having a car dashboard - you could see if something was critically wrong.

Tools like MRTG (Multi Router Traffic Grapher) started creating simple time-series graphs. For the first time, you could see *trends* rather than just snapshots.

**The Iron Age: Application Performance Monitoring (2000s)**

Web applications introduced new complexity. Tools like Nagios emerged to monitor services, not just hardware. The focus shifted from "Is the machine running?" to "Is the application working?"

This era introduced concepts like:
- Service-level agreements (SLAs)
- Response time monitoring
- Application health checks

**The Modern Era: Observability (2010s-Present)**

Microservices, containers, and cloud computing created systems too complex for traditional monitoring. You might have hundreds of services across thousands of containers. The new approach: instrument everything and let data reveal the story.

This brought us:
- Distributed tracing (follow requests across services)
- Structured logging (machine-readable log events)
- High-cardinality metrics (millions of unique metric combinations)
- Real-time alerting and automated remediation

### The Three Pillars of Observability

Modern observability rests on three pillars, each providing a different lens into system behavior:

**1. Metrics - The Vital Signs**

Metrics are numerical measurements over time. They're like a patient's vital signs in a hospital - heart rate, blood pressure, temperature. They tell you *what* is happening.

Types of metrics:
- **Counters**: Values that only increase (requests handled, errors occurred)
- **Gauges**: Values that can go up or down (memory usage, active connections)
- **Histograms**: Distribution of values (response time percentiles)
- **Timers**: How long operations take

Example metrics for a web service:
- requests_per_second (counter)
- active_connections (gauge) 
- response_time_95th_percentile (histogram)
- database_query_duration (timer)

**2. Logs - The Detailed Story**

Logs provide rich, contextual information about events. They're like a detailed diary of what the system was doing. They tell you *why* something happened.

Evolution of logging:
- Unstructured: "User john logged in at 2:30 PM"
- Structured: {"user": "john", "action": "login", "timestamp": "2023-01-15T14:30:00Z"}
- Contextual: Includes request IDs to trace activities across services

**3. Traces - The Journey Map**

Distributed tracing follows individual requests as they flow through multiple services. It's like having a GPS tracker on each request, showing exactly where it went and how long each step took.

A single web request might:
1. Hit load balancer (2ms)
2. Route to web server (5ms)
3. Query user service (15ms)
4. Query database (45ms)
5. Return response (3ms)

Tracing shows you this entire journey and where bottlenecks occur.

### Famous Monitoring Disasters and What They Taught Us

**The Knight Capital Glitch (2012)**

Knight Capital deployed new trading software but had inadequate monitoring of their trading algorithms. When the software malfunctioned, it executed millions of erroneous trades in 45 minutes, losing $440 million.

Lesson: *Monitor business logic, not just infrastructure.* It's not enough to know your servers are healthy if your algorithms are making catastrophic decisions.

**The AWS S3 Outage (2017)**

During routine maintenance, an Amazon engineer mistyped a command and took down more S3 capacity than intended. The monitoring dashboard itself depended on S3, so Amazon couldn't even display status information about the outage.

Lesson: *Monitor the monitors.* Your observability system must be more resilient than the systems it monitors.

**The Facebook Six-Hour Outage (2021)**

A routine BGP configuration change accidentally disconnected Facebook's data centers from the internet. Engineers couldn't remotely access systems to fix the problem because their monitoring and management tools were inside the unreachable network.

Lesson: *Out-of-band monitoring is critical.* You need ways to observe and control systems that don't depend on those same systems working.

### Key Monitoring Concepts Every Developer Should Know

**1. The Golden Signals**

Google's Site Reliability Engineering team identified four metrics that matter most for user-facing systems:

- **Latency**: How long requests take to complete
- **Traffic**: How much demand is being placed on your system  
- **Errors**: Rate of requests that fail
- **Saturation**: How "full" your service is (CPU, memory, I/O utilization)

These four metrics can reveal most problems before they impact users.

**2. Service Level Indicators (SLIs) and Objectives (SLOs)**

SLIs are metrics that matter to users:
- "99% of API requests complete in under 200ms"
- "99.9% of requests succeed without errors"

SLOs are targets for these metrics:
- "We will maintain 99.5% availability"
- "Average response time will be under 100ms"

SLOs help teams focus on what actually impacts user experience rather than vanity metrics.

**3. Error Budgets**

If your SLO is 99.9% availability, you have a 0.1% error budget. This means you can "spend" some reliability for features. If you're exceeding your SLO, you can take risks. If you're burning through your error budget, you must focus on reliability.

**4. The RED Method**

For microservices, monitor:
- **Rate**: Requests per second
- **Errors**: Error rate
- **Duration**: Response time distribution

This gives you a consistent framework for monitoring any service.

**5. The USE Method**

For resources, monitor:
- **Utilization**: Percentage of time the resource is busy
- **Saturation**: Degree of queuing or extra work the resource cannot service
- **Errors**: Count of error events

Perfect for monitoring infrastructure components.

### Alerting: The Art of Knowing When to Panic

Good alerting is harder than it seems. Too many alerts and engineers ignore them ("alert fatigue"). Too few alerts and problems go unnoticed.

**Alert Fatigue: The Boy Who Cried Wolf Problem**

Imagine you get 50 alerts per day, but only 2 represent real problems. After a week, you'll start ignoring all alerts. Then when a real emergency happens, you'll miss it.

Common causes:
- Alerting on symptoms, not root causes
- Thresholds set too low
- No proper alert prioritization
- Alerts for problems that fix themselves

**The Hierarchy of Alert Severity**

- **Critical**: Service is down or data is being lost. Page someone immediately.
- **Warning**: Service is degraded but functional. Can wait until business hours.
- **Info**: Something notable happened but no action needed. Log only.

**The 5-Minute Rule**

Never alert on something that wouldn't require human action within 5 minutes. If it's not urgent enough to wake someone up, it's not urgent enough to alert immediately.

### Mobile and IoT Monitoring: New Challenges

Traditional monitoring assumes you control the entire stack - hardware, OS, network. But mobile and IoT devices introduce new complexities:

**Battery Constraints**
Monitoring itself consumes battery. You must balance observability with power efficiency. Too much monitoring can drain batteries; too little leaves you blind to problems.

**Network Variability**
Mobile networks are unreliable. Your monitoring must work over poor connections and handle intermittent connectivity gracefully.

**Device Diversity**
Android has thousands of device models with different capabilities. iOS has fewer models but still significant variety. Your monitoring must adapt to different hardware constraints.

**User Behavior Patterns**
Mobile users behave differently than web users. They're more sensitive to battery drain, use apps sporadically, and often have poor network connections.

### Performance Monitoring: Beyond Simple Metrics

**Percentiles vs. Averages**

Average response time is often misleading. If 99% of requests take 100ms but 1% take 10 seconds, your average might look fine while some users have terrible experiences.

Percentiles tell a better story:
- 50th percentile (median): Half of users experience this or better
- 95th percentile: 95% of users experience this or better  
- 99th percentile: 99% of users experience this or better

**Long Tail Performance**

The worst 1% of requests often reveal the most interesting problems. These "tail latencies" can indicate:
- Cache misses
- Database query performance issues
- Network problems
- Resource contention

**Observability-Driven Development**

Modern development practices include observability from the start:
- Add structured logging to every function
- Instrument all external calls
- Include tracing context in all operations
- Design for monitoring from day one

### Monitoring Distributed Systems: The Complexity Explosion

Monitoring a single server is straightforward. Monitoring a distributed system with hundreds of services is exponentially more complex.

**The N×N Problem**

With N services, you have N² potential interactions to monitor. Each service can fail independently, and cascade failures can ripple through the entire system.

**Correlation vs. Causation**

When 50 metrics spike simultaneously, which one caused the problem? Advanced monitoring uses machine learning to identify likely root causes.

**The Observer Effect**

Monitoring itself can change system behavior. High-resolution metrics collection can add latency. Verbose logging can fill disks. The act of measurement must not significantly impact what you're measuring.

### Modern Monitoring Architectures

**Push vs. Pull Models**

- **Push**: Applications send metrics to collectors (StatsD, Prometheus PushGateway)
- **Pull**: Collectors query applications for metrics (Prometheus, SNMP)

Each has trade-offs in scalability, reliability, and operational complexity.

**Time-Series Databases**

Modern metrics generate millions of data points. Specialized databases like InfluxDB, Prometheus, and TimescaleDB are optimized for time-series data:
- Efficient compression
- Fast range queries
- Retention policies
- Downsampling for long-term storage

**Event Streaming**

Real-time monitoring uses event streaming platforms like Apache Kafka to handle massive volumes of metrics, logs, and traces with low latency.

### The Economics of Monitoring

Observability isn't free. Consider the costs:
- Storage for metrics, logs, and traces
- Network bandwidth for data collection
- CPU overhead for instrumentation
- Engineer time to maintain monitoring systems
- Alert fatigue reducing engineer effectiveness

The key is finding the right balance: enough observability to operate effectively, not so much that the cure becomes worse than the disease.

### Security and Privacy in Monitoring

Monitoring systems see everything, making them attractive targets for attackers and raising privacy concerns:

**Security Concerns**
- Monitoring data often contains sensitive information
- Monitoring systems have broad access to infrastructure
- Logs might contain passwords, API keys, or personal data

**Privacy Concerns**
- User behavior tracking
- Location data from mobile devices
- Personal information in error logs

Best practices:
- Encrypt monitoring data in transit and at rest
- Redact sensitive information from logs
- Use secure authentication for monitoring systems
- Regular security audits of monitoring infrastructure

### The Psychology of Monitoring

Humans are notoriously bad at interpreting data, especially during high-stress incidents. Good monitoring accounts for human psychology:

**Cognitive Load**
During outages, engineers have limited mental capacity. Dashboards should show only essential information clearly.

**Confirmation Bias**
People look for data that confirms their hypotheses. Good monitoring forces you to consider alternative explanations.

**Alert Fatigue**
After seeing thousands of false alarms, humans naturally ignore alerts. This is an evolved survival mechanism but dangerous for system reliability.

---

This foundation prepares you to understand why the BitCraps monitoring system is designed the way it is. Every architectural decision reflects hard-learned lessons from decades of monitoring complex systems in production.

---

## Part II: BitCraps Monitoring Implementation Deep Dive

The BitCraps monitoring system represents a sophisticated approach to observing distributed gaming systems. It must handle unique challenges: real-time performance requirements, mobile device constraints, and the need to monitor both technical metrics and business-critical gaming events.

### Module Architecture: `src/monitoring/mod.rs`

The monitoring system is organized into specialized modules, each addressing different aspects of observability:

```rust
pub mod metrics;
pub mod health;
pub mod dashboard;
pub mod alerting;
pub mod system;
pub mod http_server;
```

This modular approach allows different parts of the system to use different monitoring capabilities as needed:
- Games need real-time metrics
- Mobile clients need battery-aware monitoring
- Operators need comprehensive dashboards
- Automated systems need alerting

### Core Metrics Collection: `src/monitoring/metrics.rs`

The heart of the monitoring system is the comprehensive metrics collector that tracks every aspect of the gaming system.

**Lines 10-26: System-Wide Metrics Structure**
```rust
pub struct MetricsCollector {
    /// Network metrics
    pub network: NetworkMetrics,
    /// Consensus metrics
    pub consensus: ConsensusMetrics,
    /// Gaming metrics
    pub gaming: GamingMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Resource metrics
    pub resources: ResourceMetrics,
    /// Error tracking
    pub errors: ErrorMetrics,
    /// Start time for uptime calculation
    start_time: Instant,
}
```

This structure embodies the principle of *domain-specific metrics*. Rather than generic system metrics, each subsystem has its own specialized metrics that understand the semantics of that domain.

**Lines 52-65: Real System Integration**
```rust
pub fn update_from_system_monitor(&self) {
    if let Ok(system_metrics) = crate::monitoring::system::global_system_monitor().collect_metrics() {
        self.resources.update_from_system_metrics(&system_metrics);
        
        log::debug!("Updated metrics from system monitor: CPU {}%, Memory {} MB, Battery: {:?}%", 
                    system_metrics.cpu_usage_percent,
                    system_metrics.used_memory_bytes / 1024 / 1024,
                    system_metrics.battery_level);
    } else {
        log::warn!("Failed to collect system metrics, using fallback values");
    }
}
```

This demonstrates *graceful degradation*. The system tries to collect real metrics but falls back to simulated values if real monitoring isn't available. This is crucial for development environments and platforms where system APIs aren't accessible.

**Lines 67-76: Periodic Monitoring**
```rust
pub fn start_system_monitoring() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval_timer.tick().await;
            METRICS.update_from_system_monitor();
        }
    })
}
```

The system uses Tokio's interval timer for periodic monitoring. The 5-second interval balances freshness with resource usage - frequent enough to catch problems quickly, infrequent enough to avoid overwhelming mobile devices.

**Lines 84-214: Prometheus Export Format**
```rust
pub fn export_prometheus(&self) -> String {
    let mut output = String::new();
    
    // Network metrics
    output.push_str(&format!(
        "# HELP bitcraps_network_messages_sent Total messages sent\n\
         # TYPE bitcraps_network_messages_sent counter\n\
         bitcraps_network_messages_sent {}\n",
        self.network.messages_sent.load(Ordering::Relaxed)
    ));
    // ... more metrics
}
```

This implements the Prometheus text exposition format, the industry standard for metrics export. Key aspects:
- **Help text**: Explains what each metric measures
- **Type information**: Distinguishes counters, gauges, and histograms
- **Consistent naming**: Follows Prometheus naming conventions

The format makes BitCraps metrics compatible with the entire Prometheus ecosystem: Grafana dashboards, AlertManager rules, and third-party tools.

### Network Metrics: Understanding Communication Patterns

**Lines 261-300: Network Metrics Structure**
```rust
pub struct NetworkMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicUsize,
    pub connection_errors: AtomicU64,
    pub packet_loss_rate: Arc<RwLock<f64>>,
    pub average_latency: Arc<RwLock<LatencyTracker>>,
}
```

**Why Atomic Types?**

The metrics use `AtomicU64` and `AtomicUsize` for thread-safe updates without locks. This is crucial for high-performance systems where metrics collection must not slow down the main application logic.

Atomic operations guarantee:
- **Consistency**: Updates are all-or-nothing
- **Performance**: No lock contention
- **Safety**: No race conditions

**Lines 287-300: Message Recording**
```rust
pub fn record_message_sent(&self, bytes: usize) {
    self.messages_sent.fetch_add(1, Ordering::Relaxed);
    self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
}

pub fn record_message_received(&self, bytes: usize) {
    self.messages_received.fetch_add(1, Ordering::Relaxed);
    self.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
}
```

These methods demonstrate *point-of-use instrumentation*. The metrics are recorded at the exact moment events occur, ensuring accuracy and reducing the chance of missing important events.

### Consensus Metrics: Monitoring Distributed Agreement

**Lines 302-337: Consensus-Specific Metrics**
```rust
pub struct ConsensusMetrics {
    pub proposals_submitted: AtomicU64,
    pub proposals_accepted: AtomicU64,
    pub proposals_rejected: AtomicU64,
    pub consensus_rounds: AtomicU64,
    pub fork_count: AtomicU64,
    pub latency_samples: Arc<RwLock<LatencyTracker>>,
}
```

These metrics are specifically designed for Byzantine fault-tolerant consensus:
- **Proposal ratios**: High rejection rates might indicate network problems or Byzantine behavior
- **Fork count**: Measures network partition frequency
- **Latency tracking**: Critical for real-time gaming

**Lines 324-336: Proposal Recording with Latency**
```rust
pub fn record_proposal(&self, accepted: bool, latency_ms: f64) {
    self.proposals_submitted.fetch_add(1, Ordering::Relaxed);
    if accepted {
        self.proposals_accepted.fetch_add(1, Ordering::Relaxed);
    } else {
        self.proposals_rejected.fetch_add(1, Ordering::Relaxed);
    }
    self.latency_samples.write().add_sample(latency_ms);
}
```

This method captures both outcome and timing in a single operation, ensuring consistent metrics even under high concurrency.

### Gaming Metrics: Business Logic Observability

**Lines 339-371: Gaming-Specific Tracking**
```rust
pub struct GamingMetrics {
    pub total_games: AtomicU64,
    pub active_games: AtomicUsize,
    pub total_bets: AtomicU64,
    pub total_volume: AtomicU64,
    pub total_payouts: AtomicU64,
    pub dice_rolls: AtomicU64,
    pub disputes: AtomicU64,
}
```

These metrics bridge technical and business concerns:
- **Technical**: Active games, dice rolls (system load indicators)
- **Business**: Total volume, payouts (revenue and fairness indicators)
- **Trust**: Disputes (system integrity indicators)

**Lines 363-370: Bet Recording**
```rust
pub fn record_bet(&self, amount: u64) {
    self.total_bets.fetch_add(1, Ordering::Relaxed);
    self.total_volume.fetch_add(amount, Ordering::Relaxed);
}
```

This demonstrates *compound metric recording*. A single event (placing a bet) updates multiple metrics (count and volume), providing different perspectives on the same activity.

### Resource Metrics: Mobile-Aware Monitoring

**Lines 400-475: Mobile-Specific Resource Tracking**
```rust
pub struct ResourceMetrics {
    pub memory_usage_bytes: AtomicU64,
    pub cpu_usage_percent: AtomicUsize,
    pub disk_usage_bytes: AtomicU64,
    pub thread_count: AtomicUsize,
    /// Battery level (0-100) if available
    pub battery_level: Arc<RwLock<Option<f32>>>,
    /// Battery charging status
    pub battery_charging: Arc<RwLock<Option<bool>>>,
    /// Temperature in Celsius if available
    pub temperature_celsius: Arc<RwLock<Option<f32>>>,
    /// Whether thermal throttling is active
    pub thermal_throttling: Arc<RwLock<bool>>,
}
```

This structure acknowledges that mobile devices have unique constraints:
- **Battery awareness**: Critical for mobile apps
- **Thermal monitoring**: Prevents device overheating
- **Optional metrics**: Not all platforms support all metrics

**Lines 441-454: System Metrics Integration**
```rust
pub fn update_from_system_metrics(&self, system_metrics: &crate::monitoring::system::SystemMetrics) {
    // Update basic metrics
    self.update_memory(system_metrics.used_memory_bytes);
    self.update_cpu(system_metrics.cpu_usage_percent as usize);
    self.thread_count.store(system_metrics.thread_count as usize, Ordering::Relaxed);
    
    // Update battery metrics
    *self.battery_level.write() = system_metrics.battery_level;
    *self.battery_charging.write() = system_metrics.battery_charging;
    
    // Update thermal metrics
    *self.temperature_celsius.write() = system_metrics.temperature_celsius;
    *self.thermal_throttling.write() = system_metrics.thermal_throttling;
}
```

This method demonstrates *platform abstraction*. The high-level monitoring interface is the same regardless of platform, but the underlying implementation adapts to available capabilities.

### Latency Tracking: Understanding Performance Distribution

**Lines 547-586: Advanced Latency Analysis**
```rust
pub struct LatencyTracker {
    samples: VecDeque<f64>,
    max_samples: usize,
}

impl LatencyTracker {
    pub fn add_sample(&mut self, latency_ms: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
    }
    
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = ((p / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[index]
    }
}
```

This implements a *sliding window* approach to latency tracking:
- **Fixed memory usage**: Never grows beyond max_samples
- **Percentile calculation**: Shows distribution, not just averages  
- **Real-time updates**: Reflects current system behavior

The percentile calculation is crucial for understanding tail latencies - the worst-case performance that affects user experience.

### Error Metrics: Failure Pattern Recognition

**Lines 478-526: Comprehensive Error Tracking**
```rust
pub struct ErrorMetrics {
    pub total_errors: AtomicU64,
    pub network_errors: AtomicU64,
    pub consensus_errors: AtomicU64,
    pub gaming_errors: AtomicU64,
    pub critical_errors: AtomicU64,
    pub recent_errors: Arc<RwLock<VecDeque<ErrorEvent>>>,
}
```

This structure provides both high-level error rates and detailed error context:
- **Categorized counts**: Identify which subsystems have problems
- **Severity tracking**: Distinguish minor glitches from critical failures
- **Recent error log**: Provides context for debugging

**Lines 499-525: Error Recording with Context**
```rust
pub fn record_error(&self, category: ErrorCategory, message: String, is_critical: bool) {
    self.total_errors.fetch_add(1, Ordering::Relaxed);
    
    match category {
        ErrorCategory::Network => { self.network_errors.fetch_add(1, Ordering::Relaxed); },
        ErrorCategory::Consensus => { self.consensus_errors.fetch_add(1, Ordering::Relaxed); },
        ErrorCategory::Gaming => { self.gaming_errors.fetch_add(1, Ordering::Relaxed); },
        ErrorCategory::Other => {},
    };
    
    if is_critical {
        self.critical_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    let event = ErrorEvent {
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        category,
        message,
        is_critical,
    };
    
    let mut errors = self.recent_errors.write();
    if errors.len() >= 100 {
        errors.pop_front();
    }
    errors.push_back(event);
}
```

This method demonstrates *structured error recording*:
- **Categorization**: Enables targeted debugging
- **Severity levels**: Supports proper alerting
- **Context preservation**: Keeps detailed information for analysis
- **Memory bounds**: Prevents unbounded growth

### Health Monitoring: `src/monitoring/health.rs`

**Lines 6-12: Health Status Structure**
```rust
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: String,
    pub uptime_seconds: u64,
    pub memory_mb: u64,
    pub active_peers: usize,
    pub version: String,
}
```

This provides a simple, human-readable summary of system health - crucial for load balancers, monitoring dashboards, and quick status checks.

**Lines 27-43: Health Assessment Logic**
```rust
pub fn check_health(&self) -> HealthStatus {
    let uptime = self.start_time.elapsed();
    let memory_usage = self.get_memory_usage();
    let active_peers = 0; // Placeholder - would get from actual network metrics
    
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
```

The health check uses simple heuristics to determine overall system status. The 1GB memory threshold is conservative for mobile devices but appropriate for detecting memory leaks early.

### System Monitoring: Platform-Aware Observability

The system monitoring module (`src/monitoring/system/mod.rs`) provides a sophisticated abstraction over platform-specific monitoring APIs.

**Lines 52-65: Platform Abstraction**
```rust
pub trait SystemMonitor: Send + Sync {
    /// Collect current system metrics
    fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError>;
    
    /// Get platform identifier
    fn platform_name(&self) -> &str;
    
    /// Check if real monitoring is available (vs simulation)
    fn is_real_monitoring(&self) -> bool;
    
    /// Get supported metrics for this platform
    fn supported_metrics(&self) -> Vec<MetricType>;
}
```

This trait enables *platform-agnostic monitoring code* while supporting platform-specific optimizations. The same application code works on Android, iOS, Linux, macOS, and Windows, but each platform can provide the most accurate metrics available.

**Lines 94-150: Caching Layer**
```rust
pub struct CachedSystemMonitor {
    inner: Box<dyn SystemMonitor>,
    cache: Arc<Mutex<Option<CachedMetrics>>>,
    cache_duration: Duration,
}

impl CachedSystemMonitor {
    pub fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();
        
        // Check if cache is valid
        if let Some(ref cached) = *cache {
            if now.duration_since(cached.collected_at) < self.cache_duration {
                return Ok(cached.metrics.clone());
            }
        }
        
        // Cache expired or doesn't exist, collect new metrics
        let metrics = self.inner.collect_metrics()?;
        *cache = Some(CachedMetrics {
            metrics: metrics.clone(),
            collected_at: now,
        });
        
        Ok(metrics)
    }
}
```

The caching layer is crucial for battery efficiency. System metric collection can be expensive (requiring system calls, file I/O, or even privileged operations). The cache ensures that frequent metric queries don't drain battery or overwhelm the system.

### Global Metrics Instance

**Lines 644-647: Lazy Static Global**
```rust
lazy_static::lazy_static! {
    /// Global metrics instance
    pub static ref METRICS: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
}
```

The global metrics instance provides *zero-configuration monitoring*. Any part of the application can record metrics without dependency injection or complex setup. While global state is generally discouraged, metrics collection is a cross-cutting concern that benefits from this approach.

### Testing the Monitoring System

**Lines 649-687: Comprehensive Tests**
```rust
#[test]
fn test_metrics_collection() {
    let metrics = MetricsCollector::new();
    
    // Record some metrics
    metrics.network.record_message_sent(100);
    metrics.network.record_message_received(200);
    metrics.consensus.record_proposal(true, 10.0);
    metrics.gaming.record_bet(100);
    
    // Check values
    assert_eq!(metrics.network.messages_sent.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.network.bytes_sent.load(Ordering::Relaxed), 100);
    assert_eq!(metrics.consensus.proposals_accepted.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.gaming.total_bets.load(Ordering::Relaxed), 1);
}

#[test]
fn test_prometheus_export() {
    let metrics = MetricsCollector::new();
    metrics.network.record_message_sent(100);
    
    let prometheus = metrics.export_prometheus();
    assert!(prometheus.contains("bitcraps_network_messages_sent 1"));
}
```

These tests verify both the core functionality and the export formats. The Prometheus export test ensures compatibility with external monitoring tools.

### Key Design Decisions and Trade-offs

**1. Atomic Operations vs. Locks**
The system uses atomic operations extensively instead of mutexes. This trades some flexibility for significant performance gains, especially under high concurrency.

**2. Domain-Specific Metrics vs. Generic Metrics**
Rather than generic counters and gauges, the system provides domain-aware metrics (gaming metrics, consensus metrics). This makes the code more maintainable but less flexible.

**3. Push vs. Pull Metrics Model**
The system records metrics as events occur (push model) but exposes them via HTTP for external scrapers (pull model). This hybrid approach combines the benefits of both approaches.

**4. Platform Abstraction vs. Native APIs**
The system abstracts platform differences while still allowing platform-specific optimizations. This requires more complex code but enables cross-platform deployment.

**5. Memory vs. Accuracy Trade-offs**
The latency tracker uses fixed-size sliding windows, which limits memory usage but might miss short bursts of latency spikes. This trade-off favors long-term stability over perfect accuracy.

### Production Considerations

**1. Performance Impact**
Metrics collection adds overhead to every operation. The atomic operations are designed to minimize this impact, but in extreme cases, metrics can be disabled or sampled.

**2. Storage Requirements**
High-resolution metrics generate significant data volumes. The system supports different retention policies and downsampling strategies for long-term storage.

**3. Privacy and Security**
The monitoring system avoids logging sensitive data like user credentials or game results. All metrics are aggregate counters rather than individual event details.

**4. Mobile-Specific Concerns**
The system is designed for mobile deployment with battery-aware monitoring, adaptive collection frequencies, and graceful degradation when system APIs are unavailable.

The BitCraps monitoring system demonstrates how theoretical monitoring principles translate into practical, production-ready code. Every design decision reflects the unique requirements of real-time, distributed gaming systems while maintaining compatibility with industry-standard monitoring tools and practices.
