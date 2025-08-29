pub mod metrics;
pub mod health;
pub mod dashboard;
pub mod alerting;
pub mod system;
pub mod http_server;
pub mod real_metrics;
pub mod logging;
pub mod prometheus_server;

pub use dashboard::{NetworkDashboard, NetworkMetrics, HealthCheck};
pub use alerting::{
    AlertingSystem, AlertingConfig, Alert, AlertSeverity, AlertStatus, 
    AlertStatistics, SystemHealth, NotificationConfig, EscalationConfig
};
pub use system::{
    SystemMonitor, SystemMetrics, SystemMonitorError, MetricType, NetworkInterface,
    CachedSystemMonitor, SystemMonitorFactory, global_system_monitor
};
pub use logging::{
    LoggingSystem, LoggingConfig, CorrelationContext, init_production_logging,
    set_correlation_context, get_correlation_context, clear_correlation_context
};
pub use prometheus_server::{PrometheusServer, PrometheusConfig, init_prometheus_server};