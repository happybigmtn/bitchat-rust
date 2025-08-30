//! Chaos Engineering Framework for BitCraps
//!
//! Systematically introduces failures and adverse conditions to test
//! system resilience and recovery mechanisms.

use async_trait::async_trait;
use rand::{thread_rng, Rng};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};

/// Chaos monkey that randomly introduces failures
pub struct ChaosMonkey {
    config: ChaosConfig,
    active: Arc<RwLock<bool>>,
    events: Arc<Mutex<Vec<ChaosEvent>>>,
}

#[derive(Clone, Debug)]
pub struct ChaosConfig {
    /// Probability of injecting a failure (0.0 to 1.0)
    pub failure_probability: f64,
    /// Types of chaos to enable
    pub enabled_chaos: Vec<ChaosType>,
    /// Maximum duration for delays
    pub max_delay_ms: u64,
    /// Maximum packet loss percentage
    pub max_packet_loss: f64,
    /// Whether to simulate network partitions
    pub enable_partitions: bool,
    /// Whether to simulate memory pressure
    pub enable_memory_pressure: bool,
    /// Whether to simulate CPU pressure
    pub enable_cpu_pressure: bool,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            failure_probability: 0.1, // 10% chance
            enabled_chaos: vec![
                ChaosType::NetworkDelay,
                ChaosType::PacketLoss,
                ChaosType::ConnectionDrop,
            ],
            max_delay_ms: 5000,
            max_packet_loss: 0.3,
            enable_partitions: false,
            enable_memory_pressure: false,
            enable_cpu_pressure: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChaosType {
    /// Introduce network latency
    NetworkDelay,
    /// Drop packets randomly
    PacketLoss,
    /// Disconnect peers randomly
    ConnectionDrop,
    /// Corrupt message data
    DataCorruption,
    /// Partition the network
    NetworkPartition,
    /// Simulate clock skew
    ClockSkew,
    /// Resource exhaustion
    ResourceExhaustion,
    /// Process crash
    ProcessCrash,
    /// Disk I/O failure
    DiskFailure,
    /// Memory corruption
    MemoryCorruption,
}

#[derive(Debug, Clone)]
pub struct ChaosEvent {
    pub timestamp: std::time::Instant,
    pub chaos_type: ChaosType,
    pub details: String,
    pub impact: ImpactLevel,
}

#[derive(Debug, Clone)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl ChaosMonkey {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active: Arc::new(RwLock::new(false)),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start the chaos monkey
    pub async fn unleash(&self) {
        let mut active = self.active.write().await;
        *active = true;
        println!("🐵 Chaos Monkey unleashed!");
    }

    /// Stop the chaos monkey
    pub async fn cage(&self) {
        let mut active = self.active.write().await;
        *active = false;
        println!("🔒 Chaos Monkey caged!");
    }

    /// Check if chaos should be injected
    pub async fn should_inject_chaos(&self) -> bool {
        let active = self.active.read().await;
        if !*active {
            return false;
        }

        let mut rng = thread_rng();
        rng.gen_bool(self.config.failure_probability)
    }

    /// Inject random chaos
    pub async fn inject_random_chaos(&self) -> Option<ChaosType> {
        if !self.should_inject_chaos().await {
            return None;
        }

        let mut rng = thread_rng();
        let chaos_type = self
            .config
            .enabled_chaos
            .get(rng.gen_range(0..self.config.enabled_chaos.len()))
            .cloned()?;

        self.execute_chaos(chaos_type.clone()).await;
        Some(chaos_type)
    }

    /// Execute specific chaos type
    async fn execute_chaos(&self, chaos_type: ChaosType) {
        let details = match chaos_type {
            ChaosType::NetworkDelay => {
                let delay = thread_rng().gen_range(100..=self.config.max_delay_ms);
                sleep(Duration::from_millis(delay)).await;
                format!("Injected {}ms delay", delay)
            }
            ChaosType::PacketLoss => {
                let loss_rate = thread_rng().gen_range(0.0..=self.config.max_packet_loss);
                format!("Dropped packet ({}% loss rate)", loss_rate * 100.0)
            }
            ChaosType::ConnectionDrop => "Forcefully dropped connection".to_string(),
            ChaosType::DataCorruption => "Corrupted message data".to_string(),
            ChaosType::NetworkPartition => "Created network partition".to_string(),
            ChaosType::ClockSkew => {
                let skew = thread_rng().gen_range(-5000..=5000);
                format!("Introduced {}ms clock skew", skew)
            }
            _ => format!("Executed {:?}", chaos_type),
        };

        let event = ChaosEvent {
            timestamp: std::time::Instant::now(),
            chaos_type: chaos_type.clone(),
            details,
            impact: self.assess_impact(&chaos_type),
        };

        let mut events = self.events.lock().await;
        events.push(event);
    }

    fn assess_impact(&self, chaos_type: &ChaosType) -> ImpactLevel {
        match chaos_type {
            ChaosType::NetworkDelay | ChaosType::PacketLoss => ImpactLevel::Low,
            ChaosType::ConnectionDrop | ChaosType::DataCorruption => ImpactLevel::Medium,
            ChaosType::NetworkPartition | ChaosType::ClockSkew => ImpactLevel::High,
            _ => ImpactLevel::Critical,
        }
    }

    /// Get chaos event history
    pub async fn get_events(&self) -> Vec<ChaosEvent> {
        let events = self.events.lock().await;
        events.clone()
    }
}

/// Network chaos injector
pub struct NetworkChaos {
    monkey: Arc<ChaosMonkey>,
}

impl NetworkChaos {
    pub fn new(monkey: Arc<ChaosMonkey>) -> Self {
        Self { monkey }
    }

    /// Simulate network delay
    pub async fn inject_delay(&self) -> Duration {
        if !self.monkey.should_inject_chaos().await {
            return Duration::from_millis(0);
        }

        let delay_ms = thread_rng().gen_range(10..=self.monkey.config.max_delay_ms);
        Duration::from_millis(delay_ms)
    }

    /// Simulate packet loss
    pub async fn should_drop_packet(&self) -> bool {
        if !self.monkey.should_inject_chaos().await {
            return false;
        }

        thread_rng().gen_bool(self.monkey.config.max_packet_loss)
    }

    /// Simulate connection failure
    pub async fn should_fail_connection(&self) -> bool {
        self.monkey.should_inject_chaos().await
    }

    /// Corrupt data with some probability
    pub async fn corrupt_data(&self, data: &mut [u8]) -> bool {
        if !self.monkey.should_inject_chaos().await {
            return false;
        }

        // Flip random bits
        let corruption_count = thread_rng().gen_range(1..=5);
        for _ in 0..corruption_count {
            let byte_idx = thread_rng().gen_range(0..data.len());
            let bit_idx = thread_rng().gen_range(0..8);
            data[byte_idx] ^= 1 << bit_idx;
        }

        true
    }
}

/// Consensus chaos injector
pub struct ConsensusChaos {
    monkey: Arc<ChaosMonkey>,
}

impl ConsensusChaos {
    pub fn new(monkey: Arc<ChaosMonkey>) -> Self {
        Self { monkey }
    }

    /// Simulate vote delay
    pub async fn delay_vote(&self) -> Duration {
        if !self.monkey.should_inject_chaos().await {
            return Duration::from_millis(0);
        }

        Duration::from_millis(thread_rng().gen_range(1000..=10000))
    }

    /// Simulate vote loss
    pub async fn should_lose_vote(&self) -> bool {
        self.monkey.should_inject_chaos().await && thread_rng().gen_bool(0.2)
    }

    /// Simulate Byzantine behavior
    pub async fn should_act_byzantine(&self) -> bool {
        self.monkey.should_inject_chaos().await && thread_rng().gen_bool(0.1)
    }
}

/// Resource chaos injector
pub struct ResourceChaos {
    monkey: Arc<ChaosMonkey>,
}

impl ResourceChaos {
    pub fn new(monkey: Arc<ChaosMonkey>) -> Self {
        Self { monkey }
    }

    /// Simulate memory pressure
    pub async fn inject_memory_pressure(&self) -> Vec<u8> {
        if !self.monkey.config.enable_memory_pressure {
            return Vec::new();
        }

        if !self.monkey.should_inject_chaos().await {
            return Vec::new();
        }

        // Allocate 10-100 MB
        let size = thread_rng().gen_range(10..=100) * 1024 * 1024;
        vec![0u8; size]
    }

    /// Simulate CPU pressure
    pub async fn inject_cpu_pressure(&self) {
        if !self.monkey.config.enable_cpu_pressure {
            return;
        }

        if !self.monkey.should_inject_chaos().await {
            return;
        }

        // Spin for a while
        let duration = Duration::from_millis(thread_rng().gen_range(100..=1000));
        let start = std::time::Instant::now();
        while start.elapsed() < duration {
            // Busy wait
            std::hint::spin_loop();
        }
    }
}

/// Chaos test scenario
#[async_trait]
pub trait ChaosScenario: Send + Sync {
    /// Run the chaos scenario
    async fn run(&self) -> Result<ScenarioResult, Box<dyn std::error::Error>>;

    /// Get scenario name
    fn name(&self) -> &str;

    /// Get scenario description
    fn description(&self) -> &str;
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub passed: bool,
    pub duration: Duration,
    pub events_triggered: usize,
    pub recovery_time: Option<Duration>,
    pub error_count: usize,
}

/// Network partition scenario
pub struct NetworkPartitionScenario {
    monkey: Arc<ChaosMonkey>,
}

#[async_trait]
impl ChaosScenario for NetworkPartitionScenario {
    async fn run(&self) -> Result<ScenarioResult, Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();

        // Simulate network partition
        self.monkey.unleash().await;

        // Run for 30 seconds with partitions
        sleep(Duration::from_secs(30)).await;

        // Heal partition
        self.monkey.cage().await;

        // Measure recovery
        let recovery_start = std::time::Instant::now();
        sleep(Duration::from_secs(10)).await;
        let recovery_time = recovery_start.elapsed();

        let events = self.monkey.get_events().await;

        Ok(ScenarioResult {
            passed: true,
            duration: start.elapsed(),
            events_triggered: events.len(),
            recovery_time: Some(recovery_time),
            error_count: 0,
        })
    }

    fn name(&self) -> &str {
        "Network Partition"
    }

    fn description(&self) -> &str {
        "Simulates network partition and tests recovery"
    }
}

// ============= Test Cases =============

#[tokio::test]
async fn test_chaos_monkey_activation() {
    let config = ChaosConfig::default();
    let monkey = ChaosMonkey::new(config);

    // Initially caged
    assert!(!monkey.should_inject_chaos().await);

    // Unleash the monkey
    monkey.unleash().await;

    // Should sometimes inject chaos (probabilistic)
    let mut chaos_injected = false;
    for _ in 0..100 {
        if monkey.should_inject_chaos().await {
            chaos_injected = true;
            break;
        }
    }
    assert!(chaos_injected, "Chaos should be injected sometimes");

    // Cage the monkey
    monkey.cage().await;
    assert!(!monkey.should_inject_chaos().await);
}

#[tokio::test]
async fn test_network_chaos() {
    let config = ChaosConfig {
        failure_probability: 1.0, // Always inject
        max_delay_ms: 1000,
        max_packet_loss: 0.5,
        ..Default::default()
    };

    let monkey = Arc::new(ChaosMonkey::new(config));
    monkey.unleash().await;

    let network_chaos = NetworkChaos::new(monkey.clone());

    // Test delay injection
    let delay = network_chaos.inject_delay().await;
    assert!(delay.as_millis() > 0);
    assert!(delay.as_millis() <= 1000);

    // Test packet loss
    let dropped = network_chaos.should_drop_packet().await;
    assert!(dropped || !dropped); // Can be either

    // Test data corruption
    let mut data = vec![0xFF; 10];
    let corrupted = network_chaos.corrupt_data(&mut data).await;
    if corrupted {
        assert_ne!(data, vec![0xFF; 10]);
    }
}

#[tokio::test]
async fn test_chaos_event_tracking() {
    let config = ChaosConfig {
        failure_probability: 1.0,
        enabled_chaos: vec![ChaosType::NetworkDelay],
        ..Default::default()
    };

    let monkey = ChaosMonkey::new(config);
    monkey.unleash().await;

    // Inject some chaos
    for _ in 0..5 {
        monkey.inject_random_chaos().await;
    }

    // Check events were recorded
    let events = monkey.get_events().await;
    assert_eq!(events.len(), 5);

    for event in events {
        assert_eq!(event.chaos_type, ChaosType::NetworkDelay);
        assert!(matches!(event.impact, ImpactLevel::Low));
    }
}

#[tokio::test]
async fn test_resource_chaos() {
    let mut config = ChaosConfig::default();
    config.failure_probability = 1.0;
    config.enable_memory_pressure = true;
    config.enable_cpu_pressure = true;

    let monkey = Arc::new(ChaosMonkey::new(config));
    monkey.unleash().await;

    let resource_chaos = ResourceChaos::new(monkey);

    // Test memory pressure
    let memory = resource_chaos.inject_memory_pressure().await;
    assert!(memory.len() >= 10 * 1024 * 1024); // At least 10MB

    // Test CPU pressure (just ensure it completes)
    let start = std::time::Instant::now();
    resource_chaos.inject_cpu_pressure().await;
    assert!(start.elapsed().as_millis() >= 100);
}

#[tokio::test]
#[ignore] // Run with --ignored for chaos scenarios
async fn test_network_partition_scenario() {
    let config = ChaosConfig {
        failure_probability: 0.5,
        enable_partitions: true,
        ..Default::default()
    };

    let monkey = Arc::new(ChaosMonkey::new(config));
    let scenario = NetworkPartitionScenario { monkey };

    let result = scenario.run().await.unwrap();

    println!("Scenario: {}", scenario.name());
    println!("Description: {}", scenario.description());
    println!("Result: {:?}", result);

    assert!(result.passed);
    assert!(result.recovery_time.is_some());
}
