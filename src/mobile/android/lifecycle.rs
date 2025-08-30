//! Android BLE Lifecycle Management
//!
//! This module handles Android application and service lifecycle events
//! to properly manage BLE operations, ensuring graceful handling of
//! background/foreground transitions, service restarts, and battery optimizations.

use super::{callbacks::CallbackManager, AndroidBleManager};
use crate::error::BitCrapsError;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::watch;

#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JClass, JObject, JString};
#[cfg(target_os = "android")]
use jni::{JNIEnv, JavaVM};

/// Android application lifecycle states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LifecycleState {
    Created,
    Started,
    Resumed,
    Paused,
    Stopped,
    Destroyed,
}

/// Android BLE power mode based on app state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlePowerMode {
    HighPerformance, // Foreground, active gaming
    Balanced,        // Foreground, idle
    PowerSaver,      // Background, limited scanning
    UltraLowPower,   // Background, minimal activity
}

/// Battery optimization detection and handling
#[derive(Debug, Clone)]
pub struct BatteryOptimizationState {
    pub is_whitelisted: bool,
    pub doze_mode_active: bool,
    pub battery_saver_active: bool,
    pub thermal_throttling_active: bool,
    pub last_check: Instant,
}

/// Android BLE lifecycle manager
pub struct AndroidBleLifecycleManager {
    ble_manager: Arc<AndroidBleManager>,
    callback_manager: Arc<CallbackManager>,

    current_state: Arc<RwLock<LifecycleState>>,
    power_mode: Arc<RwLock<BlePowerMode>>,
    battery_state: Arc<Mutex<BatteryOptimizationState>>,

    // Lifecycle event sender/receiver
    state_sender: watch::Sender<LifecycleState>,
    state_receiver: watch::Receiver<LifecycleState>,

    // Background operation management
    background_scan_timer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    last_activity: Arc<Mutex<Instant>>,

    #[cfg(target_os = "android")]
    java_vm: Option<JavaVM>,
    #[cfg(target_os = "android")]
    lifecycle_callback: Option<GlobalRef>,
}

impl AndroidBleLifecycleManager {
    pub fn new(
        ble_manager: Arc<AndroidBleManager>,
        callback_manager: Arc<CallbackManager>,
    ) -> Self {
        let (sender, receiver) = watch::channel(LifecycleState::Created);

        Self {
            ble_manager,
            callback_manager,
            current_state: Arc::new(RwLock::new(LifecycleState::Created)),
            power_mode: Arc::new(RwLock::new(BlePowerMode::Balanced)),
            battery_state: Arc::new(Mutex::new(BatteryOptimizationState {
                is_whitelisted: false,
                doze_mode_active: false,
                battery_saver_active: false,
                thermal_throttling_active: false,
                last_check: Instant::now(),
            })),
            state_sender: sender,
            state_receiver: receiver,
            background_scan_timer: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(Mutex::new(Instant::now())),
            #[cfg(target_os = "android")]
            java_vm: None,
            #[cfg(target_os = "android")]
            lifecycle_callback: None,
        }
    }

    #[cfg(target_os = "android")]
    pub fn set_java_vm(&mut self, vm: JavaVM) {
        self.java_vm = Some(vm);
    }

    #[cfg(target_os = "android")]
    pub fn set_lifecycle_callback(&mut self, callback: GlobalRef) {
        self.lifecycle_callback = Some(callback);
    }

    /// Start lifecycle management
    pub async fn start(&self) -> Result<(), BitCrapsError> {
        // Start monitoring lifecycle changes
        self.start_lifecycle_monitoring().await?;

        // Start battery optimization monitoring
        self.start_battery_monitoring().await?;

        // Update activity timestamp
        self.update_activity();

        log::info!("BLE lifecycle manager started");
        Ok(())
    }

    /// Stop lifecycle management
    pub async fn stop(&self) -> Result<(), BitCrapsError> {
        // Stop background tasks
        if let Ok(mut timer) = self.background_scan_timer.lock() {
            if let Some(handle) = timer.take() {
                handle.abort();
            }
        }

        log::info!("BLE lifecycle manager stopped");
        Ok(())
    }

    /// Handle Android lifecycle state change
    pub async fn on_lifecycle_changed(
        &self,
        new_state: LifecycleState,
    ) -> Result<(), BitCrapsError> {
        let old_state = {
            let mut current =
                self.current_state
                    .write()
                    .map_err(|_| BitCrapsError::BluetoothError {
                        message: "Failed to lock current state".to_string(),
                    })?;
            let old = *current;
            *current = new_state;
            old
        };

        if old_state == new_state {
            return Ok(());
        }

        log::info!(
            "Lifecycle state changed: {:?} -> {:?}",
            old_state,
            new_state
        );

        // Send state update
        let _ = self.state_sender.send(new_state);

        // Handle state-specific logic
        match new_state {
            LifecycleState::Created => {
                self.on_created().await?;
            }
            LifecycleState::Started => {
                self.on_started().await?;
            }
            LifecycleState::Resumed => {
                self.on_resumed().await?;
            }
            LifecycleState::Paused => {
                self.on_paused().await?;
            }
            LifecycleState::Stopped => {
                self.on_stopped().await?;
            }
            LifecycleState::Destroyed => {
                self.on_destroyed().await?;
            }
        }

        // Update power mode based on new state
        self.update_power_mode().await?;

        Ok(())
    }

    /// Handle application created
    async fn on_created(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application created");
        Ok(())
    }

    /// Handle application started
    async fn on_started(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application started");

        // Start callback manager if not running
        if !self.callback_manager.is_running() {
            self.callback_manager.start()?;
        }

        Ok(())
    }

    /// Handle application resumed (foreground)
    async fn on_resumed(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application resumed");

        self.update_activity();

        // Enable full BLE operations in foreground
        if !self.ble_manager.is_advertising() {
            self.ble_manager.start_advertising().await?;
        }

        if !self.ble_manager.is_scanning() {
            self.ble_manager.start_scanning().await?;
        }

        // Stop background scan timer since we're in foreground
        if let Ok(mut timer) = self.background_scan_timer.lock() {
            if let Some(handle) = timer.take() {
                handle.abort();
            }
        }

        Ok(())
    }

    /// Handle application paused (background)
    async fn on_paused(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application paused");

        self.update_activity();

        // Reduce BLE activity in background
        self.start_background_mode().await?;

        Ok(())
    }

    /// Handle application stopped
    async fn on_stopped(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application stopped");

        // Further reduce BLE activity
        self.enter_minimal_mode().await?;

        Ok(())
    }

    /// Handle application destroyed
    async fn on_destroyed(&self) -> Result<(), BitCrapsError> {
        log::debug!("Application destroyed");

        // Stop all BLE operations
        if self.ble_manager.is_advertising() {
            let _ = self.ble_manager.stop_advertising().await;
        }

        if self.ble_manager.is_scanning() {
            let _ = self.ble_manager.stop_scanning().await;
        }

        // Stop callback manager
        let _ = self.callback_manager.stop();

        Ok(())
    }

    /// Start background operation mode
    async fn start_background_mode(&self) -> Result<(), BitCrapsError> {
        log::info!("Entering background mode");

        // Implement intermittent scanning to preserve battery
        self.start_background_scan_timer().await?;

        Ok(())
    }

    /// Enter minimal power mode
    async fn enter_minimal_mode(&self) -> Result<(), BitCrapsError> {
        log::info!("Entering minimal power mode");

        // Stop continuous scanning, keep advertising if possible
        if self.ble_manager.is_scanning() {
            self.ble_manager.stop_scanning().await?;
        }

        // Start very infrequent scanning
        self.start_minimal_scan_timer().await?;

        Ok(())
    }

    /// Start background scan timer (intermittent scanning)
    async fn start_background_scan_timer(&self) -> Result<(), BitCrapsError> {
        let ble_manager = Arc::clone(&self.ble_manager);
        let timer_handle = Arc::clone(&self.background_scan_timer);

        // Cancel existing timer
        if let Ok(mut timer) = timer_handle.lock() {
            if let Some(handle) = timer.take() {
                handle.abort();
            }
        }

        // Start new timer
        let handle = tokio::spawn(async move {
            loop {
                // Scan for 10 seconds every 60 seconds in background
                tokio::time::sleep(Duration::from_secs(60)).await;

                log::debug!("Background scan cycle starting");

                if let Ok(()) = ble_manager.start_scanning().await {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    let _ = ble_manager.stop_scanning().await;
                }

                log::debug!("Background scan cycle completed");
            }
        });

        if let Ok(mut timer) = timer_handle.lock() {
            *timer = Some(handle);
        }

        Ok(())
    }

    /// Start minimal scan timer (very infrequent scanning)
    async fn start_minimal_scan_timer(&self) -> Result<(), BitCrapsError> {
        let ble_manager = Arc::clone(&self.ble_manager);
        let timer_handle = Arc::clone(&self.background_scan_timer);

        // Cancel existing timer
        if let Ok(mut timer) = timer_handle.lock() {
            if let Some(handle) = timer.take() {
                handle.abort();
            }
        }

        // Start new timer
        let handle = tokio::spawn(async move {
            loop {
                // Scan for 5 seconds every 5 minutes in minimal mode
                tokio::time::sleep(Duration::from_secs(300)).await;

                log::debug!("Minimal scan cycle starting");

                if let Ok(()) = ble_manager.start_scanning().await {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let _ = ble_manager.stop_scanning().await;
                }

                log::debug!("Minimal scan cycle completed");
            }
        });

        if let Ok(mut timer) = timer_handle.lock() {
            *timer = Some(handle);
        }

        Ok(())
    }

    /// Start lifecycle monitoring
    async fn start_lifecycle_monitoring(&self) -> Result<(), BitCrapsError> {
        // Monitor lifecycle state changes
        let mut receiver = self.state_receiver.clone();
        let lifecycle_manager = Arc::new(self as *const _ as usize); // Weak reference alternative

        tokio::spawn(async move {
            while receiver.changed().await.is_ok() {
                let state = *receiver.borrow();
                log::debug!("Lifecycle state monitor: {:?}", state);

                // Additional monitoring logic can be added here
            }
        });

        Ok(())
    }

    /// Start battery optimization monitoring
    async fn start_battery_monitoring(&self) -> Result<(), BitCrapsError> {
        let battery_state = Arc::clone(&self.battery_state);
        let power_mode = Arc::clone(&self.power_mode);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Check battery optimization state
                if let Ok(mut state) = battery_state.lock() {
                    state.last_check = Instant::now();

                    // Update power mode if necessary
                    let new_mode = if state.battery_saver_active || state.thermal_throttling_active
                    {
                        BlePowerMode::UltraLowPower
                    } else if state.doze_mode_active {
                        BlePowerMode::PowerSaver
                    } else {
                        BlePowerMode::Balanced
                    };

                    if let Ok(mut current_mode) = power_mode.write() {
                        if *current_mode != new_mode {
                            *current_mode = new_mode;
                            log::info!("Power mode changed to: {:?}", new_mode);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Update power mode based on current state
    async fn update_power_mode(&self) -> Result<(), BitCrapsError> {
        let current_state = self.get_current_state();
        let battery_state =
            self.battery_state
                .lock()
                .map_err(|_| BitCrapsError::BluetoothError {
                    message: "Failed to lock battery state".to_string(),
                })?;

        let new_mode = match current_state {
            LifecycleState::Resumed => {
                if battery_state.thermal_throttling_active {
                    BlePowerMode::PowerSaver
                } else {
                    BlePowerMode::HighPerformance
                }
            }
            LifecycleState::Paused => {
                if battery_state.battery_saver_active {
                    BlePowerMode::UltraLowPower
                } else {
                    BlePowerMode::PowerSaver
                }
            }
            LifecycleState::Stopped | LifecycleState::Destroyed => BlePowerMode::UltraLowPower,
            _ => BlePowerMode::Balanced,
        };

        let mut power_mode =
            self.power_mode
                .write()
                .map_err(|_| BitCrapsError::BluetoothError {
                    message: "Failed to lock power mode".to_string(),
                })?;

        if *power_mode != new_mode {
            *power_mode = new_mode;
            log::info!("Power mode updated to: {:?}", new_mode);
        }

        Ok(())
    }

    /// Update last activity timestamp
    pub fn update_activity(&self) {
        if let Ok(mut activity) = self.last_activity.lock() {
            *activity = Instant::now();
        }
    }

    /// Get current lifecycle state
    pub fn get_current_state(&self) -> LifecycleState {
        self.current_state
            .read()
            .map(|state| *state)
            .unwrap_or(LifecycleState::Created)
    }

    /// Get current power mode
    pub fn get_power_mode(&self) -> BlePowerMode {
        self.power_mode
            .read()
            .map(|mode| *mode)
            .unwrap_or(BlePowerMode::Balanced)
    }

    /// Get battery optimization state
    pub fn get_battery_state(&self) -> Result<BatteryOptimizationState, BitCrapsError> {
        let state = self
            .battery_state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock battery state".to_string(),
            })?;

        Ok(state.clone())
    }

    /// Update battery optimization state
    pub fn update_battery_state(
        &self,
        is_whitelisted: bool,
        doze_mode_active: bool,
        battery_saver_active: bool,
        thermal_throttling_active: bool,
    ) -> Result<(), BitCrapsError> {
        let mut state = self
            .battery_state
            .lock()
            .map_err(|_| BitCrapsError::BluetoothError {
                message: "Failed to lock battery state".to_string(),
            })?;

        state.is_whitelisted = is_whitelisted;
        state.doze_mode_active = doze_mode_active;
        state.battery_saver_active = battery_saver_active;
        state.thermal_throttling_active = thermal_throttling_active;
        state.last_check = Instant::now();

        log::debug!(
            "Battery state updated: whitelisted={}, doze={}, battery_saver={}, thermal={}",
            is_whitelisted,
            doze_mode_active,
            battery_saver_active,
            thermal_throttling_active
        );

        Ok(())
    }

    /// Check if app has been inactive for too long
    pub fn is_inactive(&self, threshold: Duration) -> bool {
        if let Ok(activity) = self.last_activity.lock() {
            activity.elapsed() > threshold
        } else {
            false
        }
    }

    /// Get state receiver for external monitoring
    pub fn get_state_receiver(&self) -> watch::Receiver<LifecycleState> {
        self.state_receiver.clone()
    }
}
