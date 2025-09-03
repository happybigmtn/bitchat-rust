//! PgBouncer configuration generator for BitCraps
//!
//! Generates optimized PgBouncer configurations for:
//! - Connection pooling with multiple databases
//! - Load balancing across read replicas
//! - Failover handling
//! - Performance tuning for high-throughput workloads

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// PgBouncer pool modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolMode {
    /// Session pooling - one connection per session
    Session,
    /// Transaction pooling - one connection per transaction (recommended)
    Transaction,
    /// Statement pooling - one connection per statement (highest performance)
    Statement,
}

/// PgBouncer server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgBouncerServer {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
    pub pool_size: u32,
    pub pool_mode: PoolMode,
    pub max_client_conn: u32,
    pub reserve_pool_size: u32,
    pub server_round_robin: bool,
}

/// PgBouncer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgBouncerConfig {
    /// Listen address and port
    pub listen_addr: String,
    pub listen_port: u16,
    
    /// Authentication
    pub auth_type: String,
    pub auth_file: Option<String>,
    
    /// Connection limits
    pub max_client_conn: u32,
    pub default_pool_size: u32,
    pub max_db_connections: u32,
    pub max_user_connections: u32,
    
    /// Pool configuration
    pub pool_mode: PoolMode,
    pub server_round_robin: bool,
    pub ignore_startup_parameters: Vec<String>,
    
    /// Timeouts (in seconds)
    pub server_connect_timeout: u32,
    pub server_login_retry: u32,
    pub server_lifetime: u32,
    pub server_idle_timeout: u32,
    pub query_timeout: u32,
    pub query_wait_timeout: u32,
    pub client_idle_timeout: u32,
    pub client_login_timeout: u32,
    
    /// Logging
    pub log_connections: bool,
    pub log_disconnections: bool,
    pub log_pooler_errors: bool,
    pub log_stats: bool,
    pub stats_period: u32,
    
    /// Performance tuning
    pub tcp_keepalive: bool,
    pub tcp_keepcnt: u32,
    pub tcp_keepidle: u32,
    pub tcp_keepintvl: u32,
    
    /// SSL configuration
    pub server_tls_sslmode: String,
    pub client_tls_sslmode: String,
    
    /// Database definitions
    pub databases: HashMap<String, PgBouncerDatabase>,
    
    /// User definitions
    pub users: HashMap<String, PgBouncerUser>,
}

/// PgBouncer database definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgBouncerDatabase {
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub pool_size: Option<u32>,
    pub pool_mode: Option<PoolMode>,
    pub max_db_connections: Option<u32>,
    pub auth_user: Option<String>,
}

/// PgBouncer user definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgBouncerUser {
    pub password: Option<String>,
    pub pool_mode: Option<PoolMode>,
    pub max_user_connections: Option<u32>,
}

impl Default for PgBouncerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1".to_string(),
            listen_port: 6432,
            auth_type: "md5".to_string(),
            auth_file: Some("/etc/pgbouncer/userlist.txt".to_string()),
            max_client_conn: 1000,
            default_pool_size: 25,
            max_db_connections: 100,
            max_user_connections: 100,
            pool_mode: PoolMode::Transaction,
            server_round_robin: true,
            ignore_startup_parameters: vec![
                "extra_float_digits".to_string(),
                "search_path".to_string(),
            ],
            server_connect_timeout: 15,
            server_login_retry: 15,
            server_lifetime: 3600,
            server_idle_timeout: 600,
            query_timeout: 0, // No timeout
            query_wait_timeout: 120,
            client_idle_timeout: 0, // No timeout
            client_login_timeout: 60,
            log_connections: true,
            log_disconnections: true,
            log_pooler_errors: true,
            log_stats: true,
            stats_period: 60,
            tcp_keepalive: true,
            tcp_keepcnt: 3,
            tcp_keepidle: 7200,
            tcp_keepintvl: 75,
            server_tls_sslmode: "prefer".to_string(),
            client_tls_sslmode: "disable".to_string(),
            databases: HashMap::new(),
            users: HashMap::new(),
        }
    }
}

impl PgBouncerConfig {
    /// Create a production-optimized configuration for BitCraps
    pub fn for_bitcraps_production(
        primary_host: &str,
        primary_port: u16,
        read_replicas: &[(String, u16)],
        database_name: &str,
        username: &str,
    ) -> Self {
        let mut config = Self {
            listen_addr: "0.0.0.0".to_string(),
            listen_port: 5432, // Standard PostgreSQL port
            max_client_conn: 2000,
            default_pool_size: 50,
            max_db_connections: 200,
            pool_mode: PoolMode::Transaction, // Optimal for OLTP workloads
            server_round_robin: true,
            server_lifetime: 7200, // 2 hours
            server_idle_timeout: 300, // 5 minutes
            query_timeout: 30, // 30 seconds
            query_wait_timeout: 60, // 1 minute
            tcp_keepalive: true,
            log_stats: true,
            stats_period: 30,
            ..Default::default()
        };
        
        // Primary database for writes
        config.databases.insert(
            format!("{}_primary", database_name),
            PgBouncerDatabase {
                host: primary_host.to_string(),
                port: primary_port,
                dbname: database_name.to_string(),
                pool_size: Some(30), // Higher pool for primary
                pool_mode: Some(PoolMode::Transaction),
                max_db_connections: Some(50),
                auth_user: Some(username.to_string()),
            }
        );
        
        // Read replicas for read-only queries
        for (i, (replica_host, replica_port)) in read_replicas.iter().enumerate() {
            config.databases.insert(
                format!("{}_replica_{}", database_name, i + 1),
                PgBouncerDatabase {
                    host: replica_host.clone(),
                    port: *replica_port,
                    dbname: database_name.to_string(),
                    pool_size: Some(20), // Smaller pool for replicas
                    pool_mode: Some(PoolMode::Transaction),
                    max_db_connections: Some(30),
                    auth_user: Some(username.to_string()),
                }
            );
        }
        
        // Load balancer database (round-robin across replicas)
        if !read_replicas.is_empty() {
            config.databases.insert(
                format!("{}_readonly", database_name),
                PgBouncerDatabase {
                    host: read_replicas[0].0.clone(), // PgBouncer will handle round-robin
                    port: read_replicas[0].1,
                    dbname: database_name.to_string(),
                    pool_size: Some(40),
                    pool_mode: Some(PoolMode::Transaction),
                    max_db_connections: Some(80),
                    auth_user: Some(username.to_string()),
                }
            );
        }
        
        // User configuration
        config.users.insert(
            username.to_string(),
            PgBouncerUser {
                password: None, // Will be loaded from auth_file
                pool_mode: Some(PoolMode::Transaction),
                max_user_connections: Some(100),
            }
        );
        
        config
    }
    
    /// Create a development-friendly configuration
    pub fn for_development(
        host: &str,
        port: u16,
        database_name: &str,
        username: &str,
    ) -> Self {
        let mut config = Self {
            listen_addr: "127.0.0.1".to_string(),
            listen_port: 6432,
            max_client_conn: 100,
            default_pool_size: 5,
            max_db_connections: 20,
            pool_mode: PoolMode::Session, // Easier debugging
            server_round_robin: false,
            log_connections: true,
            log_disconnections: true,
            log_pooler_errors: true,
            client_tls_sslmode: "disable".to_string(),
            server_tls_sslmode: "disable".to_string(),
            ..Default::default()
        };
        
        config.databases.insert(
            database_name.to_string(),
            PgBouncerDatabase {
                host: host.to_string(),
                port,
                dbname: database_name.to_string(),
                pool_size: Some(5),
                pool_mode: Some(PoolMode::Session),
                max_db_connections: Some(10),
                auth_user: Some(username.to_string()),
            }
        );
        
        config.users.insert(
            username.to_string(),
            PgBouncerUser {
                password: None,
                pool_mode: Some(PoolMode::Session),
                max_user_connections: Some(20),
            }
        );
        
        config
    }
    
    /// Generate PgBouncer configuration file content
    pub fn to_config_file(&self) -> Result<String> {
        let mut content = String::new();
        
        // [databases] section
        content.push_str("[databases]\n");
        for (name, db) in &self.databases {
            content.push_str(&format!(
                "{} = host={} port={} dbname={}",
                name, db.host, db.port, db.dbname
            ));
            
            if let Some(pool_size) = db.pool_size {
                content.push_str(&format!(" pool_size={}", pool_size));
            }
            
            if let Some(ref pool_mode) = db.pool_mode {
                let mode_str = match pool_mode {
                    PoolMode::Session => "session",
                    PoolMode::Transaction => "transaction",
                    PoolMode::Statement => "statement",
                };
                content.push_str(&format!(" pool_mode={}", mode_str));
            }
            
            if let Some(max_db_connections) = db.max_db_connections {
                content.push_str(&format!(" max_db_connections={}", max_db_connections));
            }
            
            if let Some(ref auth_user) = db.auth_user {
                content.push_str(&format!(" auth_user={}", auth_user));
            }
            
            content.push('\n');
        }
        
        content.push('\n');
        
        // [users] section
        content.push_str("[users]\n");
        for (name, user) in &self.users {
            content.push_str(&format!("{} = ", name));
            
            if let Some(ref password) = user.password {
                content.push_str(&format!(" password={}", password));
            }
            
            if let Some(ref pool_mode) = user.pool_mode {
                let mode_str = match pool_mode {
                    PoolMode::Session => "session",
                    PoolMode::Transaction => "transaction",
                    PoolMode::Statement => "statement",
                };
                content.push_str(&format!(" pool_mode={}", mode_str));
            }
            
            if let Some(max_user_connections) = user.max_user_connections {
                content.push_str(&format!(" max_user_connections={}", max_user_connections));
            }
            
            content.push('\n');
        }
        
        content.push('\n');
        
        // [pgbouncer] section
        content.push_str("[pgbouncer]\n");
        content.push_str(&format!("listen_addr = {}\n", self.listen_addr));
        content.push_str(&format!("listen_port = {}\n", self.listen_port));
        content.push_str(&format!("auth_type = {}\n", self.auth_type));
        
        if let Some(ref auth_file) = self.auth_file {
            content.push_str(&format!("auth_file = {}\n", auth_file));
        }
        
        content.push_str(&format!("max_client_conn = {}\n", self.max_client_conn));
        content.push_str(&format!("default_pool_size = {}\n", self.default_pool_size));
        content.push_str(&format!("max_db_connections = {}\n", self.max_db_connections));
        content.push_str(&format!("max_user_connections = {}\n", self.max_user_connections));
        
        let pool_mode_str = match self.pool_mode {
            PoolMode::Session => "session",
            PoolMode::Transaction => "transaction",
            PoolMode::Statement => "statement",
        };
        content.push_str(&format!("pool_mode = {}\n", pool_mode_str));
        
        content.push_str(&format!("server_round_robin = {}\n", 
            if self.server_round_robin { "1" } else { "0" }));
        
        if !self.ignore_startup_parameters.is_empty() {
            content.push_str(&format!("ignore_startup_parameters = {}\n",
                self.ignore_startup_parameters.join(",")));
        }
        
        // Timeouts
        content.push_str(&format!("server_connect_timeout = {}\n", self.server_connect_timeout));
        content.push_str(&format!("server_login_retry = {}\n", self.server_login_retry));
        content.push_str(&format!("server_lifetime = {}\n", self.server_lifetime));
        content.push_str(&format!("server_idle_timeout = {}\n", self.server_idle_timeout));
        content.push_str(&format!("query_timeout = {}\n", self.query_timeout));
        content.push_str(&format!("query_wait_timeout = {}\n", self.query_wait_timeout));
        content.push_str(&format!("client_idle_timeout = {}\n", self.client_idle_timeout));
        content.push_str(&format!("client_login_timeout = {}\n", self.client_login_timeout));
        
        // Logging
        content.push_str(&format!("log_connections = {}\n", 
            if self.log_connections { "1" } else { "0" }));
        content.push_str(&format!("log_disconnections = {}\n", 
            if self.log_disconnections { "1" } else { "0" }));
        content.push_str(&format!("log_pooler_errors = {}\n", 
            if self.log_pooler_errors { "1" } else { "0" }));
        content.push_str(&format!("log_stats = {}\n", 
            if self.log_stats { "1" } else { "0" }));
        content.push_str(&format!("stats_period = {}\n", self.stats_period));
        
        // TCP keepalive
        content.push_str(&format!("tcp_keepalive = {}\n", 
            if self.tcp_keepalive { "1" } else { "0" }));
        content.push_str(&format!("tcp_keepcnt = {}\n", self.tcp_keepcnt));
        content.push_str(&format!("tcp_keepidle = {}\n", self.tcp_keepidle));
        content.push_str(&format!("tcp_keepintvl = {}\n", self.tcp_keepintvl));
        
        // SSL
        content.push_str(&format!("server_tls_sslmode = {}\n", self.server_tls_sslmode));
        content.push_str(&format!("client_tls_sslmode = {}\n", self.client_tls_sslmode));
        
        Ok(content)
    }
    
    /// Generate userlist.txt file content for authentication
    pub fn generate_userlist(&self, users_with_passwords: &HashMap<String, String>) -> String {
        let mut content = String::new();
        
        for (username, password) in users_with_passwords {
            // Format: "username" "password_hash"
            content.push_str(&format!("\"{}\" \"{}\"\n", username, password));
        }
        
        content
    }
    
    /// Write configuration files to disk
    pub fn write_config_files(
        &self,
        config_dir: impl AsRef<Path>,
        users_with_passwords: &HashMap<String, String>,
    ) -> Result<()> {
        let config_dir = config_dir.as_ref();
        
        // Create directory if it doesn't exist
        std::fs::create_dir_all(config_dir)
            .map_err(|e| Error::Database(format!("Failed to create config directory: {}", e)))?;
        
        // Write pgbouncer.ini
        let config_content = self.to_config_file()?;
        let config_path = config_dir.join("pgbouncer.ini");
        fs::write(&config_path, config_content)
            .map_err(|e| Error::Database(format!("Failed to write pgbouncer.ini: {}", e)))?;
        
        // Write userlist.txt
        let userlist_content = self.generate_userlist(users_with_passwords);
        let userlist_path = config_dir.join("userlist.txt");
        fs::write(&userlist_path, userlist_content)
            .map_err(|e| Error::Database(format!("Failed to write userlist.txt: {}", e)))?;
        
        log::info!("PgBouncer configuration written to: {}", config_dir.display());
        log::info!("Config file: {}", config_path.display());
        log::info!("User list file: {}", userlist_path.display());
        
        Ok(())
    }
    
    /// Generate Docker Compose configuration for PgBouncer
    pub fn generate_docker_compose(&self, service_name: &str) -> String {
        format!(r#"version: '3.8'

services:
  {}:
    image: pgbouncer/pgbouncer:latest
    environment:
      - DATABASES_HOST=postgres
      - DATABASES_PORT=5432
      - DATABASES_USER=bitcraps
      - DATABASES_PASSWORD=bitcraps_password
      - DATABASES_DBNAME=bitcraps
      - POOL_MODE=transaction
      - SERVER_RESET_QUERY=DISCARD ALL
      - MAX_CLIENT_CONN=1000
      - DEFAULT_POOL_SIZE=50
      - MAX_DB_CONNECTIONS=200
      - SERVER_LIFETIME=7200
      - SERVER_IDLE_TIMEOUT=600
      - LOG_CONNECTIONS=1
      - LOG_DISCONNECTIONS=1
      - LOG_POOLER_ERRORS=1
    ports:
      - "{}:5432"
    volumes:
      - ./pgbouncer:/etc/pgbouncer:ro
    restart: unless-stopped
    depends_on:
      - postgres
    networks:
      - bitcraps-network

networks:
  bitcraps-network:
    driver: bridge
"#, service_name, self.listen_port)
    }
    
    /// Generate systemd service file for PgBouncer
    pub fn generate_systemd_service(&self, config_path: &str, user: &str) -> String {
        format!(r#"[Unit]
Description=PgBouncer connection pooler for BitCraps
Documentation=https://www.pgbouncer.org/
Wants=postgresql.service
After=postgresql.service network.target

[Service]
Type=forking
User={}
Group={}
ExecStart=/usr/bin/pgbouncer -d {}
ExecReload=/bin/kill -HUP $MAINPID
KillMode=mixed
KillSignal=SIGINT
TimeoutSec=120

# Security settings
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes

# Resource limits
LimitNOFILE=65536
LimitNPROC=32768

[Install]
WantedBy=multi-user.target
"#, user, user, config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = PgBouncerConfig::default();
        assert_eq!(config.listen_port, 6432);
        assert_eq!(config.max_client_conn, 1000);
        assert_eq!(config.default_pool_size, 25);
        assert!(matches!(config.pool_mode, PoolMode::Transaction));
    }
    
    #[test]
    fn test_production_config() {
        let replicas = vec![
            ("replica1.example.com".to_string(), 5432),
            ("replica2.example.com".to_string(), 5432),
        ];
        
        let config = PgBouncerConfig::for_bitcraps_production(
            "primary.example.com",
            5432,
            &replicas,
            "bitcraps",
            "bitcraps_user",
        );
        
        assert_eq!(config.listen_addr, "0.0.0.0");
        assert_eq!(config.max_client_conn, 2000);
        assert_eq!(config.databases.len(), 4); // primary + 2 replicas + readonly
        
        assert!(config.databases.contains_key("bitcraps_primary"));
        assert!(config.databases.contains_key("bitcraps_replica_1"));
        assert!(config.databases.contains_key("bitcraps_replica_2"));
        assert!(config.databases.contains_key("bitcraps_readonly"));
    }
    
    #[test]
    fn test_config_file_generation() {
        let mut config = PgBouncerConfig::default();
        config.databases.insert(
            "test_db".to_string(),
            PgBouncerDatabase {
                host: "localhost".to_string(),
                port: 5432,
                dbname: "test".to_string(),
                pool_size: Some(10),
                pool_mode: Some(PoolMode::Transaction),
                max_db_connections: Some(20),
                auth_user: Some("test_user".to_string()),
            }
        );
        
        let config_content = config.to_config_file().unwrap();
        
        assert!(config_content.contains("[databases]"));
        assert!(config_content.contains("[users]"));
        assert!(config_content.contains("[pgbouncer]"));
        assert!(config_content.contains("test_db = host=localhost port=5432"));
        assert!(config_content.contains("pool_mode = transaction"));
        assert!(config_content.contains("listen_port = 6432"));
    }
    
    #[test]
    fn test_userlist_generation() {
        let config = PgBouncerConfig::default();
        let mut users = HashMap::new();
        users.insert("user1".to_string(), "md5hash1".to_string());
        users.insert("user2".to_string(), "md5hash2".to_string());
        
        let userlist = config.generate_userlist(&users);
        
        assert!(userlist.contains("\"user1\" \"md5hash1\""));
        assert!(userlist.contains("\"user2\" \"md5hash2\""));
    }
}