# Chapter 87: Configuration Management - Making Systems Flexible and Maintainable

## Understanding Configuration Through BitCraps Settings
*"Configuration is like a recipe - you want it flexible enough to adapt to different tastes, but structured enough that you don't accidentally poison anyone."*

---

## Part I: The Configuration Challenge

Imagine you're developing a video game that needs to work:
- On phones with different screen sizes
- In different countries with different laws
- With different network conditions (WiFi, cellular, offline)
- For players with different skill levels and preferences
- In development, testing, and production environments

Without good configuration management, you'd need to hard-code everything or maintain separate versions of your software. With BitCraps handling real money across multiple platforms and jurisdictions, configuration management becomes critical for both functionality and compliance.

Let's explore how the `src/config/` module handles these complex configuration needs.

## Part II: The BitCraps Configuration Architecture

### Hierarchical Configuration System

```rust
// From src/config/mod.rs
pub struct ConfigurationManager {
    base_config: BaseConfiguration,
    environment_config: EnvironmentConfiguration,
    user_config: UserConfiguration,
    runtime_overrides: RuntimeOverrides,
    config_sources: Vec<ConfigSource>,
    config_validators: Vec<ConfigValidator>,
    change_listeners: Vec<Arc<dyn ConfigChangeListener>>,
}

impl ConfigurationManager {
    pub async fn load_configuration(&mut self) -> Result<BitCrapsConfig, ConfigError> {
        // Load configurations in order of precedence (lowest to highest)
        
        // 1. Default/base configuration (embedded in code)
        let mut config = self.load_base_config()?;
        
        // 2. Environment-specific configuration (files on disk)
        let env_config = self.load_environment_config().await?;
        config.merge(env_config)?;
        
        // 3. User-specific configuration (user preferences)
        let user_config = self.load_user_config().await?;
        config.merge(user_config)?;
        
        // 4. Runtime overrides (command-line arguments, environment variables)
        let runtime_config = self.load_runtime_overrides()?;
        config.merge(runtime_config)?;
        
        // 5. Validate final configuration
        self.validate_configuration(&config).await?;
        
        // 6. Apply platform-specific adjustments
        let final_config = self.apply_platform_adjustments(config).await?;
        
        Ok(final_config)
    }
    
    fn load_base_config(&self) -> Result<BitCrapsConfig, ConfigError> {
        Ok(BitCrapsConfig {
            // Network settings
            network: NetworkConfig {
                max_connections: 50,
                connection_timeout: Duration::from_secs(30),
                retry_attempts: 3,
                preferred_protocols: vec![Protocol::Bluetooth, Protocol::WiFi],
                discovery_timeout: Duration::from_secs(60),
            },
            
            // Game settings
            game: GameConfig {
                max_simultaneous_games: 5,
                default_bet_limits: BetLimits {
                    minimum: 1,
                    maximum: 1000,
                },
                dice_animation_duration: Duration::from_millis(2000),
                auto_save_interval: Duration::from_secs(30),
            },
            
            // Security settings
            security: SecurityConfig {
                encryption_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                key_derivation_iterations: 100_000,
                session_timeout: Duration::from_secs(3600),
                require_biometric_auth: false,
                audit_level: AuditLevel::Standard,
            },
            
            // Performance settings
            performance: PerformanceConfig {
                cache_size_mb: 50,
                max_cpu_usage_percent: 80,
                background_processing_enabled: true,
                battery_optimization_mode: BatteryMode::Balanced,
            },
            
            // Platform-specific settings (will be overridden)
            platform: PlatformConfig::default(),
        })
    }
    
    async fn load_environment_config(&self) -> Result<EnvironmentConfiguration, ConfigError> {
        let env = self.detect_environment().await?;
        
        let config_file = match env {
            Environment::Development => "config/development.toml",
            Environment::Testing => "config/testing.toml",
            Environment::Staging => "config/staging.toml",
            Environment::Production => "config/production.toml",
        };
        
        // Try to load from file
        if let Ok(file_contents) = tokio::fs::read_to_string(config_file).await {
            let env_config: EnvironmentConfiguration = toml::from_str(&file_contents)?;
            Ok(env_config)
        } else {
            // File doesn't exist - use environment-specific defaults
            Ok(self.get_environment_defaults(env))
        }
    }
    
    async fn load_user_config(&self) -> Result<UserConfiguration, ConfigError> {
        let user_config_path = self.get_user_config_path().await?;
        
        if user_config_path.exists() {
            // Load existing user configuration
            let config_contents = tokio::fs::read_to_string(&user_config_path).await?;
            let user_config: UserConfiguration = serde_json::from_str(&config_contents)?;
            Ok(user_config)
        } else {
            // Create default user configuration
            let default_user_config = UserConfiguration::default();
            self.save_user_config(&default_user_config).await?;
            Ok(default_user_config)
        }
    }
    
    fn load_runtime_overrides(&self) -> Result<RuntimeOverrides, ConfigError> {
        let mut overrides = RuntimeOverrides::new();
        
        // Command-line arguments take precedence
        let args = std::env::args().collect::<Vec<_>>();
        for arg in args.windows(2) {
            match arg[0].as_str() {
                "--max-connections" => {
                    overrides.max_connections = Some(arg[1].parse()?);
                }
                "--log-level" => {
                    overrides.log_level = Some(arg[1].parse()?);
                }
                "--disable-bluetooth" => {
                    overrides.bluetooth_enabled = Some(false);
                }
                "--production-mode" => {
                    overrides.environment = Some(Environment::Production);
                }
                _ => {} // Ignore unknown arguments
            }
        }
        
        // Environment variables
        if let Ok(max_conn) = std::env::var("BITCRAPS_MAX_CONNECTIONS") {
            overrides.max_connections = Some(max_conn.parse()?);
        }
        
        if let Ok(debug) = std::env::var("BITCRAPS_DEBUG") {
            overrides.debug_mode = Some(debug.parse().unwrap_or(false));
        }
        
        Ok(overrides)
    }
}
```

### Platform-Specific Configuration

Different platforms have different capabilities and constraints:

```rust
// From src/config/mod.rs (platform-specific extensions)
impl ConfigurationManager {
    async fn apply_platform_adjustments(&self, mut config: BitCrapsConfig) -> Result<BitCrapsConfig, ConfigError> {
        let platform = self.detect_platform().await;
        
        match platform {
            Platform::Android => {
                config = self.apply_android_adjustments(config).await?;
            }
            Platform::iOS => {
                config = self.apply_ios_adjustments(config).await?;
            }
            Platform::Desktop => {
                config = self.apply_desktop_adjustments(config).await?;
            }
            Platform::Web => {
                config = self.apply_web_adjustments(config).await?;
            }
        }
        
        Ok(config)
    }
    
    async fn apply_android_adjustments(&self, mut config: BitCrapsConfig) -> Result<BitCrapsConfig, ConfigError> {
        // Android-specific adjustments
        
        // Check if device has limited RAM
        let device_ram = self.get_device_ram().await?;
        if device_ram < 4_000_000_000 { // Less than 4GB
            config.performance.cache_size_mb = 25; // Reduce cache size
            config.game.max_simultaneous_games = 3; // Fewer concurrent games
        }
        
        // Check battery optimization settings
        if self.is_battery_optimized().await? {
            config.performance.background_processing_enabled = false;
            config.network.connection_timeout = Duration::from_secs(60); // Longer timeout for doze mode
        }
        
        // Check for specific Android version limitations
        let android_version = self.get_android_version().await?;
        if android_version < 26 { // Android 8.0
            // Limited background processing on older Android
            config.performance.battery_optimization_mode = BatteryMode::Aggressive;
        }
        
        // Bluetooth restrictions
        if !self.has_bluetooth_permission().await? {
            config.network.preferred_protocols.retain(|p| *p != Protocol::Bluetooth);
        }
        
        // Storage-based adjustments
        let available_storage = self.get_available_storage().await?;
        if available_storage < 1_000_000_000 { // Less than 1GB free
            config.game.auto_save_interval = Duration::from_secs(60); // Save less frequently
            config.performance.cache_size_mb = 10; // Minimal cache
        }
        
        Ok(config)
    }
    
    async fn apply_ios_adjustments(&self, mut config: BitCrapsConfig) -> Result<BitCrapsConfig, ConfigError> {
        // iOS-specific adjustments
        
        // iOS has strict memory limits
        config.performance.cache_size_mb = 30; // Conservative cache size
        
        // iOS background processing is very limited
        config.performance.background_processing_enabled = false;
        
        // iOS requires biometric authentication for financial apps
        config.security.require_biometric_auth = true;
        
        // Check iOS version for feature availability
        let ios_version = self.get_ios_version().await?;
        if ios_version >= 15.0 {
            // iOS 15+ has better Bluetooth capabilities
            config.network.discovery_timeout = Duration::from_secs(45);
        } else {
            // Older iOS has more Bluetooth limitations
            config.network.discovery_timeout = Duration::from_secs(90);
        }
        
        // Check device capabilities
        let device_model = self.get_device_model().await?;
        if device_model.contains("iPhone SE") || device_model.contains("iPhone 8") {
            // Older/smaller devices need more conservative settings
            config.game.max_simultaneous_games = 2;
            config.performance.max_cpu_usage_percent = 60;
        }
        
        Ok(config)
    }
    
    async fn apply_desktop_adjustments(&self, mut config: BitCrapsConfig) -> Result<BitCrapsConfig, ConfigError> {
        // Desktop systems typically have more resources
        config.performance.cache_size_mb = 200;
        config.game.max_simultaneous_games = 10;
        config.network.max_connections = 100;
        
        // Desktop can handle more aggressive processing
        config.performance.max_cpu_usage_percent = 90;
        config.performance.background_processing_enabled = true;
        
        // Check for development environment
        if self.is_development_environment().await? {
            config.security.audit_level = AuditLevel::Verbose;
            config.game.dice_animation_duration = Duration::from_millis(500); // Faster for testing
        }
        
        Ok(config)
    }
}
```

### Dynamic Configuration Updates

BitCraps can update configuration at runtime without restarting:

```rust
pub struct DynamicConfigurationManager {
    current_config: Arc<RwLock<BitCrapsConfig>>,
    config_watchers: Vec<ConfigWatcher>,
    update_queue: mpsc::UnboundedReceiver<ConfigUpdate>,
    validation_engine: ConfigValidationEngine,
}

impl DynamicConfigurationManager {
    pub async fn start_watching(&mut self) -> Result<(), ConfigError> {
        // Watch configuration files for changes
        let file_watcher = self.create_file_watcher().await?;
        
        // Watch for remote configuration updates
        let remote_watcher = self.create_remote_watcher().await?;
        
        // Watch for system changes that might affect configuration
        let system_watcher = self.create_system_watcher().await?;
        
        // Start processing configuration updates
        tokio::spawn({
            let config = self.current_config.clone();
            let validation_engine = self.validation_engine.clone();
            let mut update_receiver = self.update_queue.clone();
            
            async move {
                while let Some(update) = update_receiver.recv().await {
                    if let Err(e) = Self::process_config_update(
                        &config,
                        &validation_engine,
                        update
                    ).await {
                        eprintln!("Failed to process config update: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    async fn process_config_update(
        config: &Arc<RwLock<BitCrapsConfig>>,
        validator: &ConfigValidationEngine,
        update: ConfigUpdate
    ) -> Result<(), ConfigError> {
        
        // Create proposed new configuration
        let mut proposed_config = {
            let current = config.read().await;
            current.clone()
        };
        
        // Apply the update
        match update.update_type {
            ConfigUpdateType::NetworkSettings(network_update) => {
                proposed_config.network = network_update;
            }
            ConfigUpdateType::GameSettings(game_update) => {
                proposed_config.game = game_update;
            }
            ConfigUpdateType::SecuritySettings(security_update) => {
                proposed_config.security = security_update;
            }
            ConfigUpdateType::PerformanceSettings(performance_update) => {
                proposed_config.performance = performance_update;
            }
        }
        
        // Validate proposed configuration
        if let Err(validation_error) = validator.validate(&proposed_config).await {
            return Err(ConfigError::ValidationFailed(validation_error));
        }
        
        // Test proposed configuration if possible
        if update.requires_testing {
            if let Err(test_error) = Self::test_configuration(&proposed_config).await {
                return Err(ConfigError::TestFailed(test_error));
            }
        }
        
        // Apply the configuration atomically
        {
            let mut current = config.write().await;
            *current = proposed_config;
        }
        
        // Notify listeners of configuration change
        Self::notify_configuration_changed(&update).await;
        
        Ok(())
    }
    
    pub async fn update_configuration_from_ui(&self, 
        ui_settings: UISettings
    ) -> Result<(), ConfigError> {
        
        let config_update = self.convert_ui_settings_to_config(ui_settings).await?;
        
        // Validate UI-driven changes more strictly
        self.validate_ui_configuration_update(&config_update).await?;
        
        // Queue the update
        self.queue_configuration_update(config_update).await?;
        
        Ok(())
    }
    
    async fn validate_ui_configuration_update(&self, 
        update: &ConfigUpdate
    ) -> Result<(), ConfigError> {
        
        // UI updates need additional validation
        match &update.update_type {
            ConfigUpdateType::GameSettings(game_settings) => {
                // Ensure bet limits are reasonable
                if game_settings.default_bet_limits.maximum > 10_000 {
                    return Err(ConfigError::UnsafeBetLimit);
                }
                
                // Ensure user doesn't disable all safety features
                if game_settings.max_simultaneous_games > 20 {
                    return Err(ConfigError::ExcessiveConcurrency);
                }
            }
            
            ConfigUpdateType::SecuritySettings(security_settings) => {
                // Don't allow users to disable critical security features
                if security_settings.audit_level == AuditLevel::None {
                    return Err(ConfigError::SecurityDowngrade);
                }
                
                // Ensure reasonable session timeout
                if security_settings.session_timeout > Duration::from_secs(86400) {
                    return Err(ConfigError::ExcessiveSessionTimeout);
                }
            }
            
            _ => {} // Other settings are generally safe for user modification
        }
        
        Ok(())
    }
}
```

### Configuration Validation and Testing

BitCraps validates configurations before applying them:

```rust
pub struct ConfigValidationEngine {
    validators: Vec<Box<dyn ConfigValidator>>,
    test_environment: Option<TestEnvironment>,
}

impl ConfigValidationEngine {
    pub async fn validate(&self, config: &BitCrapsConfig) -> Result<(), ValidationError> {
        // Run all validators
        for validator in &self.validators {
            validator.validate(config).await?;
        }
        
        Ok(())
    }
    
    pub async fn test_configuration(&self, config: &BitCrapsConfig) -> Result<TestResults, TestError> {
        let test_env = self.test_environment.as_ref()
            .ok_or(TestError::NoTestEnvironment)?;
        
        let mut test_results = TestResults::new();
        
        // Test network connectivity with new settings
        let network_test = test_env.test_network_configuration(&config.network).await?;
        test_results.add_result("network", network_test);
        
        // Test game functionality
        let game_test = test_env.test_game_configuration(&config.game).await?;
        test_results.add_result("game", game_test);
        
        // Test security settings
        let security_test = test_env.test_security_configuration(&config.security).await?;
        test_results.add_result("security", security_test);
        
        // Test performance impact
        let performance_test = test_env.test_performance_configuration(&config.performance).await?;
        test_results.add_result("performance", performance_test);
        
        Ok(test_results)
    }
}

// Specific validator implementations
pub struct NetworkConfigValidator;

impl ConfigValidator for NetworkConfigValidator {
    async fn validate(&self, config: &BitCrapsConfig) -> Result<(), ValidationError> {
        let network = &config.network;
        
        // Validate connection limits
        if network.max_connections == 0 {
            return Err(ValidationError::InvalidValue("max_connections cannot be zero".to_string()));
        }
        
        if network.max_connections > 1000 {
            return Err(ValidationError::InvalidValue("max_connections too high".to_string()));
        }
        
        // Validate timeout values
        if network.connection_timeout < Duration::from_secs(5) {
            return Err(ValidationError::InvalidValue("connection_timeout too short".to_string()));
        }
        
        if network.connection_timeout > Duration::from_secs(300) {
            return Err(ValidationError::InvalidValue("connection_timeout too long".to_string()));
        }
        
        // Ensure at least one protocol is enabled
        if network.preferred_protocols.is_empty() {
            return Err(ValidationError::InvalidValue("must have at least one preferred protocol".to_string()));
        }
        
        Ok(())
    }
}

pub struct SecurityConfigValidator;

impl ConfigValidator for SecurityConfigValidator {
    async fn validate(&self, config: &BitCrapsConfig) -> Result<(), ValidationError> {
        let security = &config.security;
        
        // Validate encryption settings
        match security.encryption_algorithm {
            EncryptionAlgorithm::ChaCha20Poly1305 |
            EncryptionAlgorithm::AES256GCM => {
                // These are acceptable
            }
            EncryptionAlgorithm::AES128 => {
                return Err(ValidationError::WeakSecurity("AES128 not recommended for financial data".to_string()));
            }
        }
        
        // Validate key derivation iterations
        if security.key_derivation_iterations < 10_000 {
            return Err(ValidationError::WeakSecurity("key derivation iterations too low".to_string()));
        }
        
        // Validate session timeout
        if security.session_timeout < Duration::from_secs(60) {
            return Err(ValidationError::InvalidValue("session timeout too short".to_string()));
        }
        
        // In production, certain security features must be enabled
        if std::env::var("BITCRAPS_ENV").unwrap_or_default() == "production" {
            if security.audit_level == AuditLevel::None {
                return Err(ValidationError::ProductionSecurityRequirement("audit logging required in production".to_string()));
            }
        }
        
        Ok(())
    }
}
```

## Part III: Configuration Secrets Management

Sensitive configuration data requires special handling:

```rust
// From src/config/secrets.rs
pub struct SecretsManager {
    vault_client: Option<VaultClient>,
    local_keystore: LocalKeystore,
    encryption_key: SecretKey,
    secrets_cache: Arc<RwLock<HashMap<String, CachedSecret>>>,
}

impl SecretsManager {
    pub async fn get_secret(&self, key: &str) -> Result<String, SecretsError> {
        // Check cache first (with expiration)
        {
            let cache = self.secrets_cache.read().await;
            if let Some(cached_secret) = cache.get(key) {
                if !cached_secret.is_expired() {
                    return Ok(cached_secret.value.clone());
                }
            }
        }
        
        // Try to load from vault (if available)
        if let Some(vault) = &self.vault_client {
            if let Ok(secret) = vault.get_secret(key).await {
                self.cache_secret(key, &secret, Duration::from_secs(3600)).await;
                return Ok(secret);
            }
        }
        
        // Fall back to local keystore
        let secret = self.local_keystore.get_secret(key).await?;
        self.cache_secret(key, &secret, Duration::from_secs(300)).await; // Shorter cache for local secrets
        
        Ok(secret)
    }
    
    pub async fn set_secret(&self, key: &str, value: &str) -> Result<(), SecretsError> {
        // Always store in local keystore as backup
        self.local_keystore.set_secret(key, value).await?;
        
        // Store in vault if available
        if let Some(vault) = &self.vault_client {
            vault.set_secret(key, value).await?;
        }
        
        // Update cache
        self.cache_secret(key, value, Duration::from_secs(3600)).await;
        
        Ok(())
    }
    
    async fn cache_secret(&self, key: &str, value: &str, ttl: Duration) {
        let cached_secret = CachedSecret {
            value: value.to_string(),
            expires_at: Instant::now() + ttl,
        };
        
        let mut cache = self.secrets_cache.write().await;
        cache.insert(key.to_string(), cached_secret);
    }
    
    // Load secrets into configuration
    pub async fn resolve_configuration_secrets(&self, config: &mut BitCrapsConfig) -> Result<(), SecretsError> {
        // Database connection string
        if config.database.connection_string.starts_with("$SECRET:") {
            let secret_key = config.database.connection_string.strip_prefix("$SECRET:").unwrap();
            config.database.connection_string = self.get_secret(secret_key).await?;
        }
        
        // API keys
        for api_config in &mut config.external_apis {
            if api_config.api_key.starts_with("$SECRET:") {
                let secret_key = api_config.api_key.strip_prefix("$SECRET:").unwrap();
                api_config.api_key = self.get_secret(secret_key).await?;
            }
        }
        
        // Encryption keys
        if config.security.master_key.starts_with("$SECRET:") {
            let secret_key = config.security.master_key.strip_prefix("$SECRET:").unwrap();
            config.security.master_key = self.get_secret(secret_key).await?;
        }
        
        Ok(())
    }
}

struct LocalKeystore {
    keystore_path: PathBuf,
    encryption_key: SecretKey,
}

impl LocalKeystore {
    async fn get_secret(&self, key: &str) -> Result<String, KeystoreError> {
        let encrypted_path = self.keystore_path.join(format!("{}.encrypted", key));
        
        if !encrypted_path.exists() {
            return Err(KeystoreError::SecretNotFound);
        }
        
        let encrypted_data = tokio::fs::read(&encrypted_path).await?;
        let decrypted_data = self.decrypt_data(&encrypted_data)?;
        
        Ok(String::from_utf8(decrypted_data)?)
    }
    
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), KeystoreError> {
        let encrypted_data = self.encrypt_data(value.as_bytes())?;
        let encrypted_path = self.keystore_path.join(format!("{}.encrypted", key));
        
        // Ensure keystore directory exists
        if let Some(parent) = encrypted_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Write encrypted secret to file
        tokio::fs::write(&encrypted_path, &encrypted_data).await?;
        
        // Set restrictive permissions (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&encrypted_path, permissions)?;
        }
        
        Ok(())
    }
}
```

## Part IV: Environment-Aware Configuration

BitCraps adapts its configuration based on the deployment environment:

```rust
pub struct EnvironmentAwareConfig {
    environment: Environment,
    region: Region,
    deployment_stage: DeploymentStage,
    feature_flags: FeatureFlags,
}

impl EnvironmentAwareConfig {
    pub async fn create_environment_config(&self) -> Result<BitCrapsConfig, ConfigError> {
        let mut config = BitCrapsConfig::default();
        
        // Apply environment-specific settings
        match self.environment {
            Environment::Development => {
                config.security.audit_level = AuditLevel::Verbose;
                config.game.dice_animation_duration = Duration::from_millis(100); // Fast for testing
                config.network.connection_timeout = Duration::from_secs(5); // Quick failures
                config.performance.cache_size_mb = 10; // Small cache for testing
            }
            
            Environment::Testing => {
                config.security.audit_level = AuditLevel::Standard;
                config.game.max_simultaneous_games = 1; // Isolated testing
                config.network.max_connections = 10; // Limited connections
                config.performance.background_processing_enabled = false; // Deterministic testing
            }
            
            Environment::Staging => {
                // Production-like but with some testing accommodations
                config = self.create_production_config().await?;
                config.security.audit_level = AuditLevel::Verbose; // Extra logging
                config.game.auto_save_interval = Duration::from_secs(10); // Frequent saves for testing
            }
            
            Environment::Production => {
                config = self.create_production_config().await?;
            }
        }
        
        // Apply regional settings
        self.apply_regional_settings(&mut config).await?;
        
        // Apply deployment stage settings
        self.apply_deployment_stage_settings(&mut config).await?;
        
        // Apply feature flags
        self.apply_feature_flags(&mut config).await?;
        
        Ok(config)
    }
    
    async fn create_production_config(&self) -> Result<BitCrapsConfig, ConfigError> {
        Ok(BitCrapsConfig {
            network: NetworkConfig {
                max_connections: 500,
                connection_timeout: Duration::from_secs(30),
                retry_attempts: 5,
                preferred_protocols: vec![Protocol::WiFi, Protocol::Cellular],
                discovery_timeout: Duration::from_secs(120),
            },
            
            game: GameConfig {
                max_simultaneous_games: 10,
                default_bet_limits: BetLimits {
                    minimum: 1,
                    maximum: 1000,
                },
                dice_animation_duration: Duration::from_millis(2000),
                auto_save_interval: Duration::from_secs(60),
            },
            
            security: SecurityConfig {
                encryption_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                key_derivation_iterations: 200_000, // Higher for production
                session_timeout: Duration::from_secs(1800), // 30 minutes
                require_biometric_auth: true,
                audit_level: AuditLevel::Standard,
            },
            
            performance: PerformanceConfig {
                cache_size_mb: 200,
                max_cpu_usage_percent: 80,
                background_processing_enabled: true,
                battery_optimization_mode: BatteryMode::Balanced,
            },
            
            platform: PlatformConfig::default(),
        })
    }
    
    async fn apply_regional_settings(&self, config: &mut BitCrapsConfig) -> Result<(), ConfigError> {
        match self.region {
            Region::UnitedStates => {
                // US-specific regulations
                config.security.audit_level = AuditLevel::Verbose; // Enhanced compliance
                config.game.default_bet_limits.maximum = 500; // State gambling limits
                config.security.require_biometric_auth = true;
            }
            
            Region::EuropeanUnion => {
                // GDPR and other EU requirements
                config.security.audit_level = AuditLevel::Privacy; // GDPR compliance
                config.data_retention_days = 30; // EU data retention limits
                config.security.require_explicit_consent = true;
            }
            
            Region::Japan => {
                // Japan-specific gaming regulations
                config.game.default_bet_limits.maximum = 100; // Strict gambling laws
                config.game.max_play_time_per_day = Some(Duration::from_secs(3600)); // 1 hour limit
            }
            
            Region::China => {
                // China-specific restrictions
                config.network.preferred_protocols = vec![Protocol::WiFi]; // No Bluetooth in some regions
                config.game.default_bet_limits.maximum = 0; // No gambling
                config.game.play_money_only = true;
            }
            
            _ => {
                // Default international settings
            }
        }
        
        Ok(())
    }
    
    async fn apply_feature_flags(&self, config: &mut BitCrapsConfig) -> Result<(), ConfigError> {
        // Check feature flags to enable/disable features
        if self.feature_flags.is_enabled("advanced_crypto").await? {
            config.security.encryption_algorithm = EncryptionAlgorithm::PostQuantum;
        }
        
        if self.feature_flags.is_enabled("beta_ui").await? {
            config.ui.enable_experimental_features = true;
        }
        
        if self.feature_flags.is_enabled("performance_mode").await? {
            config.performance.max_cpu_usage_percent = 95;
            config.performance.cache_size_mb *= 2;
        }
        
        if !self.feature_flags.is_enabled("bluetooth_support").await? {
            config.network.preferred_protocols.retain(|p| *p != Protocol::Bluetooth);
        }
        
        Ok(())
    }
}
```

## Part V: Practical Configuration Exercise

Let's build a flexible configuration system:

**Exercise: Multi-Environment Game Config**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub game_rules: GameRulesConfig,
    pub security: SecuritySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_players: usize,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRulesConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_games_per_player: u8,
    pub dice_sides: u8,
}

pub struct ConfigManager {
    environment: String,
    config_sources: Vec<ConfigSource>,
    cached_config: Option<GameServerConfig>,
}

impl ConfigManager {
    pub fn new() -> Self {
        let environment = std::env::var("GAME_ENV").unwrap_or_else(|_| "development".to_string());
        
        ConfigManager {
            environment,
            config_sources: vec![
                ConfigSource::File(format!("config/{}.toml", environment)),
                ConfigSource::EnvironmentVariables,
                ConfigSource::CommandLineArgs,
            ],
            cached_config: None,
        }
    }
    
    pub async fn load_config(&mut self) -> Result<GameServerConfig, ConfigError> {
        // Start with base configuration
        let mut config = self.get_base_config();
        
        // Apply each config source in order
        for source in &self.config_sources {
            match source {
                ConfigSource::File(path) => {
                    if let Ok(file_config) = self.load_from_file(path).await {
                        config = self.merge_configs(config, file_config);
                    }
                }
                ConfigSource::EnvironmentVariables => {
                    let env_config = self.load_from_env();
                    config = self.merge_configs(config, env_config);
                }
                ConfigSource::CommandLineArgs => {
                    let args_config = self.load_from_args();
                    config = self.merge_configs(config, args_config);
                }
            }
        }
        
        // Validate configuration
        self.validate_config(&config)?;
        
        // Cache the result
        self.cached_config = Some(config.clone());
        
        Ok(config)
    }
    
    fn get_base_config(&self) -> GameServerConfig {
        GameServerConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                max_players: 100,
                timeout_seconds: 30,
            },
            database: DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                max_connections: 10,
                timeout_seconds: 5,
            },
            game_rules: GameRulesConfig {
                min_bet: 1,
                max_bet: 100,
                max_games_per_player: 5,
                dice_sides: 6,
            },
            security: SecuritySettings {
                require_auth: false,
                max_failed_logins: 3,
                session_timeout_minutes: 30,
            },
        }
    }
    
    async fn load_from_file(&self, path: &str) -> Result<GameServerConfig, std::io::Error> {
        let contents = tokio::fs::read_to_string(path).await?;
        let config: GameServerConfig = toml::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }
    
    fn load_from_env(&self) -> GameServerConfig {
        let mut config = self.get_base_config();
        
        // Override with environment variables
        if let Ok(host) = std::env::var("GAME_SERVER_HOST") {
            config.server.host = host;
        }
        
        if let Ok(port) = std::env::var("GAME_SERVER_PORT") {
            if let Ok(port_num) = port.parse() {
                config.server.port = port_num;
            }
        }
        
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database.url = db_url;
        }
        
        if let Ok(max_bet) = std::env::var("GAME_MAX_BET") {
            if let Ok(bet_amount) = max_bet.parse() {
                config.game_rules.max_bet = bet_amount;
            }
        }
        
        config
    }
    
    fn load_from_args(&self) -> GameServerConfig {
        let mut config = self.get_base_config();
        let args: Vec<String> = std::env::args().collect();
        
        // Simple argument parsing
        for i in 0..args.len() - 1 {
            match args[i].as_str() {
                "--port" => {
                    if let Ok(port) = args[i + 1].parse() {
                        config.server.port = port;
                    }
                }
                "--max-players" => {
                    if let Ok(max) = args[i + 1].parse() {
                        config.server.max_players = max;
                    }
                }
                "--database-url" => {
                    config.database.url = args[i + 1].clone();
                }
                _ => {}
            }
        }
        
        config
    }
    
    fn merge_configs(&self, base: GameServerConfig, override_config: GameServerConfig) -> GameServerConfig {
        // In a real implementation, this would be more sophisticated
        // For now, we'll just use the override values where they differ from defaults
        GameServerConfig {
            server: ServerConfig {
                host: if override_config.server.host != base.server.host {
                    override_config.server.host
                } else {
                    base.server.host
                },
                port: if override_config.server.port != 8080 {
                    override_config.server.port
                } else {
                    base.server.port
                },
                max_players: if override_config.server.max_players != 100 {
                    override_config.server.max_players
                } else {
                    base.server.max_players
                },
                timeout_seconds: if override_config.server.timeout_seconds != 30 {
                    override_config.server.timeout_seconds
                } else {
                    base.server.timeout_seconds
                },
            },
            database: override_config.database,
            game_rules: override_config.game_rules,
            security: override_config.security,
        }
    }
    
    fn validate_config(&self, config: &GameServerConfig) -> Result<(), ConfigError> {
        // Validate server settings
        if config.server.port == 0 {
            return Err(ConfigError::InvalidValue("Server port cannot be 0".to_string()));
        }
        
        if config.server.max_players == 0 {
            return Err(ConfigError::InvalidValue("Max players must be greater than 0".to_string()));
        }
        
        // Validate game rules
        if config.game_rules.min_bet >= config.game_rules.max_bet {
            return Err(ConfigError::InvalidValue("Min bet must be less than max bet".to_string()));
        }
        
        if config.game_rules.dice_sides < 2 {
            return Err(ConfigError::InvalidValue("Dice must have at least 2 sides".to_string()));
        }
        
        // Validate database
        if config.database.url.is_empty() {
            return Err(ConfigError::InvalidValue("Database URL cannot be empty".to_string()));
        }
        
        Ok(())
    }
    
    pub fn get_cached_config(&self) -> Option<&GameServerConfig> {
        self.cached_config.as_ref()
    }
}

#[derive(Debug)]
enum ConfigSource {
    File(String),
    EnvironmentVariables,
    CommandLineArgs,
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidValue(String),
    FileNotFound(String),
    ParseError(String),
}

#[tokio::test]
async fn test_config_loading() {
    // Set environment variable
    std::env::set_var("GAME_SERVER_PORT", "9090");
    std::env::set_var("GAME_MAX_BET", "500");
    
    let mut config_manager = ConfigManager::new();
    let config = config_manager.load_config().await.unwrap();
    
    // Should have picked up environment variables
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.game_rules.max_bet, 500);
    
    // Should still have defaults for other values
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.game_rules.min_bet, 1);
}
```

## Conclusion: Configuration as System DNA

Configuration management is the DNA of your system - it determines how your software behaves in different environments and situations. In BitCraps, good configuration management enables:

1. **Multi-environment deployment** - Same code, different behaviors
2. **Platform adaptation** - Mobile vs desktop optimizations  
3. **Regional compliance** - Meeting different legal requirements
4. **Feature toggling** - Safe rollout of new capabilities
5. **Runtime adaptation** - Responding to changing conditions

The key insights for configuration design:

1. **Layer configurations by precedence** - Defaults → files → environment → runtime
2. **Validate rigorously** - Bad config can be worse than no config
3. **Secure sensitive data** - API keys and secrets need special handling
4. **Make it observable** - Log what configuration is actually being used
5. **Plan for change** - Configuration will evolve, design for it

Remember: Configuration is how you make one codebase work in a thousand different scenarios. In a system like BitCraps that handles real money across global jurisdictions, configuration management isn't just about convenience - it's about compliance, security, and user trust.

Good configuration management lets you ship once and deploy everywhere. Bad configuration management forces you to maintain separate codebases or ship buggy, insecure software.