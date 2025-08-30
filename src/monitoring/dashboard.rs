use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Network metrics aggregated from all nodes
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub node_count: u64,
    pub game_count: u64,
    pub volume: u64,
    pub hash_rate: u64,
}

/// Production monitoring dashboard for BitCraps network
///
/// Feynman: Like a mission control center for our casino network.
/// All the screens showing vital signs - how many players are online,
/// how much money is flowing, whether the network is healthy.
pub struct NetworkDashboard {
    total_nodes: Arc<AtomicU64>,
    active_games: Arc<AtomicU64>,
    total_volume: Arc<AtomicU64>,
    mining_rate: Arc<AtomicU64>,
}

impl NetworkDashboard {
    pub fn new() -> Self {
        Self {
            total_nodes: Arc::new(AtomicU64::new(0)),
            active_games: Arc::new(AtomicU64::new(0)),
            total_volume: Arc::new(AtomicU64::new(0)),
            mining_rate: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Start the metrics collection loop
    pub async fn start_collection(&self) {
        let total_nodes = self.total_nodes.clone();
        let active_games = self.active_games.clone();
        let total_volume = self.total_volume.clone();
        let mining_rate = self.mining_rate.clone();

        tokio::spawn(async move {
            let dashboard = NetworkDashboard {
                total_nodes,
                active_games,
                total_volume,
                mining_rate,
            };
            dashboard.collect_metrics().await;
        });
    }

    /// Continuously collect network metrics
    pub async fn collect_metrics(&self) {
        loop {
            // Collect from all bootstrap nodes
            let metrics = self.aggregate_network_metrics().await;

            // Update dashboard
            self.total_nodes
                .store(metrics.node_count, Ordering::Relaxed);
            self.active_games
                .store(metrics.game_count, Ordering::Relaxed);
            self.total_volume.store(metrics.volume, Ordering::Relaxed);
            self.mining_rate.store(metrics.hash_rate, Ordering::Relaxed);

            // Log to monitoring service
            println!("📊 Network Stats:");
            println!("   Nodes: {}", metrics.node_count);
            println!("   Games: {}", metrics.game_count);
            println!("   Volume: {} CRAP", metrics.volume / 1_000_000);
            println!("   Mining: {} msgs/sec", metrics.hash_rate);

            // Send to external monitoring (Grafana, etc.)
            self.export_to_grafana(&metrics).await;

            sleep(Duration::from_secs(30)).await;
        }
    }

    /// Aggregate metrics from all known bootstrap nodes
    async fn aggregate_network_metrics(&self) -> NetworkMetrics {
        // In a real implementation, this would query multiple bootstrap nodes
        // For now, return mock data
        NetworkMetrics {
            node_count: self.total_nodes.load(Ordering::Relaxed).saturating_add(1),
            game_count: self.active_games.load(Ordering::Relaxed),
            volume: self.total_volume.load(Ordering::Relaxed),
            hash_rate: self.mining_rate.load(Ordering::Relaxed),
        }
    }

    /// Export metrics to Grafana/Prometheus
    async fn export_to_grafana(&self, metrics: &NetworkMetrics) {
        // In a real implementation, this would push to a metrics endpoint
        // For now, just log in Prometheus format
        println!("# HELP bitcraps_total_nodes Total number of active nodes");
        println!("# TYPE bitcraps_total_nodes gauge");
        println!("bitcraps_total_nodes {}", metrics.node_count);

        println!("# HELP bitcraps_active_games Number of active games");
        println!("# TYPE bitcraps_active_games gauge");
        println!("bitcraps_active_games {}", metrics.game_count);

        println!("# HELP bitcraps_total_volume_crap Total volume in CRAP tokens");
        println!("# TYPE bitcraps_total_volume_crap counter");
        println!("bitcraps_total_volume_crap {}", metrics.volume);

        println!("# HELP bitcraps_mining_rate_msgs_per_sec Messages relayed per second");
        println!("# TYPE bitcraps_mining_rate_msgs_per_sec gauge");
        println!("bitcraps_mining_rate_msgs_per_sec {}", metrics.hash_rate);
    }

    /// Get current network statistics
    pub fn get_stats(&self) -> NetworkMetrics {
        NetworkMetrics {
            node_count: self.total_nodes.load(Ordering::Relaxed),
            game_count: self.active_games.load(Ordering::Relaxed),
            volume: self.total_volume.load(Ordering::Relaxed),
            hash_rate: self.mining_rate.load(Ordering::Relaxed),
        }
    }

    /// Update node count
    pub fn update_node_count(&self, count: u64) {
        self.total_nodes.store(count, Ordering::Relaxed);
    }

    /// Update active games count
    pub fn update_game_count(&self, count: u64) {
        self.active_games.store(count, Ordering::Relaxed);
    }

    /// Add to total volume
    pub fn add_volume(&self, amount: u64) {
        self.total_volume.fetch_add(amount, Ordering::Relaxed);
    }

    /// Update mining/relay rate
    pub fn update_mining_rate(&self, rate: u64) {
        self.mining_rate.store(rate, Ordering::Relaxed);
    }
}

impl Default for NetworkDashboard {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check endpoint for load balancers
pub struct HealthCheck {
    dashboard: Arc<NetworkDashboard>,
}

impl HealthCheck {
    pub fn new(dashboard: Arc<NetworkDashboard>) -> Self {
        Self { dashboard }
    }

    /// Check if the network is healthy
    pub fn is_healthy(&self) -> bool {
        let stats = self.dashboard.get_stats();

        // Network is healthy if:
        // - We have at least 1 node
        // - Mining rate is reasonable (not stuck)
        stats.node_count > 0 && stats.hash_rate < 10000 // Prevent spam
    }

    /// Get health status as JSON string
    pub fn health_json(&self) -> String {
        let healthy = self.is_healthy();
        let stats = self.dashboard.get_stats();

        format!(
            r#"{{
    "healthy": {},
    "nodes": {},
    "games": {},
    "volume_crap": {},
    "mining_rate": {}
}}"#,
            healthy,
            stats.node_count,
            stats.game_count,
            stats.volume / 1_000_000,
            stats.hash_rate
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_metrics() {
        let dashboard = NetworkDashboard::new();

        // Update some metrics
        dashboard.update_node_count(5);
        dashboard.update_game_count(3);
        dashboard.add_volume(1_000_000);
        dashboard.update_mining_rate(15);

        let stats = dashboard.get_stats();
        assert_eq!(stats.node_count, 5);
        assert_eq!(stats.game_count, 3);
        assert_eq!(stats.volume, 1_000_000);
        assert_eq!(stats.hash_rate, 15);
    }

    #[test]
    fn test_health_check() {
        let dashboard = Arc::new(NetworkDashboard::new());
        let health = HealthCheck::new(dashboard.clone());

        // Should be unhealthy with no nodes
        assert!(!health.is_healthy());

        // Add a node
        dashboard.update_node_count(1);
        assert!(health.is_healthy());

        // Test JSON output
        let json = health.health_json();
        assert!(json.contains("\"healthy\": true"));
        assert!(json.contains("\"nodes\": 1"));
    }
}
