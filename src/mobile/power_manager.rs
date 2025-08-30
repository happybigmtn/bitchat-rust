//! Power state management system for mobile devices
//!
//! This module provides comprehensive power management to optimize battery life:
//! - Dynamic power state transitions based on battery level, thermal state, and activity
//! - Component coordination to reduce power consumption across all subsystems
//! - Battery monitoring with predictive analytics
//! - Thermal management to prevent overheating
//! - Charging state optimization
//!
//! Target: <5% battery drain per hour under normal operation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, Mutex, RwLock};

use super::performance::{PowerState, ThermalState};

/// Power management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerManagerConfig {
    /// Battery level thresholds for power state transitions
    pub battery_thresholds: BatteryThresholds,
    /// Thermal thresholds in Celsius
    pub thermal_thresholds: ThermalThresholds,
    /// Component power limits
    pub component_limits: ComponentPowerLimits,
    /// Monitoring intervals
    pub monitoring: MonitoringConfig,
    /// Power saving strategies
    pub strategies: PowerSavingStrategies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryThresholds {
    /// Switch to critical power mode
    pub critical_percent: f64,
    /// Switch to power saver mode
    pub power_saver_percent: f64,
    /// Resume normal operation (with hysteresis)
    pub normal_resume_percent: f64,
    /// Consider device as charging threshold
    pub charging_threshold_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalThresholds {
    /// Normal operating temperature (°C)
    pub normal_max_celsius: f64,
    /// Warm threshold - start minor throttling (°C)
    pub warm_threshold_celsius: f64,
    /// Hot threshold - aggressive throttling (°C)  
    pub hot_threshold_celsius: f64,
    /// Critical threshold - emergency throttling (°C)
    pub critical_threshold_celsius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPowerLimits {
    /// Maximum CPU usage percentage per power state
    pub cpu_limits: HashMap<PowerState, f64>,
    /// Maximum memory usage MB per power state
    pub memory_limits: HashMap<PowerState, f64>,
    /// BLE duty cycle per power state
    pub ble_duty_cycles: HashMap<PowerState, f64>,
    /// Network bandwidth limits per power state (bytes/sec)
    pub network_limits: HashMap<PowerState, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Battery monitoring interval
    pub battery_check_interval_secs: u64,
    /// Thermal monitoring interval
    pub thermal_check_interval_secs: u64,
    /// Power usage monitoring interval
    pub power_usage_interval_secs: u64,
    /// Prediction analysis interval
    pub prediction_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSavingStrategies {
    /// Enable adaptive frequency scaling
    pub adaptive_cpu_scaling: bool,
    /// Enable memory pressure management
    pub memory_pressure_management: bool,
    /// Enable background task suspension
    pub background_task_suspension: bool,
    /// Enable predictive power management
    pub predictive_management: bool,
}

impl Default for PowerManagerConfig {
    fn default() -> Self {
        let mut cpu_limits = HashMap::new();
        cpu_limits.insert(PowerState::Active, 80.0);
        cpu_limits.insert(PowerState::PowerSaver, 50.0);
        cpu_limits.insert(PowerState::Standby, 20.0);
        cpu_limits.insert(PowerState::Critical, 10.0);
        cpu_limits.insert(PowerState::Charging, 100.0);

        let mut memory_limits = HashMap::new();
        memory_limits.insert(PowerState::Active, 150.0);
        memory_limits.insert(PowerState::PowerSaver, 100.0);
        memory_limits.insert(PowerState::Standby, 75.0);
        memory_limits.insert(PowerState::Critical, 50.0);
        memory_limits.insert(PowerState::Charging, 200.0);

        let mut ble_duty_cycles = HashMap::new();
        ble_duty_cycles.insert(PowerState::Active, 0.2); // 20%
        ble_duty_cycles.insert(PowerState::PowerSaver, 0.1); // 10%
        ble_duty_cycles.insert(PowerState::Standby, 0.05); // 5%
        ble_duty_cycles.insert(PowerState::Critical, 0.02); // 2%
        ble_duty_cycles.insert(PowerState::Charging, 0.3); // 30%

        let mut network_limits = HashMap::new();
        network_limits.insert(PowerState::Active, 1024 * 1024); // 1 MB/s
        network_limits.insert(PowerState::PowerSaver, 512 * 1024); // 512 KB/s
        network_limits.insert(PowerState::Standby, 128 * 1024); // 128 KB/s
        network_limits.insert(PowerState::Critical, 64 * 1024); // 64 KB/s
        network_limits.insert(PowerState::Charging, 2048 * 1024); // 2 MB/s

        Self {
            battery_thresholds: BatteryThresholds {
                critical_percent: 15.0,
                power_saver_percent: 30.0,
                normal_resume_percent: 40.0,
                charging_threshold_percent: 1.0, // Any charging is significant
            },
            thermal_thresholds: ThermalThresholds {
                normal_max_celsius: 35.0,
                warm_threshold_celsius: 40.0,
                hot_threshold_celsius: 45.0,
                critical_threshold_celsius: 50.0,
            },
            component_limits: ComponentPowerLimits {
                cpu_limits,
                memory_limits,
                ble_duty_cycles,
                network_limits,
            },
            monitoring: MonitoringConfig {
                battery_check_interval_secs: 30,
                thermal_check_interval_secs: 15,
                power_usage_interval_secs: 60,
                prediction_interval_secs: 300, // 5 minutes
            },
            strategies: PowerSavingStrategies {
                adaptive_cpu_scaling: true,
                memory_pressure_management: true,
                background_task_suspension: true,
                predictive_management: true,
            },
        }
    }
}

/// Battery information with extended metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    /// Current battery level percentage (0-100)
    pub level_percent: f64,
    /// Is device currently charging
    pub is_charging: bool,
    /// Charging rate in watts (positive when charging)
    pub charging_rate_watts: f64,
    /// Estimated time to full charge (minutes)
    pub time_to_full_minutes: Option<u64>,
    /// Estimated time remaining on battery (minutes)
    pub time_remaining_minutes: Option<u64>,
    /// Current power draw in watts
    pub current_draw_watts: f64,
    /// Battery temperature in Celsius
    pub temperature_celsius: f64,
    /// Battery health percentage (0-100)
    pub health_percent: f64,
    /// Voltage (volts)
    pub voltage: f64,
    /// Last update timestamp
    pub timestamp: SystemTime,
}

/// System thermal information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalInfo {
    /// CPU temperature in Celsius
    pub cpu_temperature: f64,
    /// Battery temperature in Celsius
    pub battery_temperature: f64,
    /// Ambient temperature in Celsius (if available)
    pub ambient_temperature: Option<f64>,
    /// Current thermal state
    pub thermal_state: ThermalState,
    /// Thermal pressure (0.0 - 1.0)
    pub thermal_pressure: f64,
    /// Is thermal throttling active
    pub is_throttling: bool,
    /// Last update timestamp
    pub timestamp: SystemTime,
}

/// Power consumption by component
#[derive(Debug, Clone)]
pub struct PowerConsumption {
    /// CPU power consumption (watts)
    pub cpu_watts: f64,
    /// Display/UI power consumption (watts)
    pub display_watts: f64,
    /// Bluetooth power consumption (watts)
    pub bluetooth_watts: f64,
    /// Network power consumption (watts)
    pub network_watts: f64,
    /// Background services power consumption (watts)
    pub background_watts: f64,
    /// Other/misc power consumption (watts)
    pub other_watts: f64,
    /// Total power consumption (watts)
    pub total_watts: f64,
    /// Timestamp of measurement
    pub timestamp: SystemTime,
}

/// Power prediction data
#[derive(Debug, Clone)]
pub struct PowerPrediction {
    /// Predicted battery life remaining (minutes)
    pub predicted_life_minutes: u64,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f64,
    /// Recommended power state
    pub recommended_state: PowerState,
    /// Predicted battery level at specific time
    pub predicted_levels: Vec<(Duration, f64)>,
    /// Key factors affecting prediction
    pub factors: Vec<String>,
}

/// Power state transition event
#[derive(Debug, Clone)]
pub struct PowerStateEvent {
    /// Previous power state
    pub old_state: PowerState,
    /// New power state
    pub new_state: PowerState,
    /// Reason for transition
    pub reason: String,
    /// Timestamp of transition
    pub timestamp: SystemTime,
    /// Component limits for new state
    pub new_limits: ComponentPowerLimits,
}

/// Power management statistics
#[derive(Debug, Clone)]
pub struct PowerStats {
    /// Total time in each power state (seconds)
    pub time_in_states: HashMap<PowerState, u64>,
    /// Average battery drain per hour by state (%/hour)
    pub drain_rates: HashMap<PowerState, f64>,
    /// Number of state transitions
    pub state_transitions: u64,
    /// Power savings achieved (%)
    pub estimated_savings_percent: f64,
    /// Battery cycles avoided
    pub cycles_saved: f64,
    /// Average thermal throttling time per hour (%)
    pub thermal_throttling_percent: f64,
}

/// Main power management system
pub struct PowerManager {
    /// Configuration
    config: Arc<RwLock<PowerManagerConfig>>,

    /// Current power state
    power_state: Arc<RwLock<PowerState>>,

    /// Current battery information
    battery_info: Arc<RwLock<Option<BatteryInfo>>>,

    /// Current thermal information
    thermal_info: Arc<RwLock<Option<ThermalInfo>>>,

    /// Power consumption history
    consumption_history: Arc<RwLock<VecDeque<PowerConsumption>>>,

    /// Battery level history for predictions
    battery_history: Arc<RwLock<VecDeque<(SystemTime, f64)>>>,

    /// Power management statistics
    stats: Arc<RwLock<PowerStats>>,

    /// Event broadcaster for power state changes
    event_sender: broadcast::Sender<PowerStateEvent>,

    /// Control flags
    is_running: Arc<AtomicBool>,

    /// Task handles
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    prediction_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Power state transition history
    state_history: Arc<RwLock<VecDeque<(SystemTime, PowerState)>>>,

    /// Last prediction
    last_prediction: Arc<RwLock<Option<PowerPrediction>>>,
}

impl PowerManager {
    /// Create new power manager
    pub fn new(config: PowerManagerConfig) -> Self {
        let (event_sender, _) = broadcast::channel(100);

        Self {
            config: Arc::new(RwLock::new(config)),
            power_state: Arc::new(RwLock::new(PowerState::Active)),
            battery_info: Arc::new(RwLock::new(None)),
            thermal_info: Arc::new(RwLock::new(None)),
            consumption_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            battery_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            stats: Arc::new(RwLock::new(PowerStats::new())),
            event_sender,
            is_running: Arc::new(AtomicBool::new(false)),
            monitoring_task: Arc::new(Mutex::new(None)),
            prediction_task: Arc::new(Mutex::new(None)),
            state_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            last_prediction: Arc::new(RwLock::new(None)),
        }
    }

    /// Start power management
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running.swap(true, Ordering::Relaxed) {
            return Ok(()); // Already running
        }

        log::info!("Starting power management system");

        // Initialize battery and thermal monitoring
        self.update_battery_info().await?;
        self.update_thermal_info().await?;

        // Start monitoring tasks
        self.start_monitoring_loop().await;
        self.start_prediction_loop().await;

        // Record initial state
        let current_state = *self.power_state.read().await;
        self.state_history
            .write()
            .await
            .push_back((SystemTime::now(), current_state));

        log::info!("Power management system started successfully");
        Ok(())
    }

    /// Stop power management
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_running.swap(false, Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }

        log::info!("Stopping power management system");

        // Stop monitoring tasks
        if let Some(task) = self.monitoring_task.lock().await.take() {
            task.abort();
        }

        if let Some(task) = self.prediction_task.lock().await.take() {
            task.abort();
        }

        log::info!("Power management system stopped");
        Ok(())
    }

    /// Get current power state
    pub async fn get_power_state(&self) -> PowerState {
        *self.power_state.read().await
    }

    /// Manually set power state (with validation)
    pub async fn set_power_state(
        &self,
        new_state: PowerState,
        reason: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_state = *self.power_state.read().await;

        if old_state != new_state {
            log::info!(
                "Power state transition: {:?} -> {:?} ({})",
                old_state,
                new_state,
                reason
            );

            // Update state
            *self.power_state.write().await = new_state;

            // Record transition
            self.state_history
                .write()
                .await
                .push_back((SystemTime::now(), new_state));
            if self.state_history.read().await.len() > 100 {
                self.state_history.write().await.pop_front();
            }

            // Get component limits for new state
            let config = self.config.read().await;
            let new_limits = config.component_limits.clone();

            // Create and send event
            let event = PowerStateEvent {
                old_state,
                new_state,
                reason: reason.to_string(),
                timestamp: SystemTime::now(),
                new_limits,
            };

            // Update statistics
            self.update_stats_for_transition(old_state, new_state).await;

            // Broadcast event (ignore errors if no receivers)
            let _ = self.event_sender.send(event);
        }

        Ok(())
    }

    /// Subscribe to power state change events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<PowerStateEvent> {
        self.event_sender.subscribe()
    }

    /// Get current battery information
    pub async fn get_battery_info(&self) -> Option<BatteryInfo> {
        self.battery_info.read().await.clone()
    }

    /// Get current thermal information  
    pub async fn get_thermal_info(&self) -> Option<ThermalInfo> {
        self.thermal_info.read().await.clone()
    }

    /// Get power consumption limits for current state
    pub async fn get_current_limits(&self) -> ComponentPowerLimits {
        let state = *self.power_state.read().await;
        let config = self.config.read().await;
        config.component_limits.clone()
    }

    /// Get component power limit for current state
    pub async fn get_cpu_limit(&self) -> f64 {
        let state = *self.power_state.read().await;
        let config = self.config.read().await;
        config
            .component_limits
            .cpu_limits
            .get(&state)
            .copied()
            .unwrap_or(50.0)
    }

    pub async fn get_memory_limit(&self) -> f64 {
        let state = *self.power_state.read().await;
        let config = self.config.read().await;
        config
            .component_limits
            .memory_limits
            .get(&state)
            .copied()
            .unwrap_or(100.0)
    }

    pub async fn get_ble_duty_cycle(&self) -> f64 {
        let state = *self.power_state.read().await;
        let config = self.config.read().await;
        config
            .component_limits
            .ble_duty_cycles
            .get(&state)
            .copied()
            .unwrap_or(0.1)
    }

    pub async fn get_network_limit(&self) -> u64 {
        let state = *self.power_state.read().await;
        let config = self.config.read().await;
        config
            .component_limits
            .network_limits
            .get(&state)
            .copied()
            .unwrap_or(512 * 1024)
    }

    /// Get latest power prediction
    pub async fn get_prediction(&self) -> Option<PowerPrediction> {
        self.last_prediction.read().await.clone()
    }

    /// Get power management statistics
    pub async fn get_stats(&self) -> PowerStats {
        self.stats.read().await.clone()
    }

    /// Record power consumption measurement
    pub async fn record_power_consumption(&self, consumption: PowerConsumption) {
        let mut history = self.consumption_history.write().await;
        history.push_back(consumption);

        // Keep only last 1000 measurements
        if history.len() > 1000 {
            history.pop_front();
        }
    }

    /// Update battery information (would interface with platform APIs)
    async fn update_battery_info(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would call platform-specific APIs
        // For now, simulate battery data
        let battery = BatteryInfo {
            level_percent: 75.0, // Would be read from system
            is_charging: false,  // Would be read from system
            charging_rate_watts: 0.0,
            time_to_full_minutes: None,
            time_remaining_minutes: Some(480), // 8 hours
            current_draw_watts: 2.5,
            temperature_celsius: 30.0,
            health_percent: 95.0,
            voltage: 3.7,
            timestamp: SystemTime::now(),
        };

        // Record battery level for history
        self.battery_history
            .write()
            .await
            .push_back((battery.timestamp, battery.level_percent));
        if self.battery_history.read().await.len() > 1000 {
            self.battery_history.write().await.pop_front();
        }

        *self.battery_info.write().await = Some(battery);
        Ok(())
    }

    /// Update thermal information
    async fn update_thermal_info(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, this would read from thermal sensors
        let thermal = ThermalInfo {
            cpu_temperature: 35.0, // Would be read from system
            battery_temperature: 30.0,
            ambient_temperature: Some(25.0),
            thermal_state: ThermalState::Normal,
            thermal_pressure: 0.0,
            is_throttling: false,
            timestamp: SystemTime::now(),
        };

        *self.thermal_info.write().await = Some(thermal);
        Ok(())
    }

    /// Evaluate power state based on current conditions
    async fn evaluate_power_state(&self) -> PowerState {
        let battery = self.battery_info.read().await.clone();
        let thermal = self.thermal_info.read().await.clone();
        let config = self.config.read().await;

        if let Some(battery) = battery {
            // Check charging state first
            if battery.is_charging {
                return PowerState::Charging;
            }

            // Check critical battery level
            if battery.level_percent <= config.battery_thresholds.critical_percent {
                return PowerState::Critical;
            }

            // Check power saver threshold
            if battery.level_percent <= config.battery_thresholds.power_saver_percent {
                return PowerState::PowerSaver;
            }

            // Check thermal conditions
            if let Some(thermal) = thermal {
                match thermal.thermal_state {
                    ThermalState::Critical | ThermalState::Hot => {
                        return PowerState::PowerSaver; // Reduce power to cool down
                    }
                    ThermalState::Warm => {
                        // Only reduce power if battery is also somewhat low
                        if battery.level_percent <= 50.0 {
                            return PowerState::PowerSaver;
                        }
                    }
                    ThermalState::Normal => {} // Continue with battery-based logic
                }
            }

            // Normal operation if battery level is good
            if battery.level_percent >= config.battery_thresholds.normal_resume_percent {
                return PowerState::Active;
            }
        }

        // Default to power saver if no battery info available
        PowerState::PowerSaver
    }

    /// Start main monitoring loop
    async fn start_monitoring_loop(&self) {
        let config = self.config.clone();
        let power_state = self.power_state.clone();
        let battery_info = self.battery_info.clone();
        let thermal_info = self.thermal_info.clone();
        let is_running = self.is_running.clone();
        let event_sender = self.event_sender.clone();
        let state_history = self.state_history.clone();
        let stats = self.stats.clone();

        let task = tokio::spawn(async move {
            let mut battery_interval = tokio::time::interval(Duration::from_secs(
                config.read().await.monitoring.battery_check_interval_secs,
            ));
            let mut thermal_interval = tokio::time::interval(Duration::from_secs(
                config.read().await.monitoring.thermal_check_interval_secs,
            ));

            while is_running.load(Ordering::Relaxed) {
                tokio::select! {
                    _ = battery_interval.tick() => {
                        // Update battery info (simulate reading from system)
                        let mut battery_guard = battery_info.write().await;
                        if let Some(ref mut battery) = *battery_guard {
                            // Simulate battery drain (would be real readings in practice)
                            battery.level_percent = (battery.level_percent - 0.1).max(0.0);
                            battery.timestamp = SystemTime::now();
                        }
                        drop(battery_guard);

                        // Evaluate power state
                        let current_state = *power_state.read().await;
                        let new_state = Self::evaluate_power_state_static(
                            &battery_info,
                            &thermal_info,
                            &config
                        ).await;

                        if current_state != new_state {
                            log::info!("Auto power state transition: {:?} -> {:?}", current_state, new_state);

                            *power_state.write().await = new_state;
                            state_history.write().await.push_back((SystemTime::now(), new_state));

                            let event = PowerStateEvent {
                                old_state: current_state,
                                new_state,
                                reason: "Automatic based on battery/thermal conditions".to_string(),
                                timestamp: SystemTime::now(),
                                new_limits: config.read().await.component_limits.clone(),
                            };

                            let _ = event_sender.send(event);

                            // Update statistics
                            Self::update_stats_for_transition_static(&stats, current_state, new_state).await;
                        }
                    },

                    _ = thermal_interval.tick() => {
                        // Update thermal info (simulate reading from sensors)
                        let mut thermal_guard = thermal_info.write().await;
                        if let Some(ref mut thermal) = *thermal_guard {
                            // Simulate temperature fluctuations
                            thermal.cpu_temperature += (rand::random::<f64>() - 0.5) * 2.0;
                            thermal.cpu_temperature = thermal.cpu_temperature.clamp(25.0, 55.0);
                            thermal.timestamp = SystemTime::now();

                            // Update thermal state based on temperature
                            thermal.thermal_state = if thermal.cpu_temperature >= 50.0 {
                                ThermalState::Critical
                            } else if thermal.cpu_temperature >= 45.0 {
                                ThermalState::Hot
                            } else if thermal.cpu_temperature >= 40.0 {
                                ThermalState::Warm
                            } else {
                                ThermalState::Normal
                            };
                        }
                        drop(thermal_guard);
                    },
                }
            }
        });

        *self.monitoring_task.lock().await = Some(task);
    }

    /// Start prediction loop
    async fn start_prediction_loop(&self) {
        let config = self.config.clone();
        let battery_history = self.battery_history.clone();
        let consumption_history = self.consumption_history.clone();
        let last_prediction = self.last_prediction.clone();
        let is_running = self.is_running.clone();

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                config.read().await.monitoring.prediction_interval_secs,
            ));

            while is_running.load(Ordering::Relaxed) {
                interval.tick().await;

                // Generate power prediction
                let prediction =
                    Self::generate_prediction_static(&battery_history, &consumption_history).await;

                *last_prediction.write().await = Some(prediction.clone());

                log::debug!(
                    "Updated power prediction: {} minutes remaining, confidence: {:.1}%",
                    prediction.predicted_life_minutes,
                    prediction.confidence * 100.0
                );
            }
        });

        *self.prediction_task.lock().await = Some(task);
    }

    /// Static version of evaluate_power_state for use in async contexts
    async fn evaluate_power_state_static(
        battery_info: &Arc<RwLock<Option<BatteryInfo>>>,
        thermal_info: &Arc<RwLock<Option<ThermalInfo>>>,
        config: &Arc<RwLock<PowerManagerConfig>>,
    ) -> PowerState {
        let battery = battery_info.read().await.clone();
        let thermal = thermal_info.read().await.clone();
        let config = config.read().await;

        if let Some(battery) = battery {
            if battery.is_charging {
                return PowerState::Charging;
            }

            if battery.level_percent <= config.battery_thresholds.critical_percent {
                return PowerState::Critical;
            }

            if battery.level_percent <= config.battery_thresholds.power_saver_percent {
                return PowerState::PowerSaver;
            }

            if let Some(thermal) = thermal {
                match thermal.thermal_state {
                    ThermalState::Critical | ThermalState::Hot => return PowerState::PowerSaver,
                    ThermalState::Warm if battery.level_percent <= 50.0 => {
                        return PowerState::PowerSaver
                    }
                    _ => {}
                }
            }

            if battery.level_percent >= config.battery_thresholds.normal_resume_percent {
                return PowerState::Active;
            }
        }

        PowerState::PowerSaver
    }

    /// Update statistics for state transition
    async fn update_stats_for_transition(&self, old_state: PowerState, new_state: PowerState) {
        Self::update_stats_for_transition_static(&self.stats, old_state, new_state).await;
    }

    /// Static version of update_stats_for_transition
    async fn update_stats_for_transition_static(
        stats: &Arc<RwLock<PowerStats>>,
        _old_state: PowerState,
        _new_state: PowerState,
    ) {
        let mut stats_guard = stats.write().await;
        stats_guard.state_transitions += 1;
        // Additional statistics updates would go here
    }

    /// Generate power consumption prediction
    async fn generate_prediction_static(
        battery_history: &Arc<RwLock<VecDeque<(SystemTime, f64)>>>,
        _consumption_history: &Arc<RwLock<VecDeque<PowerConsumption>>>,
    ) -> PowerPrediction {
        let history = battery_history.read().await;

        if history.len() < 2 {
            // Not enough data for prediction
            return PowerPrediction {
                predicted_life_minutes: 240, // Default 4 hours
                confidence: 0.1,
                recommended_state: PowerState::Active,
                predicted_levels: vec![],
                factors: vec!["Insufficient data".to_string()],
            };
        }

        // Simple linear regression on recent battery levels
        let recent_points: Vec<_> = history.iter().rev().take(10).collect();

        if recent_points.len() >= 2 {
            let first = recent_points.last().unwrap();
            let last = recent_points.first().unwrap();

            let time_diff = last
                .0
                .duration_since(first.0)
                .unwrap_or_default()
                .as_secs_f64()
                / 3600.0; // hours
            let level_diff = last.1 - first.1; // percentage

            if time_diff > 0.0 && level_diff < 0.0 {
                let drain_rate = -level_diff / time_diff; // %/hour
                let remaining_hours = last.1 / drain_rate;

                PowerPrediction {
                    predicted_life_minutes: (remaining_hours * 60.0).max(0.0) as u64,
                    confidence: 0.7,
                    recommended_state: if remaining_hours < 2.0 {
                        PowerState::PowerSaver
                    } else {
                        PowerState::Active
                    },
                    predicted_levels: vec![
                        (Duration::from_secs(3600), last.1 - drain_rate),
                        (Duration::from_secs(7200), last.1 - drain_rate * 2.0),
                    ],
                    factors: vec![format!("Drain rate: {:.1}%/hour", drain_rate)],
                }
            } else {
                PowerPrediction {
                    predicted_life_minutes: 480, // 8 hours default
                    confidence: 0.3,
                    recommended_state: PowerState::Active,
                    predicted_levels: vec![],
                    factors: vec!["Stable or charging".to_string()],
                }
            }
        } else {
            PowerPrediction {
                predicted_life_minutes: 240,
                confidence: 0.2,
                recommended_state: PowerState::Active,
                predicted_levels: vec![],
                factors: vec!["Limited data".to_string()],
            }
        }
    }
}

impl PowerStats {
    fn new() -> Self {
        Self {
            time_in_states: HashMap::new(),
            drain_rates: HashMap::new(),
            state_transitions: 0,
            estimated_savings_percent: 0.0,
            cycles_saved: 0.0,
            thermal_throttling_percent: 0.0,
        }
    }
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            level_percent: 50.0,
            is_charging: false,
            charging_rate_watts: 0.0,
            time_to_full_minutes: None,
            time_remaining_minutes: None,
            current_draw_watts: 2.0,
            temperature_celsius: 25.0,
            health_percent: 100.0,
            voltage: 3.7,
            timestamp: SystemTime::now(),
        }
    }
}
