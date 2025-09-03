//! Plugin Communication System for BitCraps
//!
//! This module handles inter-plugin communication, message routing,
//! and event distribution across the plugin ecosystem.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock, mpsc};
use tokio::time::timeout;
use uuid::Uuid;
use tracing::{debug, warn, error, info};

use super::core::{PluginResult, PluginError};

/// Plugin communication coordinator
pub struct PluginCommunicator {
    config: Config,
    channels: Arc<RwLock<HashMap<String, PluginChannel>>>,
    message_bus: Arc<MessageBus>,
    stats: Arc<CommunicationStats>,
}

impl PluginCommunicator {
    /// Create new plugin communicator
    pub fn new(config: Config) -> PluginResult<Self> {
        let message_bus = Arc::new(MessageBus::new(config.message_buffer_size));

        Ok(Self {
            config,
            channels: Arc::new(RwLock::new(HashMap::new())),
            message_bus,
            stats: Arc::new(CommunicationStats::new()),
        })
    }

    /// Setup communication channels for a plugin
    pub async fn setup_plugin_channels(&self, plugin_id: &str) -> PluginResult<()> {
        let mut channels = self.channels.write().await;

        if channels.contains_key(plugin_id) {
            return Err(PluginError::CommunicationError(
                format!("Channels already exist for plugin: {}", plugin_id)
            ));
        }

        let (sender, receiver) = mpsc::channel(self.config.plugin_buffer_size);
        let (broadcast_sender, _) = broadcast::channel(self.config.event_buffer_size);

        let channel = PluginChannel {
            plugin_id: plugin_id.to_string(),
            created_at: SystemTime::now(),
            sender,
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            broadcast_sender: Arc::new(broadcast_sender),
            message_count: AtomicU64::new(0),
            last_activity: Arc::new(RwLock::new(SystemTime::now())),
        };

        channels.insert(plugin_id.to_string(), channel);
        
        // Register with message bus
        self.message_bus.register_plugin(plugin_id).await?;

        info!("Setup communication channels for plugin: {}", plugin_id);
        Ok(())
    }

    /// Cleanup communication channels for a plugin
    pub async fn cleanup_plugin_channels(&self, plugin_id: &str) -> PluginResult<()> {
        let mut channels = self.channels.write().await;
        
        if let Some(_channel) = channels.remove(plugin_id) {
            // Unregister from message bus
            self.message_bus.unregister_plugin(plugin_id).await?;
            
            info!("Cleaned up communication channels for plugin: {}", plugin_id);
        }

        Ok(())
    }

    /// Send message between plugins
    pub async fn send_message(
        &self,
        from_plugin: &str,
        to_plugin: &str,
        message: PluginMessage,
    ) -> PluginResult<()> {
        let channels = self.channels.read().await;
        
        // Validate sender exists
        let from_channel = channels.get(from_plugin)
            .ok_or_else(|| PluginError::CommunicationError(
                format!("Sender plugin not found: {}", from_plugin)
            ))?;

        // Validate receiver exists  
        let to_channel = channels.get(to_plugin)
            .ok_or_else(|| PluginError::CommunicationError(
                format!("Receiver plugin not found: {}", to_plugin)
            ))?;

        // Create envelope
        let envelope = MessageEnvelope {
            id: Uuid::new_v4().to_string(),
            from_plugin: from_plugin.to_string(),
            to_plugin: to_plugin.to_string(),
            message,
            timestamp: SystemTime::now(),
            priority: MessagePriority::Normal,
        };

        // Send message with timeout
        let send_result = timeout(
            Duration::from_millis(self.config.message_timeout_ms),
            to_channel.sender.send(envelope.clone())
        ).await;

        match send_result {
            Ok(Ok(())) => {
                // Update statistics
                from_channel.message_count.fetch_add(1, Ordering::Relaxed);
                *from_channel.last_activity.write().await = SystemTime::now();
                
                self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
                
                debug!("Sent message from {} to {}: {:?}", from_plugin, to_plugin, envelope.message);
                Ok(())
            }
            Ok(Err(_)) => {
                self.stats.messages_dropped.fetch_add(1, Ordering::Relaxed);
                Err(PluginError::CommunicationError(
                    format!("Channel full for plugin: {}", to_plugin)
                ))
            }
            Err(_) => {
                self.stats.messages_timeout.fetch_add(1, Ordering::Relaxed);
                Err(PluginError::CommunicationError(
                    "Message send timeout".to_string()
                ))
            }
        }
    }

    /// Broadcast message to all plugins
    pub async fn broadcast_message(
        &self,
        from_plugin: &str,
        message: PluginMessage,
    ) -> PluginResult<usize> {
        let channels = self.channels.read().await;
        let mut sent_count = 0;

        let envelope = MessageEnvelope {
            id: Uuid::new_v4().to_string(),
            from_plugin: from_plugin.to_string(),
            to_plugin: "broadcast".to_string(),
            message,
            timestamp: SystemTime::now(),
            priority: MessagePriority::Normal,
        };

        for (plugin_id, channel) in channels.iter() {
            if plugin_id == from_plugin {
                continue; // Don't send to self
            }

            if let Err(e) = channel.broadcast_sender.send(envelope.clone()) {
                debug!("Failed to broadcast to {}: {:?}", plugin_id, e);
            } else {
                sent_count += 1;
            }
        }

        self.stats.messages_broadcast.fetch_add(1, Ordering::Relaxed);
        info!("Broadcast message from {} to {} plugins", from_plugin, sent_count);
        
        Ok(sent_count)
    }

    /// Receive messages for a plugin
    pub async fn receive_message(&self, plugin_id: &str) -> PluginResult<Option<MessageEnvelope>> {
        let channels = self.channels.read().await;
        let channel = channels.get(plugin_id)
            .ok_or_else(|| PluginError::CommunicationError(
                format!("No channel for plugin: {}", plugin_id)
            ))?;

        let mut receiver = channel.receiver.lock().await;
        
        match timeout(
            Duration::from_millis(self.config.receive_timeout_ms),
            receiver.recv()
        ).await {
            Ok(Some(envelope)) => {
                *channel.last_activity.write().await = SystemTime::now();
                self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
                Ok(Some(envelope))
            }
            Ok(None) => {
                // Channel closed
                Ok(None)
            }
            Err(_) => {
                // Timeout - no message available
                Ok(None)
            }
        }
    }

    /// Subscribe to broadcast messages
    pub async fn subscribe_broadcasts(&self, plugin_id: &str) -> PluginResult<broadcast::Receiver<MessageEnvelope>> {
        let channels = self.channels.read().await;
        let channel = channels.get(plugin_id)
            .ok_or_else(|| PluginError::CommunicationError(
                format!("No channel for plugin: {}", plugin_id)
            ))?;

        Ok(channel.broadcast_sender.subscribe())
    }

    /// Send system event to plugins
    pub async fn send_system_event(&self, event: SystemEvent) -> PluginResult<()> {
        self.message_bus.broadcast_system_event(event).await
    }

    /// Get message count for plugin
    pub async fn get_message_count(&self) -> u64 {
        self.stats.messages_sent.load(Ordering::Relaxed) +
        self.stats.messages_received.load(Ordering::Relaxed) +
        self.stats.messages_broadcast.load(Ordering::Relaxed)
    }

    /// Cleanup stale channels
    pub async fn cleanup_stale_channels(&self) {
        let mut channels = self.channels.write().await;
        let stale_threshold = Duration::from_secs(self.config.channel_stale_timeout_sec);
        let now = SystemTime::now();
        
        let stale_plugins: Vec<String> = channels
            .iter()
            .filter_map(|(plugin_id, channel)| {
                let last_activity = *channel.last_activity.blocking_read();
                if now.duration_since(last_activity).unwrap_or(Duration::MAX) > stale_threshold {
                    Some(plugin_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for plugin_id in stale_plugins {
            channels.remove(&plugin_id);
            warn!("Cleaned up stale channel for plugin: {}", plugin_id);
        }
    }

    /// Get communication statistics
    pub async fn get_statistics(&self) -> CommunicationStatistics {
        let channels = self.channels.read().await;
        
        CommunicationStatistics {
            active_channels: channels.len(),
            messages_sent: self.stats.messages_sent.load(Ordering::Relaxed),
            messages_received: self.stats.messages_received.load(Ordering::Relaxed),
            messages_broadcast: self.stats.messages_broadcast.load(Ordering::Relaxed),
            messages_dropped: self.stats.messages_dropped.load(Ordering::Relaxed),
            messages_timeout: self.stats.messages_timeout.load(Ordering::Relaxed),
        }
    }
}

/// Message bus for system-wide communication
pub struct MessageBus {
    plugins: Arc<RwLock<HashMap<String, PluginMessageHandler>>>,
    system_event_sender: broadcast::Sender<SystemEvent>,
    buffer_size: usize,
}

impl MessageBus {
    /// Create new message bus
    fn new(buffer_size: usize) -> Self {
        let (system_event_sender, _) = broadcast::channel(buffer_size);
        
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            system_event_sender,
            buffer_size,
        }
    }

    /// Register plugin with message bus
    async fn register_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        let mut plugins = self.plugins.write().await;
        
        let handler = PluginMessageHandler {
            plugin_id: plugin_id.to_string(),
            system_event_receiver: self.system_event_sender.subscribe(),
        };
        
        plugins.insert(plugin_id.to_string(), handler);
        Ok(())
    }

    /// Unregister plugin from message bus
    async fn unregister_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        let mut plugins = self.plugins.write().await;
        plugins.remove(plugin_id);
        Ok(())
    }

    /// Broadcast system event to all plugins
    async fn broadcast_system_event(&self, event: SystemEvent) -> PluginResult<()> {
        if let Err(e) = self.system_event_sender.send(event) {
            debug!("No system event subscribers: {:?}", e);
        }
        Ok(())
    }
}

/// Plugin communication channel
struct PluginChannel {
    plugin_id: String,
    created_at: SystemTime,
    sender: mpsc::Sender<MessageEnvelope>,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<MessageEnvelope>>>,
    broadcast_sender: Arc<broadcast::Sender<MessageEnvelope>>,
    message_count: AtomicU64,
    last_activity: Arc<RwLock<SystemTime>>,
}

/// Plugin message handler
struct PluginMessageHandler {
    plugin_id: String,
    system_event_receiver: broadcast::Receiver<SystemEvent>,
}

/// Message envelope for plugin communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub id: String,
    pub from_plugin: String,
    pub to_plugin: String,
    pub message: PluginMessage,
    pub timestamp: SystemTime,
    pub priority: MessagePriority,
}

/// Plugin message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginMessage {
    /// Game state synchronization
    GameStateSync {
        session_id: String,
        state_data: serde_json::Value,
    },
    /// Player action notification
    PlayerAction {
        session_id: String,
        player_id: String,
        action: crate::gaming::GameAction,
    },
    /// Plugin lifecycle event
    LifecycleEvent {
        event_type: LifecycleEventType,
        data: serde_json::Value,
    },
    /// Custom plugin message
    Custom {
        message_type: String,
        data: serde_json::Value,
    },
    /// Request/response pattern
    Request {
        request_id: String,
        method: String,
        params: serde_json::Value,
    },
    Response {
        request_id: String,
        result: Result<serde_json::Value, String>,
    },
    /// Error notification
    Error {
        error_type: String,
        message: String,
        context: serde_json::Value,
    },
}

/// Message types for routing
#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Direct,
    Broadcast,
    SystemEvent,
    Request,
    Response,
}

/// Message priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Plugin lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LifecycleEventType {
    PluginStarted,
    PluginStopped,
    PluginError,
    ConfigurationChanged,
    CapabilityGranted,
    CapabilityRevoked,
}

/// System events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    Startup,
    Shutdown,
    MaintenanceMode(bool),
    NetworkStatusChanged(NetworkStatus),
    SecurityAlert(String),
    ResourceAlert(ResourceAlert),
}

/// Network status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkStatus {
    Online,
    Offline,
    Limited,
    Reconnecting,
}

/// Resource alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAlert {
    pub resource_type: String,
    pub usage_percent: f32,
    pub threshold_percent: f32,
    pub message: String,
}

/// Communication configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub message_buffer_size: usize,
    pub plugin_buffer_size: usize,
    pub event_buffer_size: usize,
    pub message_timeout_ms: u64,
    pub receive_timeout_ms: u64,
    pub channel_stale_timeout_sec: u64,
    pub enable_message_encryption: bool,
    pub max_message_size_kb: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            message_buffer_size: 10000,
            plugin_buffer_size: 1000,
            event_buffer_size: 1000,
            message_timeout_ms: 5000,
            receive_timeout_ms: 1000,
            channel_stale_timeout_sec: 300, // 5 minutes
            enable_message_encryption: false,
            max_message_size_kb: 1024, // 1MB
        }
    }
}

/// Communication statistics
struct CommunicationStats {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    messages_broadcast: AtomicU64,
    messages_dropped: AtomicU64,
    messages_timeout: AtomicU64,
}

impl CommunicationStats {
    fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            messages_broadcast: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
            messages_timeout: AtomicU64::new(0),
        }
    }
}

/// Communication statistics snapshot
#[derive(Debug, Clone)]
pub struct CommunicationStatistics {
    pub active_channels: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_broadcast: u64,
    pub messages_dropped: u64,
    pub messages_timeout: u64,
}

/// Communication error types
#[derive(Debug, thiserror::Error)]
pub enum CommunicationError {
    #[error("Channel not found for plugin: {0}")]
    ChannelNotFound(String),
    
    #[error("Message send failed: {0}")]
    SendFailed(String),
    
    #[error("Message receive timeout")]
    ReceiveTimeout,
    
    #[error("Message too large: {0} KB > {1} KB")]
    MessageTooLarge(usize, usize),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Channel closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_communicator_creation() {
        let config = Config::default();
        let communicator = PluginCommunicator::new(config).unwrap();
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.active_channels, 0);
    }

    #[tokio::test]
    async fn test_channel_setup() {
        let config = Config::default();
        let communicator = PluginCommunicator::new(config).unwrap();
        
        communicator.setup_plugin_channels("test-plugin").await.unwrap();
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.active_channels, 1);
        
        communicator.cleanup_plugin_channels("test-plugin").await.unwrap();
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.active_channels, 0);
    }

    #[tokio::test]
    async fn test_message_sending() {
        let config = Config::default();
        let communicator = PluginCommunicator::new(config).unwrap();
        
        communicator.setup_plugin_channels("plugin-a").await.unwrap();
        communicator.setup_plugin_channels("plugin-b").await.unwrap();
        
        let message = PluginMessage::Custom {
            message_type: "test".to_string(),
            data: serde_json::json!({"test": "data"}),
        };
        
        communicator.send_message("plugin-a", "plugin-b", message).await.unwrap();
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.messages_sent, 1);
        
        // Receive message
        let received = communicator.receive_message("plugin-b").await.unwrap();
        assert!(received.is_some());
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.messages_received, 1);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let config = Config::default();
        let communicator = PluginCommunicator::new(config).unwrap();
        
        communicator.setup_plugin_channels("plugin-a").await.unwrap();
        communicator.setup_plugin_channels("plugin-b").await.unwrap();
        communicator.setup_plugin_channels("plugin-c").await.unwrap();
        
        let message = PluginMessage::Custom {
            message_type: "broadcast".to_string(),
            data: serde_json::json!({"broadcast": "data"}),
        };
        
        let sent_count = communicator.broadcast_message("plugin-a", message).await.unwrap();
        assert_eq!(sent_count, 2); // Should send to plugin-b and plugin-c, not plugin-a
        
        let stats = communicator.get_statistics().await;
        assert_eq!(stats.messages_broadcast, 1);
    }
}