# Chapter 79: Error Recovery Mechanisms - Building Resilient Systems

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


*In distributed systems, failure isn't a possibility - it's a certainty. The difference between a toy and production system is how gracefully it handles failure. Let's explore BitCraps' comprehensive resilience mechanisms.*

## The Failure Landscape

In our distributed casino, failures come in many forms:
- Network partitions isolate nodes
- Nodes crash without warning
- Messages get lost or corrupted
- Byzantine nodes act maliciously
- Resources become exhausted

Our resilience layer in `/src/resilience/mod.rs` handles all these gracefully.

## Connection State Management

Every connection is actively monitored:

```rust
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: TransportAddress,
    pub state: ConnectionState,
    pub last_seen: Instant,
    pub reconnect_attempts: u32,
    pub consecutive_failures: u32,
    pub latency_ms: Option<u32>,
    pub packet_loss_rate: f32,
}

pub enum ConnectionState {
    Connected,      // Everything working
    Disconnected,   // Connection lost
    Reconnecting,   // Actively trying to reconnect
    Failed,         // Given up (for now)
}
```

This tracks not just whether a connection exists, but its quality and reliability.

## Circuit Breaker Pattern

Circuit breakers prevent cascading failures - like electrical circuit breakers prevent fires:

```rust
pub struct CircuitBreaker {
    name: String,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    config: CircuitBreakerConfig,
}

pub enum CircuitState {
    Closed,     // Normal operation - requests flow through
    Open,       // Too many failures - block requests
    HalfOpen,   // Testing recovery - allow limited requests
}
```

### How Circuit Breakers Work

1. **Closed State** (Normal):
   - Requests pass through
   - Count failures
   - If failures exceed threshold → Open

2. **Open State** (Protected):
   - Reject requests immediately
   - No load on failing service
   - After timeout → HalfOpen

3. **HalfOpen State** (Testing):
   - Allow limited test requests
   - If successful → Closed
   - If failures continue → Open

Implementation in action:

```rust
impl CircuitBreaker {
    pub async fn call<F, T>(&mut self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>
    {
        match self.state {
            CircuitState::Open => {
                if self.should_attempt_reset() {
                    self.state = CircuitState::HalfOpen;
                } else {
                    return Err(Error::CircuitBreakerOpen);
                }
            }
            CircuitState::HalfOpen => {
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                }
            }
            CircuitState::Closed => {}
        }
        
        // Attempt the operation
        match f.await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(e)
            }
        }
    }
    
    fn on_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());
        
        if self.failure_count >= self.config.failure_threshold {
            self.state = CircuitState::Open;
            log::warn!("Circuit breaker {} opened", self.name);
        }
    }
}
```

## Intelligent Retry Policies

Not all retries are created equal. We use exponential backoff with jitter:

```rust
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f32,
    pub jitter: bool,
}

impl RetryPolicy {
    pub async fn execute<F, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Future<Output = Result<T>>
    {
        let mut attempt = 0;
        let mut delay = self.initial_delay;
        
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt >= self.max_attempts => return Err(e),
                Err(_) => {
                    attempt += 1;
                    
                    // Calculate next delay with exponential backoff
                    delay = Duration::from_secs_f32(
                        delay.as_secs_f32() * self.exponential_base
                    ).min(self.max_delay);
                    
                    // Add jitter to prevent thundering herd
                    if self.jitter {
                        let jitter_amount = delay.as_secs_f32() * 0.1 * fastrand::f32();
                        delay = Duration::from_secs_f32(
                            delay.as_secs_f32() + jitter_amount
                        );
                    }
                    
                    sleep(delay).await;
                }
            }
        }
    }
}
```

### Why Jitter Matters

Without jitter, all clients retry simultaneously, creating traffic spikes:
```
Time: 0s   | | | | | (5 requests fail)
Time: 1s   | | | | | (5 retries together)
Time: 2s   | | | | | (5 retries together)
```

With jitter, retries spread out:
```
Time: 0s   | | | | | (5 requests fail)
Time: 0.9s |         (1 retry)
Time: 1.0s   |       (1 retry)
Time: 1.1s     |     (1 retry)
Time: 1.2s       |   (1 retry)
Time: 1.3s         | (1 retry)
```

## Automatic Reconnection

The reconnection scheduler manages failed connections:

```rust
pub struct ReconnectScheduler {
    queue: Arc<Mutex<VecDeque<ReconnectTask>>>,
    base_delay: Duration,
    max_delay: Duration,
}

impl ReconnectScheduler {
    pub async fn schedule_reconnect(&self, peer_id: PeerId, attempt: u32) {
        // Calculate delay with exponential backoff
        let delay = self.calculate_delay(attempt);
        
        let task = ReconnectTask {
            peer_id,
            attempt,
            scheduled_at: Instant::now() + delay,
        };
        
        let mut queue = self.queue.lock().await;
        queue.push_back(task);
        
        log::info!("Scheduled reconnect for {} in {:?}", peer_id, delay);
    }
    
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let exponential = self.base_delay * 2u32.pow(attempt.min(10));
        exponential.min(self.max_delay)
    }
    
    pub async fn run(self: Arc<Self>) {
        loop {
            sleep(Duration::from_secs(1)).await;
            
            let now = Instant::now();
            let ready_tasks = self.get_ready_tasks(now).await;
            
            for task in ready_tasks {
                tokio::spawn(Self::attempt_reconnect(task));
            }
        }
    }
}
```

## Health Checking

Proactive health checks detect problems early:

```rust
pub struct HealthChecker {
    check_interval: Duration,
    timeout: Duration,
    unhealthy_threshold: u32,
    healthy_threshold: u32,
}

impl HealthChecker {
    pub async fn monitor_connection(&self, conn: &mut Connection) {
        let mut consecutive_failures = 0;
        let mut consecutive_successes = 0;
        
        let mut ticker = interval(self.check_interval);
        
        loop {
            ticker.tick().await;
            
            match timeout(self.timeout, conn.ping()).await {
                Ok(Ok(_)) => {
                    consecutive_successes += 1;
                    consecutive_failures = 0;
                    
                    if consecutive_successes >= self.healthy_threshold {
                        conn.mark_healthy();
                    }
                }
                Ok(Err(_)) | Err(_) => {
                    consecutive_failures += 1;
                    consecutive_successes = 0;
                    
                    if consecutive_failures >= self.unhealthy_threshold {
                        conn.mark_unhealthy();
                        self.trigger_recovery(conn).await;
                    }
                }
            }
        }
    }
}
```

## Failure Detection

We use multiple strategies to detect failures quickly:

### 1. Heartbeat Timeout
```rust
if last_seen.elapsed() > HEARTBEAT_TIMEOUT {
    mark_as_failed(peer_id);
}
```

### 2. Phi Accrual Failure Detector
More sophisticated - adapts to network conditions:

```rust
pub struct PhiAccrualDetector {
    window_size: usize,
    intervals: VecDeque<Duration>,
    threshold: f64,
}

impl PhiAccrualDetector {
    pub fn phi(&self, now: Instant, last_heartbeat: Instant) -> f64 {
        let elapsed = now.duration_since(last_heartbeat);
        let mean = self.mean_interval();
        let stddev = self.std_deviation();
        
        // Calculate probability using normal distribution
        let p = 1.0 - self.normal_cdf(elapsed.as_secs_f64(), mean, stddev);
        
        // Convert to phi value
        -p.log10()
    }
    
    pub fn is_failed(&self, phi: f64) -> bool {
        phi > self.threshold  // Typically 8.0
    }
}
```

### 3. SWIM Protocol
Gossip-based failure detection:

```rust
pub async fn indirect_probe(&self, target: PeerId) {
    // Ask k random nodes to probe the target
    let helpers = self.select_random_peers(3);
    
    for helper in helpers {
        if helper.probe(target).await.is_ok() {
            // Target is alive, just couldn't reach us directly
            return;
        }
    }
    
    // No helper could reach target - likely failed
    self.mark_suspected(target);
}
```

## Cascading Failure Prevention

Prevent one failure from taking down the system:

```rust
pub struct RateLimiter {
    capacity: u32,
    tokens: AtomicU32,
    refill_rate: Duration,
}

impl RateLimiter {
    pub fn try_acquire(&self) -> bool {
        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current == 0 {
                return false;  // No tokens available
            }
            
            if self.tokens.compare_exchange(
                current,
                current - 1,
                Ordering::Release,
                Ordering::Acquire
            ).is_ok() {
                return true;
            }
        }
    }
}
```

## Bulkhead Pattern

Isolate failures to prevent total system failure:

```rust
pub struct Bulkhead {
    semaphore: Semaphore,
    queue_size: usize,
    waiting: AtomicUsize,
}

impl Bulkhead {
    pub async fn execute<F, T>(&self, f: F) -> Result<T>
    where F: Future<Output = T>
    {
        // Check queue size
        if self.waiting.load(Ordering::Acquire) >= self.queue_size {
            return Err(Error::BulkheadFull);
        }
        
        self.waiting.fetch_add(1, Ordering::AcqRel);
        
        // Acquire permit
        let permit = self.semaphore.acquire().await?;
        self.waiting.fetch_sub(1, Ordering::AcqRel);
        
        let result = f.await;
        drop(permit);
        
        Ok(result)
    }
}
```

## Recovery Strategies

When failures occur, we have multiple recovery options:

### 1. Failover
```rust
pub async fn failover(&self, primary: PeerId) -> Result<PeerId> {
    let replicas = self.get_replicas(primary);
    
    for replica in replicas {
        if self.is_healthy(replica).await {
            self.promote_to_primary(replica);
            return Ok(replica);
        }
    }
    
    Err(Error::NoHealthyReplicas)
}
```

### 2. State Reconstruction
```rust
pub async fn reconstruct_state(&self, failed_node: PeerId) -> Result<State> {
    // Collect state fragments from peers
    let fragments = self.collect_state_fragments(failed_node).await?;
    
    // Reconstruct using Reed-Solomon erasure coding
    let state = self.decode_fragments(fragments)?;
    
    Ok(state)
}
```

### 3. Graceful Degradation
```rust
pub fn degrade_service(&mut self) {
    // Disable non-essential features
    self.features.disable(Feature::Analytics);
    self.features.disable(Feature::Recommendations);
    
    // Increase cache TTL to reduce load
    self.cache_ttl *= 2;
    
    // Switch to read-only mode if necessary
    if self.load > CRITICAL_THRESHOLD {
        self.read_only_mode = true;
    }
}
```

## Exercise: Implement Adaptive Timeout

Create a timeout mechanism that adapts to network conditions:

```rust
pub struct AdaptiveTimeout {
    history: VecDeque<Duration>,
    min_timeout: Duration,
    max_timeout: Duration,
}

impl AdaptiveTimeout {
    pub fn calculate_timeout(&self) -> Duration {
        // TODO: Calculate based on recent response times
        // TODO: Add safety margin (e.g., p99 + 50%)
        // TODO: Bound between min and max
    }
    
    pub fn record_response(&mut self, duration: Duration) {
        // TODO: Update history
        // TODO: Remove old entries
    }
}
```

## Key Takeaways

1. **Expect Failure**: Design assuming components will fail
2. **Circuit Breakers**: Prevent cascading failures
3. **Smart Retries**: Exponential backoff with jitter
4. **Health Checks**: Detect problems proactively
5. **Multiple Detectors**: Heartbeats, Phi accrual, SWIM
6. **Rate Limiting**: Prevent overload during recovery
7. **Bulkheads**: Isolate failures to subsystems
8. **Graceful Degradation**: Maintain core functionality

Resilience isn't about preventing failures - it's about recovering quickly and gracefully. Our comprehensive approach ensures BitCraps stays operational even when individual components fail.

Next, we'll explore how we test all these resilience mechanisms to ensure they work when needed.
