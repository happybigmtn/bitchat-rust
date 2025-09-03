//! Metrics integration service - connects real application data to Prometheus metrics
//!
//! This module provides a service that periodically collects real application
//! data and updates the global METRICS instance used by Prometheus.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use log::{info, warn, error};

use crate::monitoring::metrics::METRICS;
use crate::app::BitCrapsApp;

// Direct integration with BitCrapsApp - no placeholder structs needed

/// Metrics integration service that updates global metrics with real application data
pub struct MetricsIntegrationService {
    app_ref: Arc<BitCrapsApp>,
    update_interval: Duration,
}

impl MetricsIntegrationService {
    /// Create new metrics integration service
    pub fn new(app_ref: Arc<BitCrapsApp>) -> Self {
        Self {
            app_ref,
            update_interval: Duration::from_secs(5), // Update every 5 seconds
        }
    }

    /// Start the metrics integration service
    pub async fn start(&self) {
        info!("Starting metrics integration service");
        let mut interval = interval(self.update_interval);

        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_metrics().await {
                warn!("Failed to update metrics: {}", e);
            }
        }
    }

    /// Update global metrics with real application data
    async fn update_metrics(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Get real data from the application
        let active_games = self.app_ref.get_active_games().await
            .unwrap_or_else(|e| {
                log::warn!("Failed to get active games: {}", e);
                Vec::new()
            });
        
        // Get peer ID for identification
        let peer_id = self.app_ref.peer_id();
        
        // Get memory pool stats for performance metrics
        let memory_pools = self.app_ref.get_memory_pools();
        let pool_stats = memory_pools.combined_stats().await;
        
        // Update metrics based on real application state
        let active_game_count = active_games.len();
        let connected_peers = METRICS.network.active_connections.load(std::sync::atomic::Ordering::Relaxed);
        let active_sessions = connected_peers; // Sessions equal connections for now
        
        // Update gaming metrics with real data
        METRICS.gaming.active_games.store(
            active_game_count,
            std::sync::atomic::Ordering::Relaxed
        );

        METRICS.gaming.total_games.store(
            active_game_count as u64,
            std::sync::atomic::Ordering::Relaxed
        );
        
        // Calculate cache efficiency
        let cache_efficiency = if pool_stats.vec_u8_stats.allocations > 0 {
            (pool_stats.vec_u8_stats.cache_hits as f64 / pool_stats.vec_u8_stats.allocations as f64) * 100.0
        } else {
            0.0
        };
        
        // Update cache efficiency for performance monitoring
        {
            let mut cache_hit_rate = METRICS.performance.cache_hit_rate.write();
            *cache_hit_rate = cache_efficiency;
        }
        
        // Log performance metrics
        if active_game_count > 0 {
            let games_per_minute = active_game_count as f64 * 60.0 / self.update_interval.as_secs() as f64;
            info!("Metrics: {} active games, {:.2} games/min, {:.1}% cache efficiency, peer: {:?}",
                active_game_count, games_per_minute, cache_efficiency, peer_id);
        }
        
        // Update network health monitoring
        if connected_peers == 0 && active_game_count > 0 {
            // Games active but no peers - potential issue
            METRICS.errors.network_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            log::warn!("Network issue detected: games active but no peer connections");
        }

        Ok(())
    }
}

/// Initialize and start metrics integration with the application
pub async fn start_metrics_integration(app: Arc<BitCrapsApp>) -> tokio::task::JoinHandle<()> {
    let service = MetricsIntegrationService::new(app);
    
    tokio::spawn(async move {
        service.start().await;
    })
}

/// Helper function to record custom gaming metrics
pub fn record_game_event(event_type: &str, game_id: &str) {
    match event_type {
        "game_created" => {
            METRICS.gaming.total_games.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            info!("Game created: {}", game_id);
        },
        "bet_placed" => {
            METRICS.gaming.total_bets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        "dice_rolled" => {
            METRICS.gaming.dice_rolls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        "game_completed" => {
            METRICS.gaming.active_games.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        },
        _ => {
            warn!("Unknown game event type: {}", event_type);
        }
    }
}

/// Record network events
pub fn record_network_event(event_type: &str, peer_id: Option<&str>) {
    match event_type {
        "peer_connected" => {
            METRICS.network.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(id) = peer_id {
                info!("Peer connected: {}", id);
            }
        },
        "peer_disconnected" => {
            METRICS.network.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            if let Some(id) = peer_id {
                info!("Peer disconnected: {}", id);
            }
        },
        "message_sent" => {
            METRICS.network.messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        "message_received" => {
            METRICS.network.messages_received.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        _ => {
            warn!("Unknown network event type: {}", event_type);
        }
    }
}

/// Record errors for monitoring
pub fn record_error(category: &str, severity: &str, message: &str) {
    match category {
        "network" => {
            METRICS.errors.network_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        "consensus" => {
            METRICS.errors.consensus_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        "gaming" => {
            METRICS.errors.gaming_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
        _ => {
            METRICS.errors.critical_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    
    match severity {
        "critical" | "error" => error!("[{}] {}", category, message),
        "warn" | "warning" => warn!("[{}] {}", category, message),
        _ => info!("[{}] {}", category, message),
    }
}