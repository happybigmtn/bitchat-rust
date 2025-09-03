use crate::monitoring::metrics::{PerformanceMetrics, METRICS};
use crate::monitoring::system::global_system_monitor;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime_seconds: u64,
    pub memory_mb: u64,
    pub active_peers: usize,
    pub version: String,
    pub health_checks: HashMap<String, ComponentHealth>,
    pub overall_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: ComponentStatus,
    pub score: f64,
    pub message: String,
    pub last_check: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

pub struct HealthCheck {
    start_time: Instant,
    metrics: Arc<PerformanceMetrics>,
}

impl HealthCheck {
    pub fn new(metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            start_time: Instant::now(),
            metrics,
        }
    }

    pub async fn check_health(&self) -> HealthStatus {
        let uptime = self.start_time.elapsed();
        let memory_usage = self.get_memory_usage();
        
        // Get real metrics from global METRICS
        let active_peers = METRICS.network.active_connections.load(std::sync::atomic::Ordering::Relaxed);
        let active_games = METRICS.gaming.active_games.load(std::sync::atomic::Ordering::Relaxed) as u64;
        let network_errors = METRICS.errors.network_errors.load(std::sync::atomic::Ordering::Relaxed);
        let consensus_errors = METRICS.errors.consensus_errors.load(std::sync::atomic::Ordering::Relaxed);
        
        // Perform comprehensive health checks
        let mut health_checks = HashMap::new();
        
        // Network health
        let network_health = self.check_network_health(active_peers, network_errors).await;
        health_checks.insert("network".to_string(), network_health);
        
        // Memory health
        let memory_health = self.check_memory_health(memory_usage);
        health_checks.insert("memory".to_string(), memory_health);
        
        // Gaming health
        let gaming_health = self.check_gaming_health(active_games).await;
        health_checks.insert("gaming".to_string(), gaming_health);
        
        // Consensus health
        let consensus_health = self.check_consensus_health(consensus_errors).await;
        health_checks.insert("consensus".to_string(), consensus_health);
        
        // Calculate overall health score
        let overall_score = health_checks.values()
            .map(|h| h.score)
            .sum::<f64>() / health_checks.len() as f64;
        
        let status = match overall_score {
            score if score >= 0.9 => "healthy",
            score if score >= 0.7 => "degraded", 
            _ => "unhealthy",
        };
        
        HealthStatus {
            status: status.to_string(),
            uptime_seconds: uptime.as_secs(),
            memory_mb: memory_usage / 1024 / 1024,
            active_peers,
            version: env!("CARGO_PKG_VERSION").to_string(),
            health_checks,
            overall_score,
        }
    }

    fn get_memory_usage(&self) -> u64 {
        // Get real memory usage from system monitor
        if let Ok(metrics) = global_system_monitor().collect_metrics() {
            metrics.used_memory_bytes
        } else {
            // Fallback estimation
            1024 * 1024 * 128 // 128MB placeholder
        }
    }
    
    async fn check_network_health(&self, active_peers: usize, network_errors: u64) -> ComponentHealth {
        let score = match (active_peers, network_errors) {
            (0, _) => 0.0, // No peers = unhealthy
            (1..=2, errors) if errors > 10 => 0.3, // Few peers with many errors
            (1..=2, _) => 0.6, // Few peers but working
            (3..=10, errors) if errors > 5 => 0.7, // Good peers but some errors  
            (3..=10, _) => 0.9, // Good peers, healthy
            (_, errors) if errors > 0 => 0.8, // Many peers but some errors
            _ => 1.0, // Many peers, no errors = perfect
        };
        
        let status = match score {
            s if s >= 0.8 => ComponentStatus::Healthy,
            s if s >= 0.5 => ComponentStatus::Degraded,
            s if s > 0.0 => ComponentStatus::Unhealthy,
            _ => ComponentStatus::Unknown,
        };
        
        let message = format!("Network: {} peers connected, {} errors", active_peers, network_errors);
        
        ComponentHealth {
            status,
            score,
            message,
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    fn check_memory_health(&self, memory_usage: u64) -> ComponentHealth {
        let memory_gb = memory_usage as f64 / (1024.0 * 1024.0 * 1024.0);
        
        let (score, status) = match memory_gb {
            mem if mem < 0.5 => (1.0, ComponentStatus::Healthy), // < 500MB
            mem if mem < 1.0 => (0.8, ComponentStatus::Healthy), // < 1GB  
            mem if mem < 2.0 => (0.6, ComponentStatus::Degraded), // < 2GB
            mem if mem < 4.0 => (0.4, ComponentStatus::Degraded), // < 4GB
            _ => (0.2, ComponentStatus::Unhealthy), // >= 4GB
        };
        
        ComponentHealth {
            status,
            score,
            message: format!("Memory usage: {:.2} GB", memory_gb),
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    async fn check_gaming_health(&self, active_games: u64) -> ComponentHealth {
        let score = match active_games {
            0 => 0.7, // No games but that's ok for a new node
            1..=5 => 1.0, // Good number of games
            6..=20 => 0.9, // Many games, slightly higher load
            _ => 0.6, // Too many games might indicate issues
        };
        
        ComponentHealth {
            status: if score >= 0.8 { ComponentStatus::Healthy } else { ComponentStatus::Degraded },
            score,
            message: format!("Gaming: {} active games", active_games),
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    async fn check_consensus_health(&self, consensus_errors: u64) -> ComponentHealth {
        let score = match consensus_errors {
            0 => 1.0, // No consensus errors = perfect
            1..=3 => 0.8, // Few errors, acceptable
            4..=10 => 0.5, // More errors, degraded
            _ => 0.2, // Many errors, unhealthy
        };
        
        let status = match score {
            s if s >= 0.8 => ComponentStatus::Healthy,
            s if s >= 0.5 => ComponentStatus::Degraded,
            _ => ComponentStatus::Unhealthy,
        };
        
        ComponentHealth {
            status,
            score,
            message: format!("Consensus: {} errors detected", consensus_errors),
            last_check: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}
