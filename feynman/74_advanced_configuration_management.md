# Chapter 74: Advanced Configuration Management

## Introduction: The Control Panel of Complex Systems

Imagine controlling a spaceship with thousands of switches, dials, and settings. Each configuration affects how the ship operates, and wrong settings could mean disaster. This is the challenge of configuration management in production systems—providing flexible, safe, and observable control over system behavior.

## The Fundamentals: Configuration Challenges

Modern configuration systems must handle:
- Environment-specific settings
- Secret management
- Dynamic reconfiguration
- Type safety and validation
- Configuration versioning

## Deep Dive: Layered Configuration

### Hierarchical Configuration System

```rust
pub struct LayeredConfig {
    /// Configuration layers in priority order
    layers: Vec<Box<dyn ConfigSource>>,
    
    /// Merged configuration
    merged: Arc<RwLock<Config>>,
    
    /// Change notifier
    notifier: ConfigChangeNotifier,
}

pub trait ConfigSource: Send + Sync {
    fn load(&self) -> Result<ConfigLayer>;
    fn watch(&self) -> Option<Box<dyn Stream<Item = ConfigChange>>>;
    fn priority(&self) -> u32;
}

impl LayeredConfig {
    pub fn new() -> Self {
        let mut config = Self {
            layers: Vec::new(),
            merged: Arc::new(RwLock::new(Config::default())),
            notifier: ConfigChangeNotifier::new(),
        };
        
        // Add layers in priority order
        config.add_layer(Box::new(DefaultConfig));
        config.add_layer(Box::new(FileConfig::new("/etc/bitcraps/config.toml")));
        config.add_layer(Box::new(EnvConfig::with_prefix("BITCRAPS_")));
        config.add_layer(Box::new(CliConfig::from_args()));
        
        config
    }
    
    pub async fn reload(&mut self) -> Result<()> {
        let mut merged = Config::default();
        
        // Load and merge all layers
        for layer in &self.layers {
            let config = layer.load()?;
            merged.merge(config);
        }
        
        // Validate merged configuration
        merged.validate()?;
        
        // Detect changes
        let old = self.merged.read().await.clone();
        let changes = merged.diff(&old);
        
        // Update configuration
        *self.merged.write().await = merged;
        
        // Notify watchers
        if !changes.is_empty() {
            self.notifier.notify(changes).await;
        }
        
        Ok(())
    }
}
```

## Secret Management

### Secure Secret Storage

```rust
pub struct SecretManager {
    /// Secret store backend
    store: Box<dyn SecretStore>,
    
    /// Encryption key
    master_key: Arc<SecretKey>,
    
    /// Secret cache with TTL
    cache: Arc<RwLock<TtlCache<String, Secret>>>,
}

pub trait SecretStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<EncryptedSecret>;
    async fn set(&self, key: &str, secret: &EncryptedSecret) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

impl SecretManager {
    pub async fn get_secret(&self, key: &str) -> Result<SecretString> {
        // Check cache first
        if let Some(secret) = self.cache.read().await.get(key) {
            return Ok(secret.value.clone());
        }
        
        // Fetch from store
        let encrypted = self.store.get(key).await?;
        
        // Decrypt
        let decrypted = self.decrypt(&encrypted)?;
        
        // Cache with TTL
        self.cache.write().await.insert(
            key.to_string(),
            Secret {
                value: decrypted.clone(),
                expires_at: Instant::now() + Duration::from_secs(300),
            },
        );
        
        Ok(decrypted)
    }
    
    fn decrypt(&self, encrypted: &EncryptedSecret) -> Result<SecretString> {
        use aes_gcm::{Aes256Gcm, Key, Nonce};
        use aes_gcm::aead::{Aead, NewAead};
        
        let cipher = Aes256Gcm::new(Key::from_slice(&self.master_key.0));
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| Error::DecryptionFailed(e))?;
        
        Ok(SecretString::from(plaintext))
    }
}
```

## Dynamic Reconfiguration

### Hot Reload Without Restart

```rust
pub struct DynamicConfig {
    /// Current configuration
    current: Arc<ArcSwap<Config>>,
    
    /// Configuration watcher
    watcher: ConfigWatcher,
    
    /// Reload hooks
    reload_hooks: Vec<Box<dyn ReloadHook>>,
}

#[async_trait]
pub trait ReloadHook: Send + Sync {
    async fn before_reload(&self, old: &Config, new: &Config) -> Result<()>;
    async fn after_reload(&self, old: &Config, new: &Config);
}

impl DynamicConfig {
    pub async fn watch_for_changes(&mut self) {
        let mut events = self.watcher.events();
        
        while let Some(event) = events.next().await {
            match event {
                ConfigEvent::FileChanged(path) => {
                    tracing::info!("Configuration file changed: {:?}", path);
                    
                    if let Err(e) = self.reload().await {
                        tracing::error!("Failed to reload configuration: {}", e);
                    }
                }
            }
        }
    }
    
    async fn reload(&mut self) -> Result<()> {
        let new_config = self.load_config().await?;
        let old_config = self.current.load();
        
        // Run pre-reload hooks
        for hook in &self.reload_hooks {
            hook.before_reload(&old_config, &new_config).await?;
        }
        
        // Atomic configuration swap
        self.current.store(Arc::new(new_config.clone()));
        
        // Run post-reload hooks
        for hook in &self.reload_hooks {
            hook.after_reload(&old_config, &new_config).await;
        }
        
        tracing::info!("Configuration reloaded successfully");
        Ok(())
    }
}
```

## Type-Safe Configuration

### Compile-Time Validation

```rust
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct NetworkConfig {
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,
    
    #[validate(ip)]
    pub bind_address: String,
    
    #[validate(range(min = 1, max = 10000))]
    pub max_connections: usize,
    
    #[validate(custom = "validate_buffer_size")]
    pub buffer_size: usize,
}

fn validate_buffer_size(size: &usize) -> Result<(), ValidationError> {
    if *size < 1024 || *size > 1024 * 1024 * 10 {
        return Err(ValidationError::new("invalid_buffer_size"));
    }
    
    // Must be power of 2
    if (*size & (*size - 1)) != 0 {
        return Err(ValidationError::new("buffer_size_not_power_of_2"));
    }
    
    Ok(())
}

/// Strongly typed configuration with builder pattern
pub struct ConfigBuilder {
    network: Option<NetworkConfig>,
    storage: Option<StorageConfig>,
    security: Option<SecurityConfig>,
}

impl ConfigBuilder {
    pub fn network(mut self, config: NetworkConfig) -> Self {
        self.network = Some(config);
        self
    }
    
    pub fn build(self) -> Result<Config> {
        let config = Config {
            network: self.network.ok_or(Error::MissingConfig("network"))?,
            storage: self.storage.ok_or(Error::MissingConfig("storage"))?,
            security: self.security.ok_or(Error::MissingConfig("security"))?,
        };
        
        config.validate()?;
        Ok(config)
    }
}
```

## Feature Flags

### Runtime Feature Control

```rust
pub struct FeatureFlags {
    /// Flag storage
    flags: Arc<DashMap<String, FeatureFlag>>,
    
    /// Evaluation context
    context: EvaluationContext,
    
    /// Flag provider
    provider: Box<dyn FlagProvider>,
}

pub struct FeatureFlag {
    name: String,
    enabled: bool,
    rollout_percentage: Option<f32>,
    conditions: Vec<Condition>,
}

impl FeatureFlags {
    pub fn is_enabled(&self, flag_name: &str, user_id: Option<&str>) -> bool {
        if let Some(flag) = self.flags.get(flag_name) {
            // Check basic enabled state
            if !flag.enabled {
                return false;
            }
            
            // Check rollout percentage
            if let Some(percentage) = flag.rollout_percentage {
                if let Some(id) = user_id {
                    let hash = xxhash::xxh3::xxh3_64(id.as_bytes());
                    let threshold = (percentage * u64::MAX as f32) as u64;
                    if hash > threshold {
                        return false;
                    }
                }
            }
            
            // Check conditions
            for condition in &flag.conditions {
                if !condition.evaluate(&self.context) {
                    return false;
                }
            }
            
            true
        } else {
            false
        }
    }
}
```

## Testing Configuration

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_layered_config() {
        let mut config = LayeredConfig::new();
        
        // Add test layers
        config.add_layer(Box::new(TestConfig::from_map(hashmap! {
            "port" => "8080",
            "debug" => "false",
        })));
        
        config.add_layer(Box::new(TestConfig::from_map(hashmap! {
            "debug" => "true", // Override
        })));
        
        let merged = config.reload().unwrap();
        
        assert_eq!(merged.get::<u16>("port"), Some(8080));
        assert_eq!(merged.get::<bool>("debug"), Some(true)); // Overridden
    }
    
    #[test]
    fn test_secret_encryption() {
        let manager = SecretManager::new(test_key());
        
        // Store secret
        manager.set_secret("api_key", "super_secret_key").await.unwrap();
        
        // Retrieve and verify
        let retrieved = manager.get_secret("api_key").await.unwrap();
        assert_eq!(retrieved.expose_secret(), "super_secret_key");
        
        // Verify it's encrypted in storage
        let encrypted = manager.store.get("api_key").await.unwrap();
        assert_ne!(encrypted.ciphertext, b"super_secret_key");
    }
}
```

## Conclusion

Advanced configuration management provides the flexibility and safety needed for production systems. Through layered configuration, secret management, and dynamic reconfiguration, we can build systems that adapt to changing requirements without downtime.

Key takeaways:
1. **Layered configuration** provides flexibility with precedence
2. **Secret management** protects sensitive data
3. **Dynamic reconfiguration** enables hot reloads
4. **Type safety** catches errors at compile time
5. **Feature flags** allow gradual rollouts
6. **Validation** ensures configuration correctness

Remember: Configuration is code—it deserves the same attention to testing, versioning, and review as your application logic.
