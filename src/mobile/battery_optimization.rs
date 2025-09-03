//! Battery optimization strategies for mobile platforms

use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Battery manager for mobile platforms (alias for compatibility)
pub type BatteryManager = BatteryOptimizationHandler;

/// Battery optimization detector and handler
pub struct BatteryOptimizationHandler {
    platform_type: PlatformType,
    optimization_state: Arc<Mutex<OptimizationState>>,
    event_sender: Option<mpsc::UnboundedSender<GameEvent>>,
    power_manager: Arc<PowerManager>,
    scan_history: Arc<Mutex<VecDeque<ScanEvent>>>,
}

#[derive(Clone, Default)]
struct OptimizationState {
    battery_level: Option<f32>,
    is_charging: bool,
    doze_mode_detected: bool,
    app_standby_detected: bool,
    background_restricted: bool,
    scan_throttling_detected: bool,
    last_optimization_warning: Option<SystemTime>,
    optimization_warnings_sent: u32,
}

#[derive(Clone)]
struct ScanEvent {
    timestamp: SystemTime,
    expected_interval_ms: u64,
    actual_interval_ms: u64,
    success: bool,
}

impl BatteryOptimizationHandler {
    /// Create a new battery optimization handler
    pub fn new(
        platform_type: PlatformType,
        power_manager: Arc<PowerManager>,
        event_sender: Option<mpsc::UnboundedSender<GameEvent>>,
    ) -> Self {
        Self {
            platform_type,
            optimization_state: Arc::new(Mutex::new(OptimizationState::default())),
            event_sender,
            power_manager,
            scan_history: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Start monitoring for battery optimization interference
    pub async fn start_monitoring(&self) {
        let optimization_state = Arc::clone(&self.optimization_state);
        let scan_history = Arc::clone(&self.scan_history);
        let event_sender = self.event_sender.clone();
        let platform_type = self.platform_type;
        let power_manager = Arc::clone(&self.power_manager);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30)); // Check every 30 seconds

            loop {
                interval.tick().await;

                // Check for battery optimization interference
                let optimization_detected = Self::check_optimization_interference(
                    &optimization_state,
                    &scan_history,
                    &platform_type,
                )
                .await;

                if let Some(optimization_info) = optimization_detected {
                    // Send warning event
                    if let Some(sender) = &event_sender {
                        let _ = sender.send(GameEvent::BatteryOptimizationDetected {
                            reason: optimization_info.clone(),
                        });
                    }

                    // Apply adaptive optimizations
                    Self::apply_adaptive_optimizations(
                        &power_manager,
                        &optimization_state,
                        &platform_type,
                    )
                    .await;

                    log::warn!("Battery optimization detected: {}", optimization_info);
                }

                // Update system state
                Self::update_system_state(&optimization_state, &platform_type).await;
            }
        });
    }

    /// Record a scan event for optimization detection
    pub fn record_scan_event(
        &self,
        expected_interval_ms: u64,
        actual_interval_ms: u64,
        success: bool,
    ) {
        if let Ok(mut history) = self.scan_history.lock() {
            let event = ScanEvent {
                timestamp: SystemTime::now(),
                expected_interval_ms,
                actual_interval_ms,
                success,
            };

            history.push_back(event);

            // Keep only recent events (last 5 minutes)
            let cutoff = SystemTime::now() - Duration::from_secs(300);
            while let Some(front) = history.front() {
                if front.timestamp < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }

            // Limit total size
            if history.len() > 100 {
                history.pop_front();
            }
        }
    }

    /// Get optimization recommendations for the user
    pub fn get_optimization_recommendations(&self) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        if let Ok(state) = self.optimization_state.lock() {
            match self.platform_type {
                PlatformType::Android => {
                    if state.background_restricted {
                        recommendations.push(OptimizationRecommendation {
                            title: "Battery Optimization Detected".to_string(),
                            description: "Android's battery optimization is limiting BitCraps. Please add BitCraps to your battery whitelist.".to_string(),
                            action: "Open battery optimization settings".to_string(),
                            urgency: RecommendationUrgency::High,
                        });
                    }

                    if state.doze_mode_detected {
                        recommendations.push(OptimizationRecommendation {
                            title: "Doze Mode Interference".to_string(),
                            description: "Android Doze mode is affecting background operation. Consider using the app in foreground when possible.".to_string(),
                            action: "Keep app in foreground".to_string(),
                            urgency: RecommendationUrgency::Medium,
                        });
                    }

                    if state.scan_throttling_detected {
                        recommendations.push(OptimizationRecommendation {
                            title: "BLE Scanning Throttled".to_string(),
                            description: "Android is throttling Bluetooth scanning. This may affect peer discovery.".to_string(),
                            action: "Enable high performance mode".to_string(),
                            urgency: RecommendationUrgency::Medium,
                        });
                    }
                }
                PlatformType::Ios => {
                    if state.background_restricted {
                        recommendations.push(OptimizationRecommendation {
                            title: "Background App Refresh Disabled".to_string(),
                            description: "iOS Background App Refresh is disabled. BitCraps cannot operate in the background.".to_string(),
                            action: "Enable Background App Refresh".to_string(),
                            urgency: RecommendationUrgency::High,
                        });
                    }

                    // iOS-specific recommendations
                    recommendations.push(OptimizationRecommendation {
                        title: "iOS Background Limitations".to_string(),
                        description: "iOS severely limits background Bluetooth operations. Keep the app in foreground for best performance.".to_string(),
                        action: "Keep app active".to_string(),
                        urgency: RecommendationUrgency::Info,
                    });
                }
                _ => {}
            }

            // Battery level recommendations
            if let Some(battery_level) = state.battery_level {
                if battery_level < 0.2 && !state.is_charging {
                    recommendations.push(OptimizationRecommendation {
                        title: "Low Battery".to_string(),
                        description: "Battery level is low. Consider enabling battery saver mode."
                            .to_string(),
                        action: "Enable battery saver".to_string(),
                        urgency: RecommendationUrgency::Medium,
                    });
                }
            }
        }

        recommendations
    }

    /// Check if device is in low power mode
    pub fn is_low_power_mode(&self) -> bool {
        if let Ok(state) = self.optimization_state.lock() {
            // Consider it low power mode if:
            // - Battery level is below 20% and not charging
            // - Doze mode is detected (Android)
            // - App standby is detected (Android)
            // - Background is restricted
            let low_battery = state.battery_level.unwrap_or(100.0) < 20.0 && !state.is_charging;
            low_battery
                || state.doze_mode_detected
                || state.app_standby_detected
                || state.background_restricted
        } else {
            false // Default to normal mode if we can't determine
        }
    }

    /// Check for battery optimization interference
    async fn check_optimization_interference(
        optimization_state: &Arc<Mutex<OptimizationState>>,
        scan_history: &Arc<Mutex<VecDeque<ScanEvent>>>,
        platform_type: &PlatformType,
    ) -> Option<String> {
        // Analyze scan history for patterns indicating battery optimization
        let scan_issues = Self::analyze_scan_patterns(scan_history).await;

        // Check system state
        let system_issues =
            Self::check_system_restrictions(optimization_state, platform_type).await;

        // Combine issues
        let mut issues = Vec::new();
        issues.extend(scan_issues);
        issues.extend(system_issues);

        if !issues.is_empty() {
            Some(issues.join("; "))
        } else {
            None
        }
    }

    /// Analyze scan patterns for optimization interference
    async fn analyze_scan_patterns(scan_history: &Arc<Mutex<VecDeque<ScanEvent>>>) -> Vec<String> {
        let mut issues = Vec::new();

        if let Ok(history) = scan_history.lock() {
            if history.len() < 5 {
                return issues; // Not enough data
            }

            let recent_events: Vec<_> = history.iter().rev().take(10).collect();

            // Check for scan throttling (actual intervals much longer than expected)
            let throttled_count = recent_events
                .iter()
                .filter(|event| event.actual_interval_ms > event.expected_interval_ms * 3)
                .count();

            if throttled_count > recent_events.len() / 2 {
                issues.push("Bluetooth scanning is being throttled".to_string());
            }

            // Check for scan failures
            let failure_count = recent_events.iter().filter(|event| !event.success).count();

            if failure_count > recent_events.len() / 3 {
                issues.push("Bluetooth scans are frequently failing".to_string());
            }

            // Check for irregular patterns (sign of doze mode or app standby)
            let intervals: Vec<_> = recent_events
                .windows(2)
                .map(|pair| {
                    pair[0]
                        .timestamp
                        .duration_since(pair[1].timestamp)
                        .unwrap_or_default()
                        .as_millis() as u64
                })
                .collect();

            if intervals.len() > 3 {
                let avg_interval = intervals.iter().sum::<u64>() / intervals.len() as u64;
                let irregular_count = intervals
                    .iter()
                    .filter(|&interval| *interval > avg_interval * 5)
                    .count();

                if irregular_count > intervals.len() / 4 {
                    issues
                        .push("Irregular scan patterns detected (possible doze mode)".to_string());
                }
            }
        }

        issues
    }

    /// Check system-level restrictions
    async fn check_system_restrictions(
        optimization_state: &Arc<Mutex<OptimizationState>>,
        platform_type: &PlatformType,
    ) -> Vec<String> {
        let mut issues = Vec::new();

        // Get platform-specific state first to avoid holding lock across await
        let (
            is_doze_mode,
            is_app_standby,
            is_background_restricted,
            is_ios_background_refresh_disabled,
        ) = match platform_type {
            PlatformType::Android => {
                let doze = Self::detect_android_doze_mode().await;
                let standby = Self::detect_android_app_standby().await;
                let restricted = Self::detect_android_background_restrictions().await;
                (doze, standby, restricted, false)
            }
            PlatformType::Ios => {
                let refresh_disabled = Self::detect_ios_background_refresh().await;
                (false, false, false, refresh_disabled)
            }
            _ => (false, false, false, false),
        };

        if let Ok(mut state) = optimization_state.lock() {
            match platform_type {
                PlatformType::Android => {
                    // Check for Android-specific issues

                    if is_doze_mode && !state.doze_mode_detected {
                        state.doze_mode_detected = true;
                        issues.push("Android Doze mode is active".to_string());
                    }

                    if is_app_standby && !state.app_standby_detected {
                        state.app_standby_detected = true;
                        issues.push("App is in Android App Standby".to_string());
                    }

                    if is_background_restricted && !state.background_restricted {
                        state.background_restricted = true;
                        issues.push("App has background restrictions".to_string());
                    }
                }
                PlatformType::Ios => {
                    // Check for iOS-specific issues
                    if is_ios_background_refresh_disabled && !state.background_restricted {
                        state.background_restricted = true;
                        issues.push("Background App Refresh is disabled".to_string());
                    }
                }
                _ => {}
            }
        }

        issues
    }

    /// Apply adaptive optimizations based on detected battery optimization
    async fn apply_adaptive_optimizations(
        power_manager: &Arc<PowerManager>,
        optimization_state: &Arc<Mutex<OptimizationState>>,
        platform_type: &PlatformType,
    ) {
        if let Ok(state) = optimization_state.lock() {
            // Adjust power mode based on detected issues
            if state.doze_mode_detected || state.app_standby_detected {
                // Switch to ultra low power mode to work within system constraints
                let _ = power_manager.set_mode(PowerMode::UltraLowPower);
            } else if state.background_restricted {
                // Use battery saver mode
                let _ = power_manager.set_mode(PowerMode::BatterySaver);
            }

            // Adjust scan intervals based on platform
            match platform_type {
                PlatformType::Android => {
                    if state.doze_mode_detected {
                        // Use very long intervals during doze mode
                        let _ = power_manager.set_scan_interval(30000); // 30 seconds
                    } else if state.scan_throttling_detected {
                        // Reduce scan frequency to avoid throttling
                        let _ = power_manager.set_scan_interval(5000); // 5 seconds
                    }
                }
                PlatformType::Ios => {
                    if state.background_restricted {
                        // iOS background scanning is very limited
                        let _ = power_manager.set_scan_interval(10000); // 10 seconds
                    }
                }
                _ => {}
            }
        }
    }

    /// Update system state information
    async fn update_system_state(
        optimization_state: &Arc<Mutex<OptimizationState>>,
        platform_type: &PlatformType,
    ) {
        // Get battery info first to avoid holding lock across await
        let battery_level = Self::get_battery_level(platform_type).await;
        let is_charging = Self::is_device_charging(platform_type).await;

        if let Ok(mut state) = optimization_state.lock() {
            // Update battery level
            state.battery_level = battery_level;
            state.is_charging = is_charging;

            // Reset detection flags periodically to re-evaluate
            let now = SystemTime::now();
            if let Some(last_warning) = state.last_optimization_warning {
                if now.duration_since(last_warning).unwrap_or_default() > Duration::from_secs(300) {
                    // Reset flags every 5 minutes to re-evaluate
                    state.doze_mode_detected = false;
                    state.app_standby_detected = false;
                    state.scan_throttling_detected = false;
                }
            }
        }
    }

    // Platform-specific detection methods (stubs - would be implemented with actual platform APIs)

    async fn detect_android_doze_mode() -> bool {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            match call_android_method("android/os/PowerManager", "isDeviceIdleMode", "()Z", &[]) {
                Ok(result) => result.z().unwrap_or(false),
                Err(e) => {
                    log::error!("Failed to check Android Doze mode: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        false
    }

    async fn detect_android_app_standby() -> bool {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            match call_android_method(
                "android/app/usage/UsageStatsManager",
                "isAppInactive",
                "(Ljava/lang/String;)Z",
                &["com.bitcraps.app".into()],
            ) {
                Ok(result) => result.z().unwrap_or(false),
                Err(e) => {
                    log::error!("Failed to check Android App Standby: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        false
    }

    async fn detect_android_background_restrictions() -> bool {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            match call_android_method(
                "android/app/ActivityManager",
                "isBackgroundRestricted",
                "()Z",
                &[],
            ) {
                Ok(result) => result.z().unwrap_or(false),
                Err(e) => {
                    log::error!("Failed to check Android background restrictions: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        false
    }

    async fn detect_ios_background_refresh() -> bool {
        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            match call_ios_method("UIApplication", "backgroundRefreshStatus", &[]) {
                Ok(status) => {
                    let refresh_status = status.as_i32();
                    // UIBackgroundRefreshStatusDenied = 0, UIBackgroundRefreshStatusRestricted = 2
                    refresh_status == 0 || refresh_status == 2
                }
                Err(e) => {
                    log::error!("Failed to check iOS background refresh status: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        false
    }

    async fn get_battery_level(platform_type: &PlatformType) -> Option<f32> {
        match platform_type {
            PlatformType::Android => {
                #[cfg(target_os = "android")]
                {
                    use crate::mobile::android::jni_helpers::call_android_method;

                    match call_android_method(
                        "android/os/BatteryManager",
                        "getIntProperty",
                        "(I)I",
                        &[4.into()], // BATTERY_PROPERTY_CAPACITY
                    ) {
                        Ok(result) => {
                            let level = result.i().unwrap_or(-1);
                            if level >= 0 && level <= 100 {
                                Some(level as f32 / 100.0)
                            } else {
                                None
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to get Android battery level: {}", e);
                            None
                        }
                    }
                }

                #[cfg(not(target_os = "android"))]
                None
            }
            PlatformType::Ios => {
                #[cfg(target_os = "ios")]
                {
                    use crate::mobile::ios::ffi_helpers::call_ios_method;

                    // Enable battery monitoring first
                    let _ =
                        call_ios_method("UIDevice", "setBatteryMonitoringEnabled:", &[true.into()]);

                    match call_ios_method("UIDevice", "batteryLevel", &[]) {
                        Ok(result) => {
                            let level = result.as_f32();
                            if level >= 0.0 && level <= 1.0 {
                                Some(level)
                            } else {
                                None
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to get iOS battery level: {}", e);
                            None
                        }
                    }
                }

                #[cfg(not(target_os = "ios"))]
                None
            }
            _ => None,
        }
    }

    async fn is_device_charging(platform_type: &PlatformType) -> bool {
        match platform_type {
            PlatformType::Android => {
                #[cfg(target_os = "android")]
                {
                    use crate::mobile::android::jni_helpers::call_android_method;

                    match call_android_method(
                        "android/os/BatteryManager",
                        "getIntProperty",
                        "(I)I",
                        &[6.into()], // BATTERY_PROPERTY_STATUS
                    ) {
                        Ok(result) => {
                            let status = result.i().unwrap_or(1); // BATTERY_STATUS_UNKNOWN = 1
                                                                  // BATTERY_STATUS_CHARGING = 2, BATTERY_STATUS_FULL = 5
                            status == 2 || status == 5
                        }
                        Err(e) => {
                            log::error!("Failed to get Android charging status: {}", e);
                            false
                        }
                    }
                }

                #[cfg(not(target_os = "android"))]
                false
            }
            PlatformType::Ios => {
                #[cfg(target_os = "ios")]
                {
                    use crate::mobile::ios::ffi_helpers::call_ios_method;

                    // Enable battery monitoring first
                    let _ =
                        call_ios_method("UIDevice", "setBatteryMonitoringEnabled:", &[true.into()]);

                    match call_ios_method("UIDevice", "batteryState", &[]) {
                        Ok(result) => {
                            let state = result.as_i32();
                            // UIDeviceBatteryStateCharging = 2, UIDeviceBatteryStateFull = 3
                            state == 2 || state == 3
                        }
                        Err(e) => {
                            log::error!("Failed to get iOS battery state: {}", e);
                            false
                        }
                    }
                }

                #[cfg(not(target_os = "ios"))]
                false
            }
            _ => false,
        }
    }
}

/// Battery optimization recommendation for users
#[derive(Clone)]
pub struct OptimizationRecommendation {
    pub title: String,
    pub description: String,
    pub action: String,
    pub urgency: RecommendationUrgency,
}

#[derive(Clone)]
pub enum RecommendationUrgency {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl RecommendationUrgency {
    pub fn priority(&self) -> u8 {
        match self {
            RecommendationUrgency::Info => 0,
            RecommendationUrgency::Low => 1,
            RecommendationUrgency::Medium => 2,
            RecommendationUrgency::High => 3,
            RecommendationUrgency::Critical => 4,
        }
    }
}

/// Battery-aware scanning strategy
pub struct BatteryAwareScanStrategy {
    platform_config: PlatformConfig,
    current_battery_level: Option<f32>,
    is_charging: bool,
    optimization_detected: bool,
}

impl BatteryAwareScanStrategy {
    pub fn new(platform_config: PlatformConfig) -> Self {
        Self {
            platform_config,
            current_battery_level: None,
            is_charging: false,
            optimization_detected: false,
        }
    }

    /// Update battery state
    pub fn update_battery_state(&mut self, battery_level: Option<f32>, is_charging: bool) {
        self.current_battery_level = battery_level;
        self.is_charging = is_charging;
    }

    /// Update optimization detection state
    pub fn set_optimization_detected(&mut self, detected: bool) {
        self.optimization_detected = detected;
    }

    /// Get optimal scan parameters based on current conditions
    pub fn get_optimal_scan_parameters(&self) -> (u32, u32, f32) {
        let base_window = self.platform_config.scan_window_ms;
        let base_interval = self.platform_config.scan_interval_ms;
        let mut duty_cycle = 1.0;

        // Adjust based on battery level
        if let Some(battery_level) = self.current_battery_level {
            if !self.is_charging {
                if battery_level < 0.1 {
                    // Critical battery - minimal scanning
                    duty_cycle = 0.1;
                } else if battery_level < 0.2 {
                    // Low battery - reduced scanning
                    duty_cycle = 0.3;
                } else if battery_level < 0.5 {
                    // Medium battery - moderate reduction
                    duty_cycle = 0.7;
                }
            }
        }

        // Adjust for optimization interference
        if self.optimization_detected {
            duty_cycle *= 0.5; // Reduce by 50% when optimization is detected
        }

        // Platform-specific adjustments
        let (window, interval) = match self.platform_config.platform {
            PlatformType::Android => {
                if self.optimization_detected {
                    // Use longer intervals to work with battery optimization
                    (base_window / 2, base_interval * 4)
                } else {
                    (base_window, base_interval)
                }
            }
            PlatformType::Ios => {
                // iOS is more restrictive
                (base_window / 2, base_interval * 2)
            }
            _ => (base_window, base_interval),
        };

        (window, interval, duty_cycle)
    }

    /// Get recommended power mode based on current conditions
    pub fn get_recommended_power_mode(&self) -> PowerMode {
        if let Some(battery_level) = self.current_battery_level {
            if !self.is_charging {
                if battery_level < 0.1 {
                    return PowerMode::UltraLowPower;
                } else if battery_level < 0.2 {
                    return PowerMode::BatterySaver;
                }
            }
        }

        if self.optimization_detected {
            PowerMode::BatterySaver
        } else {
            PowerMode::Balanced
        }
    }
}

/// Alias for backward compatibility
pub type BatteryOptimizationManager = BatteryOptimizationHandler;
