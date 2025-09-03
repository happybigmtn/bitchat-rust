//! Platform-specific adaptations for mobile devices

use super::*;
use serde::{Deserialize, Serialize};

/// Platform type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlatformType {
    Android,
    Ios,
    Desktop,
    Web,
    Unknown,
}

/// Platform adaptation manager for handling platform-specific behavior
pub struct PlatformAdaptationManager {
    platform_type: PlatformType,
    android_adapter: Option<AndroidAdapter>,
    ios_adapter: Option<IOSAdapter>,
}

impl Default for PlatformAdaptationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAdaptationManager {
    /// Create a new platform adaptation manager
    pub fn new() -> Self {
        let platform_type = PlatformDetection::detect_platform();

        let android_adapter = if matches!(platform_type, PlatformType::Android) {
            Some(AndroidAdapter::new())
        } else {
            None
        };

        let ios_adapter = if matches!(platform_type, PlatformType::Ios) {
            Some(IOSAdapter::new())
        } else {
            None
        };

        Self {
            platform_type,
            android_adapter,
            ios_adapter,
        }
    }

    /// Initialize platform-specific features
    pub async fn initialize(&self) -> Result<(), BitCrapsError> {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.initialize().await?;
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.initialize().await?;
                }
            }
            _ => {
                log::info!(
                    "No platform-specific initialization needed for {:?}",
                    self.platform_type
                );
            }
        }

        Ok(())
    }

    /// Handle app lifecycle changes
    pub async fn handle_lifecycle_change(
        &self,
        state: AppLifecycleState,
    ) -> Result<(), BitCrapsError> {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.handle_lifecycle_change(state).await?;
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.handle_lifecycle_change(state).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Request required permissions
    pub async fn request_permissions(&self) -> Result<PermissionStatus, BitCrapsError> {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.request_permissions().await
                } else {
                    Ok(PermissionStatus::Granted)
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.request_permissions().await
                } else {
                    Ok(PermissionStatus::Granted)
                }
            }
            _ => Ok(PermissionStatus::Granted),
        }
    }

    /// Configure for battery optimization
    pub async fn configure_battery_optimization(&self) -> Result<(), BitCrapsError> {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.configure_battery_optimization().await?;
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.configure_battery_optimization().await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Get platform-specific Bluetooth configuration
    pub fn get_bluetooth_config(&self) -> BluetoothPlatformConfig {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.get_bluetooth_config()
                } else {
                    BluetoothPlatformConfig::default_android()
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.get_bluetooth_config()
                } else {
                    BluetoothPlatformConfig::default_ios()
                }
            }
            _ => BluetoothPlatformConfig::default_desktop(),
        }
    }

    /// Check if background operation is available
    pub async fn is_background_operation_available(&self) -> bool {
        match self.platform_type {
            PlatformType::Android => {
                if let Some(adapter) = &self.android_adapter {
                    adapter.is_background_operation_available().await
                } else {
                    false
                }
            }
            PlatformType::Ios => {
                if let Some(adapter) = &self.ios_adapter {
                    adapter.is_background_operation_available().await
                } else {
                    false
                }
            }
            _ => true, // Desktop platforms generally support background operation
        }
    }
}

/// Android-specific adapter
struct AndroidAdapter {
    // JNI interface for Android-specific operations
}

impl AndroidAdapter {
    fn new() -> Self {
        Self {}
    }

    async fn initialize(&self) -> Result<(), BitCrapsError> {
        log::info!("Initializing Android-specific features");

        // Initialize logging
        #[cfg(target_os = "android")]
        {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_min_level(log::Level::Info)
                    .with_tag("BitCraps"),
            );
        }

        // Configure foreground service
        self.configure_foreground_service().await?;

        Ok(())
    }

    async fn handle_lifecycle_change(&self, state: AppLifecycleState) -> Result<(), BitCrapsError> {
        match state {
            AppLifecycleState::Foreground => {
                log::info!("App moved to foreground - enabling full features");
                // Enable full scanning and networking
            }
            AppLifecycleState::Background => {
                log::info!("App moved to background - enabling power saving");
                // Switch to background service mode
                self.enable_background_service().await?;
            }
            AppLifecycleState::Suspended => {
                log::info!("App suspended - minimal operation");
                // Stop non-essential operations
            }
        }

        Ok(())
    }

    async fn request_permissions(&self) -> Result<PermissionStatus, BitCrapsError> {
        log::info!("Requesting Android permissions via JNI");

        let required_permissions = [
            "android.permission.BLUETOOTH",
            "android.permission.BLUETOOTH_ADMIN",
            "android.permission.BLUETOOTH_ADVERTISE",
            "android.permission.BLUETOOTH_CONNECT",
            "android.permission.BLUETOOTH_SCAN",
            "android.permission.ACCESS_FINE_LOCATION",
            "android.permission.POST_NOTIFICATIONS",
        ];

        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            for permission in &required_permissions {
                // Check if permission is already granted
                match call_android_method(
                    "android/content/Context",
                    "checkSelfPermission",
                    "(Ljava/lang/String;)I",
                    &[permission.to_string().into()],
                ) {
                    Ok(result) => {
                        let status = result.i().unwrap_or(-1);
                        if status != 0 {
                            // PERMISSION_GRANTED = 0
                            log::info!("Requesting permission: {}", permission);

                            // Request the permission
                            let _ = call_android_method(
                                "androidx/core/app/ActivityCompat",
                                "requestPermissions",
                                "(Landroid/app/Activity;[Ljava/lang/String;I)V",
                                &[
                                    "activity".into(),
                                    vec![permission.to_string()].into(),
                                    1001.into(), // REQUEST_CODE
                                ],
                            );
                        } else {
                            log::debug!("Permission already granted: {}", permission);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to check permission {}: {}", permission, e);
                        return Err(BitCrapsError::Platform(format!(
                            "Failed to check permission {}: {}",
                            permission, e
                        )));
                    }
                }
            }

            Ok(PermissionStatus::Granted)
        }

        #[cfg(not(target_os = "android"))]
        Ok(PermissionStatus::Granted)
    }

    async fn configure_battery_optimization(&self) -> Result<(), BitCrapsError> {
        log::info!("Configuring Android battery optimization");

        // Check if app is whitelisted
        let is_whitelisted = self.check_battery_whitelist().await;

        if !is_whitelisted {
            // Request user to whitelist the app
            self.request_battery_whitelist().await?;
        }

        Ok(())
    }

    async fn configure_foreground_service(&self) -> Result<(), BitCrapsError> {
        log::info!("Configuring Android foreground service");

        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            // Create notification channel for foreground service
            match call_android_method(
                "android/app/NotificationChannel",
                "new",
                "(Ljava/lang/String;Ljava/lang/String;I)V",
                &[
                    "bitcraps_service".into(),
                    "BitCraps Game Service".into(),
                    3.into(), // IMPORTANCE_DEFAULT
                ],
            ) {
                Ok(_) => {
                    // Create the notification for foreground service
                    let notification_result = call_android_method(
                        "android/app/Notification$Builder",
                        "setSmallIcon",
                        "(I)Landroid/app/Notification$Builder;",
                        &[17301613.into()], // android.R.drawable.ic_dialog_info
                    )
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "setContentTitle",
                            "(Ljava/lang/String;)Landroid/app/Notification$Builder;",
                            &["BitCraps Gaming".into()],
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "setContentText",
                            "(Ljava/lang/String;)Landroid/app/Notification$Builder;",
                            &["Mesh gaming service running".into()],
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "build",
                            "()Landroid/app/Notification;",
                            &[],
                        )
                    });

                    match notification_result {
                        Ok(_) => {
                            // Start the foreground service
                            match call_android_method(
                                "android/app/Service",
                                "startForeground",
                                "(ILandroid/app/Notification;)V",
                                &[1.into(), "notification".into()],
                            ) {
                                Ok(_) => {
                                    log::info!("Android foreground service started successfully");
                                    Ok(())
                                }
                                Err(e) => {
                                    log::error!("Failed to start foreground service: {}", e);
                                    Err(BitCrapsError::Platform(format!(
                                        "Failed to start foreground service: {}",
                                        e
                                    )))
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create notification: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to create notification: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create notification channel: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to create notification channel: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            log::info!("Foreground service not needed on this platform");
            Ok(())
        }
    }

    async fn enable_background_service(&self) -> Result<(), BitCrapsError> {
        log::info!("Enabling Android background service");

        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            // Transition to background service mode by adjusting scanning parameters
            match call_android_method(
                "android/bluetooth/BluetoothAdapter",
                "getBluetoothLeScanner",
                "()Landroid/bluetooth/le/BluetoothLeScanner;",
                &[],
            ) {
                Ok(_) => {
                    // Set background-friendly scan settings
                    let scan_settings_result = call_android_method(
                        "android/bluetooth/le/ScanSettings$Builder",
                        "setScanMode",
                        "(I)Landroid/bluetooth/le/ScanSettings$Builder;",
                        &[0.into()], // SCAN_MODE_OPPORTUNISTIC for background
                    )
                    .and_then(|_| {
                        call_android_method(
                            "android/bluetooth/le/ScanSettings$Builder",
                            "setCallbackType",
                            "(I)Landroid/bluetooth/le/ScanSettings$Builder;",
                            &[1.into()], // CALLBACK_TYPE_ALL_MATCHES
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/bluetooth/le/ScanSettings$Builder",
                            "build",
                            "()Landroid/bluetooth/le/ScanSettings;",
                            &[],
                        )
                    });

                    match scan_settings_result {
                        Ok(_) => {
                            log::info!("Background service mode enabled successfully");
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to configure background scan settings: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to configure background scanning: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to get BLE scanner: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to enable background service: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            log::info!("Background service mode not applicable on this platform");
            Ok(())
        }
    }

    async fn check_battery_whitelist(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            match call_android_method(
                "android/os/PowerManager",
                "isIgnoringBatteryOptimizations",
                "(Ljava/lang/String;)Z",
                &["com.bitcraps.app".into()],
            ) {
                Ok(result) => result.z().unwrap_or(false),
                Err(e) => {
                    log::error!("Failed to check battery whitelist: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        true // Not applicable on other platforms
    }

    async fn request_battery_whitelist(&self) -> Result<(), BitCrapsError> {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            log::info!("Requesting battery optimization whitelist via Intent");

            // Create Intent for battery optimization settings
            match call_android_method(
                "android/content/Intent",
                "new",
                "(Ljava/lang/String;)V",
                &["android.settings.IGNORE_BATTERY_OPTIMIZATION_SETTINGS".into()],
            ) {
                Ok(_) => {
                    // Start the activity
                    match call_android_method(
                        "android/app/Activity",
                        "startActivity",
                        "(Landroid/content/Intent;)V",
                        &["intent".into()],
                    ) {
                        Ok(_) => {
                            log::info!("Battery optimization settings opened successfully");
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to start battery optimization activity: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to open battery settings: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create battery optimization intent: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to create battery optimization intent: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            log::info!("Battery optimization not applicable on this platform");
            Ok(())
        }
    }

    fn get_bluetooth_config(&self) -> BluetoothPlatformConfig {
        BluetoothPlatformConfig {
            max_connections: 7, // Most Android devices support 7 concurrent connections
            preferred_mtu: 244, // Android default MTU
            scan_window_ms: 500,
            scan_interval_ms: 1000,
            supports_peripheral_mode: true,
            supports_background_scanning: true,
            requires_location_permission: true,
            service_uuids_required: false,
            connection_priority: ConnectionPriority::Balanced,
        }
    }

    async fn is_background_operation_available(&self) -> bool {
        // Check if background restrictions are in place
        let has_restrictions = self.check_background_restrictions().await;
        let is_whitelisted = self.check_battery_whitelist().await;

        !has_restrictions || is_whitelisted
    }

    async fn check_background_restrictions(&self) -> bool {
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
                    log::error!("Failed to check background restrictions: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        false // Not applicable on other platforms
    }
}

/// iOS-specific adapter
struct IOSAdapter {
    // FFI interface for iOS-specific operations
}

impl IOSAdapter {
    fn new() -> Self {
        Self {}
    }

    async fn initialize(&self) -> Result<(), BitCrapsError> {
        log::info!("Initializing iOS-specific features");

        // Configure Core Bluetooth for background operation
        self.configure_core_bluetooth().await?;

        Ok(())
    }

    async fn handle_lifecycle_change(&self, state: AppLifecycleState) -> Result<(), BitCrapsError> {
        match state {
            AppLifecycleState::Foreground => {
                log::info!("App moved to foreground - enabling full features");
                // iOS allows full BLE operations in foreground
            }
            AppLifecycleState::Background => {
                log::info!("App moved to background - switching to background mode");
                // iOS severely limits background BLE operations
                self.enable_background_mode().await?;
            }
            AppLifecycleState::Suspended => {
                log::info!("App suspended - BLE operations will be terminated");
                // iOS terminates most operations when app is suspended
            }
        }

        Ok(())
    }

    async fn request_permissions(&self) -> Result<PermissionStatus, BitCrapsError> {
        log::info!("Requesting iOS permissions via FFI");

        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            // Check and request Bluetooth authorization
            match call_ios_method("CBManager", "authorization", &[]) {
                Ok(auth_status) => {
                    let status = auth_status.as_i32();
                    match status {
                        0 => {
                            // CBManagerAuthorizationNotDetermined
                            log::info!(
                                "Bluetooth authorization not determined, will prompt on first use"
                            );
                            Ok(PermissionStatus::NotDetermined)
                        }
                        1 => {
                            // CBManagerAuthorizationRestricted
                            log::warn!("Bluetooth access is restricted");
                            Ok(PermissionStatus::Restricted)
                        }
                        2 => {
                            // CBManagerAuthorizationDenied
                            log::warn!("Bluetooth access denied by user");
                            Ok(PermissionStatus::Denied)
                        }
                        3 => {
                            // CBManagerAuthorizationAllowedAlways
                            log::info!("Bluetooth access granted");
                            Ok(PermissionStatus::Granted)
                        }
                        _ => {
                            log::warn!("Unknown authorization status: {}", status);
                            Ok(PermissionStatus::NotDetermined)
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to check Bluetooth authorization: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to check Bluetooth permissions: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        Ok(PermissionStatus::Granted)
    }

    async fn configure_battery_optimization(&self) -> Result<(), BitCrapsError> {
        log::info!("Configuring iOS battery optimization");

        // Check if Background App Refresh is enabled
        let background_refresh_enabled = self.check_background_app_refresh().await;

        if !background_refresh_enabled {
            log::warn!("Background App Refresh is disabled - background operation will be limited");
        }

        Ok(())
    }

    async fn configure_core_bluetooth(&self) -> Result<(), BitCrapsError> {
        log::info!("Configuring iOS Core Bluetooth");

        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            // Configure CBCentralManager with background options
            match call_ios_method(
                "CBCentralManager",
                "initWithDelegate:queue:options:",
                &[
                    "self".into(),
                    "nil".into(),
                    "{
                        CBCentralManagerOptionShowPowerAlertKey: true,
                        CBCentralManagerOptionRestoreIdentifierKey: \"bitcraps_central\"
                    }"
                    .into(),
                ],
            ) {
                Ok(_) => {
                    log::info!("CBCentralManager configured for background operation");

                    // Set up service filtering for background scanning
                    let service_uuid = "12345678-1234-5678-1234-567812345678";
                    match call_ios_method("CBUUID", "UUIDWithString:", &[service_uuid.into()]) {
                        Ok(_) => {
                            log::info!("Service UUID configured for background scanning");
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to configure service UUID: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to configure service UUID: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to configure CBCentralManager: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to configure Core Bluetooth: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::info!("Core Bluetooth configuration not needed on this platform");
            Ok(())
        }
    }

    async fn enable_background_mode(&self) -> Result<(), BitCrapsError> {
        log::info!("Enabling iOS background mode");

        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            // Begin background task to extend execution time
            match call_ios_method(
                "UIApplication",
                "beginBackgroundTaskWithName:expirationHandler:",
                &[
                    "BitCraps Background BLE".into(),
                    "^{ NSLog(@\"Background task expired\"); }".into(),
                ],
            ) {
                Ok(_) => {
                    log::info!("Background task initiated");

                    // Configure scanning for background mode with service UUID filtering
                    match call_ios_method(
                        "CBCentralManager",
                        "scanForPeripheralsWithServices:options:",
                        &[
                            "@[[CBUUID UUIDWithString:@\"12345678-1234-5678-1234-567812345678\"]]".into(),
                            "@{
                                CBCentralManagerScanOptionAllowDuplicatesKey: @NO,
                                CBCentralManagerScanOptionSolicitedServiceUUIDsKey: @[[CBUUID UUIDWithString:@\"12345678-1234-5678-1234-567812345678\"]]
                            }".into()
                        ]
                    ) {
                        Ok(_) => {
                            log::info!("Background scanning configured with service UUID filtering");
                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to configure background scanning: {}", e);
                            Err(BitCrapsError::Platform(
                                format!("Failed to configure background scanning: {}", e)
                            ))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to begin background task: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to enable background mode: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::info!("Background mode not applicable on this platform");
            Ok(())
        }
    }

    async fn check_bluetooth_authorization(&self) -> bool {
        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            match call_ios_method("CBManager", "authorization", &[]) {
                Ok(auth_status) => {
                    let status = auth_status.as_i32();
                    // CBManagerAuthorizationAllowedAlways = 3
                    status == 3
                }
                Err(e) => {
                    log::error!("Failed to check Bluetooth authorization: {}", e);
                    false
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        true
    }

    async fn check_background_app_refresh(&self) -> bool {
        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            match call_ios_method("UIApplication", "backgroundRefreshStatus", &[]) {
                Ok(status) => {
                    let refresh_status = status.as_i32();
                    // UIBackgroundRefreshStatusAvailable = 1
                    refresh_status == 1
                }
                Err(e) => {
                    log::error!("Failed to check background app refresh status: {}", e);
                    true // Assume enabled if we can't check
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        true
    }

    fn get_bluetooth_config(&self) -> BluetoothPlatformConfig {
        BluetoothPlatformConfig {
            max_connections: 10, // iOS typically supports more connections
            preferred_mtu: 185,  // iOS conservative MTU
            scan_window_ms: 300,
            scan_interval_ms: 2000,
            supports_peripheral_mode: true,
            supports_background_scanning: false, // Very limited
            requires_location_permission: false,
            service_uuids_required: true, // Required for background
            connection_priority: ConnectionPriority::LowPower,
        }
    }

    async fn is_background_operation_available(&self) -> bool {
        let background_refresh = self.check_background_app_refresh().await;
        let bluetooth_authorized = self.check_bluetooth_authorization().await;

        background_refresh && bluetooth_authorized
    }
}

// Supporting types and enums

#[derive(Clone, Debug)]
pub enum AppLifecycleState {
    Foreground,
    Background,
    Suspended,
}

#[derive(Clone, Debug)]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
}

#[derive(Clone, Debug)]
pub struct BluetoothPlatformConfig {
    pub max_connections: u32,
    pub preferred_mtu: u16,
    pub scan_window_ms: u32,
    pub scan_interval_ms: u32,
    pub supports_peripheral_mode: bool,
    pub supports_background_scanning: bool,
    pub requires_location_permission: bool,
    pub service_uuids_required: bool,
    pub connection_priority: ConnectionPriority,
}

#[derive(Clone, Debug)]
pub enum ConnectionPriority {
    HighPerformance,
    Balanced,
    LowPower,
}

impl BluetoothPlatformConfig {
    pub fn default_android() -> Self {
        Self {
            max_connections: 7,
            preferred_mtu: 244,
            scan_window_ms: 500,
            scan_interval_ms: 1000,
            supports_peripheral_mode: true,
            supports_background_scanning: true,
            requires_location_permission: true,
            service_uuids_required: false,
            connection_priority: ConnectionPriority::Balanced,
        }
    }

    pub fn default_ios() -> Self {
        Self {
            max_connections: 10,
            preferred_mtu: 185,
            scan_window_ms: 300,
            scan_interval_ms: 2000,
            supports_peripheral_mode: true,
            supports_background_scanning: false,
            requires_location_permission: false,
            service_uuids_required: true,
            connection_priority: ConnectionPriority::LowPower,
        }
    }

    pub fn default_desktop() -> Self {
        Self {
            max_connections: 20,
            preferred_mtu: 512,
            scan_window_ms: 1000,
            scan_interval_ms: 1000,
            supports_peripheral_mode: true,
            supports_background_scanning: true,
            requires_location_permission: false,
            service_uuids_required: false,
            connection_priority: ConnectionPriority::HighPerformance,
        }
    }
}

/// Cross-platform notification manager
pub struct NotificationManager {
    platform_type: PlatformType,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            platform_type: PlatformDetection::detect_platform(),
        }
    }

    /// Show a notification to the user
    pub async fn show_notification(
        &self,
        notification: PlatformNotification,
    ) -> Result<(), BitCrapsError> {
        match self.platform_type {
            PlatformType::Android => self.show_android_notification(notification).await,
            PlatformType::Ios => self.show_ios_notification(notification).await,
            _ => {
                log::info!(
                    "Notification: {} - {}",
                    notification.title,
                    notification.body
                );
                Ok(())
            }
        }
    }

    async fn show_android_notification(
        &self,
        notification: PlatformNotification,
    ) -> Result<(), BitCrapsError> {
        #[cfg(target_os = "android")]
        {
            use crate::mobile::android::jni_helpers::call_android_method;

            log::info!(
                "Showing Android notification: {} - {}",
                notification.title,
                notification.body
            );

            // Get notification importance level
            let importance = match notification.priority {
                NotificationPriority::Low => 2,      // IMPORTANCE_LOW
                NotificationPriority::Normal => 3,   // IMPORTANCE_DEFAULT
                NotificationPriority::High => 4,     // IMPORTANCE_HIGH
                NotificationPriority::Critical => 5, // IMPORTANCE_MAX
            };

            // Create notification channel if needed (Android 8.0+)
            let channel_result = call_android_method(
                "android/app/NotificationChannel",
                "new",
                "(Ljava/lang/String;Ljava/lang/String;I)V",
                &[
                    "bitcraps_notifications".into(),
                    "BitCraps Game Notifications".into(),
                    importance.into(),
                ],
            );

            match channel_result {
                Ok(_) => {
                    // Build the notification
                    match call_android_method(
                        "android/app/Notification$Builder",
                        "setSmallIcon",
                        "(I)Landroid/app/Notification$Builder;",
                        &[17301640.into()], // android.R.drawable.ic_dialog_alert
                    )
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "setContentTitle",
                            "(Ljava/lang/String;)Landroid/app/Notification$Builder;",
                            &[notification.title.into()],
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "setContentText",
                            "(Ljava/lang/String;)Landroid/app/Notification$Builder;",
                            &[notification.body.into()],
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "setAutoCancel",
                            "(Z)Landroid/app/Notification$Builder;",
                            &[true.into()],
                        )
                    })
                    .and_then(|_| {
                        call_android_method(
                            "android/app/Notification$Builder",
                            "build",
                            "()Landroid/app/Notification;",
                            &[],
                        )
                    }) {
                        Ok(_) => {
                            // Show the notification
                            match call_android_method(
                                "android/app/NotificationManager",
                                "notify",
                                "(ILandroid/app/Notification;)V",
                                &[1001.into(), "notification".into()],
                            ) {
                                Ok(_) => {
                                    log::info!("Android notification shown successfully");
                                    Ok(())
                                }
                                Err(e) => {
                                    log::error!("Failed to show notification: {}", e);
                                    Err(BitCrapsError::Platform(format!(
                                        "Failed to show notification: {}",
                                        e
                                    )))
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to build notification: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to build notification: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create notification channel: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to create notification channel: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            log::info!(
                "Android notification: {} - {}",
                notification.title,
                notification.body
            );
            Ok(())
        }
    }

    async fn show_ios_notification(
        &self,
        notification: PlatformNotification,
    ) -> Result<(), BitCrapsError> {
        #[cfg(target_os = "ios")]
        {
            use crate::mobile::ios::ffi_helpers::call_ios_method;

            log::info!(
                "Showing iOS notification: {} - {}",
                notification.title,
                notification.body
            );

            // Create notification content
            match call_ios_method("UNMutableNotificationContent", "new", &[]) {
                Ok(_) => {
                    // Set notification content properties
                    let content_result = call_ios_method(
                        "UNMutableNotificationContent",
                        "setTitle:",
                        &[notification.title.into()],
                    )
                    .and_then(|_| {
                        call_ios_method(
                            "UNMutableNotificationContent",
                            "setBody:",
                            &[notification.body.into()],
                        )
                    })
                    .and_then(|_| {
                        // Set sound based on priority
                        let sound = match notification.priority {
                            NotificationPriority::Critical => "UNNotificationSoundDefaultCritical",
                            _ => "UNNotificationSoundDefault",
                        };
                        call_ios_method(
                            "UNMutableNotificationContent",
                            "setSound:",
                            &[sound.into()],
                        )
                    });

                    match content_result {
                        Ok(_) => {
                            // Create notification request
                            let request_id = format!(
                                "bitcraps_{}",
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                            );

                            match call_ios_method(
                                "UNNotificationRequest",
                                "requestWithIdentifier:content:trigger:",
                                &[
                                    request_id.into(),
                                    "content".into(),
                                    "nil".into(), // Immediate notification
                                ],
                            ) {
                                Ok(_) => {
                                    // Schedule the notification
                                    match call_ios_method(
                                        "UNUserNotificationCenter",
                                        "addNotificationRequest:withCompletionHandler:",
                                        &[
                                            "request".into(),
                                            "^(NSError *error) {
                                                if (error) {
                                                    NSLog(@\"Notification error: %@\", error);
                                                }
                                            }"
                                            .into(),
                                        ],
                                    ) {
                                        Ok(_) => {
                                            log::info!("iOS notification scheduled successfully");
                                            Ok(())
                                        }
                                        Err(e) => {
                                            log::error!("Failed to schedule notification: {}", e);
                                            Err(BitCrapsError::Platform(format!(
                                                "Failed to schedule notification: {}",
                                                e
                                            )))
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to create notification request: {}", e);
                                    Err(BitCrapsError::Platform(format!(
                                        "Failed to create notification request: {}",
                                        e
                                    )))
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to configure notification content: {}", e);
                            Err(BitCrapsError::Platform(format!(
                                "Failed to configure notification content: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create notification content: {}", e);
                    Err(BitCrapsError::Platform(format!(
                        "Failed to create notification content: {}",
                        e
                    )))
                }
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::info!(
                "iOS notification: {} - {}",
                notification.title,
                notification.body
            );
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct PlatformNotification {
    pub title: String,
    pub body: String,
    pub category: NotificationCategory,
    pub priority: NotificationPriority,
}

#[derive(Clone)]
pub enum NotificationCategory {
    GameInvite,
    GameUpdate,
    PeerDiscovered,
    BatteryOptimization,
    Error,
}

#[derive(Clone)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}
