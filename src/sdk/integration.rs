//! Integration Helpers for BitCraps SDK

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

/// Integration helper for external services
pub struct IntegrationHelper {
    webhook_manager: WebhookManager,
    event_bridge: EventBridge,
}

impl IntegrationHelper {
    pub fn new() -> Self {
        Self {
            webhook_manager: WebhookManager::new(),
            event_bridge: EventBridge::new(),
        }
    }

    /// Register webhook endpoint
    pub async fn register_webhook(&mut self, webhook: WebhookConfig) -> Result<String, IntegrationError> {
        self.webhook_manager.register(webhook).await
    }

    /// Send webhook notification
    pub async fn send_webhook(&self, webhook_id: &str, payload: serde_json::Value) -> Result<(), IntegrationError> {
        self.webhook_manager.send(webhook_id, payload).await
    }

    /// Bridge events to external systems
    pub async fn bridge_event(&self, event: ExternalEvent) -> Result<(), IntegrationError> {
        self.event_bridge.forward_event(event).await
    }
}

/// Webhook management
pub struct WebhookManager {
    webhooks: HashMap<String, WebhookConfig>,
    event_sender: broadcast::Sender<WebhookEvent>,
}

impl WebhookManager {
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        Self {
            webhooks: HashMap::new(),
            event_sender,
        }
    }

    pub async fn register(&mut self, webhook: WebhookConfig) -> Result<String, IntegrationError> {
        let webhook_id = uuid::Uuid::new_v4().to_string();
        self.webhooks.insert(webhook_id.clone(), webhook);
        Ok(webhook_id)
    }

    pub async fn send(&self, webhook_id: &str, payload: serde_json::Value) -> Result<(), IntegrationError> {
        if let Some(webhook) = self.webhooks.get(webhook_id) {
            // Send HTTP request to webhook URL
            // This would use an HTTP client like reqwest
            println!("Sending webhook to {}: {:?}", webhook.url, payload);
            Ok(())
        } else {
            Err(IntegrationError::WebhookNotFound(webhook_id.to_string()))
        }
    }
}

/// Event bridge for external integrations
pub struct EventBridge {
    event_handlers: Vec<ExternalEventHandler>,
}

impl EventBridge {
    pub fn new() -> Self {
        Self {
            event_handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: ExternalEventHandler) {
        self.event_handlers.push(handler);
    }

    pub async fn forward_event(&self, event: ExternalEvent) -> Result<(), IntegrationError> {
        for handler in &self.event_handlers {
            if let Err(e) = handler.handle_event(&event).await {
                eprintln!("Event handler error: {:?}", e);
            }
        }
        Ok(())
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub secret: Option<String>,
    pub headers: HashMap<String, String>,
    pub retry_config: RetryConfig,
}

/// Retry configuration for webhooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

/// Webhook event
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub webhook_id: String,
    pub payload: serde_json::Value,
    pub timestamp: std::time::SystemTime,
}

/// External event for bridging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEvent {
    pub event_type: String,
    pub source: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

/// External event handler trait
#[async_trait::async_trait]
pub trait ExternalEventHandler: Send + Sync {
    async fn handle_event(&self, event: &ExternalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Simple external event handler implementation
pub struct SimpleExternalEventHandler<F>
where
    F: Fn(&ExternalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    handler_fn: F,
}

impl<F> SimpleExternalEventHandler<F>
where
    F: Fn(&ExternalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    pub fn new(handler_fn: F) -> Self {
        Self { handler_fn }
    }
}

#[async_trait::async_trait]
impl<F> ExternalEventHandler for SimpleExternalEventHandler<F>
where
    F: Fn(&ExternalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    async fn handle_event(&self, event: &ExternalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        (self.handler_fn)(event)
    }
}

#[derive(Debug)]
pub enum IntegrationError {
    WebhookNotFound(String),
    NetworkError(String),
    ConfigurationError(String),
    HandlerError(String),
}

impl std::fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntegrationError::WebhookNotFound(id) => write!(f, "Webhook not found: {}", id),
            IntegrationError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            IntegrationError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            IntegrationError::HandlerError(msg) => write!(f, "Handler error: {}", msg),
        }
    }
}

impl std::error::Error for IntegrationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_helper_creation() {
        let helper = IntegrationHelper::new();
        // Basic test - just verify it can be created
    }

    #[tokio::test]
    async fn test_webhook_registration() {
        let mut helper = IntegrationHelper::new();
        
        let webhook_config = WebhookConfig {
            url: "https://example.com/webhook".to_string(),
            secret: Some("secret".to_string()),
            headers: HashMap::new(),
            retry_config: RetryConfig {
                max_retries: 3,
                initial_delay_ms: 100,
                max_delay_ms: 5000,
            },
        };

        let webhook_id = helper.register_webhook(webhook_config).await.unwrap();
        assert!(!webhook_id.is_empty());
    }

    #[test]
    fn test_external_event_handler() {
        let handler = SimpleExternalEventHandler::new(|event| {
            println!("Handling external event: {:?}", event);
            Ok(())
        });

        let test_event = ExternalEvent {
            event_type: "test".to_string(),
            source: "test_source".to_string(),
            data: serde_json::json!({"test": "data"}),
            timestamp: 123456789,
        };

        // Test would call handler.handle_event(&test_event) in async context
    }
}