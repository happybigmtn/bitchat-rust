//! BitCraps Database Scaling Example
//!
//! Demonstrates:
//! - PostgreSQL backend configuration
//! - Database sharding setup  
//! - Connection pooling with PgBouncer
//! - Migration management
//! - Repository pattern usage

use bitcraps::database::*;
use bitcraps::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("🚀 BitCraps Database Scaling Example");
    println!("=====================================");

    // 1. Configure PostgreSQL with sharding
    demo_postgresql_sharding().await?;

    // 2. Configure connection pooling
    demo_connection_pooling().await?;

    // 3. Repository pattern usage
    demo_repository_pattern().await?;

    // 4. Generate deployment configurations
    demo_deployment_configs().await?;

    println!("✅ Database scaling example completed!");
    Ok(())
}

/// Demonstrate PostgreSQL with horizontal sharding
async fn demo_postgresql_sharding() -> Result<()> {
    println!("\n📊 1. PostgreSQL Sharding Configuration");
    println!("----------------------------------------");

    // Create user-based sharding configuration
    let user_sharding = BitCrapsShardingStrategies::user_based_sharding();
    println!("✓ User-based sharding: {} shards", user_sharding.shards.len());

    // Create game-based sharding configuration  
    let game_sharding = BitCrapsShardingStrategies::game_based_sharding();
    println!("✓ Game-based sharding: {} shards", game_sharding.shards.len());

    // Create production sharding configuration
    let production_config = ProductionShardingConfig::for_region("us-west-2", "production");
    println!("✓ Production config for region: {}", production_config.region);
    println!("  - Strategies: {}", production_config.sharding_strategies.len());
    println!("  - Auto-rebalancing: {}", production_config.global_settings.auto_rebalancing);

    // Test consistent hashing
    let hash_ring = ConsistentHashRing::new(HashFunction::Blake3);
    println!("✓ Consistent hash ring created");

    Ok(())
}

/// Demonstrate connection pooling configuration
async fn demo_connection_pooling() -> Result<()> {
    println!("\n🔗 2. Connection Pooling Configuration");
    println!("---------------------------------------");

    // Create PgBouncer configuration for production
    let pgbouncer_config = PgBouncerConfig::for_bitcraps_production(
        "primary.bitcraps.io",
        5432,
        &[
            ("replica1.bitcraps.io".to_string(), 5432),
            ("replica2.bitcraps.io".to_string(), 5432),
        ],
        "bitcraps",
        "bitcraps_user",
    );

    println!("✓ PgBouncer configuration created");
    println!("  - Listen port: {}", pgbouncer_config.listen_port);
    println!("  - Max client connections: {}", pgbouncer_config.max_client_conn);
    println!("  - Pool mode: {:?}", pgbouncer_config.pool_mode);
    println!("  - Databases configured: {}", pgbouncer_config.databases.len());

    // Generate configuration files
    let config_content = pgbouncer_config.to_config_file()?;
    println!("✓ Generated pgbouncer.ini ({} bytes)", config_content.len());

    // Generate userlist
    let mut users = HashMap::new();
    users.insert("bitcraps_user".to_string(), "md5hash123".to_string());
    let userlist = pgbouncer_config.generate_userlist(&users);
    println!("✓ Generated userlist.txt ({} bytes)", userlist.len());

    // Generate Docker Compose
    let docker_compose = pgbouncer_config.generate_docker_compose("pgbouncer");
    println!("✓ Generated Docker Compose configuration");

    Ok(())
}

/// Demonstrate repository pattern with database backends
async fn demo_repository_pattern() -> Result<()> {
    println!("\n📚 3. Repository Pattern Usage");
    println!("------------------------------");

    // For demonstration, use SQLite (in production, would use PostgreSQL)
    let config = DatabaseManagerConfig {
        primary: DatabaseConnection {
            backend: DatabaseBackend::SQLite,
            connection_string: ":memory:".to_string(),
            pool_config: PoolConfiguration::default(),
        },
        read_replicas: Vec::new(),
        sharding: None,
        health_check: HealthCheckConfig {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_failures: 3,
            recovery_interval: Duration::from_secs(60),
        },
        failover: FailoverConfig {
            enable_automatic_failover: true,
            max_retry_attempts: 3,
            retry_interval: Duration::from_millis(100),
            circuit_breaker_threshold: 0.5,
        },
        migrations: MigrationConfig {
            migrations_directory: "src/database/migrations".to_string(),
            auto_migrate: true,
            backup_before_migration: true,
        },
    };

    // Create database manager
    let db_manager = Arc::new(DatabaseManager::new(config).await?);
    println!("✓ Database manager created");

    // Create repository factory
    let repo_factory = RepositoryFactory::new(db_manager.clone());
    println!("✓ Repository factory created");

    // Test repositories
    let user_repo = repo_factory.users();
    let game_repo = repo_factory.games();
    let bet_repo = repo_factory.bets();
    let transaction_repo = repo_factory.transactions();

    println!("✓ All repositories initialized");
    println!("  - User repository: Ready");
    println!("  - Game repository: Ready");
    println!("  - Bet repository: Ready");
    println!("  - Transaction repository: Ready");

    // Test database health
    let health = db_manager.get_health_status().await?;
    println!("✓ Database health check: {}", 
        if health.primary_healthy { "Healthy" } else { "Unhealthy" });

    // Test pool statistics
    let stats = db_manager.get_pool_statistics();
    println!("✓ Connection pool stats:");
    println!("  - Active connections: {}", stats.primary.active_connections);
    println!("  - Total connections: {}", stats.primary.total_connections);

    Ok(())
}

/// Demonstrate deployment configuration generation
async fn demo_deployment_configs() -> Result<()> {
    println!("\n🚀 4. Deployment Configuration Generation");
    println!("------------------------------------------");

    // Create production sharding configuration
    let production_config = ProductionShardingConfig::for_region("us-east-1", "production");

    // Generate Docker Compose for sharded deployment
    let docker_compose = production_config.generate_docker_compose()?;
    println!("✓ Generated Docker Compose for sharded deployment");
    println!("  - Services defined: Multiple PostgreSQL instances + PgBouncer");
    println!("  - Networks: bitcraps-sharded-network");
    println!("  - Volumes: Persistent data storage");

    // Generate Kubernetes manifests
    let k8s_manifests = production_config.generate_kubernetes_manifests()?;
    println!("✓ Generated Kubernetes manifests");
    println!("  - StatefulSets: {}", 
        k8s_manifests.iter().filter(|(name, _)| name.contains("statefulset")).count());
    println!("  - ConfigMaps: {}", 
        k8s_manifests.iter().filter(|(name, _)| name.contains("config")).count());

    // Show example manifest names
    println!("✓ Example manifest files:");
    for (filename, _) in k8s_manifests.iter().take(3) {
        println!("  - {}", filename);
    }

    // Generate systemd service for PgBouncer
    let pgbouncer_config = PgBouncerConfig::for_bitcraps_production(
        "localhost", 5432, &[], "bitcraps", "bitcraps"
    );
    let systemd_service = pgbouncer_config.generate_systemd_service(
        "/etc/pgbouncer/pgbouncer.ini", 
        "pgbouncer"
    );
    println!("✓ Generated systemd service file");
    println!("  - User: pgbouncer");
    println!("  - Security hardened: Yes");

    Ok(())
}

/// Demonstrate different sharding strategies
#[allow(dead_code)]
async fn demo_sharding_strategies() -> Result<()> {
    println!("\n🎯 Advanced Sharding Strategies");
    println!("--------------------------------");

    // User-based sharding
    let user_sharding = BitCrapsShardingStrategies::user_based_sharding();
    println!("👥 User-based sharding:");
    println!("  - Shards: {}", user_sharding.shards.len());
    println!("  - Hash function: {:?}", user_sharding.hash_function);
    println!("  - Auto-scaling: {}", user_sharding.auto_scaling);

    // Game-based sharding
    let game_sharding = BitCrapsShardingStrategies::game_based_sharding();
    println!("🎲 Game-based sharding:");
    println!("  - Shards: {}", game_sharding.shards.len());
    println!("  - Hot shard weight: {}", 
        game_sharding.shards.iter().max_by_key(|s| s.weight).unwrap().weight);

    // Geographic sharding
    let geo_sharding = BitCrapsShardingStrategies::geographic_sharding();
    println!("🌍 Geographic sharding:");
    println!("  - Regions: {}", geo_sharding.shards.len());
    println!("  - Auto-scaling: {}", geo_sharding.auto_scaling);

    // Time-based metrics sharding
    let metrics_sharding = BitCrapsShardingStrategies::time_based_metrics_sharding();
    println!("⏰ Time-based metrics sharding:");
    println!("  - Shards: {}", metrics_sharding.shards.len());
    println!("  - Current shard weight: {}", 
        metrics_sharding.shards.iter().max_by_key(|s| s.weight).unwrap().weight);

    // Hybrid configuration
    let hybrid_configs = BitCrapsShardingStrategies::hybrid_sharding_config();
    println!("🔀 Hybrid sharding configuration:");
    for (strategy, config) in &hybrid_configs {
        println!("  - {}: {} shards", strategy, config.shards.len());
    }

    Ok(())
}

/// Demonstrate migration management
#[allow(dead_code)]
async fn demo_migration_management() -> Result<()> {
    println!("\n📦 Migration Management");
    println!("------------------------");

    // Create in-memory database for demo
    let backend = Box::new(SqliteBackend::new(&DatabaseConnection {
        backend: DatabaseBackend::SQLite,
        connection_string: ":memory:".to_string(),
        pool_config: PoolConfiguration::default(),
    }).await?);

    // Create migration manager
    let migration_manager = MigrationManager::new(
        backend,
        "src/database/migrations/sqlite"
    ).await?;

    println!("✓ Migration manager created");

    // Check migration status
    let status = migration_manager.get_status().await?;
    println!("✓ Migration status: {:?}", status);

    // Get pending migrations
    let pending = migration_manager.get_pending_migrations().await?;
    println!("✓ Pending migrations: {}", pending.len());

    // Validate schema
    let validation = migration_manager.validate_schema().await?;
    println!("✓ Schema validation: {}", 
        if validation.is_valid { "Valid" } else { "Invalid" });

    if !validation.is_valid {
        for issue in &validation.issues {
            println!("  - Issue: {}", issue);
        }
    }

    Ok(())
}

/// Demonstrate database scaling monitoring
#[allow(dead_code)]  
async fn demo_scaling_monitoring() -> Result<()> {
    println!("\n📊 Database Scaling Monitoring");
    println!("-------------------------------");

    // This would integrate with your monitoring system
    println!("✓ Key metrics to monitor:");
    println!("  - Connection pool utilization");
    println!("  - Query response times");
    println!("  - Shard load distribution");
    println!("  - Replication lag");
    println!("  - Cache hit ratios");
    println!("  - Lock contention");

    println!("✓ Alerting thresholds:");
    println!("  - Pool utilization > 80%");
    println!("  - Query response time > 500ms");
    println!("  - Replication lag > 1 second");
    println!("  - Error rate > 1%");

    Ok(())
}