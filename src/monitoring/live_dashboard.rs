//! Live monitoring dashboard HTTP API
//!
//! This module provides a comprehensive HTTP API for monitoring BitCraps
//! with real-time KPIs for operators to observe peers, routes, messages, and games.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use warp::{Filter, Reply};

use crate::monitoring::health::HealthCheck;
use crate::monitoring::metrics::{METRICS, PerformanceMetrics};
use crate::monitoring::dashboard::NetworkDashboard;

/// Comprehensive live dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveDashboardData {
    /// Network overview
    pub network: NetworkOverview,
    /// Gaming metrics
    pub gaming: GamingOverview,
    /// System health
    pub health: SystemHealth,
    /// Performance metrics
    pub performance: PerformanceOverview,
    /// Real-time activity feed
    pub activity: Vec<ActivityEvent>,
    /// Timestamp of this snapshot
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkOverview {
    pub connected_peers: usize,
    pub active_sessions: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connection_errors: u64,
    pub network_quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingOverview {
    pub active_games: u64,
    pub total_games: u64,
    pub total_bets: u64,
    pub total_volume: u64,
    pub total_payouts: u64,
    pub dice_rolls: u64,
    pub disputes: u64,
    pub avg_game_duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: String,
    pub overall_score: f64,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
    pub uptime_seconds: u64,
    pub components: HashMap<String, ComponentStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub status: String,
    pub score: f64,
    pub message: String,
    pub last_check: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceOverview {
    pub consensus_proposals_submitted: u64,
    pub consensus_proposals_accepted: u64,
    pub consensus_proposals_rejected: u64,
    pub consensus_forks: u64,
    pub avg_consensus_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub description: String,
    pub severity: String,
}

/// Live dashboard service
pub struct LiveDashboardService {
    health_checker: HealthCheck,
    dashboard: NetworkDashboard,
}

impl LiveDashboardService {
    pub fn new() -> Self {
        let metrics = std::sync::Arc::new(PerformanceMetrics::new());
        let health_checker = HealthCheck::new(metrics);
        let dashboard = NetworkDashboard::new();
        
        Self {
            health_checker,
            dashboard,
        }
    }

    /// Get comprehensive dashboard data
    pub async fn get_dashboard_data(&self) -> LiveDashboardData {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Collect real metrics from global METRICS
        let network = self.collect_network_overview();
        let gaming = self.collect_gaming_overview();
        let health = self.collect_system_health().await;
        let performance = self.collect_performance_overview();
        let activity = self.collect_recent_activity();

        LiveDashboardData {
            network,
            gaming,
            health,
            performance,
            activity,
            timestamp,
        }
    }

    fn collect_network_overview(&self) -> NetworkOverview {
        let metrics = &*METRICS;
        
        let connected_peers = metrics.network.active_connections.load(Ordering::Relaxed);
        let active_sessions = metrics.network.active_connections.load(Ordering::Relaxed);
        let messages_sent = metrics.network.messages_sent.load(Ordering::Relaxed) as u64;
        let messages_received = metrics.network.messages_received.load(Ordering::Relaxed) as u64;
        let bytes_sent = metrics.network.bytes_sent.load(Ordering::Relaxed) as u64;
        let bytes_received = metrics.network.bytes_received.load(Ordering::Relaxed) as u64;
        let connection_errors = metrics.network.connection_errors.load(Ordering::Relaxed) as u64;
        
        let network_quality = match (connected_peers, connection_errors) {
            (0, _) => "Disconnected",
            (1..=2, e) if e > 10 => "Poor",
            (1..=2, _) => "Limited",
            (3..=10, e) if e > 5 => "Fair",
            (3..=10, _) => "Good",
            (_, e) if e > 0 => "Good",
            _ => "Excellent",
        }.to_string();

        NetworkOverview {
            connected_peers,
            active_sessions,
            messages_sent,
            messages_received,
            bytes_sent,
            bytes_received,
            connection_errors,
            network_quality,
        }
    }

    fn collect_gaming_overview(&self) -> GamingOverview {
        let metrics = &*METRICS;
        
        GamingOverview {
            active_games: metrics.gaming.active_games.load(Ordering::Relaxed) as u64,
            total_games: metrics.gaming.total_games.load(Ordering::Relaxed),
            total_bets: metrics.gaming.total_bets.load(Ordering::Relaxed),
            total_volume: metrics.gaming.total_volume.load(Ordering::Relaxed),
            total_payouts: metrics.gaming.total_payouts.load(Ordering::Relaxed),
            dice_rolls: metrics.gaming.dice_rolls.load(Ordering::Relaxed),
            disputes: metrics.gaming.disputes.load(Ordering::Relaxed),
            avg_game_duration: 120.0, // Placeholder - would calculate from real data
        }
    }

    async fn collect_system_health(&self) -> SystemHealth {
        let health_status = self.health_checker.check_health().await;
        
        let mut components = HashMap::new();
        for (name, health) in health_status.health_checks {
            components.insert(name, ComponentStatus {
                status: format!("{:?}", health.status),
                score: health.score,
                message: health.message,
                last_check: health.last_check,
            });
        }

        SystemHealth {
            overall_status: health_status.status,
            overall_score: health_status.overall_score,
            memory_usage_mb: health_status.memory_mb,
            cpu_usage_percent: 0.0, // Would get from system monitor
            uptime_seconds: health_status.uptime_seconds,
            components,
        }
    }

    fn collect_performance_overview(&self) -> PerformanceOverview {
        let metrics = &*METRICS;
        
        PerformanceOverview {
            consensus_proposals_submitted: metrics.consensus.proposals_submitted.load(Ordering::Relaxed),
            consensus_proposals_accepted: metrics.consensus.proposals_accepted.load(Ordering::Relaxed),
            consensus_proposals_rejected: metrics.consensus.proposals_rejected.load(Ordering::Relaxed),
            consensus_forks: metrics.consensus.fork_count.load(Ordering::Relaxed),
            avg_consensus_latency_ms: 50.0, // Placeholder
            throughput_ops_per_sec: 100.0, // Placeholder
        }
    }

    fn collect_recent_activity(&self) -> Vec<ActivityEvent> {
        // In a real implementation, this would read from an event log
        // For now, generate some sample activity based on current metrics
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let connected_peers = METRICS.network.active_connections.load(Ordering::Relaxed);
        let active_games = METRICS.gaming.active_games.load(Ordering::Relaxed);
        
        let mut activities = Vec::new();
        
        if connected_peers > 0 {
            activities.push(ActivityEvent {
                timestamp: timestamp - 30,
                event_type: "network".to_string(),
                description: format!("{} peers connected", connected_peers),
                severity: "info".to_string(),
            });
        }
        
        if active_games > 0 {
            activities.push(ActivityEvent {
                timestamp: timestamp - 15,
                event_type: "gaming".to_string(),
                description: format!("{} active games running", active_games),
                severity: "info".to_string(),
            });
        }
        
        activities.push(ActivityEvent {
            timestamp,
            event_type: "system".to_string(),
            description: "Dashboard data refreshed".to_string(),
            severity: "debug".to_string(),
        });

        activities
    }
}

/// Create warp filters for the live dashboard API
pub fn dashboard_routes() -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let dashboard_service = std::sync::Arc::new(LiveDashboardService::new());

    let dashboard_data = warp::path("api")
        .and(warp::path("dashboard"))
        .and(warp::get())
        .and(with_dashboard_service(dashboard_service.clone()))
        .and_then(handle_dashboard_data);

    let health_check = warp::path("health")
        .and(warp::get())
        .and(with_dashboard_service(dashboard_service.clone()))
        .and_then(handle_health_check);

    let metrics_summary = warp::path("api")
        .and(warp::path("metrics"))
        .and(warp::path("summary"))
        .and(warp::get())
        .and(with_dashboard_service(dashboard_service))
        .and_then(handle_metrics_summary);

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "OPTIONS"]);

    dashboard_data
        .or(health_check)
        .or(metrics_summary)
        .with(cors)
}

fn with_dashboard_service(
    service: std::sync::Arc<LiveDashboardService>,
) -> impl Filter<Extract = (std::sync::Arc<LiveDashboardService>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || service.clone())
}

async fn handle_dashboard_data(
    service: std::sync::Arc<LiveDashboardService>,
) -> Result<impl Reply, warp::Rejection> {
    let data = service.get_dashboard_data().await;
    Ok(warp::reply::json(&data))
}

async fn handle_health_check(
    service: std::sync::Arc<LiveDashboardService>,
) -> Result<impl Reply, warp::Rejection> {
    let health = service.collect_system_health().await;
    Ok(warp::reply::json(&health))
}

async fn handle_metrics_summary(
    _service: std::sync::Arc<LiveDashboardService>,
) -> Result<impl Reply, warp::Rejection> {
    let metrics = &*METRICS;
    
    let summary = serde_json::json!({
        "network": {
            "connected_peers": metrics.network.active_connections.load(Ordering::Relaxed),
            "messages_sent": metrics.network.messages_sent.load(Ordering::Relaxed),
            "messages_received": metrics.network.messages_received.load(Ordering::Relaxed),
        },
        "gaming": {
            "active_games": metrics.gaming.active_games.load(Ordering::Relaxed),
            "total_bets": metrics.gaming.total_bets.load(Ordering::Relaxed),
            "total_volume": metrics.gaming.total_volume.load(Ordering::Relaxed),
        },
        "errors": {
            "network_errors": metrics.errors.network_errors.load(Ordering::Relaxed),
            "consensus_errors": metrics.errors.consensus_errors.load(Ordering::Relaxed),
            "gaming_errors": metrics.errors.gaming_errors.load(Ordering::Relaxed),
        }
    });
    
    Ok(warp::reply::json(&summary))
}

/// Start the live dashboard HTTP server
pub async fn start_dashboard_server() -> Result<(), Box<dyn std::error::Error>> {
    let routes = dashboard_routes();
    
    log::info!("Starting live dashboard server on http://0.0.0.0:8080");
    log::info!("Dashboard API available at: http://0.0.0.0:8080/api/dashboard");
    log::info!("Health check available at: http://0.0.0.0:8080/health");
    log::info!("Metrics summary available at: http://0.0.0.0:8080/api/metrics/summary");
    
    warp::serve(routes)
        .run(([0, 0, 0, 0], 8080))
        .await;
    
    Ok(())
}