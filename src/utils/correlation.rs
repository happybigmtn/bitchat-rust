//! Request/Response Correlation ID System
//!
//! This module provides correlation IDs for tracking requests and responses
//! across distributed system boundaries, essential for debugging, monitoring,
//! and tracing in production environments.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{field, Span, Instrument};

/// Unique correlation identifier for request tracing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    /// Create from existing string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }
    
    /// Get the underlying string
    pub fn as_str(&self) -> &str {
        &self.0
    }
    
    /// Convert to string
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Request context with correlation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    /// Primary correlation ID
    pub correlation_id: CorrelationId,
    /// Parent correlation ID (for nested requests)
    pub parent_id: Option<CorrelationId>,
    /// Request trace ID (for distributed tracing)
    pub trace_id: Option<String>,
    /// Request span ID
    pub span_id: Option<String>,
    /// Request timestamp
    pub timestamp: SystemTime,
    /// Request source identifier
    pub source: Option<String>,
    /// Request operation name
    pub operation: Option<String>,
    /// User/session identifier
    pub user_id: Option<String>,
}

impl RequestContext {
    /// Create new request context
    pub fn new() -> Self {
        Self {
            correlation_id: CorrelationId::new(),
            parent_id: None,
            trace_id: None,
            span_id: None,
            timestamp: SystemTime::now(),
            source: None,
            operation: None,
            user_id: None,
        }
    }
    
    /// Create child context from parent
    pub fn child_of(&self, operation: Option<String>) -> Self {
        Self {
            correlation_id: CorrelationId::new(),
            parent_id: Some(self.correlation_id.clone()),
            trace_id: self.trace_id.clone(),
            span_id: None, // New span for child
            timestamp: SystemTime::now(),
            source: self.source.clone(),
            operation,
            user_id: self.user_id.clone(),
        }
    }
    
    /// Set operation name
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = Some(operation);
        self
    }
    
    /// Set source identifier
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }
    
    /// Set user identifier
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    /// Get elapsed time since request start
    pub fn elapsed(&self) -> Duration {
        self.timestamp.elapsed().unwrap_or_default()
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Active request tracking entry
#[derive(Debug, Clone)]
struct RequestEntry {
    context: RequestContext,
    created_at: SystemTime,
    last_activity: SystemTime,
    status: RequestStatus,
}

/// Request status tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestStatus {
    Active,
    Completed { duration_ms: u64 },
    Failed { error: String, duration_ms: u64 },
    Timeout,
}

/// Correlation ID manager for tracking active requests
pub struct CorrelationManager {
    /// Active requests
    active_requests: Arc<RwLock<HashMap<CorrelationId, RequestEntry>>>,
    /// Configuration
    config: CorrelationConfig,
    /// Cleanup task handle
    _cleanup_task: tokio::task::JoinHandle<()>,
}

/// Configuration for correlation management
#[derive(Debug, Clone)]
pub struct CorrelationConfig {
    /// Maximum time to keep completed requests in memory
    pub max_retention_duration: Duration,
    /// Cleanup interval
    pub cleanup_interval: Duration,
    /// Maximum number of active requests to track
    pub max_active_requests: usize,
    /// Enable detailed logging
    pub enable_logging: bool,
}

impl Default for CorrelationConfig {
    fn default() -> Self {
        Self {
            max_retention_duration: Duration::from_secs(3600), // 1 hour
            cleanup_interval: Duration::from_secs(300),        // 5 minutes
            max_active_requests: 10000,
            enable_logging: true,
        }
    }
}

impl CorrelationManager {
    /// Create new correlation manager
    pub fn new(config: CorrelationConfig) -> Self {
        let active_requests = Arc::new(RwLock::new(HashMap::new()));
        
        // Start cleanup task
        let cleanup_requests = Arc::clone(&active_requests);
        let cleanup_config = config.clone();
        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_config.cleanup_interval);
            loop {
                interval.tick().await;
                Self::cleanup_expired_requests(&cleanup_requests, &cleanup_config).await;
            }
        });
        
        Self {
            active_requests,
            config,
            _cleanup_task: cleanup_task,
        }
    }
    
    /// Start tracking a new request
    pub async fn start_request(&self, context: RequestContext) -> crate::error::Result<()> {
        let mut requests = self.active_requests.write().await;
        
        // Check capacity
        if requests.len() >= self.config.max_active_requests {
            return Err(crate::error::Error::ResourceExhausted(
                "Too many active requests being tracked".to_string()
            ));
        }
        
        let entry = RequestEntry {
            context: context.clone(),
            created_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            status: RequestStatus::Active,
        };
        
        requests.insert(context.correlation_id.clone(), entry);
        
        if self.config.enable_logging {
            tracing::info!(
                correlation_id = %context.correlation_id,
                operation = ?context.operation,
                source = ?context.source,
                "Started tracking request"
            );
        }
        
        Ok(())
    }
    
    /// Update request activity
    pub async fn update_activity(&self, correlation_id: &CorrelationId) {
        let mut requests = self.active_requests.write().await;
        if let Some(entry) = requests.get_mut(correlation_id) {
            entry.last_activity = SystemTime::now();
        }
    }
    
    /// Complete a request successfully
    pub async fn complete_request(&self, correlation_id: &CorrelationId) -> crate::error::Result<Duration> {
        let mut requests = self.active_requests.write().await;
        
        if let Some(entry) = requests.get_mut(correlation_id) {
            let duration = entry.created_at.elapsed().unwrap_or_default();
            let duration_ms = duration.as_millis() as u64;
            
            entry.status = RequestStatus::Completed { duration_ms };
            entry.last_activity = SystemTime::now();
            
            if self.config.enable_logging {
                tracing::info!(
                    correlation_id = %correlation_id,
                    duration_ms = duration_ms,
                    operation = ?entry.context.operation,
                    "Request completed successfully"
                );
            }
            
            Ok(duration)
        } else {
            Err(crate::error::Error::NotFound(
                format!("Request {} not found", correlation_id)
            ))
        }
    }
    
    /// Fail a request with error
    pub async fn fail_request(&self, correlation_id: &CorrelationId, error: String) -> crate::error::Result<Duration> {
        let mut requests = self.active_requests.write().await;
        
        if let Some(entry) = requests.get_mut(correlation_id) {
            let duration = entry.created_at.elapsed().unwrap_or_default();
            let duration_ms = duration.as_millis() as u64;
            
            entry.status = RequestStatus::Failed { error: error.clone(), duration_ms };
            entry.last_activity = SystemTime::now();
            
            if self.config.enable_logging {
                tracing::error!(
                    correlation_id = %correlation_id,
                    duration_ms = duration_ms,
                    error = %error,
                    operation = ?entry.context.operation,
                    "Request failed"
                );
            }
            
            Ok(duration)
        } else {
            Err(crate::error::Error::NotFound(
                format!("Request {} not found", correlation_id)
            ))
        }
    }
    
    /// Get request context
    pub async fn get_context(&self, correlation_id: &CorrelationId) -> Option<RequestContext> {
        let requests = self.active_requests.read().await;
        requests.get(correlation_id).map(|entry| entry.context.clone())
    }
    
    /// Get all active requests
    pub async fn get_active_requests(&self) -> Vec<(CorrelationId, RequestContext, RequestStatus)> {
        let requests = self.active_requests.read().await;
        requests.iter()
            .filter(|(_, entry)| matches!(entry.status, RequestStatus::Active))
            .map(|(id, entry)| (id.clone(), entry.context.clone(), entry.status.clone()))
            .collect()
    }
    
    /// Get request statistics
    pub async fn get_statistics(&self) -> CorrelationStatistics {
        let requests = self.active_requests.read().await;
        
        let mut stats = CorrelationStatistics::default();
        stats.total_requests = requests.len();
        
        for entry in requests.values() {
            match &entry.status {
                RequestStatus::Active => stats.active_requests += 1,
                RequestStatus::Completed { duration_ms } => {
                    stats.completed_requests += 1;
                    stats.total_completion_time_ms += duration_ms;
                    if *duration_ms > stats.max_completion_time_ms {
                        stats.max_completion_time_ms = *duration_ms;
                    }
                }
                RequestStatus::Failed { duration_ms, .. } => {
                    stats.failed_requests += 1;
                    stats.total_completion_time_ms += duration_ms;
                }
                RequestStatus::Timeout => stats.timeout_requests += 1,
            }
        }
        
        if stats.completed_requests > 0 {
            stats.avg_completion_time_ms = stats.total_completion_time_ms / (stats.completed_requests as u64);
        }
        
        stats
    }
    
    /// Cleanup expired requests
    async fn cleanup_expired_requests(
        requests: &Arc<RwLock<HashMap<CorrelationId, RequestEntry>>>,
        config: &CorrelationConfig,
    ) {
        let mut requests = requests.write().await;
        let now = SystemTime::now();
        let cutoff = now - config.max_retention_duration;
        
        let expired_keys: Vec<_> = requests.iter()
            .filter(|(_, entry)| entry.last_activity < cutoff)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in expired_keys {
            requests.remove(&key);
        }
        
        if config.enable_logging && !requests.is_empty() {
            tracing::debug!(
                active_requests = requests.len(),
                "Cleaned up expired correlation entries"
            );
        }
    }
}

/// Correlation statistics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CorrelationStatistics {
    pub total_requests: usize,
    pub active_requests: usize,
    pub completed_requests: usize,
    pub failed_requests: usize,
    pub timeout_requests: usize,
    pub avg_completion_time_ms: u64,
    pub max_completion_time_ms: u64,
    pub total_completion_time_ms: u64,
}

/// Tracing span extensions for correlation
pub trait CorrelationSpanExt {
    /// Add correlation ID to span
    fn with_correlation_id(&self, correlation_id: &CorrelationId) -> &Self;
    
    /// Add request context to span
    fn with_request_context(&self, context: &RequestContext) -> &Self;
}

impl CorrelationSpanExt for Span {
    fn with_correlation_id(&self, correlation_id: &CorrelationId) -> &Self {
        self.record("correlation_id", field::display(correlation_id));
        self
    }
    
    fn with_request_context(&self, context: &RequestContext) -> &Self {
        self.record("correlation_id", field::display(&context.correlation_id));
        if let Some(ref parent_id) = context.parent_id {
            self.record("parent_correlation_id", field::display(parent_id));
        }
        if let Some(ref trace_id) = context.trace_id {
            self.record("trace_id", field::display(trace_id));
        }
        if let Some(ref operation) = context.operation {
            self.record("operation", field::display(operation));
        }
        if let Some(ref source) = context.source {
            self.record("source", field::display(source));
        }
        self
    }
}

/// Middleware for automatic correlation ID injection
pub struct CorrelationMiddleware {
    manager: Arc<CorrelationManager>,
}

impl CorrelationMiddleware {
    pub fn new(manager: Arc<CorrelationManager>) -> Self {
        Self { manager }
    }
    
    /// Process request with automatic correlation tracking
    pub async fn process_request<F, Fut, T>(&self, context: RequestContext, handler: F) -> crate::error::Result<T>
    where
        F: FnOnce(RequestContext) -> Fut,
        Fut: std::future::Future<Output = crate::error::Result<T>>,
    {
        // Start tracking
        self.manager.start_request(context.clone()).await?;
        
        // Create tracing span  
        let span = tracing::info_span!("request_processing");
        
        let result = async move {
            let result = handler(context.clone()).await;
            
            // Update tracking based on result
            match &result {
                Ok(_) => {
                    let _ = self.manager.complete_request(&context.correlation_id).await;
                }
                Err(e) => {
                    let _ = self.manager.fail_request(&context.correlation_id, e.to_string()).await;
                }
            }
            
            result
        }.instrument(span.clone()).await;
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[test]
    fn test_correlation_id_generation() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        
        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
        assert!(!id2.as_str().is_empty());
    }
    
    #[test]
    fn test_request_context_creation() {
        let context = RequestContext::new()
            .with_operation("test_op".to_string())
            .with_source("test_source".to_string())
            .with_user_id("user123".to_string());
        
        assert_eq!(context.operation, Some("test_op".to_string()));
        assert_eq!(context.source, Some("test_source".to_string()));
        assert_eq!(context.user_id, Some("user123".to_string()));
        assert!(context.parent_id.is_none());
    }
    
    #[test]
    fn test_child_context_creation() {
        let parent = RequestContext::new()
            .with_operation("parent_op".to_string());
        
        let child = parent.child_of(Some("child_op".to_string()));
        
        assert_eq!(child.parent_id, Some(parent.correlation_id.clone()));
        assert_eq!(child.operation, Some("child_op".to_string()));
        assert_ne!(child.correlation_id, parent.correlation_id);
    }
    
    #[tokio::test]
    async fn test_correlation_manager_tracking() {
        let config = CorrelationConfig::default();
        let manager = CorrelationManager::new(config);
        
        let context = RequestContext::new()
            .with_operation("test_request".to_string());
        
        // Start tracking
        assert!(manager.start_request(context.clone()).await.is_ok());
        
        // Check it's being tracked
        let active = manager.get_active_requests().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].1.correlation_id, context.correlation_id);
        
        // Complete the request
        let duration = manager.complete_request(&context.correlation_id).await.unwrap();
        assert!(duration.as_millis() > 0);
        
        // Check statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.completed_requests, 1);
        assert_eq!(stats.active_requests, 0);
    }
    
    #[tokio::test]
    async fn test_correlation_manager_failure() {
        let config = CorrelationConfig::default();
        let manager = CorrelationManager::new(config);
        
        let context = RequestContext::new();
        
        // Start tracking
        assert!(manager.start_request(context.clone()).await.is_ok());
        
        // Fail the request
        let duration = manager.fail_request(
            &context.correlation_id, 
            "Test error".to_string()
        ).await.unwrap();
        assert!(duration.as_millis() > 0);
        
        // Check statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.active_requests, 0);
    }
    
    #[tokio::test]
    async fn test_correlation_middleware() {
        let config = CorrelationConfig::default();
        let manager = Arc::new(CorrelationManager::new(config));
        let middleware = CorrelationMiddleware::new(Arc::clone(&manager));
        
        let context = RequestContext::new()
            .with_operation("middleware_test".to_string());
        
        // Process successful request
        let result = middleware.process_request(context.clone(), |ctx| async move {
            assert_eq!(ctx.operation, Some("middleware_test".to_string()));
            Ok("success")
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        
        // Check statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.completed_requests, 1);
    }
    
    #[tokio::test] 
    async fn test_correlation_middleware_error() {
        let config = CorrelationConfig::default();
        let manager = Arc::new(CorrelationManager::new(config));
        let middleware = CorrelationMiddleware::new(Arc::clone(&manager));
        
        let context = RequestContext::new();
        
        // Process failing request
        let result = middleware.process_request(context, |_| async move {
            Err(crate::error::Error::InvalidInput("Test error".to_string()))
        }).await;
        
        assert!(result.is_err());
        
        // Check statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.failed_requests, 1);
    }
}