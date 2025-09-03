//! HTTP Server for Health Checks and Monitoring Endpoints
//!
//! Provides production-ready HTTP endpoints for:
//! - Health checks (/health)
//! - Readiness probes (/ready)
//! - Metrics export (/metrics)
//! - Status information (/status)

use prometheus::{Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use warp::{Filter, Rejection, Reply};

use crate::database::AsyncDatabasePool;
use crate::mesh::MeshService;
use crate::monitoring::health::HealthCheck;
use crate::monitoring::metrics::PerformanceMetrics;
use crate::Error;

/// HTTP monitoring server configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub bind_address: SocketAddr,
    pub enable_metrics: bool,
    pub enable_health: bool,
    pub enable_ready: bool,
    pub enable_status: bool,
    pub metrics_path: String,
    pub health_path: String,
    pub ready_path: String,
    pub status_path: String,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            bind_address: ([0, 0, 0, 0], 8080).into(),
            enable_metrics: true,
            enable_health: true,
            enable_ready: true,
            enable_status: true,
            metrics_path: "/metrics".to_string(),
            health_path: "/health".to_string(),
            ready_path: "/ready".to_string(),
            status_path: "/status".to_string(),
        }
    }
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub version: String,
    pub checks: Vec<HealthCheckResult>,
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
    pub duration_ms: u64,
}

/// Readiness response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub timestamp: u64,
    pub checks: Vec<ReadinessCheck>,
}

/// Individual readiness check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub ready: bool,
    pub message: Option<String>,
}

/// Status response with detailed system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub timestamp: u64,
    pub network: NetworkStatus,
    pub database: DatabaseStatus,
    pub resources: ResourceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected_peers: usize,
    pub active_connections: usize,
    pub total_messages: u64,
    pub consensus_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub healthy: bool,
    pub connections_active: usize,
    pub connections_total: usize,
    pub transactions_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatus {
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub disk_free_gb: f64,
}

/// HTTP monitoring server
pub struct MonitoringServer {
    config: MonitoringConfig,
    health_check: Arc<HealthCheck>,
    metrics: Arc<PerformanceMetrics>,
    database: Option<Arc<AsyncDatabasePool>>,
    mesh_service: Option<Arc<MeshService>>,
    start_time: Instant,
    ready_state: Arc<RwLock<bool>>,
}

impl MonitoringServer {
    /// Create new monitoring server
    pub fn new(config: MonitoringConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            config,
            health_check: Arc::new(HealthCheck::new(metrics.clone())),
            metrics,
            database: None,
            mesh_service: None,
            start_time: Instant::now(),
            ready_state: Arc::new(RwLock::new(false)),
        }
    }

    /// Set database pool for health checks
    pub fn with_database(mut self, database: Arc<AsyncDatabasePool>) -> Self {
        self.database = Some(database);
        self
    }

    /// Set mesh service for status reporting
    pub fn with_mesh_service(mut self, mesh_service: Arc<MeshService>) -> Self {
        self.mesh_service = Some(mesh_service);
        self
    }

    /// Mark service as ready
    pub async fn set_ready(&self, ready: bool) {
        *self.ready_state.write().await = ready;
    }

    /// Start the HTTP server
    pub async fn start(self: Arc<Self>) -> Result<(), Error> {
        let routes = self.build_routes();

        log::info!(
            "Starting monitoring HTTP server on {}",
            self.config.bind_address
        );

        warp::serve(routes).run(self.config.bind_address).await;

        Ok(())
    }

    /// Build all HTTP routes
    fn build_routes(
        self: &Arc<Self>,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        // Start with ping endpoint
        let ping = warp::path("ping")
            .and(warp::get())
            .map(|| warp::reply::with_status("pong", warp::http::StatusCode::OK));

        // Health endpoint
        let health = warp::path("health")
            .and(warp::get())
            .and(with_server(self.clone()))
            .and_then(handle_health);

        // Readiness endpoint
        let ready = warp::path("ready")
            .and(warp::get())
            .and(with_server(self.clone()))
            .and_then(handle_ready);

        // Metrics endpoint
        let metrics = warp::path("metrics")
            .and(warp::get())
            .and(with_server(self.clone()))
            .and_then(handle_metrics);

        // Status endpoint
        let status = warp::path("status")
            .and(warp::get())
            .and(with_server(self.clone()))
            .and_then(handle_status);

        // Combine all routes
        ping.or(health).or(ready).or(metrics).or(status)
    }
}

/// Warp filter to inject server instance
fn with_server(
    server: Arc<MonitoringServer>,
) -> impl Filter<Extract = (Arc<MonitoringServer>,), Error = Infallible> + Clone {
    warp::any().map(move || server.clone())
}

/// Handle health check endpoint
async fn handle_health(server: Arc<MonitoringServer>) -> Result<impl Reply, Rejection> {
    let start = Instant::now();
    let mut checks = Vec::new();
    let mut overall_healthy = true;

    // Basic health check
    let basic_health = server.health_check.check_health().await;
    checks.push(HealthCheckResult {
        name: "system".to_string(),
        status: basic_health.status.clone(),
        message: None,
        duration_ms: start.elapsed().as_millis() as u64,
    });

    if basic_health.status != "healthy" {
        overall_healthy = false;
    }

    // Database health check
    if let Some(ref db) = server.database {
        let db_start = Instant::now();
        // Check database is healthy by querying stats
        let db_healthy = db.get_stats().await.active_connections < 100;

        checks.push(HealthCheckResult {
            name: "database".to_string(),
            status: if db_healthy { "healthy" } else { "unhealthy" }.to_string(),
            message: None,
            duration_ms: db_start.elapsed().as_millis() as u64,
        });

        if !db_healthy {
            overall_healthy = false;
        }
    }

    // Network health check
    if let Some(ref mesh) = server.mesh_service {
        let net_start = Instant::now();
        let peer_count = mesh.get_connected_peers().await.len();
        let net_healthy = peer_count > 0;

        checks.push(HealthCheckResult {
            name: "network".to_string(),
            status: if net_healthy { "healthy" } else { "degraded" }.to_string(),
            message: Some(format!("{} peers connected", peer_count)),
            duration_ms: net_start.elapsed().as_millis() as u64,
        });

        // Network degradation doesn't make service unhealthy
    }

    let response = HealthResponse {
        status: if overall_healthy {
            "healthy"
        } else {
            "unhealthy"
        }
        .to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        uptime_seconds: server.start_time.elapsed().as_secs(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        checks,
    };

    let status_code = if overall_healthy {
        warp::http::StatusCode::OK
    } else {
        warp::http::StatusCode::SERVICE_UNAVAILABLE
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        status_code,
    ))
}

/// Handle readiness endpoint
async fn handle_ready(server: Arc<MonitoringServer>) -> Result<impl Reply, Rejection> {
    let mut checks = Vec::new();
    let mut overall_ready = *server.ready_state.read().await;

    // Database readiness
    if let Some(ref db) = server.database {
        let stats = db.get_stats().await;
        let db_ready = stats.active_connections < stats.total_connections;
        checks.push(ReadinessCheck {
            name: "database".to_string(),
            ready: db_ready,
            message: None,
        });
        overall_ready = overall_ready && db_ready;
    }

    // Network readiness
    if let Some(ref mesh) = server.mesh_service {
        let net_ready = mesh.get_connected_peers().await.len() > 0;
        checks.push(ReadinessCheck {
            name: "network".to_string(),
            ready: net_ready,
            message: None,
        });
        // Network is optional for readiness
    }

    let response = ReadinessResponse {
        ready: overall_ready,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        checks,
    };

    let status_code = if overall_ready {
        warp::http::StatusCode::OK
    } else {
        warp::http::StatusCode::SERVICE_UNAVAILABLE
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        status_code,
    ))
}

/// Handle metrics endpoint (Prometheus format)
async fn handle_metrics(_server: Arc<MonitoringServer>) -> Result<impl Reply, Rejection> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).map_err(|e| {
        log::error!("Failed to encode metrics: {}", e);
        warp::reject::reject()
    })?;

    Ok(warp::reply::with_header(
        buffer,
        "Content-Type",
        "text/plain; version=0.0.4",
    ))
}

/// Handle status endpoint
async fn handle_status(server: Arc<MonitoringServer>) -> Result<impl Reply, Rejection> {
    // Gather network status
    let network = if let Some(ref mesh) = server.mesh_service {
        let peers = mesh.get_connected_peers().await;
        NetworkStatus {
            connected_peers: peers.len(),
            active_connections: peers.len(),
            total_messages: 0,   // Would need to track this in metrics
            consensus_height: 0, // Would need consensus module reference
        }
    } else {
        NetworkStatus {
            connected_peers: 0,
            active_connections: 0,
            total_messages: 0,
            consensus_height: 0,
        }
    };

    // Gather database status
    let database = if let Some(ref db) = server.database {
        let stats = db.get_stats().await;
        DatabaseStatus {
            healthy: true,
            connections_active: stats.active_connections,
            connections_total: stats.total_connections,
            transactions_total: stats.total_queries,
        }
    } else {
        DatabaseStatus {
            healthy: true,
            connections_active: 0,
            connections_total: 0,
            transactions_total: 0,
        }
    };

    // Gather resource status
    let health = server.health_check.check_health().await;
    let resources = ResourceStatus {
        memory_mb: health.memory_mb,
        cpu_percent: 0.0,  // Would need actual CPU monitoring
        disk_free_gb: 0.0, // Would need disk monitoring
    };

    let response = StatusResponse {
        status: health.status,
        version: health.version,
        uptime_seconds: health.uptime_seconds,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        network,
        database,
        resources,
    };

    Ok(warp::reply::json(&response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitoring_server_creation() {
        let config = MonitoringConfig::default();
        let metrics = Arc::new(PerformanceMetrics::new());
        let server = MonitoringServer::new(config, metrics);

        assert!(!*server.ready_state.read().await);

        server.set_ready(true).await;
        assert!(*server.ready_state.read().await);
    }

    #[tokio::test]
    async fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            timestamp: 1234567890,
            uptime_seconds: 3600,
            version: "0.1.0".to_string(),
            checks: vec![HealthCheckResult {
                name: "test".to_string(),
                status: "healthy".to_string(),
                message: None,
                duration_ms: 10,
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("0.1.0"));
    }
}
