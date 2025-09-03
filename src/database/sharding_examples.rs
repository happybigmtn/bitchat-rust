//! Examples and configurations for database sharding in BitCraps
//!
//! Demonstrates:
//! - User-based sharding strategy
//! - Game-based sharding strategy  
//! - Geographic sharding
//! - Time-based sharding for metrics

use crate::database::abstractions::*;
use crate::database::database_manager::{DatabaseManager, DatabaseManagerConfig};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sharding strategy examples for BitCraps
pub struct BitCrapsShardingStrategies;

impl BitCrapsShardingStrategies {
    /// User-based sharding configuration
    /// Shards users by hash of user ID for even distribution
    pub fn user_based_sharding() -> ShardingConfig {
        ShardingConfig {
            enabled: true,
            hash_function: HashFunction::Blake3,
            rebalance_threshold: 0.8, // Rebalance when 80% full
            auto_scaling: true,
            shards: vec![
                ShardDefinition {
                    id: "users_shard_1".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db1.example.com:5432/bitcraps_users_1".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: 0,
                    range_end: u64::MAX / 4,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "users_shard_2".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db2.example.com:5432/bitcraps_users_2".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: u64::MAX / 4 + 1,
                    range_end: u64::MAX / 2,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "users_shard_3".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db3.example.com:5432/bitcraps_users_3".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: u64::MAX / 2 + 1,
                    range_end: (u64::MAX / 4) * 3,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "users_shard_4".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db4.example.com:5432/bitcraps_users_4".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: (u64::MAX / 4) * 3 + 1,
                    range_end: u64::MAX,
                    status: ShardStatus::Active,
                },
            ],
        }
    }
    
    /// Game-based sharding configuration
    /// Shards games by game ID with higher capacity for hot shards
    pub fn game_based_sharding() -> ShardingConfig {
        ShardingConfig {
            enabled: true,
            hash_function: HashFunction::Consistent,
            rebalance_threshold: 0.75,
            auto_scaling: true,
            shards: vec![
                ShardDefinition {
                    id: "games_hot_shard".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-games-hot.example.com:5432/bitcraps_games_hot".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 100, // Higher capacity
                            min_connections: 20,
                            ..Default::default()
                        },
                    },
                    weight: 3, // Higher weight for more traffic
                    range_start: 0,
                    range_end: u64::MAX / 3,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "games_shard_1".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-games-1.example.com:5432/bitcraps_games_1".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: u64::MAX / 3 + 1,
                    range_end: (u64::MAX / 3) * 2,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "games_shard_2".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-games-2.example.com:5432/bitcraps_games_2".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: (u64::MAX / 3) * 2 + 1,
                    range_end: u64::MAX,
                    status: ShardStatus::Active,
                },
            ],
        }
    }
    
    /// Geographic sharding configuration
    /// Shards data by geographic region for latency optimization
    pub fn geographic_sharding() -> ShardingConfig {
        ShardingConfig {
            enabled: true,
            hash_function: HashFunction::Murmur3,
            rebalance_threshold: 0.85,
            auto_scaling: false, // Manual scaling for geographic regions
            shards: vec![
                ShardDefinition {
                    id: "us_west_shard".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-usw.example.com:5432/bitcraps_usw".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 75,
                            ..Default::default()
                        },
                    },
                    weight: 2, // Higher population
                    range_start: 0,
                    range_end: u64::MAX / 3,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "us_east_shard".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-use.example.com:5432/bitcraps_use".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 75,
                            ..Default::default()
                        },
                    },
                    weight: 2,
                    range_start: u64::MAX / 3 + 1,
                    range_end: (u64::MAX / 3) * 2,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "eu_shard".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-eu.example.com:5432/bitcraps_eu".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: (u64::MAX / 3) * 2 + 1,
                    range_end: u64::MAX,
                    status: ShardStatus::Active,
                },
            ],
        }
    }
    
    /// Time-based sharding for metrics and analytics
    /// Shards metrics data by time period for efficient querying
    pub fn time_based_metrics_sharding() -> ShardingConfig {
        ShardingConfig {
            enabled: true,
            hash_function: HashFunction::Blake3,
            rebalance_threshold: 0.9,
            auto_scaling: true,
            shards: vec![
                ShardDefinition {
                    id: "metrics_current".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-metrics-current.example.com:5432/bitcraps_metrics_current".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 100, // High write load
                            min_connections: 30,
                            ..Default::default()
                        },
                    },
                    weight: 4, // Current data gets most traffic
                    range_start: 0,
                    range_end: u64::MAX / 4,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "metrics_recent".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-metrics-recent.example.com:5432/bitcraps_metrics_recent".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 50,
                            ..Default::default()
                        },
                    },
                    weight: 2,
                    range_start: u64::MAX / 4 + 1,
                    range_end: u64::MAX / 2,
                    status: ShardStatus::Active,
                },
                ShardDefinition {
                    id: "metrics_archive".to_string(),
                    connection: DatabaseConnection {
                        backend: DatabaseBackend::PostgreSQL,
                        connection_string: "postgresql://bitcraps:password@db-metrics-archive.example.com:5432/bitcraps_metrics_archive".to_string(),
                        pool_config: PoolConfiguration {
                            max_connections: 25, // Mostly read-only
                            ..Default::default()
                        },
                    },
                    weight: 1,
                    range_start: u64::MAX / 2 + 1,
                    range_end: u64::MAX,
                    status: ShardStatus::ReadOnly, // Archive is read-only
                },
            ],
        }
    }
    
    /// Hybrid sharding configuration
    /// Combines multiple sharding strategies for different data types
    pub fn hybrid_sharding_config() -> Vec<(String, ShardingConfig)> {
        vec![
            ("users".to_string(), Self::user_based_sharding()),
            ("games".to_string(), Self::game_based_sharding()),
            ("metrics".to_string(), Self::time_based_metrics_sharding()),
        ]
    }
}

/// Configuration for production deployment with sharding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionShardingConfig {
    pub cluster_name: String,
    pub region: String,
    pub environment: String,
    pub sharding_strategies: HashMap<String, ShardingConfig>,
    pub global_settings: GlobalShardingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalShardingSettings {
    pub enable_cross_shard_transactions: bool,
    pub max_cross_shard_operations: u32,
    pub shard_migration_timeout_seconds: u64,
    pub monitoring_interval_seconds: u64,
    pub auto_rebalancing: bool,
}

impl ProductionShardingConfig {
    /// Create production configuration for a specific region
    pub fn for_region(region: &str, environment: &str) -> Self {
        let mut sharding_strategies = HashMap::new();
        
        // Add all sharding strategies
        sharding_strategies.insert("users".to_string(), BitCrapsShardingStrategies::user_based_sharding());
        sharding_strategies.insert("games".to_string(), BitCrapsShardingStrategies::game_based_sharding());
        sharding_strategies.insert("metrics".to_string(), BitCrapsShardingStrategies::time_based_metrics_sharding());
        
        // Adjust connection strings for the region
        for (_, config) in sharding_strategies.iter_mut() {
            for shard in config.shards.iter_mut() {
                // Replace example.com with region-specific hostnames
                shard.connection.connection_string = shard.connection.connection_string
                    .replace("example.com", &format!("{}.bitcraps.io", region));
            }
        }
        
        Self {
            cluster_name: format!("bitcraps-{}-{}", region, environment),
            region: region.to_string(),
            environment: environment.to_string(),
            sharding_strategies,
            global_settings: GlobalShardingSettings {
                enable_cross_shard_transactions: true,
                max_cross_shard_operations: 10,
                shard_migration_timeout_seconds: 3600,
                monitoring_interval_seconds: 30,
                auto_rebalancing: environment == "production",
            },
        }
    }
    
    /// Generate Docker Compose for the sharded deployment
    pub fn generate_docker_compose(&self) -> Result<String> {
        let mut compose = String::from("version: '3.8'\n\nservices:\n");
        
        for (strategy_name, config) in &self.sharding_strategies {
            for (i, shard) in config.shards.iter().enumerate() {
                compose.push_str(&format!(r#"
  {}_shard_{}:
    image: postgres:15
    environment:
      - POSTGRES_DB={}
      - POSTGRES_USER=bitcraps
      - POSTGRES_PASSWORD=bitcraps_password
      - POSTGRES_MAX_CONNECTIONS={}
    ports:
      - "{}:5432"
    volumes:
      - ./{}_shard_{}_data:/var/lib/postgresql/data
      - ./init-scripts:/docker-entrypoint-initdb.d
    restart: unless-stopped
    networks:
      - bitcraps-sharded-network
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '1.0'
        reservations:
          memory: 1G
          cpus: '0.5'
"#,
                    strategy_name,
                    i + 1,
                    shard.id.replace("_", ""),
                    shard.connection.pool_config.max_connections,
                    5432 + (strategy_name.len() * 10) + i,
                    strategy_name,
                    i + 1,
                ));
            }
        }
        
        // Add PgBouncer instances for each shard type
        for strategy_name in self.sharding_strategies.keys() {
            compose.push_str(&format!(r#"
  {}_pgbouncer:
    image: pgbouncer/pgbouncer:latest
    environment:
      - POOL_MODE=transaction
      - SERVER_RESET_QUERY=DISCARD ALL
      - MAX_CLIENT_CONN=1000
      - DEFAULT_POOL_SIZE=50
      - MAX_DB_CONNECTIONS=200
    ports:
      - "{}:5432"
    volumes:
      - ./pgbouncer/{}:/etc/pgbouncer:ro
    restart: unless-stopped
    networks:
      - bitcraps-sharded-network
    depends_on:
      - {}_shard_1
"#,
                strategy_name,
                6432 + strategy_name.len() * 10,
                strategy_name,
                strategy_name,
            ));
        }
        
        compose.push_str(r#"
networks:
  bitcraps-sharded-network:
    driver: bridge

volumes:"#);
        
        // Add volume definitions
        for (strategy_name, config) in &self.sharding_strategies {
            for i in 0..config.shards.len() {
                compose.push_str(&format!(
                    "\n  {}_shard_{}_data:",
                    strategy_name,
                    i + 1
                ));
            }
        }
        
        Ok(compose)
    }
    
    /// Generate Kubernetes manifests for sharded deployment
    pub fn generate_kubernetes_manifests(&self) -> Result<Vec<(String, String)>> {
        let mut manifests = Vec::new();
        
        // Generate StatefulSet for each shard
        for (strategy_name, config) in &self.sharding_strategies {
            for (i, shard) in config.shards.iter().enumerate() {
                let statefulset = format!(r#"
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: {}-shard-{}
  namespace: bitcraps-{}
spec:
  serviceName: {}-shard-{}-headless
  replicas: 1
  selector:
    matchLabels:
      app: {}-shard-{}
      strategy: {}
  template:
    metadata:
      labels:
        app: {}-shard-{}
        strategy: {}
    spec:
      containers:
      - name: postgres
        image: postgres:15
        env:
        - name: POSTGRES_DB
          value: {}
        - name: POSTGRES_USER
          value: bitcraps
        - name: POSTGRES_PASSWORD
          valueFrom:
            secretKeyRef:
              name: postgres-secret
              key: password
        - name: POSTGRES_MAX_CONNECTIONS
          value: "{}"
        ports:
        - containerPort: 5432
          name: postgres
        volumeMounts:
        - name: postgres-data
          mountPath: /var/lib/postgresql/data
        resources:
          limits:
            memory: "2Gi"
            cpu: "1000m"
          requests:
            memory: "1Gi"
            cpu: "500m"
  volumeClaimTemplates:
  - metadata:
      name: postgres-data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      resources:
        requests:
          storage: 100Gi
      storageClassName: fast-ssd
---
apiVersion: v1
kind: Service
metadata:
  name: {}-shard-{}-headless
  namespace: bitcraps-{}
spec:
  clusterIP: None
  selector:
    app: {}-shard-{}
  ports:
  - port: 5432
    name: postgres
"#,
                    strategy_name, i + 1, self.environment,
                    strategy_name, i + 1,
                    strategy_name, i + 1, strategy_name,
                    strategy_name, i + 1, strategy_name,
                    shard.id.replace("_", ""),
                    shard.connection.pool_config.max_connections,
                    strategy_name, i + 1, self.environment,
                    strategy_name, i + 1
                );
                
                manifests.push((
                    format!("{}-shard-{}-statefulset.yaml", strategy_name, i + 1),
                    statefulset
                ));
            }
        }
        
        // Generate ConfigMap for PgBouncer
        let pgbouncer_config = r#"
apiVersion: v1
kind: ConfigMap
metadata:
  name: pgbouncer-config
  namespace: bitcraps-production
data:
  pgbouncer.ini: |
    [databases]
    * = host=postgres-service port=5432
    
    [pgbouncer]
    listen_addr = 0.0.0.0
    listen_port = 5432
    auth_type = md5
    auth_file = /etc/pgbouncer/userlist.txt
    max_client_conn = 1000
    default_pool_size = 50
    pool_mode = transaction
    server_round_robin = 1
    
  userlist.txt: |
    "bitcraps" "md5hash_here"
"#;
        
        manifests.push((
            "pgbouncer-config.yaml".to_string(),
            pgbouncer_config.to_string()
        ));
        
        Ok(manifests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_user_based_sharding_config() {
        let config = BitCrapsShardingStrategies::user_based_sharding();
        assert!(config.enabled);
        assert_eq!(config.shards.len(), 4);
        assert!(matches!(config.hash_function, HashFunction::Blake3));
        
        // Verify all shards are active
        for shard in &config.shards {
            assert_eq!(shard.status, ShardStatus::Active);
            assert_eq!(shard.connection.backend, DatabaseBackend::PostgreSQL);
        }
    }
    
    #[test]
    fn test_production_config_generation() {
        let config = ProductionShardingConfig::for_region("us-west-2", "production");
        assert_eq!(config.region, "us-west-2");
        assert_eq!(config.environment, "production");
        assert_eq!(config.sharding_strategies.len(), 3);
        
        // Verify region-specific hostnames
        for (_, shard_config) in &config.sharding_strategies {
            for shard in &shard_config.shards {
                assert!(shard.connection.connection_string.contains("us-west-2.bitcraps.io"));
            }
        }
    }
    
    #[test]
    fn test_docker_compose_generation() {
        let config = ProductionShardingConfig::for_region("test", "development");
        let compose = config.generate_docker_compose().unwrap();
        
        assert!(compose.contains("version: '3.8'"));
        assert!(compose.contains("postgres:15"));
        assert!(compose.contains("pgbouncer/pgbouncer"));
        assert!(compose.contains("bitcraps-sharded-network"));
    }
    
    #[test]
    fn test_kubernetes_manifests() {
        let config = ProductionShardingConfig::for_region("test", "production");
        let manifests = config.generate_kubernetes_manifests().unwrap();
        
        assert!(!manifests.is_empty());
        
        // Check that StatefulSets are generated
        let statefulset_count = manifests.iter()
            .filter(|(filename, _)| filename.contains("statefulset"))
            .count();
        assert!(statefulset_count > 0);
        
        // Check that ConfigMap is included
        let has_configmap = manifests.iter()
            .any(|(filename, _)| filename.contains("config"));
        assert!(has_configmap);
    }
}