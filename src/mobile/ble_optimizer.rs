//! Adaptive BLE scanning with duty cycling for battery optimization
//! 
//! This module implements intelligent BLE scanning strategies that adapt to:
//! - Battery level and charging state
//! - Network activity and peer discovery needs
//! - Power state (active, power saver, standby, critical)
//! - Thermal conditions
//! 
//! Target: Reduce continuous scanning battery drain while maintaining connectivity

use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, SystemTime};
use std::collections::{HashMap, VecDeque};
use tokio::sync::{RwLock, Mutex};
use serde::{Deserialize, Serialize};

use super::performance::{PowerState, ThermalState, BleScanConfig};

/// BLE scanning duty cycle strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanStrategy {
    /// Continuous scanning for maximum discovery
    Continuous,
    /// Standard duty cycling (20% on, 80% off)
    Standard,
    /// Aggressive power saving (10% on, 90% off)  
    PowerSaver,
    /// Minimal scanning for critical battery (5% on, 95% off)
    Critical,
    /// Adaptive based on activity and conditions
    Adaptive,
    /// Completely disabled
    Disabled,
}

/// BLE scan request priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanPriority {
    /// Background scanning
    Low = 0,
    /// Standard network maintenance
    Normal = 1,
    /// User-initiated action requiring connectivity
    High = 2,
    /// Critical system function
    Critical = 3,
}

/// Scan request with context
#[derive(Debug, Clone)]
pub struct ScanRequest {
    /// Request ID for tracking
    pub id: u64,
    /// Priority level
    pub priority: ScanPriority,
    /// Requested by component
    pub requester: String,
    /// Duration for this scan
    pub duration: Duration,
    /// Timestamp of request
    pub timestamp: SystemTime,
    /// Callback for completion notification
    pub callback: Option<String>,
}

/// BLE scanning statistics
#[derive(Debug, Clone)]
pub struct BleScanStats {
    /// Total scan attempts
    pub total_scans: u64,
    /// Successful device discoveries
    pub devices_discovered: u64,
    /// Successful connections established
    pub connections_established: u64,
    /// Total scanning time (milliseconds)
    pub total_scan_time_ms: u64,
    /// Total idle time (milliseconds)
    pub total_idle_time_ms: u64,
    /// Current duty cycle (0.0 - 1.0)
    pub current_duty_cycle: f64,
    /// Battery savings percentage
    pub estimated_battery_savings: f64,
    /// Average scan efficiency (connections/scans)
    pub scan_efficiency: f64,
}

/// Adaptive BLE scanner with duty cycling
pub struct AdaptiveBleScanner {
    /// Configuration
    config: BleScanConfig,
    
    /// Current scanning state
    state: Arc<RwLock<ScannerState>>,
    
    /// Scan request queue
    request_queue: Arc<Mutex<VecDeque<ScanRequest>>>,
    
    /// Statistics
    stats: Arc<RwLock<BleScanStats>>,
    
    /// Power state
    power_state: Arc<RwLock<PowerState>>,
    
    /// Thermal state
    thermal_state: Arc<RwLock<ThermalState>>,
    
    /// Current scan strategy
    strategy: Arc<RwLock<ScanStrategy>>,
    
    /// Activity history for adaptive algorithms
    activity_history: Arc<RwLock<ActivityHistory>>,
    
    /// Control flags
    is_running: Arc<AtomicBool>,
    force_scan: Arc<AtomicBool>,
    
    /// Scan task handle
    scan_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    
    /// Next request ID
    next_request_id: Arc<AtomicU64>,
}

/// Internal scanner state
#[derive(Debug, Clone)]
struct ScannerState {
    /// Currently scanning
    is_scanning: bool,
    /// Current scan started at
    scan_start_time: Option<SystemTime>,
    /// Last scan ended at
    last_scan_end: Option<SystemTime>,
    /// Current scan duration
    current_scan_duration: Duration,
    /// Current idle duration
    current_idle_duration: Duration,
    /// Active scan request
    active_request: Option<ScanRequest>,
    /// Peer discovery attempts in current session
    discovery_attempts: u32,
    /// Successful discoveries in current session
    successful_discoveries: u32,
}

/// Activity history for adaptive scanning
#[derive(Debug, Clone)]
struct ActivityHistory {
    /// Recent scan results (last 50 scans)
    recent_scans: VecDeque<ScanResult>,
    /// Discovery rate over time periods
    discovery_rates: HashMap<Duration, f64>,
    /// Connection success rates
    connection_rates: HashMap<Duration, f64>,
    /// Battery impact measurements
    battery_impact: VecDeque<BatteryImpactMeasurement>,
}

/// Individual scan result for history tracking
#[derive(Debug, Clone)]
struct ScanResult {
    /// Scan timestamp
    timestamp: SystemTime,
    /// Scan duration
    duration: Duration,
    /// Devices discovered
    devices_found: u32,
    /// Connections established
    connections_made: u32,
    /// Power state during scan
    power_state: PowerState,
    /// Thermal state during scan
    thermal_state: ThermalState,
    /// Battery level at scan start
    battery_level: Option<f64>,
}

/// Battery impact measurement
#[derive(Debug, Clone)]
struct BatteryImpactMeasurement {
    /// Measurement timestamp
    timestamp: SystemTime,
    /// Battery level before scanning
    battery_before: f64,
    /// Battery level after scanning
    battery_after: f64,
    /// Scan duration
    scan_duration: Duration,
    /// Estimated power consumption in watts
    estimated_power_watts: f64,
}

impl AdaptiveBleScanner {
    /// Create new adaptive BLE scanner
    pub fn new(config: BleScanConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ScannerState::default())),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(BleScanStats::default())),
            power_state: Arc::new(RwLock::new(PowerState::Active)),
            thermal_state: Arc::new(RwLock::new(ThermalState::Normal)),
            strategy: Arc::new(RwLock::new(ScanStrategy::Adaptive)),
            activity_history: Arc::new(RwLock::new(ActivityHistory::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            force_scan: Arc::new(AtomicBool::new(false)),
            scan_task: Arc::new(Mutex::new(None)),
            next_request_id: Arc::new(AtomicU64::new(1)),
        }
    }
    
    /// Start the adaptive scanner
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running.swap(true, Ordering::Relaxed) {
            return Ok(()) // Already running
        }
        
        log::info!("Starting adaptive BLE scanner with duty cycling");
        
        // Start the main scanning loop
        self.start_scanning_loop().await;
        
        // Start statistics collection
        self.start_stats_collection().await;
        
        log::info!("Adaptive BLE scanner started successfully");
        Ok(())
    }
    
    /// Stop the adaptive scanner
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_running.swap(false, Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }
        
        log::info!("Stopping adaptive BLE scanner");
        
        // Stop scanning task
        if let Some(task) = self.scan_task.lock().await.take() {
            task.abort();
        }
        
        // Stop any active scan
        let mut state = self.state.write().await;
        state.is_scanning = false;
        state.active_request = None;
        
        log::info!("Adaptive BLE scanner stopped");
        Ok(())
    }
    
    /// Request BLE scan with specified priority
    pub async fn request_scan_with_priority(
        &self,
        priority: ScanPriority,
        requester: &str,
        duration: Option<Duration>,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        
        let default_duration = self.get_default_scan_duration().await;
        let request = ScanRequest {
            id: request_id,
            priority,
            requester: requester.to_string(),
            duration: duration.unwrap_or(default_duration),
            timestamp: SystemTime::now(),
            callback: None,
        };
        
        // Add to priority queue
        self.request_queue.lock().await.push_back(request);
        
        // Sort queue by priority (highest first)
        let mut queue = self.request_queue.lock().await;
        let mut items: Vec<_> = queue.drain(..).collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        queue.extend(items);
        
        log::debug!("BLE scan requested: id={}, priority={:?}, requester={}", 
                   request_id, priority, requester);
        
        Ok(request_id)
    }
    
    /// Request standard BLE scan
    pub async fn request_scan(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.request_scan_with_priority(ScanPriority::Normal, "system", None).await?;
        Ok(())
    }
    
    /// Force immediate scan (bypasses duty cycling)
    pub async fn force_immediate_scan(&self, duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Forcing immediate BLE scan for {:?}", duration);
        
        // Set force flag
        self.force_scan.store(true, Ordering::Relaxed);
        
        // Add high-priority request
        self.request_scan_with_priority(ScanPriority::Critical, "force", Some(duration)).await?;
        
        Ok(())
    }
    
    /// Update power state
    pub async fn set_power_state(&self, state: PowerState) -> Result<(), Box<dyn std::error::Error>> {
        let old_state = *self.power_state.read().await;
        *self.power_state.write().await = state;
        
        if old_state != state {
            log::info!("BLE scanner power state: {:?} -> {:?}", old_state, state);
            
            // Update scan strategy based on power state
            let new_strategy = match state {
                PowerState::Active => ScanStrategy::Adaptive,
                PowerState::PowerSaver => ScanStrategy::PowerSaver,
                PowerState::Standby => ScanStrategy::Critical,
                PowerState::Critical => ScanStrategy::Critical,
                PowerState::Charging => ScanStrategy::Standard,
            };
            
            *self.strategy.write().await = new_strategy;
            log::info!("BLE scan strategy updated to: {:?}", new_strategy);
        }
        
        Ok(())
    }
    
    /// Update thermal state
    pub async fn set_thermal_state(&self, state: ThermalState) -> Result<(), Box<dyn std::error::Error>> {
        *self.thermal_state.write().await = state;
        
        // Adjust scanning aggressiveness based on thermal state
        match state {
            ThermalState::Normal => {},
            ThermalState::Warm => {
                // Slightly reduce scanning frequency
                log::info!("Reducing BLE scan frequency due to warm thermal state");
            },
            ThermalState::Hot => {
                // Significantly reduce scanning
                *self.strategy.write().await = ScanStrategy::PowerSaver;
                log::info!("Switching to power saver BLE scanning due to hot thermal state");
            },
            ThermalState::Critical => {
                // Minimal scanning only
                *self.strategy.write().await = ScanStrategy::Critical;
                log::info!("Switching to critical BLE scanning due to thermal throttling");
            },
        }
        
        Ok(())
    }
    
    /// Get current scanning statistics
    pub async fn get_stats(&self) -> BleScanStats {
        self.stats.read().await.clone()
    }
    
    /// Get current scanning state
    pub async fn get_state(&self) -> ScannerState {
        self.state.read().await.clone()
    }
    
    /// Get default scan duration based on current strategy
    async fn get_default_scan_duration(&self) -> Duration {
        match *self.strategy.read().await {
            ScanStrategy::Continuous => Duration::from_secs(30),
            ScanStrategy::Standard => Duration::from_millis(self.config.active_duration_ms),
            ScanStrategy::PowerSaver => Duration::from_millis(self.config.active_duration_ms / 2),
            ScanStrategy::Critical => Duration::from_millis(self.config.active_duration_ms / 4),
            ScanStrategy::Adaptive => self.calculate_adaptive_duration().await,
            ScanStrategy::Disabled => Duration::from_millis(0),
        }
    }
    
    /// Calculate adaptive scan duration based on history
    async fn calculate_adaptive_duration(&self) -> Duration {
        let history = self.activity_history.read().await;
        
        // Base duration
        let base_duration = Duration::from_millis(self.config.active_duration_ms);
        
        // Adjust based on recent success rate
        let recent_success_rate = if history.recent_scans.len() >= 5 {
            let recent_discoveries: u32 = history.recent_scans.iter()
                .rev()
                .take(5)
                .map(|s| s.devices_found)
                .sum();
            let recent_scans = 5;
            recent_discoveries as f64 / recent_scans as f64
        } else {
            0.5 // Default moderate success rate
        };
        
        // Increase duration if success rate is high, decrease if low
        let adjustment_factor = if recent_success_rate > 2.0 {
            1.5 // High discovery rate, scan longer
        } else if recent_success_rate < 0.5 {
            0.7 // Low discovery rate, scan shorter to save battery
        } else {
            1.0 // Normal rate
        };
        
        Duration::from_millis((base_duration.as_millis() as f64 * adjustment_factor) as u64)
    }
    
    /// Calculate idle duration based on current strategy
    async fn calculate_idle_duration(&self, last_scan_success: bool) -> Duration {
        let base_idle = match *self.strategy.read().await {
            ScanStrategy::Continuous => Duration::from_millis(100), // Minimal idle
            ScanStrategy::Standard => Duration::from_millis(self.config.idle_duration_ms),
            ScanStrategy::PowerSaver => Duration::from_millis(self.config.idle_duration_ms * 2),
            ScanStrategy::Critical => Duration::from_millis(self.config.idle_duration_ms * 4),
            ScanStrategy::Adaptive => self.calculate_adaptive_idle().await,
            ScanStrategy::Disabled => Duration::from_secs(3600), // 1 hour
        };
        
        // Reduce idle time if last scan was successful
        if last_scan_success {
            Duration::from_millis((base_idle.as_millis() as f64 * 0.7) as u64)
        } else {
            base_idle
        }
    }
    
    /// Calculate adaptive idle duration
    async fn calculate_adaptive_idle(&self) -> Duration {
        let thermal_state = *self.thermal_state.read().await;
        let power_state = *self.power_state.read().await;
        
        let base_idle = Duration::from_millis(self.config.idle_duration_ms);
        
        // Adjust for thermal state
        let thermal_factor = match thermal_state {
            ThermalState::Normal => 1.0,
            ThermalState::Warm => 1.5,
            ThermalState::Hot => 2.0,
            ThermalState::Critical => 4.0,
        };
        
        // Adjust for power state
        let power_factor = match power_state {
            PowerState::Active => 1.0,
            PowerState::PowerSaver => 2.0,
            PowerState::Standby => 3.0,
            PowerState::Critical => 5.0,
            PowerState::Charging => 0.8, // More aggressive when charging
        };
        
        let total_factor = thermal_factor * power_factor;
        Duration::from_millis((base_idle.as_millis() as f64 * total_factor) as u64)
    }
    
    /// Start the main scanning loop
    async fn start_scanning_loop(&self) {
        let state = self.state.clone();
        let request_queue = self.request_queue.clone();
        let stats = self.stats.clone();
        let strategy = self.strategy.clone();
        let activity_history = self.activity_history.clone();
        let is_running = self.is_running.clone();
        let force_scan = self.force_scan.clone();
        
        let task = tokio::spawn(async move {
            let mut last_scan_success = false;
            
            while is_running.load(Ordering::Relaxed) {
                // Check for pending scan requests
                let next_request = {
                    let mut queue = request_queue.lock().await;
                    queue.pop_front()
                };
                
                if let Some(request) = next_request {
                    // Execute scan request
                    let scan_result = Self::execute_scan_request(&state, &stats, request).await;
                    last_scan_success = scan_result.devices_found > 0;
                    
                    // Record scan result in history
                    activity_history.write().await.add_scan_result(scan_result);
                } else if force_scan.load(Ordering::Relaxed) {
                    // Force scan requested
                    force_scan.store(false, Ordering::Relaxed);
                    
                    let force_request = ScanRequest {
                        id: 0,
                        priority: ScanPriority::Critical,
                        requester: "force".to_string(),
                        duration: Duration::from_millis(2000),
                        timestamp: SystemTime::now(),
                        callback: None,
                    };
                    
                    let scan_result = Self::execute_scan_request(&state, &stats, force_request).await;
                    last_scan_success = scan_result.devices_found > 0;
                } else {
                    // Regular duty cycle scan
                    let current_strategy = *strategy.read().await;
                    
                    if current_strategy != ScanStrategy::Disabled {
                        // Determine if we should scan based on duty cycle
                        if Self::should_scan_now(&state, current_strategy).await {
                            let scan_duration = match current_strategy {
                                ScanStrategy::Continuous => Duration::from_secs(10),
                                ScanStrategy::Standard => Duration::from_millis(1000),
                                ScanStrategy::PowerSaver => Duration::from_millis(500),
                                ScanStrategy::Critical => Duration::from_millis(250),
                                ScanStrategy::Adaptive => Duration::from_millis(750),
                                ScanStrategy::Disabled => Duration::from_millis(0),
                            };
                            
                            let duty_request = ScanRequest {
                                id: 0,
                                priority: ScanPriority::Low,
                                requester: "duty_cycle".to_string(),
                                duration: scan_duration,
                                timestamp: SystemTime::now(),
                                callback: None,
                            };
                            
                            let scan_result = Self::execute_scan_request(&state, &stats, duty_request).await;
                            last_scan_success = scan_result.devices_found > 0;
                        }
                    }
                }
                
                // Calculate and wait for idle period
                let idle_duration = Self::calculate_idle_duration_static(
                    *strategy.read().await, 
                    last_scan_success
                ).await;
                
                if idle_duration > Duration::from_millis(100) {
                    tokio::time::sleep(idle_duration).await;
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });
        
        *self.scan_task.lock().await = Some(task);
    }
    
    /// Execute a scan request
    async fn execute_scan_request(
        state: &Arc<RwLock<ScannerState>>,
        stats: &Arc<RwLock<BleScanStats>>,
        request: ScanRequest,
    ) -> ScanResult {
        let scan_start = SystemTime::now();
        
        // Update state
        {
            let mut state_guard = state.write().await;
            state_guard.is_scanning = true;
            state_guard.scan_start_time = Some(scan_start);
            state_guard.current_scan_duration = request.duration;
            state_guard.active_request = Some(request.clone());
            state_guard.discovery_attempts += 1;
        }
        
        log::debug!("Executing BLE scan: priority={:?}, duration={:?}, requester={}", 
                   request.priority, request.duration, request.requester);
        
        // Simulate BLE scanning (in real implementation, this would call actual BLE APIs)
        tokio::time::sleep(request.duration).await;
        
        // Simulate discovery results (random for demonstration)
        let devices_found = if rand::random::<f64>() < 0.3 {
            rand::random::<u32>() % 3 + 1 // 1-3 devices
        } else {
            0 // No devices found
        };
        
        let connections_made = if devices_found > 0 && rand::random::<f64>() < 0.6 {
            1 // Usually only connect to 1 device per scan
        } else {
            0
        };
        
        let scan_end = SystemTime::now();
        let actual_duration = scan_end.duration_since(scan_start).unwrap_or_default();
        
        // Update state and statistics
        {
            let mut state_guard = state.write().await;
            state_guard.is_scanning = false;
            state_guard.scan_start_time = None;
            state_guard.last_scan_end = Some(scan_end);
            state_guard.active_request = None;
            if devices_found > 0 {
                state_guard.successful_discoveries += 1;
            }
        }
        
        // Update statistics
        {
            let mut stats_guard = stats.write().await;
            stats_guard.total_scans += 1;
            stats_guard.devices_discovered += devices_found as u64;
            stats_guard.connections_established += connections_made as u64;
            stats_guard.total_scan_time_ms += actual_duration.as_millis() as u64;
            
            // Recalculate efficiency
            if stats_guard.total_scans > 0 {
                stats_guard.scan_efficiency = 
                    stats_guard.connections_established as f64 / stats_guard.total_scans as f64;
            }
        }
        
        log::debug!("BLE scan completed: devices_found={}, connections_made={}, duration={:?}", 
                   devices_found, connections_made, actual_duration);
        
        ScanResult {
            timestamp: scan_start,
            duration: actual_duration,
            devices_found,
            connections_made,
            power_state: PowerState::Active, // Would be read from actual power state
            thermal_state: ThermalState::Normal, // Would be read from thermal monitor
            battery_level: None, // Would be read from battery monitor
        }
    }
    
    /// Determine if we should scan now based on duty cycle
    async fn should_scan_now(
        state: &Arc<RwLock<ScannerState>>,
        strategy: ScanStrategy,
    ) -> bool {
        let state_guard = state.read().await;
        
        if state_guard.is_scanning {
            return false; // Already scanning
        }
        
        // Check if enough idle time has passed
        if let Some(last_end) = state_guard.last_scan_end {
            let idle_time = last_end.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO);
            let min_idle = match strategy {
                ScanStrategy::Continuous => Duration::from_millis(100),
                ScanStrategy::Standard => Duration::from_millis(4000), // 4 seconds
                ScanStrategy::PowerSaver => Duration::from_millis(9000), // 9 seconds  
                ScanStrategy::Critical => Duration::from_millis(19000), // 19 seconds
                ScanStrategy::Adaptive => Duration::from_millis(3000), // 3 seconds
                ScanStrategy::Disabled => Duration::from_secs(3600), // Never
            };
            
            idle_time >= min_idle
        } else {
            true // First scan
        }
    }
    
    /// Calculate idle duration (static version for use in async context)
    async fn calculate_idle_duration_static(
        strategy: ScanStrategy,
        last_scan_success: bool,
    ) -> Duration {
        let base_idle = match strategy {
            ScanStrategy::Continuous => Duration::from_millis(100),
            ScanStrategy::Standard => Duration::from_millis(4000),
            ScanStrategy::PowerSaver => Duration::from_millis(8000),
            ScanStrategy::Critical => Duration::from_millis(16000),
            ScanStrategy::Adaptive => Duration::from_millis(3000),
            ScanStrategy::Disabled => Duration::from_secs(3600),
        };
        
        if last_scan_success {
            Duration::from_millis((base_idle.as_millis() as f64 * 0.7) as u64)
        } else {
            base_idle
        }
    }
    
    /// Start statistics collection task
    async fn start_stats_collection(&self) {
        // Implementation would start a background task to collect and report statistics
        // For now, this is a placeholder
        log::debug!("BLE scanner statistics collection started");
    }
}

impl Default for ScannerState {
    fn default() -> Self {
        Self {
            is_scanning: false,
            scan_start_time: None,
            last_scan_end: None,
            current_scan_duration: Duration::from_millis(1000),
            current_idle_duration: Duration::from_millis(4000),
            active_request: None,
            discovery_attempts: 0,
            successful_discoveries: 0,
        }
    }
}

impl Default for BleScanStats {
    fn default() -> Self {
        Self {
            total_scans: 0,
            devices_discovered: 0,
            connections_established: 0,
            total_scan_time_ms: 0,
            total_idle_time_ms: 0,
            current_duty_cycle: 0.2, // 20% default
            estimated_battery_savings: 0.0,
            scan_efficiency: 0.0,
        }
    }
}

impl ActivityHistory {
    fn new() -> Self {
        Self {
            recent_scans: VecDeque::with_capacity(50),
            discovery_rates: HashMap::new(),
            connection_rates: HashMap::new(),
            battery_impact: VecDeque::with_capacity(100),
        }
    }
    
    fn add_scan_result(&mut self, result: ScanResult) {
        // Add to recent scans
        self.recent_scans.push_back(result.clone());
        if self.recent_scans.len() > 50 {
            self.recent_scans.pop_front();
        }
        
        // Update discovery rates for different time windows
        let windows = [
            Duration::from_secs(300),   // 5 minutes
            Duration::from_secs(1800),  // 30 minutes  
            Duration::from_secs(3600),  // 1 hour
        ];
        
        for window in &windows {
            let cutoff = SystemTime::now() - *window;
            let recent_results: Vec<_> = self.recent_scans.iter()
                .filter(|r| r.timestamp >= cutoff)
                .collect();
            
            if !recent_results.is_empty() {
                let total_discoveries: u32 = recent_results.iter()
                    .map(|r| r.devices_found)
                    .sum();
                let total_scans = recent_results.len() as u32;
                
                self.discovery_rates.insert(*window, total_discoveries as f64 / total_scans as f64);
                
                let total_connections: u32 = recent_results.iter()
                    .map(|r| r.connections_made)
                    .sum();
                
                self.connection_rates.insert(*window, total_connections as f64 / total_scans as f64);
            }
        }
    }
}