//! Production-grade configuration management for BitCraps
//!
//! This module provides centralized configuration with:
//! - Environment-based loading (dev, staging, prod)
//! - Runtime validation
//! - Hot reloading support
//! - Secure secret management

pub mod initialization;
pub mod performance;
pub mod runtime_reload;
pub mod scalability;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use initialization::ConfigurationManager;
pub use performance::{PerformanceConfig, PerformanceProfile, PerformanceTuner};
pub use runtime_reload::{ConfigChangeEvent, ReloadSettings, RuntimeConfigManager};
pub use scalability::{PlatformType, ScalabilityConfig, ScalabilityManager};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub network: NetworkConfig,
    pub consensus: ConsensusConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub monitoring: MonitoringConfig,
    pub game: GameConfig,
    pub treasury: TreasuryConfig,
    pub performance: PerformanceProfile,
    /// Configuration version for hot-reload tracking
    pub version: u64,
    /// Last reload timestamp
    pub last_reload: Option<std::time::SystemTime>,
}

/// Application-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub version: String,
    pub environment: Environment,
    pub data_dir: PathBuf,
    pub log_level: String,
    pub worker_threads: usize,
    pub enable_tui: bool,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub keepalive_interval: Duration,
    pub max_packet_size: usize,
    pub enable_bluetooth: bool,
    pub enable_tcp: bool,
    pub enable_compression: bool,
    pub mtu_discovery: bool,
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub min_participants: usize,
    pub max_participants: usize,
    pub proposal_timeout: Duration,
    pub voting_timeout: Duration,
    pub finality_threshold: f64,
    pub byzantine_tolerance: f64,
    pub enable_fast_path: bool,
    pub checkpoint_interval: u64,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub enable_wal: bool,
    pub checkpoint_interval: Duration,
    pub backup_dir: PathBuf,
    pub backup_interval: Duration,
    pub log_retention_days: u32,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub pow_difficulty: u32,
    pub key_rotation_interval: Duration,
    pub session_timeout: Duration,
    pub max_session_count: usize,
    pub enable_rate_limiting: bool,
    pub rate_limit_requests: u32,
    pub rate_limit_window: Duration,
    pub enable_tls: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub enable_tracing: bool,
    pub tracing_endpoint: Option<String>,
    pub health_check_interval: Duration,
    pub alert_webhook: Option<String>,
    pub log_retention_days: u32,
    pub metric_retention_days: u32,
}

/// Game configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_players_per_game: usize,
    pub game_timeout: Duration,
    pub auto_resolve_timeout: Duration,
    pub enable_side_bets: bool,
    pub house_edge: f64,
}

/// Treasury configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryConfig {
    pub initial_supply: u64,
    pub treasury_address: String,
    pub rake_percentage: f64,
    pub min_reserve: u64,
    pub max_exposure: u64,
    pub rebalance_interval: Duration,
}

/// Environment enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

impl Config {
    /// Load configuration from file and environment
    pub fn load() -> Result<Self> {
        // Determine environment
        let env = env::var("BITCRAPS_ENV").unwrap_or_else(|_| "development".to_string());

        let environment = match env.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            "staging" | "stage" => Environment::Staging,
            "testing" | "test" => Environment::Testing,
            _ => Environment::Development,
        };

        // Load base configuration
        let config_path = Self::get_config_path(&environment)?;
        let mut config = Self::load_from_file(&config_path)?;

        // Override with environment variables
        config.override_from_env()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;

        let config: Config = toml::from_str(&contents)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Get configuration file path based on environment
    fn get_config_path(environment: &Environment) -> Result<PathBuf> {
        let base_path = env::var("BITCRAPS_CONFIG_PATH").unwrap_or_else(|_| "config".to_string());

        let filename = match environment {
            Environment::Production => "production.toml",
            Environment::Staging => "staging.toml",
            Environment::Testing => "testing.toml",
            Environment::Development => "development.toml",
        };

        Ok(PathBuf::from(base_path).join(filename))
    }

    /// Override configuration with environment variables
    fn override_from_env(&mut self) -> Result<()> {
        // Network overrides
        if let Ok(val) = env::var("BITCRAPS_LISTEN_PORT") {
            self.network.listen_port = val
                .parse()
                .map_err(|_| Error::Config("Invalid listen port".to_string()))?;
        }

        if let Ok(val) = env::var("BITCRAPS_MAX_CONNECTIONS") {
            self.network.max_connections = val
                .parse()
                .map_err(|_| Error::Config("Invalid max connections".to_string()))?;
        }

        // Database overrides
        if let Ok(val) = env::var("BITCRAPS_DATABASE_URL") {
            self.database.url = val;
        }

        // Security overrides
        if let Ok(val) = env::var("BITCRAPS_POW_DIFFICULTY") {
            self.security.pow_difficulty = val
                .parse()
                .map_err(|_| Error::Config("Invalid PoW difficulty".to_string()))?;
        }

        // Monitoring overrides
        if let Ok(val) = env::var("BITCRAPS_METRICS_PORT") {
            self.monitoring.metrics_port = val
                .parse()
                .map_err(|_| Error::Config("Invalid metrics port".to_string()))?;
        }

        if let Ok(val) = env::var("BITCRAPS_ALERT_WEBHOOK") {
            self.monitoring.alert_webhook = Some(val);
        }

        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Network validation
        if self.network.max_connections == 0 {
            return Err(Error::Config("Max connections must be > 0".to_string()));
        }

        if self.network.max_packet_size < 1024 {
            return Err(Error::Config("Max packet size must be >= 1024".to_string()));
        }

        // Consensus validation
        if self.consensus.min_participants < 1 {
            return Err(Error::Config("Min participants must be >= 1".to_string()));
        }

        if self.consensus.finality_threshold < 0.5 || self.consensus.finality_threshold > 1.0 {
            return Err(Error::Config(
                "Finality threshold must be between 0.5 and 1.0".to_string(),
            ));
        }

        // Database validation
        if self.database.url.is_empty() {
            return Err(Error::Config("Database URL cannot be empty".to_string()));
        }

        if self.database.max_connections == 0 {
            return Err(Error::Config(
                "Database max connections must be > 0".to_string(),
            ));
        }

        // Security validation
        if self.security.pow_difficulty == 0 {
            return Err(Error::Config("PoW difficulty must be > 0".to_string()));
        }

        if self.security.enable_rate_limiting && self.security.rate_limit_requests == 0 {
            return Err(Error::Config(
                "Rate limit requests must be > 0 when enabled".to_string(),
            ));
        }

        // Game validation
        if self.game.min_bet > self.game.max_bet {
            return Err(Error::Config("Min bet cannot exceed max bet".to_string()));
        }

        if self.game.house_edge < 0.0 || self.game.house_edge > 0.1 {
            return Err(Error::Config(
                "House edge must be between 0 and 10%".to_string(),
            ));
        }

        // Treasury validation
        if self.treasury.rake_percentage < 0.0 || self.treasury.rake_percentage > 0.1 {
            return Err(Error::Config(
                "Rake percentage must be between 0 and 10%".to_string(),
            ));
        }

        if self.treasury.min_reserve > self.treasury.initial_supply {
            return Err(Error::Config(
                "Min reserve cannot exceed initial supply".to_string(),
            ));
        }

        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(path, contents)
            .map_err(|e| Error::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Generate default configuration for an environment
    pub fn default_for_environment(environment: Environment) -> Self {
        match environment {
            Environment::Production => Self::production_defaults(),
            Environment::Staging => Self::staging_defaults(),
            Environment::Testing => Self::testing_defaults(),
            Environment::Development => Self::development_defaults(),
        }
    }

    fn production_defaults() -> Self {
        Config {
            app: AppConfig {
                name: "BitCraps".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                environment: Environment::Production,
                data_dir: PathBuf::from("/var/lib/bitcraps"),
                log_level: "info".to_string(),
                worker_threads: num_cpus::get(),
                enable_tui: false,
            },
            network: NetworkConfig {
                listen_address: "0.0.0.0".to_string(),
                listen_port: 8333,
                max_connections: 1000,
                connection_timeout: Duration::from_secs(30),
                keepalive_interval: Duration::from_secs(60),
                max_packet_size: 65536,
                enable_bluetooth: true,
                enable_tcp: true,
                enable_compression: true,
                mtu_discovery: true,
            },
            consensus: ConsensusConfig {
                min_participants: 3,
                max_participants: 100,
                proposal_timeout: Duration::from_secs(10),
                voting_timeout: Duration::from_secs(5),
                finality_threshold: 0.67,
                byzantine_tolerance: 0.33,
                enable_fast_path: true,
                checkpoint_interval: 1000,
            },
            database: DatabaseConfig {
                url: "sqlite:///var/lib/bitcraps/bitcraps.db".to_string(),
                max_connections: 32,
                connection_timeout: Duration::from_secs(10),
                idle_timeout: Duration::from_secs(300),
                enable_wal: true,
                checkpoint_interval: Duration::from_secs(300),
                backup_dir: PathBuf::from("/var/lib/bitcraps/backups"),
                backup_interval: Duration::from_secs(3600),
                log_retention_days: 30,
            },
            security: SecurityConfig {
                pow_difficulty: 20,
                key_rotation_interval: Duration::from_secs(86400),
                session_timeout: Duration::from_secs(3600),
                max_session_count: 1000,
                enable_rate_limiting: true,
                rate_limit_requests: 100,
                rate_limit_window: Duration::from_secs(60),
                enable_tls: true,
                tls_cert_path: Some(PathBuf::from("/etc/bitcraps/cert.pem")),
                tls_key_path: Some(PathBuf::from("/etc/bitcraps/key.pem")),
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_port: 9090,
                enable_tracing: true,
                tracing_endpoint: Some("http://localhost:14268/api/traces".to_string()),
                health_check_interval: Duration::from_secs(30),
                alert_webhook: None,
                log_retention_days: 30,
                metric_retention_days: 90,
            },
            game: GameConfig {
                min_bet: 10,
                max_bet: 10000,
                max_players_per_game: 8,
                game_timeout: Duration::from_secs(300),
                auto_resolve_timeout: Duration::from_secs(60),
                enable_side_bets: true,
                house_edge: 0.0136, // 1.36% for pass line
            },
            treasury: TreasuryConfig {
                initial_supply: 1_000_000_000,
                treasury_address: "0xFF".repeat(32),
                rake_percentage: 0.02,
                min_reserve: 100_000_000,
                max_exposure: 500_000_000,
                rebalance_interval: Duration::from_secs(3600),
            },
            performance: PerformanceProfile::Balanced,
            version: 1,
            last_reload: Some(std::time::SystemTime::now()),
        }
    }

    fn development_defaults() -> Self {
        let mut config = Self::production_defaults();
        config.app.environment = Environment::Development;
        config.app.data_dir = PathBuf::from("./data");
        config.app.log_level = "debug".to_string();
        config.app.enable_tui = true;
        config.network.listen_port = 8334;
        config.network.max_connections = 50;
        config.database.url = "sqlite://./data/bitcraps.db".to_string();
        config.database.backup_dir = PathBuf::from("./data/backups");
        config.security.pow_difficulty = 8;
        config.security.enable_tls = false;
        config.security.tls_cert_path = None;
        config.security.tls_key_path = None;
        config.monitoring.enable_tracing = false;
        config.monitoring.tracing_endpoint = None;
        config
    }

    fn staging_defaults() -> Self {
        let mut config = Self::production_defaults();
        config.app.environment = Environment::Staging;
        config.app.log_level = "debug".to_string();
        config.network.listen_port = 8335;
        config.security.pow_difficulty = 16;
        config
    }

    fn testing_defaults() -> Self {
        let mut config = Self::development_defaults();
        config.app.environment = Environment::Testing;
        config.app.data_dir = PathBuf::from("./test_data");
        config.network.listen_port = 8336;
        config.network.max_connections = 10;
        config.database.url = "sqlite::memory:".to_string();
        config.security.pow_difficulty = 4;
        config.security.enable_rate_limiting = false;
        config.monitoring.enable_metrics = false;
        config
    }
}

/// Global configuration instance
use once_cell::sync::Lazy;
use std::sync::RwLock;

static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    RwLock::new(Config::load().unwrap_or_else(|e| {
        eprintln!(
            "WARNING: Failed to load configuration, using defaults: {}",
            e
        );
        Config::default_for_environment(Environment::Production)
    }))
});

/// Get the global configuration instance
pub fn get_config() -> Config {
    CONFIG.read().expect("Configuration lock poisoned").clone()
}

/// Set the global configuration (for testing)
#[cfg(test)]
pub fn set_config(config: Config) {
    *CONFIG.write().expect("Configuration lock poisoned") = config;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = Config::development_defaults();
        assert!(config.validate().is_ok());

        // Test invalid configurations
        config.network.max_connections = 0;
        assert!(config.validate().is_err());

        config = Config::development_defaults();
        config.game.min_bet = 1000;
        config.game.max_bet = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_defaults() {
        let dev = Config::development_defaults();
        assert_eq!(dev.app.environment, Environment::Development);
        assert_eq!(dev.security.pow_difficulty, 8);

        let prod = Config::production_defaults();
        assert_eq!(prod.app.environment, Environment::Production);
        assert_eq!(prod.security.pow_difficulty, 20);
    }
}
