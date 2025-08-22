pub struct HealthCheck {
    start_time: Instant,
    metrics: Arc<PerformanceMetrics>,
}

impl HealthCheck {
    pub fn check_health(&self) -> HealthStatus {
        let uptime = self.start_time.elapsed();
        let memory_usage = get_memory_usage();
        let active_peers = self.metrics.active_connections.load(Ordering::Relaxed);
        
        HealthStatus {
            status: if memory_usage < 1024 * 1024 * 1024 { // 1GB limit
                "healthy"
            } else {
                "degraded"
            }.to_string(),
            uptime_seconds: uptime.as_secs(),
            memory_mb: memory_usage / 1024 / 1024,
            active_peers,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}