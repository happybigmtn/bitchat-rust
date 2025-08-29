# BitCraps Monitoring & Observability System

This directory contains comprehensive monitoring and observability infrastructure for BitCraps production deployment.

## Components

### 1. Metrics System (`src/monitoring/metrics.rs`)
- Comprehensive metric collection for all system components
- Real-time system monitoring integration
- Performance, resource, and business metrics
- Prometheus-compatible export format

### 2. Prometheus Server (`src/monitoring/prometheus_server.rs`)
- Dedicated Prometheus metrics endpoint on port 9090
- High-performance metric serialization
- Custom metric labels and business metrics
- Kubernetes service discovery compatible

### 3. Structured Logging (`src/monitoring/logging.rs`)
- JSON structured logging with tracing
- Request correlation IDs
- Configurable log levels per module
- Log aggregation support (Elasticsearch, Fluentd)

### 4. Health Monitoring (`src/monitoring/health.rs`)
- HTTP health endpoints for Kubernetes probes
- Service dependency health checks
- Readiness and liveness probe support

### 5. Alerting System (`src/monitoring/alerting.rs`)
- Real-time threat and anomaly detection
- Multi-channel notifications:
  - PagerDuty integration
  - Slack/Teams webhooks
  - Email notifications (SMTP)
  - SMS notifications (Twilio)
  - Custom webhooks
- Alert aggregation and deduplication
- Escalation procedures

## Dashboard Configurations

### 1. System Overview (`dashboards/bitcraps-system-overview.json`)
- CPU and memory usage monitoring
- Network connection and throughput metrics
- Consensus latency and proposal rates
- System health indicators

### 2. Gaming Metrics (`dashboards/bitcraps-gaming-metrics.json`)
- Total games played and active games
- Betting volume and payout tracking
- Game duration distribution
- Gaming error rates and disputes

### 3. Mobile Performance (`dashboards/bitcraps-mobile-performance.json`)
- Battery level and charging status
- Device temperature monitoring
- Thermal throttling detection
- Cache performance metrics

## Deployment Guide

### Prometheus Integration

1. **Start Prometheus Server:**
   ```bash
   # The application automatically starts Prometheus server on :9090
   cargo run --bin prometheus_server
   ```

2. **Configure Prometheus to scrape BitCraps metrics:**
   ```yaml
   # prometheus.yml
   scrape_configs:
     - job_name: 'bitcraps'
       static_configs:
         - targets: ['localhost:9090']
       scrape_interval: 15s
   ```

### Grafana Dashboard Import

1. **Import Dashboard Files:**
   ```bash
   # Import via Grafana UI or API
   curl -X POST \
     http://grafana:3000/api/dashboards/db \
     -H 'Content-Type: application/json' \
     -d @dashboards/bitcraps-system-overview.json
   ```

2. **Configure Data Source:**
   ```json
   {
     "name": "BitCraps Prometheus",
     "type": "prometheus", 
     "url": "http://prometheus:9090",
     "access": "proxy"
   }
   ```

### Alerting Configuration

1. **Initialize Alerting System:**
   ```rust
   use bitcraps::monitoring::alerting::{AlertingSystem, AlertingConfig};
   use bitcraps::monitoring::alerting::enhanced_notifications::*;

   let config = AlertingConfig::default();
   let mut alerting = AlertingSystem::new(config);
   
   // Add PagerDuty integration
   let pagerduty = PagerDutyNotifier::new("your-integration-key".to_string());
   
   // Add Slack integration  
   let slack = SlackNotifier::new(
       "https://hooks.slack.com/services/...".to_string(),
       "#alerts".to_string()
   );
   
   alerting.start().await?;
   ```

2. **Custom Alert Rules:**
   ```rust
   let custom_rule = AlertRule {
       name: "High Game Error Rate".to_string(),
       description: "Gaming errors above 5%".to_string(),
       metric_name: "gaming_error_rate".to_string(),
       condition: AlertCondition::GreaterThan(0.05),
       severity: AlertSeverity::High,
       category: "gaming".to_string(),
       evaluation_interval: Duration::from_secs(60),
       tags: vec!["gaming".to_string(), "errors".to_string()],
   };
   ```

### Logging Configuration

1. **Initialize Structured Logging:**
   ```rust
   use bitcraps::monitoring::logging::{init_production_logging, set_correlation_context, CorrelationContext};
   
   let logging = init_production_logging()?;
   
   // Set correlation context for request tracking
   let context = CorrelationContext::new()
       .with_user_id("user_123".to_string())
       .with_game_id("game_456".to_string());
   set_correlation_context(context);
   ```

2. **Use Logging Macros:**
   ```rust
   use bitcraps::{log_performance, log_business_event, log_security_event};
   
   log_performance!("consensus_decision", duration);
   log_business_event!("game_completed", game_id = "123", winner = "alice");
   log_security_event!("invalid_signature", user_id = "bob", ip = "192.168.1.1");
   ```

## Kubernetes Deployment

### Service Monitor
```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: bitcraps-metrics
spec:
  selector:
    matchLabels:
      app: bitcraps
  endpoints:
  - port: metrics
    interval: 15s
    path: /metrics
```

### Health Checks
```yaml
apiVersion: v1
kind: Service
metadata:
  name: bitcraps-health
spec:
  ports:
  - name: health
    port: 8080
    targetPort: 8080
  selector:
    app: bitcraps
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: bitcraps
spec:
  template:
    spec:
      containers:
      - name: bitcraps
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

## Metric Reference

### System Metrics
- `bitcraps_cpu_usage_percent` - CPU usage percentage
- `bitcraps_memory_usage_bytes` - Memory usage in bytes
- `bitcraps_thread_count` - Number of active threads
- `bitcraps_uptime_seconds` - System uptime

### Network Metrics  
- `bitcraps_network_messages_sent_total` - Total messages sent
- `bitcraps_network_messages_received_total` - Total messages received
- `bitcraps_network_active_connections` - Active network connections
- `bitcraps_network_latency_seconds` - Network latency distribution

### Consensus Metrics
- `bitcraps_consensus_proposals_accepted_total` - Accepted proposals
- `bitcraps_consensus_proposals_rejected_total` - Rejected proposals 
- `bitcraps_consensus_latency_seconds` - Consensus decision latency
- `bitcraps_consensus_forks_total` - Total consensus forks

### Gaming Metrics
- `bitcraps_games_total` - Total games played
- `bitcraps_games_active` - Currently active games
- `bitcraps_bets_total` - Total bets placed
- `bitcraps_betting_volume_total` - Total betting volume
- `bitcraps_payouts_total` - Total payouts
- `bitcraps_game_disputes_total` - Total disputes

### Mobile Metrics
- `bitcraps_battery_level_percent` - Battery level percentage
- `bitcraps_battery_charging` - Battery charging status
- `bitcraps_device_temperature_celsius` - Device temperature
- `bitcraps_thermal_throttling` - Thermal throttling status

### Error Metrics
- `bitcraps_errors_total{category,severity}` - Categorized error counts

## Alert Rules

The system includes predefined alert rules for:
- High CPU usage (>85%)
- High memory usage (>90%) 
- Network connectivity issues
- Consensus failures
- Gaming system errors
- Battery and thermal issues (mobile)
- Security incidents

## Troubleshooting

### Common Issues

1. **Metrics not appearing in Prometheus:**
   - Check that the BitCraps application is running
   - Verify Prometheus can reach port 9090
   - Check Prometheus logs for scraping errors

2. **Dashboards showing no data:**
   - Verify Prometheus data source configuration
   - Check metric names match dashboard queries
   - Ensure time range includes data

3. **Alerts not firing:**
   - Check alerting rule configuration
   - Verify notification channel setup
   - Review alert rule evaluation logs

4. **High memory usage from monitoring:**
   - Reduce metric collection frequency
   - Limit metric retention period
   - Use metric sampling for high-cardinality metrics

## Performance Considerations

- Metric collection runs every 15 seconds by default
- Log aggregation batches entries to reduce network overhead
- Alert deduplication prevents notification spam
- Dashboard queries are optimized for performance
- Mobile metrics adapt to device capabilities

## Security

- All sensitive data is excluded from metrics and logs
- Correlation IDs use cryptographically secure random generation
- Alert notifications can be encrypted in transit
- Log aggregation supports TLS connections
- Access to monitoring endpoints can be restricted by network policies