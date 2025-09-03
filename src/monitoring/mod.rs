// Lightweight monitoring exports for MVP to avoid compiling heavy dashboards/integration
// When `mvp` is enabled, only `metrics` and no-op record functions are exported.

#[cfg(feature = "mvp")]
pub mod metrics;

// Minimal system monitor stubs for MVP builds
#[cfg(feature = "mvp")]
pub mod system {
    #[derive(Clone, Default)]
    pub struct SystemMetrics {
        pub cpu_usage_percent: f64,
        pub used_memory_bytes: u64,
        pub thread_count: usize,
        pub battery_level: Option<f32>,
        pub battery_charging: Option<bool>,
        pub temperature_celsius: Option<f32>,
        pub thermal_throttling: bool,
    }

    pub struct DummySystemMonitor;

    pub fn global_system_monitor() -> DummySystemMonitor {
        DummySystemMonitor
    }

    impl DummySystemMonitor {
        pub fn collect_metrics(&self) -> Result<SystemMetrics, ()> {
            Ok(SystemMetrics {
                cpu_usage_percent: 0.0,
                used_memory_bytes: 0,
                thread_count: 0,
                battery_level: None,
                battery_charging: None,
                temperature_celsius: None,
                thermal_throttling: false,
            })
        }

        pub fn is_real_monitoring(&self) -> bool {
            false
        }
    }
}

#[cfg(feature = "mvp")]
#[allow(unused_variables)]
pub fn record_network_event(name: &str, detail: Option<&str>) {}

#[cfg(feature = "mvp")]
#[allow(unused_variables)]
pub fn record_game_event(name: &str, detail: &str) {}

#[cfg(feature = "mvp")]
#[allow(unused_variables)]
pub fn record_error(category: &str, msg: &str) {}

// Full monitoring stack when not in MVP feature
#[cfg(not(feature = "mvp"))]
pub mod alerting;
#[cfg(not(feature = "mvp"))]
pub mod dashboard;
#[cfg(not(feature = "mvp"))]
pub mod health;
#[cfg(not(feature = "mvp"))]
pub mod http_server;
#[cfg(not(feature = "mvp"))]
pub mod integration;
#[cfg(not(feature = "mvp"))]
pub mod live_dashboard;
#[cfg(not(feature = "mvp"))]
pub mod logging;
#[cfg(not(feature = "mvp"))]
pub mod metrics;
#[cfg(not(feature = "mvp"))]
pub mod prometheus_server;
#[cfg(not(feature = "mvp"))]
pub mod real_metrics;
#[cfg(not(feature = "mvp"))]
pub mod system;

#[cfg(not(feature = "mvp"))]
pub use alerting::{
    Alert, AlertSeverity, AlertStatistics, AlertStatus, AlertingConfig, AlertingSystem,
    EscalationConfig, NotificationConfig, SystemHealth,
};
#[cfg(not(feature = "mvp"))]
pub use dashboard::{HealthCheck, NetworkDashboard, NetworkMetrics};
#[cfg(not(feature = "mvp"))]
pub use integration::{
    start_metrics_integration, record_game_event, record_network_event, record_error,
    MetricsIntegrationService,
};
#[cfg(not(feature = "mvp"))]
pub use live_dashboard::{
    start_dashboard_server, dashboard_routes, LiveDashboardData, LiveDashboardService,
};
#[cfg(not(feature = "mvp"))]
pub use logging::{
    clear_correlation_context, get_correlation_context, init_production_logging,
    set_correlation_context, CorrelationContext, LoggingConfig, LoggingSystem,
};
#[cfg(not(feature = "mvp"))]
pub use prometheus_server::{init_prometheus_server, PrometheusConfig, PrometheusServer};
#[cfg(not(feature = "mvp"))]
pub use system::{
    global_system_monitor, CachedSystemMonitor, MetricType, NetworkInterface, SystemMetrics,
    SystemMonitor, SystemMonitorError, SystemMonitorFactory,
};
