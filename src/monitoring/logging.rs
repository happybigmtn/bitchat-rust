//! Structured logging infrastructure with distributed tracing support
//!
//! Features:
//! - Structured JSON logging with tracing
//! - Request correlation IDs
//! - Configurable log levels per module
//! - Log aggregation support
//! - Performance-optimized logging

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{field::Field, field::Visit, Event, Subscriber};
use tracing_subscriber::{
    fmt,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};
use uuid::Uuid;

/// Global logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Global log level (trace, debug, info, warn, error)
    pub level: String,
    /// Module-specific log levels
    pub module_levels: HashMap<String, String>,
    /// Enable JSON output format
    pub json_format: bool,
    /// Enable request correlation
    pub enable_correlation: bool,
    /// Maximum log files to retain
    pub max_log_files: usize,
    /// Maximum log file size in bytes
    pub max_file_size_bytes: usize,
    /// Log file prefix
    pub log_file_prefix: String,
    /// Enable async logging
    pub async_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let mut module_levels = HashMap::new();
        module_levels.insert("bitcraps::consensus".to_string(), "info".to_string());
        module_levels.insert("bitcraps::gaming".to_string(), "info".to_string());
        module_levels.insert("bitcraps::network".to_string(), "debug".to_string());
        module_levels.insert("bitcraps::monitoring".to_string(), "debug".to_string());
        module_levels.insert("btleplug".to_string(), "warn".to_string());
        module_levels.insert("warp".to_string(), "warn".to_string());

        Self {
            level: "info".to_string(),
            module_levels,
            json_format: true,
            enable_correlation: true,
            max_log_files: 10,
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            log_file_prefix: "bitcraps".to_string(),
            async_logging: true,
        }
    }
}

/// Request correlation context
#[derive(Debug, Clone)]
pub struct CorrelationContext {
    /// Unique request ID
    pub request_id: String,
    /// User ID if available
    pub user_id: Option<String>,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Game ID if in gaming context
    pub game_id: Option<String>,
    /// Custom trace context
    pub trace_context: HashMap<String, String>,
}

impl Default for CorrelationContext {
    fn default() -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            user_id: None,
            session_id: None,
            game_id: None,
            trace_context: HashMap::new(),
        }
    }
}

impl CorrelationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_game_id(mut self, game_id: String) -> Self {
        self.game_id = Some(game_id);
        self
    }

    pub fn add_context(mut self, key: String, value: String) -> Self {
        self.trace_context.insert(key, value);
        self
    }
}

// Thread-local correlation storage
thread_local! {
    static CORRELATION_CONTEXT: std::cell::RefCell<Option<CorrelationContext>> = std::cell::RefCell::new(None);
}

/// Set correlation context for current thread
pub fn set_correlation_context(context: CorrelationContext) {
    CORRELATION_CONTEXT.with(|c| {
        *c.borrow_mut() = Some(context);
    });
}

/// Get current correlation context
pub fn get_correlation_context() -> Option<CorrelationContext> {
    CORRELATION_CONTEXT.with(|c| c.borrow().clone())
}

/// Clear correlation context
pub fn clear_correlation_context() {
    CORRELATION_CONTEXT.with(|c| {
        *c.borrow_mut() = None;
    });
}

/// Structured log entry for JSON output
#[derive(Debug, Clone, Serialize)]
pub struct StructuredLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub game_id: Option<String>,
    pub span_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Custom tracing layer for structured logging
pub struct StructuredLoggingLayer;

impl<S> Layer<S> for StructuredLoggingLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = LogFieldVisitor::new();
        event.record(&mut visitor);

        let correlation = get_correlation_context();
        let current_span = ctx.current_span();

        let entry = StructuredLogEntry {
            timestamp: Utc::now(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message.unwrap_or_else(|| "".to_string()),
            fields: visitor.fields,
            request_id: correlation.as_ref().map(|c| c.request_id.clone()),
            user_id: correlation.as_ref().and_then(|c| c.user_id.clone()),
            session_id: correlation.as_ref().and_then(|c| c.session_id.clone()),
            game_id: correlation.as_ref().and_then(|c| c.game_id.clone()),
            span_id: current_span.id().map(|id| format!("{:?}", id)),
            trace_id: current_span.id().map(|id| format!("{:x}", id.into_u64())),
        };

        // Output structured JSON log
        if let Ok(json) = serde_json::to_string(&entry) {
            eprintln!("{}", json);
        }
    }
}

/// Field visitor for extracting log data
struct LogFieldVisitor {
    fields: HashMap<String, serde_json::Value>,
    message: Option<String>,
}

impl LogFieldVisitor {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
            message: None,
        }
    }
}

impl Visit for LogFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let field_name = field.name();
        if field_name == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            self.fields.insert(
                field_name.to_string(),
                serde_json::Value::String(format!("{:?}", value)),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let field_name = field.name();
        if field_name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(
                field_name.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(num) = serde_json::Number::from_f64(value) {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::Number(num));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
}

/// Global logging system
pub struct LoggingSystem {
    config: LoggingConfig,
    log_aggregators: Arc<RwLock<Vec<Box<dyn LogAggregator + Send + Sync>>>>,
}

/// Log aggregation trait for external systems
pub trait LogAggregator: Send + Sync {
    /// Send structured log entry to external system
    fn send_log(&self, entry: &StructuredLogEntry);

    /// Flush any buffered logs
    fn flush(&self);
}

/// Elasticsearch log aggregator
#[derive(Debug)]
pub struct ElasticsearchAggregator {
    endpoint: String,
    index_prefix: String,
    batch_size: usize,
    buffer: Arc<RwLock<Vec<StructuredLogEntry>>>,
}

impl ElasticsearchAggregator {
    pub fn new(endpoint: String, index_prefix: String, batch_size: usize) -> Self {
        Self {
            endpoint,
            index_prefix,
            batch_size,
            buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl LogAggregator for ElasticsearchAggregator {
    fn send_log(&self, entry: &StructuredLogEntry) {
        let mut buffer = self.buffer.write();
        buffer.push(entry.clone());

        if buffer.len() >= self.batch_size {
            // In production, send batch to Elasticsearch
            let batch: Vec<StructuredLogEntry> = buffer.drain(..).collect();
            drop(buffer);

            // Async send (would be implemented with reqwest in production)
            tokio::spawn(async move {
                log::debug!("Would send {} log entries to Elasticsearch", batch.len());
            });
        }
    }

    fn flush(&self) {
        let mut buffer = self.buffer.write();
        if !buffer.is_empty() {
            let batch: Vec<StructuredLogEntry> = buffer.drain(..).collect();
            drop(buffer);

            // Send remaining entries
            tokio::spawn(async move {
                log::debug!("Flushing {} log entries to Elasticsearch", batch.len());
            });
        }
    }
}

/// Fluentd log aggregator
#[derive(Debug)]
pub struct FluentdAggregator {
    host: String,
    port: u16,
    tag: String,
}

impl FluentdAggregator {
    pub fn new(host: String, port: u16, tag: String) -> Self {
        Self { host, port, tag }
    }
}

impl LogAggregator for FluentdAggregator {
    fn send_log(&self, entry: &StructuredLogEntry) {
        // In production, send to Fluentd via TCP/UDP
        log::debug!(
            "Would send log to Fluentd at {}:{} with tag {}",
            self.host,
            self.port,
            self.tag
        );
    }

    fn flush(&self) {
        log::debug!("Flushing Fluentd aggregator");
    }
}

impl LoggingSystem {
    /// Initialize global logging system
    pub fn init(config: LoggingConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut filter = EnvFilter::new(&config.level);

        // Add module-specific levels
        for (module, level) in &config.module_levels {
            filter = filter.add_directive(format!("{}={}", module, level).parse()?);
        }

        if config.json_format {
            // Initialize structured JSON logging
            Registry::default()
                .with(filter)
                .with(StructuredLoggingLayer)
                .init();
        } else {
            // Initialize human-readable logging
            Registry::default()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .init();
        }

        Ok(Self {
            config,
            log_aggregators: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Add log aggregator
    pub fn add_aggregator(&self, aggregator: Box<dyn LogAggregator + Send + Sync>) {
        self.log_aggregators.write().push(aggregator);
    }

    /// Create correlation context for request
    pub fn create_request_context(&self) -> CorrelationContext {
        CorrelationContext::new()
    }

    /// Get current configuration
    pub fn config(&self) -> &LoggingConfig {
        &self.config
    }

    /// Update log level dynamically
    pub fn set_log_level(&mut self, level: String) {
        self.config.level = level;
        // Would reinitialize subscriber in production
        log::info!("Updated global log level to: {}", self.config.level);
    }

    /// Flush all aggregators
    pub fn flush(&self) {
        let aggregators = self.log_aggregators.read();
        for aggregator in aggregators.iter() {
            aggregator.flush();
        }
    }
}

/// Performance logging macros for critical paths
#[macro_export]
macro_rules! log_performance {
    ($operation:expr, $duration:expr) => {
        tracing::info!(
            operation = $operation,
            duration_ms = $duration.as_millis() as u64,
            "Performance metric"
        );
    };
}

#[macro_export]
macro_rules! log_business_event {
    ($event_type:expr, $($field:ident = $value:expr),*) => {
        tracing::info!(
            event_type = $event_type,
            $($field = $value,)*
            "Business event"
        );
    };
}

#[macro_export]
macro_rules! log_security_event {
    ($event:expr, $($field:ident = $value:expr),*) => {
        tracing::warn!(
            security_event = $event,
            $($field = $value,)*
            "Security event detected"
        );
    };
}

/// Initialize logging with production configuration
pub fn init_production_logging() -> Result<LoggingSystem, Box<dyn std::error::Error>> {
    let config = LoggingConfig {
        level: "info".to_string(),
        json_format: true,
        enable_correlation: true,
        async_logging: true,
        ..Default::default()
    };

    let system = LoggingSystem::init(config)?;

    // Add Elasticsearch aggregator for log centralization
    let es_aggregator = ElasticsearchAggregator::new(
        "http://elasticsearch:9200".to_string(),
        "bitcraps-logs".to_string(),
        100,
    );
    system.add_aggregator(Box::new(es_aggregator));

    Ok(system)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_context() {
        let context = CorrelationContext::new()
            .with_user_id("test_user".to_string())
            .with_game_id("game_123".to_string())
            .add_context("custom_field".to_string(), "value".to_string());

        set_correlation_context(context.clone());

        let retrieved = get_correlation_context().unwrap();
        assert_eq!(retrieved.user_id, Some("test_user".to_string()));
        assert_eq!(retrieved.game_id, Some("game_123".to_string()));
        assert_eq!(
            retrieved.trace_context.get("custom_field"),
            Some(&"value".to_string())
        );

        clear_correlation_context();
        assert!(get_correlation_context().is_none());
    }

    #[test]
    fn test_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.json_format);
        assert!(config.enable_correlation);
        assert!(!config.module_levels.is_empty());
    }

    #[test]
    fn test_structured_log_entry_serialization() {
        let mut fields = HashMap::new();
        fields.insert(
            "test_field".to_string(),
            serde_json::Value::String("test_value".to_string()),
        );

        let entry = StructuredLogEntry {
            timestamp: Utc::now(),
            level: "info".to_string(),
            target: "test_target".to_string(),
            message: "Test message".to_string(),
            fields,
            request_id: Some("req_123".to_string()),
            user_id: Some("user_456".to_string()),
            session_id: None,
            game_id: None,
            span_id: None,
            trace_id: None,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("Test message"));
        assert!(json.contains("req_123"));
        assert!(json.contains("user_456"));
    }
}
