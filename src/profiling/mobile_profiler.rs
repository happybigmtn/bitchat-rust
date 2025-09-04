use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

use crate::error::BitCrapsError;
#[cfg(any(feature = "android", feature = "uniffi"))]
use crate::mobile::{
    power_management::{PowerManager, ThermalState},
    PowerMode,
};

/// Mobile-specific performance profiler
pub struct MobileProfiler {
    #[cfg(any(feature = "android", feature = "uniffi"))]
    power_manager: Arc<PowerManager>,
    metrics: Arc<RwLock<BatteryMetrics>>,
    thermal_metrics: Arc<RwLock<ThermalMetrics>>,
    profiling_active: Arc<RwLock<bool>>,
    sample_interval: Duration,
}

impl MobileProfiler {
    pub fn new() -> Result<Self, BitCrapsError> {
        #[cfg(any(feature = "android", feature = "uniffi"))]
        let power_manager = Arc::new(PowerManager::new(PowerMode::Balanced));

        Ok(Self {
            #[cfg(any(feature = "android", feature = "uniffi"))]
            power_manager,
            metrics: Arc::new(RwLock::new(BatteryMetrics::new())),
            thermal_metrics: Arc::new(RwLock::new(ThermalMetrics::new())),
            profiling_active: Arc::new(RwLock::new(false)),
            sample_interval: Duration::from_secs(1), // 1 sample per second for battery/thermal
        })
    }

    pub async fn start(&mut self) -> Result<(), BitCrapsError> {
        *self.profiling_active.write() = true;

        #[cfg(any(feature = "android", feature = "uniffi"))]
        {
            let power_manager = Arc::clone(&self.power_manager);
            let metrics = Arc::clone(&self.metrics);
            let thermal_metrics = Arc::clone(&self.thermal_metrics);
            let profiling_active = Arc::clone(&self.profiling_active);
            let sample_interval = self.sample_interval;

        tokio::spawn(async move {
            let mut interval = interval(sample_interval);
            let mut last_battery_level: Option<f32> = None;

            while *profiling_active.read() {
                interval.tick().await;

                // Get current battery and thermal info
                if let Ok(battery_info) = power_manager.get_battery_info().await {
                    if let Ok(thermal_info) = power_manager.get_thermal_info().await {
                        let now = Instant::now();

                        // Calculate battery drain rate
                        let current_level = battery_info.level.unwrap_or(0.0);
                        let drain_rate = if let Some(last_level) = last_battery_level {
                            let level_change = last_level - current_level;
                            if level_change > 0.0 {
                                // Convert to drain rate per hour
                                level_change * 3600.0 / sample_interval.as_secs() as f32
                            } else {
                                0.0 // Charging or stable
                            }
                        } else {
                            0.0
                        };

                        // Update battery metrics
                        {
                            let mut metrics = metrics.write();
                            metrics.add_battery_sample(BatterySample {
                                level: current_level,
                                is_charging: battery_info.is_charging,
                                drain_rate_per_hour: drain_rate,
                                timestamp: now,
                            });
                        }

                        // Update thermal metrics
                        {
                            let mut thermal = thermal_metrics.write();
                            thermal.add_thermal_sample(ThermalSample {
                                cpu_temperature: thermal_info.cpu_temperature,
                                battery_temperature: thermal_info.battery_temperature,
                                ambient_temperature: thermal_info
                                    .ambient_temperature
                                    .unwrap_or(25.0),
                                thermal_state: thermal_info.thermal_state,
                                timestamp: now,
                            });
                        }

                        last_battery_level = Some(current_level);
                    }
                }
            }
        });
        }

        #[cfg(not(any(feature = "android", feature = "uniffi")))]
        {
            tracing::debug!("Mobile profiling unavailable (mobile features not enabled)");
        }

        tracing::debug!("Mobile profiling started");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<MobileProfile, BitCrapsError> {
        *self.profiling_active.write() = false;

        tokio::time::sleep(Duration::from_secs(2)).await;

        let battery_metrics = self.metrics.read().clone();
        let thermal_metrics = self.thermal_metrics.read().clone();

        // Reset for next session
        {
            let mut metrics_guard = self.metrics.write();
            *metrics_guard = BatteryMetrics::new();
        }
        {
            let mut thermal_guard = self.thermal_metrics.write();
            *thermal_guard = ThermalMetrics::new();
        }

        Ok(MobileProfile {
            average_battery_level: battery_metrics.average_battery_level(),
            battery_drain_rate: battery_metrics.average_drain_rate(),
            total_charging_time: battery_metrics.total_charging_time(),
            battery_cycles_detected: battery_metrics.battery_cycles_detected(),
            average_cpu_temperature: thermal_metrics.average_cpu_temperature(),
            average_battery_temperature: thermal_metrics.average_battery_temperature(),
            peak_cpu_temperature: thermal_metrics.peak_cpu_temperature(),
            peak_battery_temperature: thermal_metrics.peak_battery_temperature(),
            thermal_events: thermal_metrics.thermal_event_count(),
            thermal_throttling_duration: thermal_metrics.thermal_throttling_duration(),
            profiling_duration: battery_metrics.profiling_duration(),
        })
    }

    pub async fn current_metrics(&self) -> Result<BatteryMetrics, BitCrapsError> {
        Ok(self.metrics.read().clone())
    }

    /// Record power consumption event
    pub fn record_power_event(&self, event_type: PowerEventType, power_draw_mw: f32) {
        let mut metrics = self.metrics.write();
        metrics.power_events.push(PowerEvent {
            event_type,
            power_draw_mw,
            timestamp: Instant::now(),
        });

        // Keep only recent events
        if metrics.power_events.len() > 1000 {
            metrics.power_events.remove(0);
        }
    }

    /// Record thermal throttling event
    pub fn record_thermal_throttling(&self, duration: Duration, severity: ThermalSeverity) {
        let mut thermal = self.thermal_metrics.write();
        thermal.throttling_events.push(ThermalThrottlingEvent {
            duration,
            severity,
            timestamp: Instant::now(),
        });
    }

    /// Profile power consumption of an operation
    pub async fn profile_power_consumption<F, R>(
        &mut self,
        operation_name: &str,
        operation: F,
    ) -> Result<(R, PowerConsumptionProfile), BitCrapsError>
    where
        F: std::future::Future<Output = R>,
    {
        let start_time = Instant::now();
        let start_battery = self.get_current_battery_level().await?;

        let result = operation.await;

        let end_time = Instant::now();
        let end_battery = self.get_current_battery_level().await?;

        let duration = end_time - start_time;
        let battery_consumed = (start_battery - end_battery).max(0.0);

        let profile = PowerConsumptionProfile {
            operation_name: operation_name.to_string(),
            duration,
            battery_consumed_percent: battery_consumed,
            estimated_power_draw_mw: self.estimate_power_draw(battery_consumed, duration),
        };

        // Record the power event
        self.record_power_event(PowerEventType::Operation, profile.estimated_power_draw_mw);

        Ok((result, profile))
    }

    /// Get current battery level
    async fn get_current_battery_level(&self) -> Result<f32, crate::error::Error> {
        #[cfg(any(feature = "android", feature = "uniffi"))]
        {
            let battery_info = self.power_manager.get_battery_info().await.map_err(|_| {
                crate::error::Error::InvalidData("Failed to get battery info".to_string())
            })?;
            Ok(battery_info.level.unwrap_or(0.0))
        }
        
        #[cfg(not(any(feature = "android", feature = "uniffi")))]
        {
            // Return simulated battery level when mobile features are disabled
            Ok(50.0)
        }
    }

    /// Estimate power draw based on battery consumption and time
    fn estimate_power_draw(&self, battery_consumed: f32, duration: Duration) -> f32 {
        if duration.as_secs_f32() == 0.0 {
            return 0.0;
        }

        // Simplified calculation - in practice would use device-specific battery capacity
        // Assume average smartphone battery capacity of 3000mAh at 3.7V = 11.1Wh
        let battery_capacity_wh = 11.1;
        let consumed_wh = (battery_consumed / 100.0) * battery_capacity_wh;
        let consumed_mw = consumed_wh * 1000.0;
        let duration_hours = duration.as_secs_f32() / 3600.0;

        consumed_mw / duration_hours
    }
}

/// Battery performance metrics
#[derive(Debug, Clone)]
pub struct BatteryMetrics {
    pub samples: Vec<BatterySample>,
    pub power_events: Vec<PowerEvent>,
    pub start_time: Option<Instant>,
}

impl BatteryMetrics {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            power_events: Vec::new(),
            start_time: Some(Instant::now()),
        }
    }

    pub fn add_battery_sample(&mut self, sample: BatterySample) {
        self.samples.push(sample);

        // Keep only recent samples (last 24 hours worth)
        if self.samples.len() > 86400 {
            self.samples.remove(0);
        }
    }

    pub fn average_battery_level(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        self.samples.iter().map(|s| s.level).sum::<f32>() / self.samples.len() as f32
    }

    pub fn average_drain_rate(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let drain_samples: Vec<f32> = self
            .samples
            .iter()
            .filter(|s| s.drain_rate_per_hour > 0.0)
            .map(|s| s.drain_rate_per_hour)
            .collect();

        if drain_samples.is_empty() {
            return 0.0;
        }

        drain_samples.iter().sum::<f32>() / drain_samples.len() as f32
    }

    pub fn total_charging_time(&self) -> Duration {
        let mut charging_duration = Duration::from_secs(0);
        let mut charging_start: Option<Instant> = None;

        for sample in &self.samples {
            if sample.is_charging {
                if charging_start.is_none() {
                    charging_start = Some(sample.timestamp);
                }
            } else if let Some(start) = charging_start {
                charging_duration += sample.timestamp - start;
                charging_start = None;
            }
        }

        charging_duration
    }

    pub fn battery_cycles_detected(&self) -> u32 {
        // Detect full charge/discharge cycles
        let mut cycles = 0;
        let mut last_high: Option<Instant> = None;

        for sample in &self.samples {
            if sample.level > 90.0 && last_high.is_none() {
                last_high = Some(sample.timestamp);
            } else if sample.level < 20.0 && last_high.is_some() {
                cycles += 1;
                last_high = None;
            }
        }

        cycles
    }

    pub fn profiling_duration(&self) -> Duration {
        if let (Some(start), Some(last)) = (self.start_time, self.samples.last()) {
            last.timestamp - start
        } else {
            Duration::from_nanos(0)
        }
    }
}

/// Thermal performance metrics
#[derive(Debug, Clone)]
pub struct ThermalMetrics {
    pub samples: Vec<ThermalSample>,
    pub throttling_events: Vec<ThermalThrottlingEvent>,
    pub start_time: Option<Instant>,
}

impl ThermalMetrics {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            throttling_events: Vec::new(),
            start_time: Some(Instant::now()),
        }
    }

    pub fn add_thermal_sample(&mut self, sample: ThermalSample) {
        self.samples.push(sample);

        // Keep only recent samples
        if self.samples.len() > 3600 {
            self.samples.remove(0);
        }
    }

    pub fn average_cpu_temperature(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        self.samples.iter().map(|s| s.cpu_temperature).sum::<f32>() / self.samples.len() as f32
    }

    pub fn average_battery_temperature(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        self.samples
            .iter()
            .map(|s| s.battery_temperature)
            .sum::<f32>()
            / self.samples.len() as f32
    }

    pub fn peak_cpu_temperature(&self) -> f32 {
        self.samples
            .iter()
            .map(|s| s.cpu_temperature)
            .fold(0.0, f32::max)
    }

    pub fn peak_battery_temperature(&self) -> f32 {
        self.samples
            .iter()
            .map(|s| s.battery_temperature)
            .fold(0.0, f32::max)
    }

    pub fn thermal_event_count(&self) -> u32 {
        self.samples
            .iter()
            .filter(|s| {
                matches!(
                    s.thermal_state,
                    ThermalState::Warning | ThermalState::Critical
                )
            })
            .count() as u32
    }

    pub fn thermal_throttling_duration(&self) -> Duration {
        self.throttling_events.iter().map(|e| e.duration).sum()
    }
}

/// Individual battery sample
#[derive(Debug, Clone)]
pub struct BatterySample {
    pub level: f32, // 0.0 to 1.0
    pub is_charging: bool,
    pub drain_rate_per_hour: f32, // Percentage points per hour
    pub timestamp: Instant,
}

/// Individual thermal sample
#[derive(Debug, Clone)]
pub struct ThermalSample {
    pub cpu_temperature: f32,     // Celsius
    pub battery_temperature: f32, // Celsius
    pub ambient_temperature: f32, // Celsius
    pub thermal_state: ThermalState,
    pub timestamp: Instant,
}

/// Power consumption event
#[derive(Debug, Clone)]
pub struct PowerEvent {
    pub event_type: PowerEventType,
    pub power_draw_mw: f32, // Milliwatts
    pub timestamp: Instant,
}

/// Thermal throttling event
#[derive(Debug, Clone)]
pub struct ThermalThrottlingEvent {
    pub duration: Duration,
    pub severity: ThermalSeverity,
    pub timestamp: Instant,
}

/// Types of power events
#[derive(Debug, Clone, Copy)]
pub enum PowerEventType {
    Operation,
    NetworkActivity,
    BluetoothActivity,
    DatabaseActivity,
    CryptoOperation,
    BackgroundTask,
}

/// Thermal throttling severity
#[derive(Debug, Clone, Copy)]
pub enum ThermalSeverity {
    Light,     // Minor frequency reduction
    Moderate,  // Significant frequency reduction
    Heavy,     // Major throttling
    Emergency, // Emergency shutdown risk
}

/// Complete mobile profiling results
#[derive(Debug, Clone)]
pub struct MobileProfile {
    pub average_battery_level: f32,
    pub battery_drain_rate: f32, // Percent per hour
    pub total_charging_time: Duration,
    pub battery_cycles_detected: u32,
    pub average_cpu_temperature: f32,
    pub average_battery_temperature: f32,
    pub peak_cpu_temperature: f32,
    pub peak_battery_temperature: f32,
    pub thermal_events: u32,
    pub thermal_throttling_duration: Duration,
    pub profiling_duration: Duration,
}

/// Power consumption profile for a specific operation
#[derive(Debug, Clone)]
pub struct PowerConsumptionProfile {
    pub operation_name: String,
    pub duration: Duration,
    pub battery_consumed_percent: f32,
    pub estimated_power_draw_mw: f32,
}
