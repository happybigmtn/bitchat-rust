//! BLE Optimization for P2P Casino Gaming
//! 
//! This module provides comprehensive optimization strategies for Bluetooth Low Energy
//! constraints, including power management, adaptive protocols, and intelligent
//! connection management.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::protocol::{PeerId, BitchatPacket};
use crate::protocol::p2p_messages::{ConsensusMessage, MessagePriority};
use crate::protocol::ble_dispatch::BleMessageDispatcher;
use crate::error::{Error, Result};

/// BLE optimization configuration
#[derive(Debug, Clone)]
pub struct BleOptimizationConfig {
    /// Connection interval range
    pub min_connection_interval: Duration,
    pub max_connection_interval: Duration,
    
    /// Power management
    pub enable_power_optimization: bool,
    pub idle_timeout: Duration,
    pub sleep_mode_threshold: Duration,
    
    /// Adaptive protocols
    pub enable_adaptive_mtu: bool,
    pub enable_adaptive_interval: bool,
    pub enable_compression_adaptation: bool,
    
    /// Quality of Service
    pub latency_target: Duration,
    pub throughput_target: f64, // bytes per second
    pub reliability_target: f64, // percentage
    
    /// Battery optimization
    pub battery_level_threshold: f64, // 0.0 to 1.0
    pub low_power_mode_enabled: bool,
    
    /// Connection management
    pub max_simultaneous_connections: usize,
    pub connection_priority_levels: usize,
}

impl Default for BleOptimizationConfig {
    fn default() -> Self {
        Self {
            min_connection_interval: Duration::from_millis(20),
            max_connection_interval: Duration::from_millis(100),
            enable_power_optimization: true,
            idle_timeout: Duration::from_secs(30),
            sleep_mode_threshold: Duration::from_secs(300), // 5 minutes
            enable_adaptive_mtu: true,
            enable_adaptive_interval: true,
            enable_compression_adaptation: true,
            latency_target: Duration::from_millis(50),
            throughput_target: 50_000.0, // 50 KB/s
            reliability_target: 0.98, // 98%
            battery_level_threshold: 0.15, // 15%
            low_power_mode_enabled: true,
            max_simultaneous_connections: 4,
            connection_priority_levels: 4,
        }
    }
}

/// Power management state
#[derive(Debug, Clone, PartialEq)]
pub enum PowerState {
    /// Full power - all features enabled
    Active,
    /// Reduced power - lower connection intervals
    PowerSaver,
    /// Minimum power - essential communications only
    LowPower,
    /// Sleep mode - minimal activity
    Sleep,
}

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionQuality {
    pub peer_id: PeerId,
    pub rssi: i16, // Signal strength
    pub latency: Duration,
    pub packet_loss: f64,
    pub throughput: f64, // bytes per second
    pub reliability: f64, // success rate
    pub last_updated: Instant,
}

/// BLE optimization manager
pub struct BleOptimizer {
    config: BleOptimizationConfig,
    
    // Power management
    power_state: Arc<RwLock<PowerState>>,
    last_activity: Arc<RwLock<Instant>>,
    battery_level: Arc<RwLock<f64>>,
    
    // Connection management
    connection_qualities: Arc<RwLock<HashMap<PeerId, ConnectionQuality>>>,
    connection_priorities: Arc<RwLock<HashMap<PeerId, u8>>>,
    active_connections: Arc<RwLock<HashMap<PeerId, Instant>>>,
    
    // Adaptive parameters
    current_mtu: Arc<RwLock<usize>>,
    current_interval: Arc<RwLock<Duration>>,
    compression_level: Arc<RwLock<u8>>,
    
    // Performance tracking
    message_queue_sizes: Arc<RwLock<HashMap<MessagePriority, usize>>>,
    bandwidth_utilization: Arc<RwLock<f64>>,
    error_rates: Arc<RwLock<HashMap<PeerId, f64>>>,
    
    // Statistics
    power_transitions: Arc<RwLock<u64>>,
    adaptive_adjustments: Arc<RwLock<u64>>,
    connections_dropped: Arc<RwLock<u64>>,
}

impl BleOptimizer {
    /// Create new BLE optimizer
    pub fn new(config: BleOptimizationConfig) -> Self {
        Self {
            config,
            power_state: Arc::new(RwLock::new(PowerState::Active)),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            battery_level: Arc::new(RwLock::new(1.0)), // Full battery
            connection_qualities: Arc::new(RwLock::new(HashMap::new())),
            connection_priorities: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            current_mtu: Arc::new(RwLock::new(244)), // Default BLE MTU
            current_interval: Arc::new(RwLock::new(Duration::from_millis(30))),
            compression_level: Arc::new(RwLock::new(6)), // Default LZ4 compression
            message_queue_sizes: Arc::new(RwLock::new(HashMap::new())),
            bandwidth_utilization: Arc::new(RwLock::new(0.0)),
            error_rates: Arc::new(RwLock::new(HashMap::new())),
            power_transitions: Arc::new(RwLock::new(0)),
            adaptive_adjustments: Arc::new(RwLock::new(0)),
            connections_dropped: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Start the BLE optimizer
    pub async fn start(&self) {
        self.start_power_management_task().await;
        self.start_adaptive_optimization_task().await;
        self.start_connection_quality_monitor().await;
        self.start_performance_monitor().await;
    }
    
    /// Update battery level (0.0 to 1.0)
    pub async fn update_battery_level(&self, level: f64) {
        *self.battery_level.write().await = level.clamp(0.0, 1.0);
        
        // Trigger power state evaluation
        self.evaluate_power_state().await;
    }
    
    /// Record network activity
    pub async fn record_activity(&self) {
        *self.last_activity.write().await = Instant::now();
        
        // Update power state if we were sleeping
        let current_state = self.power_state.read().await.clone();
        if current_state == PowerState::Sleep {
            self.set_power_state(PowerState::Active).await;
        }
    }
    
    /// Update connection quality metrics
    pub async fn update_connection_quality(
        &self,
        peer_id: PeerId,
        rssi: i16,
        latency: Duration,
        success_rate: f64,
    ) {
        let quality = ConnectionQuality {
            peer_id,
            rssi,
            latency,
            packet_loss: 1.0 - success_rate,
            throughput: 0.0, // Would be calculated from actual data
            reliability: success_rate,
            last_updated: Instant::now(),
        };
        
        self.connection_qualities.write().await.insert(peer_id, quality);
        
        // Update connection priority based on quality
        self.update_connection_priority(peer_id, rssi, latency, success_rate).await;
        
        // Trigger adaptive optimization if needed
        if success_rate < self.config.reliability_target {
            self.trigger_adaptive_optimization().await;
        }
    }
    
    /// Optimize message transmission based on current state
    pub async fn optimize_message(&self, message: &mut ConsensusMessage) -> Result<()> {
        let power_state = self.power_state.read().await.clone();
        let current_compression = *self.compression_level.read().await;
        
        match power_state {
            PowerState::Active => {
                // Full optimization
                if message.payload_size() >= 128 {
                    message.compress()?;
                }
            }
            PowerState::PowerSaver => {
                // Aggressive compression
                if message.payload_size() >= 64 {
                    message.compress()?;
                }
            }
            PowerState::LowPower => {
                // Maximum compression, drop non-critical messages
                if message.payload.priority() == MessagePriority::Low {
                    return Err(Error::Network("Message dropped due to low power mode".to_string()));
                }
                message.compress()?;
            }
            PowerState::Sleep => {
                // Only critical messages
                if message.payload.priority() != MessagePriority::Critical {
                    return Err(Error::Network("Message dropped due to sleep mode".to_string()));
                }
                message.compress()?;
            }
        }
        
        Ok(())
    }
    
    /// Get optimal connection parameters for a peer
    pub async fn get_connection_params(&self, peer_id: PeerId) -> ConnectionParams {
        let qualities = self.connection_qualities.read().await;
        let power_state = self.power_state.read().await.clone();
        let battery_level = *self.battery_level.read().await;
        
        let base_interval = match power_state {
            PowerState::Active => Duration::from_millis(20),
            PowerState::PowerSaver => Duration::from_millis(50),
            PowerState::LowPower => Duration::from_millis(100),
            PowerState::Sleep => Duration::from_millis(200),
        };
        
        let base_mtu = match power_state {
            PowerState::Active => 244,
            PowerState::PowerSaver => 200,
            PowerState::LowPower => 150,
            PowerState::Sleep => 100,
        };
        
        // Adjust based on connection quality
        let (interval, mtu) = if let Some(quality) = qualities.get(&peer_id) {
            let quality_factor = quality.reliability;
            let adjusted_interval = if quality_factor < 0.9 {
                base_interval / 2 // Faster retries for poor connections
            } else {
                base_interval
            };
            
            let adjusted_mtu = if quality.rssi < -70 {
                (base_mtu as f64 * 0.8) as usize // Smaller MTU for weak signal
            } else {
                base_mtu
            };
            
            (adjusted_interval, adjusted_mtu)
        } else {
            (base_interval, base_mtu)
        };
        
        ConnectionParams {
            connection_interval: interval,
            mtu_size: mtu,
            enable_compression: power_state != PowerState::Active,
            priority_boost: battery_level < self.config.battery_level_threshold,
        }
    }
    
    /// Prioritize connections based on quality and importance
    pub async fn prioritize_connections(&self) -> Vec<PeerId> {
        let qualities = self.connection_qualities.read().await;
        let priorities = self.connection_priorities.read().await;
        
        let mut peers: Vec<_> = qualities.keys().copied().collect();
        
        // Sort by priority first, then by quality
        peers.sort_by(|&a, &b| {
            let priority_a = priorities.get(&a).unwrap_or(&0);
            let priority_b = priorities.get(&b).unwrap_or(&0);
            
            match priority_b.cmp(priority_a) {
                std::cmp::Ordering::Equal => {
                    // Same priority - sort by connection quality
                    let quality_a = qualities.get(&a).map(|q| q.reliability).unwrap_or(0.0);
                    let quality_b = qualities.get(&b).map(|q| q.reliability).unwrap_or(0.0);
                    quality_b.partial_cmp(&quality_a).unwrap_or(std::cmp::Ordering::Equal)
                }
                other => other,
            }
        });
        
        // Limit to max simultaneous connections
        peers.truncate(self.config.max_simultaneous_connections);
        
        peers
    }
    
    /// Start power management task
    async fn start_power_management_task(&self) {
        let power_state = self.power_state.clone();
        let last_activity = self.last_activity.clone();
        let battery_level = self.battery_level.clone();
        let config = self.config.clone();
        let power_transitions = self.power_transitions.clone();
        
        tokio::spawn(async move {
            let mut power_check_interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                power_check_interval.tick().await;
                
                let current_state = power_state.read().await.clone();
                let idle_time = last_activity.read().await.elapsed();
                let battery = *battery_level.read().await;
                
                let new_state = if battery < config.battery_level_threshold && config.low_power_mode_enabled {
                    PowerState::LowPower
                } else if idle_time > config.sleep_mode_threshold {
                    PowerState::Sleep
                } else if idle_time > config.idle_timeout {
                    PowerState::PowerSaver
                } else {
                    PowerState::Active
                };
                
                if new_state != current_state {
                    log::info!("Power state transition: {:?} -> {:?}", current_state, new_state);
                    *power_state.write().await = new_state;
                    *power_transitions.write().await += 1;
                }
            }
        });
    }
    
    /// Start adaptive optimization task
    async fn start_adaptive_optimization_task(&self) {
        let current_mtu = self.current_mtu.clone();
        let current_interval = self.current_interval.clone();
        let connection_qualities = self.connection_qualities.clone();
        let config = self.config.clone();
        let adaptive_adjustments = self.adaptive_adjustments.clone();
        
        tokio::spawn(async move {
            let mut optimization_interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                optimization_interval.tick().await;
                
                if !config.enable_adaptive_mtu && !config.enable_adaptive_interval {
                    continue;
                }
                
                let qualities = connection_qualities.read().await;
                
                // Calculate average connection quality
                let avg_latency = if qualities.is_empty() {
                    config.latency_target
                } else {
                    let total_latency: Duration = qualities.values().map(|q| q.latency).sum();
                    total_latency / qualities.len() as u32
                };
                
                let avg_reliability = if qualities.is_empty() {
                    1.0
                } else {
                    qualities.values().map(|q| q.reliability).sum::<f64>() / qualities.len() as f64
                };
                
                let mut adjustments_made = false;
                
                // Adaptive MTU adjustment
                if config.enable_adaptive_mtu {
                    let mut mtu = current_mtu.write().await;
                    
                    if avg_reliability > 0.98 && avg_latency < config.latency_target {
                        // Good conditions - increase MTU
                        if *mtu < 244 {
                            *mtu = (*mtu + 20).min(244);
                            adjustments_made = true;
                        }
                    } else if avg_reliability < 0.95 || avg_latency > config.latency_target * 2 {
                        // Poor conditions - decrease MTU
                        if *mtu > 100 {
                            *mtu = (*mtu - 20).max(100);
                            adjustments_made = true;
                        }
                    }
                }
                
                // Adaptive interval adjustment
                if config.enable_adaptive_interval {
                    let mut interval = current_interval.write().await;
                    
                    if avg_reliability > 0.98 && avg_latency < config.latency_target {
                        // Good conditions - increase interval (save power)
                        if *interval < config.max_connection_interval {
                            *interval = (*interval + Duration::from_millis(10)).min(config.max_connection_interval);
                            adjustments_made = true;
                        }
                    } else if avg_reliability < 0.95 {
                        // Poor conditions - decrease interval (more frequent communication)
                        if *interval > config.min_connection_interval {
                            *interval = (*interval - Duration::from_millis(10)).max(config.min_connection_interval);
                            adjustments_made = true;
                        }
                    }
                }
                
                if adjustments_made {
                    *adaptive_adjustments.write().await += 1;
                    log::debug!("Adaptive optimization: MTU={}, Interval={:?}", 
                               *current_mtu.read().await, *current_interval.read().await);
                }
            }
        });
    }
    
    /// Start connection quality monitoring
    async fn start_connection_quality_monitor(&self) {
        let connection_qualities = self.connection_qualities.clone();
        let active_connections = self.active_connections.clone();
        let connections_dropped = self.connections_dropped.clone();
        
        tokio::spawn(async move {
            let mut monitor_interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                monitor_interval.tick().await;
                
                let mut qualities = connection_qualities.write().await;
                let mut connections = active_connections.write().await;
                
                // Remove stale connections (no updates in 2 minutes)
                let cutoff_time = Instant::now() - Duration::from_secs(120);
                
                let before_count = qualities.len();
                qualities.retain(|_, quality| quality.last_updated > cutoff_time);
                connections.retain(|_, last_seen| *last_seen > cutoff_time);
                
                let dropped_count = before_count - qualities.len();
                if dropped_count > 0 {
                    *connections_dropped.write().await += dropped_count as u64;
                    log::info!("Dropped {} stale connections", dropped_count);
                }
            }
        });
    }
    
    /// Start performance monitoring
    async fn start_performance_monitor(&self) {
        let bandwidth_utilization = self.bandwidth_utilization.clone();
        let message_queue_sizes = self.message_queue_sizes.clone();
        
        tokio::spawn(async move {
            let mut monitor_interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                monitor_interval.tick().await;
                
                // Monitor and log performance metrics
                let bandwidth = *bandwidth_utilization.read().await;
                let queues = message_queue_sizes.read().await;
                
                if bandwidth > 80.0 || queues.values().any(|&size| size > 100) {
                    log::warn!("Performance alert: Bandwidth={}%, Queue sizes={:?}", 
                              bandwidth, *queues);
                }
            }
        });
    }
    
    /// Evaluate and update power state
    async fn evaluate_power_state(&self) {
        let battery_level = *self.battery_level.read().await;
        let idle_time = self.last_activity.read().await.elapsed();
        
        let new_state = if battery_level < self.config.battery_level_threshold {
            PowerState::LowPower
        } else if idle_time > self.config.sleep_mode_threshold {
            PowerState::Sleep
        } else if idle_time > self.config.idle_timeout {
            PowerState::PowerSaver
        } else {
            PowerState::Active
        };
        
        self.set_power_state(new_state).await;
    }
    
    /// Set power state
    async fn set_power_state(&self, new_state: PowerState) {
        let mut power_state = self.power_state.write().await;
        if *power_state != new_state {
            log::info!("Power state changed to {:?}", new_state);
            *power_state = new_state;
            *self.power_transitions.write().await += 1;
        }
    }
    
    /// Update connection priority
    async fn update_connection_priority(
        &self,
        peer_id: PeerId,
        rssi: i16,
        latency: Duration,
        success_rate: f64,
    ) {
        // Calculate priority based on connection quality
        let quality_score = (success_rate * 100.0) + 
                           ((-rssi as f64 + 100.0) / 10.0) + 
                           (1000.0 / latency.as_millis() as f64);
        
        let priority = (quality_score / 20.0).clamp(0.0, 255.0) as u8;
        
        self.connection_priorities.write().await.insert(peer_id, priority);
    }
    
    /// Trigger adaptive optimization
    async fn trigger_adaptive_optimization(&self) {
        // This would trigger immediate optimization checks
        log::debug!("Triggering adaptive optimization due to poor connection quality");
    }
    
    /// Get current optimization statistics
    pub async fn get_optimization_stats(&self) -> BleOptimizationStats {
        BleOptimizationStats {
            power_state: self.power_state.read().await.clone(),
            battery_level: *self.battery_level.read().await,
            active_connections: self.connection_qualities.read().await.len(),
            current_mtu: *self.current_mtu.read().await,
            current_interval: *self.current_interval.read().await,
            bandwidth_utilization: *self.bandwidth_utilization.read().await,
            power_transitions: *self.power_transitions.read().await,
            adaptive_adjustments: *self.adaptive_adjustments.read().await,
            connections_dropped: *self.connections_dropped.read().await,
            average_latency: self.calculate_average_latency().await,
            average_reliability: self.calculate_average_reliability().await,
        }
    }
    
    /// Calculate average latency
    async fn calculate_average_latency(&self) -> Duration {
        let qualities = self.connection_qualities.read().await;
        if qualities.is_empty() {
            Duration::from_millis(50)
        } else {
            let total: Duration = qualities.values().map(|q| q.latency).sum();
            total / qualities.len() as u32
        }
    }
    
    /// Calculate average reliability
    async fn calculate_average_reliability(&self) -> f64 {
        let qualities = self.connection_qualities.read().await;
        if qualities.is_empty() {
            1.0
        } else {
            qualities.values().map(|q| q.reliability).sum::<f64>() / qualities.len() as f64
        }
    }
}

/// Connection parameters optimized for BLE
#[derive(Debug, Clone)]
pub struct ConnectionParams {
    pub connection_interval: Duration,
    pub mtu_size: usize,
    pub enable_compression: bool,
    pub priority_boost: bool,
}

/// BLE optimization statistics
#[derive(Debug, Clone)]
pub struct BleOptimizationStats {
    pub power_state: PowerState,
    pub battery_level: f64,
    pub active_connections: usize,
    pub current_mtu: usize,
    pub current_interval: Duration,
    pub bandwidth_utilization: f64,
    pub power_transitions: u64,
    pub adaptive_adjustments: u64,
    pub connections_dropped: u64,
    pub average_latency: Duration,
    pub average_reliability: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_power_management() {
        let config = BleOptimizationConfig::default();
        let optimizer = BleOptimizer::new(config);
        
        // Start with full battery and active state
        assert_eq!(*optimizer.power_state.read().await, PowerState::Active);
        
        // Simulate low battery
        optimizer.update_battery_level(0.1).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Should enter low power mode
        assert_eq!(*optimizer.power_state.read().await, PowerState::LowPower);
    }
    
    #[tokio::test]
    async fn test_connection_prioritization() {
        let config = BleOptimizationConfig::default();
        let optimizer = BleOptimizer::new(config);
        
        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];
        
        // Add connections with different qualities
        optimizer.update_connection_quality(peer1, -30, Duration::from_millis(20), 0.99).await;
        optimizer.update_connection_quality(peer2, -80, Duration::from_millis(100), 0.85).await;
        
        let prioritized = optimizer.prioritize_connections().await;
        
        // peer1 should be prioritized due to better quality
        assert_eq!(prioritized[0], peer1);
    }
    
    #[tokio::test]
    async fn test_message_optimization() {
        let config = BleOptimizationConfig::default();
        let optimizer = BleOptimizer::new(config);
        
        // Set to low power mode
        optimizer.set_power_state(PowerState::LowPower).await;
        
        // Create a low priority message
        let mut message = ConsensusMessage::new(
            [1u8; 32],
            [0u8; 16],
            1,
            crate::protocol::p2p_messages::ConsensusPayload::Heartbeat {
                alive_participants: vec![],
                network_view: crate::protocol::p2p_messages::NetworkView {
                    participants: vec![],
                    connections: vec![],
                    partition_id: None,
                    leader: None,
                },
            },
        );
        
        // Should be dropped in low power mode
        let result = optimizer.optimize_message(&mut message).await;
        assert!(result.is_err());
    }
}