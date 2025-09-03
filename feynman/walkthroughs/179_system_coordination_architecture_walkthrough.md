# Chapter 67: System Coordination Architecture

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: The Conductor of Distributed Systems

Imagine a massive orchestra where each musician plays a different instrument, reads from different sheet music, and sits in different rooms. Now imagine they need to perform a symphony together, perfectly synchronized, without a central conductor. This is the challenge of coordinating distributed systems.

In BitCraps, the coordination layer acts as the invisible conductor, orchestrating communication between transport protocols, managing network health, synchronizing game state, and ensuring all components work in harmony. This chapter explores how we build robust coordination systems that can handle the complexity of distributed gaming at scale.

## The Fundamentals: Understanding System Coordination

### What is System Coordination?

System coordination is the art of making independent components work together as a cohesive whole. It's about managing dependencies, handling failures gracefully, and ensuring consistent behavior across distributed nodes.

```rust
// The challenge of coordination
pub struct UncoordinatedSystem {
    transport: TransportLayer,      // Manages connections
    consensus: ConsensusEngine,      // Manages game state
    mesh: MeshNetwork,              // Manages peer discovery
    storage: StorageEngine,         // Manages persistence
    // Problem: How do these communicate and stay synchronized?
}

// The solution: Coordination layer
pub struct CoordinatedSystem {
    coordinator: SystemCoordinator,  // Central orchestration
    components: ComponentRegistry,    // Registered subsystems
    event_bus: EventBus,            // Inter-component communication
    health_monitor: HealthMonitor,   // System health tracking
}
```

### The Actor Model for Coordination

BitCraps uses an actor-based coordination model where each component is an independent actor that communicates through messages:

```rust
pub trait Actor: Send + Sync {
    type Message: Send + 'static;
    type Response: Send + 'static;
    
    async fn handle_message(&mut self, msg: Self::Message) -> Result<Self::Response>;
    async fn on_start(&mut self) -> Result<()>;
    async fn on_stop(&mut self) -> Result<()>;
}
```

## Deep Dive: The Transport Coordinator

### Managing Multiple Transport Protocols

The transport coordinator manages different connection types (TCP, UDP, Bluetooth) as a unified system:

```rust
pub struct TransportCoordinator {
    /// Available transport protocols
    transports: HashMap<TransportType, Box<dyn Transport>>,
    
    /// Active connections across all transports
    connections: Arc<RwLock<ConnectionPool>>,
    
    /// Connection routing table
    routing_table: Arc<RwLock<RoutingTable>>,
    
    /// Quality of service manager
    qos_manager: QosManager,
    
    /// Metrics collector
    metrics: Arc<TransportMetrics>,
}

#[derive(Debug, Clone)]
pub enum TransportType {
    Tcp,
    Udp,
    Quic,
    Bluetooth,
    WebSocket,
}

pub trait Transport: Send + Sync {
    /// Establish a new connection
    async fn connect(&mut self, addr: &Address) -> Result<ConnectionId>;
    
    /// Accept incoming connections
    async fn listen(&mut self, addr: &Address) -> Result<()>;
    
    /// Send data over a connection
    async fn send(&mut self, conn: ConnectionId, data: &[u8]) -> Result<()>;
    
    /// Receive data from a connection
    async fn recv(&mut self, conn: ConnectionId) -> Result<Vec<u8>>;
    
    /// Get transport capabilities
    fn capabilities(&self) -> TransportCapabilities;
}
```

### Intelligent Connection Management

```rust
impl TransportCoordinator {
    pub async fn select_best_transport(&self, peer: &PeerId) -> Result<TransportType> {
        let peer_capabilities = self.get_peer_capabilities(peer).await?;
        let network_conditions = self.measure_network_conditions().await?;
        
        // Score each transport based on current conditions
        let mut scores = HashMap::new();
        
        for (transport_type, transport) in &self.transports {
            let score = self.calculate_transport_score(
                transport_type,
                &peer_capabilities,
                &network_conditions,
            );
            scores.insert(transport_type.clone(), score);
        }
        
        // Select highest scoring transport
        scores
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(transport, _)| transport)
            .ok_or(Error::NoSuitableTransport)
    }
    
    fn calculate_transport_score(
        &self,
        transport: &TransportType,
        capabilities: &PeerCapabilities,
        conditions: &NetworkConditions,
    ) -> u32 {
        let mut score = 0;
        
        // Base score from transport characteristics
        score += match transport {
            TransportType::Quic => 100,  // Preferred for reliability + speed
            TransportType::Tcp => 80,    // Reliable but slower
            TransportType::Udp => 60,     // Fast but unreliable
            TransportType::Bluetooth => 40, // Local only
            TransportType::WebSocket => 70, // Browser compatible
        };
        
        // Adjust for network conditions
        if conditions.packet_loss > 0.01 {
            // Prefer reliable transports
            match transport {
                TransportType::Tcp | TransportType::Quic => score += 20,
                TransportType::Udp => score -= 20,
                _ => {}
            }
        }
        
        // Adjust for latency requirements
        if conditions.latency < Duration::from_millis(10) {
            // Local network, Bluetooth is viable
            if transport == &TransportType::Bluetooth {
                score += 30;
            }
        }
        
        score
    }
}
```

### Connection Pool Management

```rust
pub struct ConnectionPool {
    /// Active connections
    connections: HashMap<ConnectionId, Connection>,
    
    /// Connection metadata
    metadata: HashMap<ConnectionId, ConnectionMetadata>,
    
    /// Connection limits
    max_connections: usize,
    max_per_peer: usize,
    
    /// Idle connection management
    idle_timeout: Duration,
    idle_connections: BinaryHeap<IdleConnection>,
}

pub struct Connection {
    id: ConnectionId,
    transport: TransportType,
    peer_id: PeerId,
    state: ConnectionState,
    handle: Box<dyn ConnectionHandle>,
    created_at: Instant,
    last_activity: Arc<AtomicInstant>,
}

impl ConnectionPool {
    pub async fn get_or_create(&mut self, peer_id: &PeerId) -> Result<Arc<Connection>> {
        // Check for existing connection
        if let Some(conn) = self.find_active_connection(peer_id) {
            conn.last_activity.store(Instant::now(), Ordering::Relaxed);
            return Ok(conn);
        }
        
        // Check connection limits
        if self.connections.len() >= self.max_connections {
            self.evict_idle_connection().await?;
        }
        
        // Create new connection
        let transport = self.select_transport(peer_id).await?;
        let handle = transport.connect(peer_id).await?;
        
        let connection = Arc::new(Connection {
            id: ConnectionId::new(),
            transport: transport.transport_type(),
            peer_id: *peer_id,
            state: ConnectionState::Connected,
            handle: Box::new(handle),
            created_at: Instant::now(),
            last_activity: Arc::new(AtomicInstant::now()),
        });
        
        self.connections.insert(connection.id, connection.clone());
        Ok(connection)
    }
    
    async fn evict_idle_connection(&mut self) -> Result<()> {
        while let Some(idle) = self.idle_connections.pop() {
            if idle.idle_since.elapsed() > self.idle_timeout {
                if let Some(mut conn) = self.connections.remove(&idle.connection_id) {
                    conn.handle.close().await?;
                    return Ok(());
                }
            }
        }
        Err(Error::NoIdleConnections)
    }
}
```

## Network Monitoring and Health

### Comprehensive Health Monitoring

```rust
pub struct NetworkMonitor {
    /// Component health trackers
    component_health: Arc<RwLock<HashMap<ComponentId, HealthStatus>>>,
    
    /// Network metrics
    metrics: Arc<NetworkMetrics>,
    
    /// Anomaly detector
    anomaly_detector: AnomalyDetector,
    
    /// Alert system
    alert_manager: AlertManager,
}

#[derive(Clone, Debug)]
pub struct HealthStatus {
    component_id: ComponentId,
    status: HealthState,
    last_check: Instant,
    metrics: HealthMetrics,
    errors: VecDeque<ErrorRecord>,
}

#[derive(Clone, Debug)]
pub enum HealthState {
    Healthy,
    Degraded { reason: String },
    Unhealthy { error: String },
    Unknown,
}

impl NetworkMonitor {
    pub async fn monitor_loop(&mut self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            // Check all components
            let health_checks = self.perform_health_checks().await;
            
            // Detect anomalies
            if let Some(anomaly) = self.anomaly_detector.analyze(&health_checks) {
                self.handle_anomaly(anomaly).await;
            }
            
            // Update metrics
            self.update_metrics(&health_checks).await;
            
            // Trigger alerts if needed
            self.check_alert_conditions(&health_checks).await;
        }
    }
    
    async fn perform_health_checks(&self) -> Vec<HealthCheckResult> {
        let components = self.component_health.read().await;
        
        let futures: Vec<_> = components
            .keys()
            .map(|id| self.check_component_health(*id))
            .collect();
        
        futures::future::join_all(futures).await
    }
    
    async fn check_component_health(&self, id: ComponentId) -> HealthCheckResult {
        let start = Instant::now();
        
        let result = match id.component_type {
            ComponentType::Transport => self.check_transport_health(id).await,
            ComponentType::Consensus => self.check_consensus_health(id).await,
            ComponentType::Storage => self.check_storage_health(id).await,
            ComponentType::Mesh => self.check_mesh_health(id).await,
        };
        
        HealthCheckResult {
            component_id: id,
            status: result,
            latency: start.elapsed(),
            timestamp: SystemTime::now(),
        }
    }
}
```

### Anomaly Detection

```rust
pub struct AnomalyDetector {
    /// Historical metrics for baseline
    history: RingBuffer<MetricSnapshot>,
    
    /// Anomaly detection algorithms
    detectors: Vec<Box<dyn AnomalyAlgorithm>>,
    
    /// Anomaly thresholds
    thresholds: AnomalyThresholds,
}

pub trait AnomalyAlgorithm: Send + Sync {
    fn detect(&self, current: &MetricSnapshot, history: &[MetricSnapshot]) -> Option<Anomaly>;
}

pub struct StatisticalAnomalyDetector {
    /// Z-score threshold for anomaly detection
    z_threshold: f64,
}

impl AnomalyAlgorithm for StatisticalAnomalyDetector {
    fn detect(&self, current: &MetricSnapshot, history: &[MetricSnapshot]) -> Option<Anomaly> {
        // Calculate mean and standard deviation from history
        let mean = self.calculate_mean(history);
        let std_dev = self.calculate_std_dev(history, mean);
        
        // Calculate z-score for current metrics
        let z_score = (current.value - mean) / std_dev;
        
        if z_score.abs() > self.z_threshold {
            Some(Anomaly {
                metric: current.metric_name.clone(),
                severity: self.calculate_severity(z_score),
                description: format!("Metric deviation: z-score = {:.2}", z_score),
                recommended_action: self.recommend_action(current, z_score),
            })
        } else {
            None
        }
    }
}
```

## Component Orchestration

### Service Dependency Management

```rust
pub struct ComponentRegistry {
    /// Registered components
    components: HashMap<ComponentId, Arc<dyn Component>>,
    
    /// Component dependencies
    dependencies: DependencyGraph,
    
    /// Startup order based on dependencies
    startup_order: Vec<ComponentId>,
    
    /// Component lifecycle state
    lifecycle: HashMap<ComponentId, LifecycleState>,
}

pub struct DependencyGraph {
    edges: HashMap<ComponentId, HashSet<ComponentId>>,
}

impl ComponentRegistry {
    pub async fn start_all(&mut self) -> Result<()> {
        // Calculate startup order using topological sort
        self.startup_order = self.dependencies.topological_sort()?;
        
        // Start components in order
        for component_id in &self.startup_order {
            self.start_component(*component_id).await?;
        }
        
        Ok(())
    }
    
    async fn start_component(&mut self, id: ComponentId) -> Result<()> {
        // Check dependencies are running
        for dep_id in self.dependencies.get_dependencies(&id) {
            let dep_state = self.lifecycle.get(&dep_id)
                .ok_or(Error::UnknownComponent(dep_id))?;
            
            if *dep_state != LifecycleState::Running {
                return Err(Error::DependencyNotReady(dep_id));
            }
        }
        
        // Start the component
        let component = self.components.get(&id)
            .ok_or(Error::UnknownComponent(id))?;
        
        component.start().await?;
        
        self.lifecycle.insert(id, LifecycleState::Running);
        
        tracing::info!(component_id = ?id, "Component started successfully");
        
        Ok(())
    }
}
```

### Event-Driven Communication

```rust
pub struct EventBus {
    /// Event subscribers
    subscribers: Arc<RwLock<HashMap<EventType, Vec<Subscriber>>>>,
    
    /// Event queue for async processing
    event_queue: Arc<Mutex<VecDeque<Event>>>,
    
    /// Event processor task
    processor: JoinHandle<()>,
}

pub struct Event {
    event_type: EventType,
    source: ComponentId,
    payload: Box<dyn Any + Send>,
    timestamp: Instant,
}

pub struct Subscriber {
    component_id: ComponentId,
    handler: Arc<dyn EventHandler>,
    filter: Option<EventFilter>,
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: &Event) -> Result<()>;
}

impl EventBus {
    pub async fn publish(&self, event: Event) -> Result<()> {
        // Add to queue for async processing
        let mut queue = self.event_queue.lock().await;
        queue.push_back(event);
        
        // Wake up processor if sleeping
        self.notify_processor();
        
        Ok(())
    }
    
    pub async fn subscribe(
        &self,
        component_id: ComponentId,
        event_type: EventType,
        handler: Arc<dyn EventHandler>,
    ) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;
        
        let subscriber = Subscriber {
            component_id,
            handler,
            filter: None,
        };
        
        subscribers
            .entry(event_type)
            .or_insert_with(Vec::new)
            .push(subscriber);
        
        Ok(())
    }
    
    async fn process_events(&self) {
        loop {
            // Get next event
            let event = {
                let mut queue = self.event_queue.lock().await;
                queue.pop_front()
            };
            
            if let Some(event) = event {
                // Find subscribers
                let subscribers = self.subscribers.read().await;
                if let Some(subs) = subscribers.get(&event.event_type) {
                    // Notify all subscribers concurrently
                    let futures: Vec<_> = subs
                        .iter()
                        .filter(|s| s.filter.as_ref().map_or(true, |f| f.matches(&event)))
                        .map(|s| s.handler.handle_event(&event))
                        .collect();
                    
                    let results = futures::future::join_all(futures).await;
                    
                    // Log any errors
                    for (i, result) in results.into_iter().enumerate() {
                        if let Err(e) = result {
                            tracing::error!(
                                subscriber = ?subs[i].component_id,
                                error = ?e,
                                "Event handler failed"
                            );
                        }
                    }
                }
            } else {
                // No events, sleep briefly
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}
```

## Failure Handling and Recovery

### Circuit Breaker Pattern

```rust
pub struct CircuitBreaker {
    /// Current state of the circuit
    state: Arc<RwLock<CircuitState>>,
    
    /// Failure threshold before opening
    failure_threshold: u32,
    
    /// Success threshold before closing
    success_threshold: u32,
    
    /// Timeout before attempting to close
    timeout: Duration,
    
    /// Failure counter
    failures: Arc<AtomicU32>,
    
    /// Success counter
    successes: Arc<AtomicU32>,
}

#[derive(Debug, Clone)]
pub enum CircuitState {
    Closed,
    Open { since: Instant },
    HalfOpen { attempts: u32 },
}

impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        // Check circuit state
        let state = self.state.read().await.clone();
        
        match state {
            CircuitState::Open { since } => {
                if since.elapsed() > self.timeout {
                    // Try to transition to half-open
                    *self.state.write().await = CircuitState::HalfOpen { attempts: 0 };
                } else {
                    return Err(Error::CircuitOpen);
                }
            }
            CircuitState::HalfOpen { attempts } => {
                if attempts >= self.success_threshold {
                    // Too many attempts in half-open, go back to open
                    *self.state.write().await = CircuitState::Open { since: Instant::now() };
                    return Err(Error::CircuitOpen);
                }
            }
            CircuitState::Closed => {}
        }
        
        // Attempt the call
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }
    
    async fn on_success(&self) {
        self.successes.fetch_add(1, Ordering::Relaxed);
        self.failures.store(0, Ordering::Relaxed);
        
        let mut state = self.state.write().await;
        
        match *state {
            CircuitState::HalfOpen { attempts } => {
                if attempts + 1 >= self.success_threshold {
                    // Enough successes, close the circuit
                    *state = CircuitState::Closed;
                    tracing::info!("Circuit breaker closed after recovery");
                } else {
                    *state = CircuitState::HalfOpen { attempts: attempts + 1 };
                }
            }
            _ => {}
        }
    }
    
    async fn on_failure(&self) {
        let failures = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        self.successes.store(0, Ordering::Relaxed);
        
        if failures >= self.failure_threshold {
            let mut state = self.state.write().await;
            *state = CircuitState::Open { since: Instant::now() };
            tracing::warn!("Circuit breaker opened after {} failures", failures);
        }
    }
}
```

### Graceful Degradation

```rust
pub struct DegradationController {
    /// Service levels in order of priority
    service_levels: Vec<ServiceLevel>,
    
    /// Current active level
    current_level: Arc<RwLock<usize>>,
    
    /// Resource monitor
    resource_monitor: ResourceMonitor,
}

pub struct ServiceLevel {
    name: String,
    priority: u8,
    features: HashSet<Feature>,
    resource_requirements: ResourceRequirements,
}

impl DegradationController {
    pub async fn adjust_service_level(&mut self) -> Result<()> {
        let available_resources = self.resource_monitor.get_available().await?;
        let current = *self.current_level.read().await;
        
        // Find highest service level we can support
        let new_level = self.service_levels
            .iter()
            .enumerate()
            .rev()  // Start from highest priority
            .find(|(_, level)| {
                level.resource_requirements.can_satisfy(&available_resources)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(self.service_levels.len() - 1);  // Fallback to lowest
        
        if new_level != current {
            self.transition_to_level(new_level).await?;
        }
        
        Ok(())
    }
    
    async fn transition_to_level(&mut self, new_level: usize) -> Result<()> {
        let current = *self.current_level.read().await;
        let current_features = &self.service_levels[current].features;
        let new_features = &self.service_levels[new_level].features;
        
        // Disable features not in new level
        for feature in current_features.difference(new_features) {
            self.disable_feature(feature).await?;
        }
        
        // Enable features in new level
        for feature in new_features.difference(current_features) {
            self.enable_feature(feature).await?;
        }
        
        *self.current_level.write().await = new_level;
        
        tracing::info!(
            "Service level changed: {} -> {}",
            self.service_levels[current].name,
            self.service_levels[new_level].name
        );
        
        Ok(())
    }
}
```

## Performance Optimization

### Load Balancing Strategies

```rust
pub struct LoadBalancer {
    /// Available backends
    backends: Vec<Backend>,
    
    /// Load balancing strategy
    strategy: Box<dyn LoadBalancingStrategy>,
    
    /// Health checker
    health_checker: HealthChecker,
    
    /// Request router
    router: RequestRouter,
}

pub trait LoadBalancingStrategy: Send + Sync {
    fn select_backend(&self, request: &Request, backends: &[Backend]) -> Option<usize>;
}

pub struct WeightedRoundRobin {
    weights: Vec<u32>,
    current: Arc<AtomicUsize>,
}

impl LoadBalancingStrategy for WeightedRoundRobin {
    fn select_backend(&self, _request: &Request, backends: &[Backend]) -> Option<usize> {
        let total_weight: u32 = self.weights.iter().sum();
        let mut selection = self.current.fetch_add(1, Ordering::Relaxed) % total_weight as usize;
        
        for (idx, weight) in self.weights.iter().enumerate() {
            if selection < *weight as usize && backends[idx].is_healthy() {
                return Some(idx);
            }
            selection -= *weight as usize;
        }
        
        // Fallback to first healthy backend
        backends.iter().position(|b| b.is_healthy())
    }
}

pub struct LeastConnections;

impl LoadBalancingStrategy for LeastConnections {
    fn select_backend(&self, _request: &Request, backends: &[Backend]) -> Option<usize> {
        backends
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_healthy())
            .min_by_key(|(_, b)| b.active_connections())
            .map(|(idx, _)| idx)
    }
}
```

### Resource Pooling

```rust
pub struct ResourcePool<T: Resource> {
    /// Available resources
    available: Arc<Mutex<Vec<T>>>,
    
    /// Resources currently in use
    in_use: Arc<RwLock<HashMap<ResourceId, T>>>,
    
    /// Pool configuration
    config: PoolConfig,
    
    /// Resource factory
    factory: Arc<dyn ResourceFactory<T>>,
}

pub trait Resource: Send + Sync {
    fn id(&self) -> ResourceId;
    fn is_valid(&self) -> bool;
    fn reset(&mut self) -> Result<()>;
}

impl<T: Resource> ResourcePool<T> {
    pub async fn acquire(&self) -> Result<PooledResource<T>> {
        // Try to get from available pool
        if let Some(resource) = self.try_acquire_available().await? {
            return Ok(resource);
        }
        
        // Check if we can create new resource
        if self.in_use.read().await.len() < self.config.max_size {
            let resource = self.factory.create().await?;
            return Ok(self.wrap_resource(resource));
        }
        
        // Wait for resource to become available
        self.wait_for_available().await
    }
    
    async fn try_acquire_available(&self) -> Result<Option<PooledResource<T>>> {
        let mut available = self.available.lock().await;
        
        while let Some(mut resource) = available.pop() {
            if resource.is_valid() {
                resource.reset()?;
                return Ok(Some(self.wrap_resource(resource)));
            }
        }
        
        Ok(None)
    }
    
    fn wrap_resource(&self, resource: T) -> PooledResource<T> {
        let id = resource.id();
        self.in_use.write().await.insert(id, resource);
        
        PooledResource {
            resource: Some(resource),
            pool: Arc::downgrade(self),
            id,
        }
    }
}

pub struct PooledResource<T: Resource> {
    resource: Option<T>,
    pool: Weak<ResourcePool<T>>,
    id: ResourceId,
}

impl<T: Resource> Drop for PooledResource<T> {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            if let Some(pool) = self.pool.upgrade() {
                // Return resource to pool
                pool.available.lock().await.push(resource);
                pool.in_use.write().await.remove(&self.id);
            }
        }
    }
}
```

## Testing Coordination Systems

### Integration Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_component_startup_order() {
        let mut registry = ComponentRegistry::new();
        
        // Register components with dependencies
        registry.register(ComponentA::new(), vec![]);
        registry.register(ComponentB::new(), vec![ComponentA::id()]);
        registry.register(ComponentC::new(), vec![ComponentB::id()]);
        
        // Start all components
        registry.start_all().await.unwrap();
        
        // Verify startup order
        assert_eq!(registry.startup_order, vec![
            ComponentA::id(),
            ComponentB::id(),
            ComponentC::id(),
        ]);
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(3, 2, Duration::from_millis(100));
        
        // Simulate failures
        for _ in 0..3 {
            let _ = breaker.call(async { Err::<(), _>(Error::ServiceError) }).await;
        }
        
        // Circuit should be open
        assert!(matches!(
            breaker.call(async { Ok(()) }).await,
            Err(Error::CircuitOpen)
        ));
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Circuit should attempt half-open
        assert!(breaker.call(async { Ok(()) }).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_load_balancer() {
        let backends = vec![
            Backend::new("server1", 100),
            Backend::new("server2", 200),
            Backend::new("server3", 100),
        ];
        
        let strategy = WeightedRoundRobin::new(vec![1, 2, 1]);
        let balancer = LoadBalancer::new(backends, strategy);
        
        // Test distribution
        let mut distribution = HashMap::new();
        for _ in 0..400 {
            let backend = balancer.select_backend(&Request::default()).unwrap();
            *distribution.entry(backend).or_insert(0) += 1;
        }
        
        // Verify roughly matches weights (1:2:1)
        assert!((distribution[&0] - 100).abs() < 20);
        assert!((distribution[&1] - 200).abs() < 20);
        assert!((distribution[&2] - 100).abs() < 20);
    }
}
```

## Production Deployment

### Monitoring and Observability

```rust
pub struct CoordinatorMetrics {
    /// Component health metrics
    component_health: prometheus::GaugeVec,
    
    /// Event bus metrics
    events_processed: prometheus::CounterVec,
    event_processing_duration: prometheus::HistogramVec,
    
    /// Connection pool metrics
    active_connections: prometheus::IntGauge,
    connection_acquisitions: prometheus::Counter,
    connection_failures: prometheus::Counter,
    
    /// Circuit breaker metrics
    circuit_state: prometheus::IntGaugeVec,
    circuit_trips: prometheus::CounterVec,
}

impl CoordinatorMetrics {
    pub fn record_event_processed(&self, event_type: &str, duration: Duration) {
        self.events_processed
            .with_label_values(&[event_type])
            .inc();
        
        self.event_processing_duration
            .with_label_values(&[event_type])
            .observe(duration.as_secs_f64());
    }
    
    pub fn update_circuit_state(&self, component: &str, state: &CircuitState) {
        let value = match state {
            CircuitState::Closed => 0,
            CircuitState::Open { .. } => 1,
            CircuitState::HalfOpen { .. } => 2,
        };
        
        self.circuit_state
            .with_label_values(&[component])
            .set(value);
    }
}
```

## Conclusion

System coordination is the invisible backbone that makes distributed systems work. Through BitCraps' coordination architecture, we've seen how independent components can work together harmoniously through careful orchestration, monitoring, and failure handling.

Key takeaways from this chapter:

1. **Transport coordination** unifies multiple protocols under a single interface
2. **Health monitoring** provides early warning of system issues
3. **Event-driven architecture** enables loose coupling between components
4. **Circuit breakers** prevent cascade failures
5. **Load balancing** distributes work efficiently
6. **Resource pooling** maximizes utilization

Remember: Great coordination is invisible when it works and invaluable when things go wrong. The true test of a coordination system is not how it handles success, but how gracefully it manages failure.
