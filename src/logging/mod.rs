//! Production-grade logging and observability infrastructure
//!
//! Provides structured logging, distributed tracing, and metrics collection
//! for production monitoring and debugging.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

/// Production logger with multiple outputs
pub struct ProductionLogger {
    level: LogLevel,
    outputs: Arc<RwLock<Vec<Box<dyn LogOutput>>>>,
    context: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    trace_context: Arc<RwLock<Option<TraceContext>>>,
}

/// Trace context for distributed tracing
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub baggage: HashMap<String, String>,
}

/// Log output trait for different backends
#[async_trait::async_trait]
pub trait LogOutput: Send + Sync {
    async fn write(&mut self, entry: &LogEntry) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
}

/// Console output for development
pub struct ConsoleOutput {
    use_color: bool,
}

/// File output with rotation
pub struct FileOutput {
    _path: std::path::PathBuf,
    _current_file: Option<std::fs::File>,
    _max_size: u64,
    _max_files: usize,
    _current_size: u64,
}

/// Network output for centralized logging
pub struct NetworkOutput {
    _endpoint: String,
    _buffer: Vec<LogEntry>,
    _batch_size: usize,
    _flush_interval: Duration,
    _last_flush: Instant,
}

/// Metrics collector for observability
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, u64>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    _labels: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

/// Histogram for latency tracking
#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Vec<f64>,
    counts: Vec<u64>,
    sum: f64,
    count: u64,
}

impl ProductionLogger {
    /// Create a new production logger
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            outputs: Arc::new(RwLock::new(Vec::new())),
            context: Arc::new(RwLock::new(HashMap::new())),
            trace_context: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a log output
    pub async fn add_output(&self, output: Box<dyn LogOutput>) {
        self.outputs.write().await.push(output);
    }

    /// Set global context fields
    pub async fn set_context(&self, key: String, value: serde_json::Value) {
        self.context.write().await.insert(key, value);
    }

    /// Start a new trace
    pub async fn start_trace(&self, trace_id: String) -> TraceContext {
        let context = TraceContext {
            trace_id: trace_id.clone(),
            span_id: Self::generate_span_id(),
            parent_span_id: None,
            baggage: HashMap::new(),
        };

        *self.trace_context.write().await = Some(context.clone());
        context
    }

    /// Log a message
    pub async fn log(&self, level: LogLevel, module: &str, message: &str) {
        if level < self.level {
            return;
        }

        let mut fields = HashMap::new();

        // Add global context
        for (k, v) in self.context.read().await.iter() {
            fields.insert(k.clone(), v.clone());
        }

        // Add trace context
        let trace_context = self.trace_context.read().await;
        let (trace_id, span_id) = if let Some(ctx) = trace_context.as_ref() {
            (Some(ctx.trace_id.clone()), Some(ctx.span_id.clone()))
        } else {
            (None, None)
        };

        let entry = LogEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            level,
            module: module.to_string(),
            message: message.to_string(),
            fields,
            trace_id,
            span_id,
        };

        // Write to all outputs
        let mut outputs = self.outputs.write().await;
        for output in outputs.iter_mut() {
            if let Err(e) = output.write(&entry).await {
                eprintln!("Failed to write log: {}", e);
            }
        }
    }

    /// Log with fields
    pub async fn log_with_fields(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        fields: HashMap<String, serde_json::Value>,
    ) {
        if level < self.level {
            return;
        }

        let mut all_fields = fields;

        // Add global context
        for (k, v) in self.context.read().await.iter() {
            all_fields.entry(k.clone()).or_insert_with(|| v.clone());
        }

        // Add trace context
        let trace_context = self.trace_context.read().await;
        let (trace_id, span_id) = if let Some(ctx) = trace_context.as_ref() {
            (Some(ctx.trace_id.clone()), Some(ctx.span_id.clone()))
        } else {
            (None, None)
        };

        let entry = LogEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            level,
            module: module.to_string(),
            message: message.to_string(),
            fields: all_fields,
            trace_id,
            span_id,
        };

        // Write to all outputs
        let mut outputs = self.outputs.write().await;
        for output in outputs.iter_mut() {
            if let Err(e) = output.write(&entry).await {
                eprintln!("Failed to write log: {}", e);
            }
        }
    }

    /// Flush all outputs
    pub async fn flush(&self) -> Result<()> {
        let mut outputs = self.outputs.write().await;
        for output in outputs.iter_mut() {
            output.flush().await?;
        }
        Ok(())
    }

    fn generate_span_id() -> String {
        format!("{:016x}", rand::random::<u64>())
    }
}

#[async_trait::async_trait]
impl LogOutput for ConsoleOutput {
    async fn write(&mut self, entry: &LogEntry) -> Result<()> {
        let level_str = match entry.level {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        };

        let timestamp = SystemTime::UNIX_EPOCH + Duration::from_millis(entry.timestamp);
        let datetime = chrono::DateTime::<chrono::Utc>::from(timestamp);

        if self.use_color {
            let color = match entry.level {
                LogLevel::Trace => "\x1b[90m",
                LogLevel::Debug => "\x1b[36m",
                LogLevel::Info => "\x1b[32m",
                LogLevel::Warn => "\x1b[33m",
                LogLevel::Error => "\x1b[31m",
                LogLevel::Fatal => "\x1b[35m",
            };
            let reset = "\x1b[0m";

            println!(
                "{}{} [{}] {} - {}{} {:?}",
                color,
                datetime.format("%Y-%m-%d %H:%M:%S%.3f"),
                level_str,
                entry.module,
                entry.message,
                reset,
                if entry.fields.is_empty() {
                    String::new()
                } else {
                    format!(" {:?}", entry.fields)
                }
            );
        } else {
            println!(
                "{} [{}] {} - {} {:?}",
                datetime.format("%Y-%m-%d %H:%M:%S%.3f"),
                level_str,
                entry.module,
                entry.message,
                if entry.fields.is_empty() {
                    String::new()
                } else {
                    format!(" {:?}", entry.fields)
                }
            );
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            _labels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Increment a counter
    pub async fn inc_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += value;
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
    }

    /// Record a histogram value
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut histograms = self.histograms.write().await;
        let histogram = histograms.entry(name.to_string()).or_insert_with(|| {
            Histogram::new(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ])
        });
        histogram.record(value);
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        for (name, value) in self.counters.read().await.iter() {
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }

        // Export gauges
        for (name, value) in self.gauges.read().await.iter() {
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, value));
        }

        // Export histograms
        for (name, histogram) in self.histograms.read().await.iter() {
            output.push_str(&format!("# TYPE {} histogram\n", name));
            for (i, bucket) in histogram.buckets.iter().enumerate() {
                output.push_str(&format!(
                    "{}_bucket{{le=\"{}\"}} {}\n",
                    name, bucket, histogram.counts[i]
                ));
            }
            output.push_str(&format!("{}_sum {}\n", name, histogram.sum));
            output.push_str(&format!("{}_count {}\n", name, histogram.count));
        }

        output
    }
}

impl Histogram {
    fn new(buckets: Vec<f64>) -> Self {
        let counts = vec![0; buckets.len()];
        Self {
            buckets,
            counts,
            sum: 0.0,
            count: 0,
        }
    }

    fn record(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;

        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                self.counts[i] += 1;
            }
        }
    }
}

/// Global logger instance
use once_cell::sync::OnceCell;

static LOGGER: OnceCell<Arc<ProductionLogger>> = OnceCell::new();

/// Initialize the global logger
pub fn init_logger(level: LogLevel) -> Arc<ProductionLogger> {
    LOGGER
        .get_or_init(|| Arc::new(ProductionLogger::new(level)))
        .clone()
}

/// Get the global logger
pub fn logger() -> Option<Arc<ProductionLogger>> {
    LOGGER.get().cloned()
}

/// Convenience macros for logging
#[macro_export]
macro_rules! log_trace {
    ($module:expr, $msg:expr) => {
        if let Some(logger) = $crate::logging::logger() {
            tokio::spawn(async move {
                logger
                    .log($crate::logging::LogLevel::Trace, $module, $msg)
                    .await;
            });
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($module:expr, $msg:expr) => {
        if let Some(logger) = $crate::logging::logger() {
            tokio::spawn(async move {
                logger
                    .log($crate::logging::LogLevel::Debug, $module, $msg)
                    .await;
            });
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($module:expr, $msg:expr) => {
        if let Some(logger) = $crate::logging::logger() {
            tokio::spawn(async move {
                logger
                    .log($crate::logging::LogLevel::Info, $module, $msg)
                    .await;
            });
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($module:expr, $msg:expr) => {
        if let Some(logger) = $crate::logging::logger() {
            tokio::spawn(async move {
                logger
                    .log($crate::logging::LogLevel::Warn, $module, $msg)
                    .await;
            });
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($module:expr, $msg:expr) => {
        if let Some(logger) = $crate::logging::logger() {
            tokio::spawn(async move {
                logger
                    .log($crate::logging::LogLevel::Error, $module, $msg)
                    .await;
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging() {
        let logger = ProductionLogger::new(LogLevel::Debug);
        logger
            .add_output(Box::new(ConsoleOutput { use_color: false }))
            .await;

        logger.log(LogLevel::Info, "test", "Test message").await;

        let mut fields = HashMap::new();
        fields.insert("user".to_string(), serde_json::json!("alice"));
        fields.insert("action".to_string(), serde_json::json!("login"));

        logger
            .log_with_fields(LogLevel::Info, "auth", "User logged in", fields)
            .await;
    }

    #[tokio::test]
    async fn test_metrics() {
        let metrics = MetricsCollector::new();

        metrics.inc_counter("requests_total", 1).await;
        metrics.set_gauge("connections_active", 42.0).await;
        metrics.record_histogram("request_duration", 0.123).await;

        let prometheus = metrics.export_prometheus().await;
        assert!(prometheus.contains("requests_total"));
        assert!(prometheus.contains("connections_active"));
        assert!(prometheus.contains("request_duration"));
    }
}
