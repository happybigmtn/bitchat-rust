use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Network configuration for BitCraps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// STUN servers for NAT traversal
    pub stun_servers: Vec<String>,

    /// TURN servers for relaying
    pub turn_servers: Vec<TurnServerConfig>,

    /// Bootstrap nodes for initial discovery
    pub bootstrap_nodes: Vec<SocketAddr>,

    /// Default listening port
    pub default_port: u16,

    /// Enable IPv6 support
    pub enable_ipv6: bool,

    /// Connection timeout in seconds
    pub connection_timeout: u64,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Enable UPnP for automatic port forwarding
    pub enable_upnp: bool,
}

/// TURN server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServerConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub realm: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
                "stun3.l.google.com:19302".to_string(),
                "stun4.l.google.com:19302".to_string(),
            ],
            turn_servers: vec![
                // Public TURN servers (add your own for production)
                TurnServerConfig {
                    url: "turn:openrelay.metered.ca:80".to_string(),
                    username: Some("openrelayproject".to_string()),
                    password: Some("openrelayproject".to_string()),
                    realm: None,
                },
            ],
            bootstrap_nodes: vec![
                // Default bootstrap nodes - replace with actual nodes in production
                // These are placeholder addresses
            ],
            default_port: 42069,
            enable_ipv6: true,
            connection_timeout: 30,
            max_connections: 1000,
            enable_upnp: true,
        }
    }
}

impl NetworkConfig {
    /// Load from environment variables or config file
    pub fn from_env() -> Self {
        // Try to load from env vars first
        if let Ok(stun_servers) = std::env::var("BITCRAPS_STUN_SERVERS") {
            let mut config = Self::default();
            config.stun_servers = stun_servers.split(',').map(|s| s.to_string()).collect();

            if let Ok(bootstrap) = std::env::var("BITCRAPS_BOOTSTRAP_NODES") {
                config.bootstrap_nodes = bootstrap
                    .split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();
            }

            if let Ok(port) = std::env::var("BITCRAPS_PORT") {
                if let Ok(p) = port.parse() {
                    config.default_port = p;
                }
            }

            config
        } else {
            // Load from config file or use defaults
            Self::default()
        }
    }

    /// Get the best STUN server (could implement latency testing)
    pub fn get_stun_server(&self) -> Option<&str> {
        self.stun_servers.first().map(|s| s.as_str())
    }

    /// Get TURN servers for relay
    pub fn get_turn_servers(&self) -> &[TurnServerConfig] {
        &self.turn_servers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NetworkConfig::default();
        assert!(!config.stun_servers.is_empty());
        assert_eq!(config.default_port, 42069);
        assert!(config.enable_ipv6);
    }

    #[test]
    fn test_env_config() {
        std::env::set_var("BITCRAPS_STUN_SERVERS", "stun.example.com:3478");
        std::env::set_var("BITCRAPS_PORT", "8080");

        let config = NetworkConfig::from_env();
        assert_eq!(config.stun_servers.len(), 1);
        assert_eq!(config.stun_servers[0], "stun.example.com:3478");
        assert_eq!(config.default_port, 8080);

        // Clean up
        std::env::remove_var("BITCRAPS_STUN_SERVERS");
        std::env::remove_var("BITCRAPS_PORT");
    }
}