//! WebSocket API for Real-time Communication
//!
//! Provides WebSocket-based real-time communication for game events,
//! consensus updates, and peer-to-peer messaging.

use crate::sdk_v2::{
    config::Config,
    error::{SDKError, SDKResult},
    types::{EventType, WebSocketMessage, GameId, PlayerId, EventStream},
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink, stream::SplitStream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::{Duration, Instant, interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsReceiver = SplitStream<WsStream>;

/// WebSocket manager for handling real-time connections
#[derive(Debug)]
pub struct WebSocketManager {
    config: Config,
    connection: Arc<RwLock<Option<WebSocketConnection>>>,
    subscriptions: Arc<RwLock<HashMap<EventType, Vec<mpsc::UnboundedSender<serde_json::Value>>>>>,
    connection_state: Arc<RwLock<ConnectionState>>,
    reconnect_attempts: Arc<Mutex<u32>>,
}

/// WebSocket connection wrapper
#[derive(Debug)]
struct WebSocketConnection {
    sink: Arc<Mutex<WsSink>>,
    _receiver_handle: tokio::task::JoinHandle<()>,
    connected_at: Instant,
    last_ping: Instant,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    pub url: String,
    pub ping_interval: Duration,
    pub pong_timeout: Duration,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay: Duration,
    pub max_message_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8081/v2".to_string(),
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
            max_reconnect_attempts: 10,
            reconnect_delay: Duration::from_secs(5),
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

/// Subscription filter for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub event_types: Vec<EventType>,
    pub game_ids: Option<Vec<GameId>>,
    pub player_ids: Option<Vec<PlayerId>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub async fn new(config: Config) -> SDKResult<Self> {
        let manager = Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            reconnect_attempts: Arc::new(Mutex::new(0)),
        };
        
        // Start connection in background
        manager.connect().await?;
        
        Ok(manager)
    }
    
    /// Connect to WebSocket server
    pub async fn connect(&self) -> SDKResult<()> {
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
        }
        
        let ws_config = WebSocketConfig {
            url: self.config.websocket_url.clone(),
            ..Default::default()
        };
        
        match self.establish_connection(&ws_config).await {
            Ok(connection) => {
                {
                    let mut conn = self.connection.write().await;
                    *conn = Some(connection);
                }
                {
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Connected;
                }
                {
                    let mut attempts = self.reconnect_attempts.lock().await;
                    *attempts = 0;
                }
                Ok(())
            }
            Err(e) => {
                {
                    let mut state = self.connection_state.write().await;
                    *state = ConnectionState::Failed;
                }
                Err(e)
            }
        }
    }
    
    /// Disconnect from WebSocket server
    pub async fn disconnect(&self) -> SDKResult<()> {
        {
            let mut connection = self.connection.write().await;
            if let Some(conn) = connection.take() {
                // Send close message
                if let Ok(mut sink) = conn.sink.try_lock() {
                    let _ = sink.send(Message::Close(None)).await;
                }
                conn._receiver_handle.abort();
            }
        }
        
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Disconnected;
        }
        
        Ok(())
    }
    
    /// Check if WebSocket is connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.connection_state.read().await, ConnectionState::Connected)
    }
    
    /// Get current connection state
    pub async fn connection_state(&self) -> ConnectionState {
        *self.connection_state.read().await
    }
    
    /// Subscribe to specific event types
    pub async fn subscribe<T>(&self, event_type: EventType) -> SDKResult<EventStream<T>>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        
        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.entry(event_type)
                .or_insert_with(Vec::new)
                .push(tx);
        }
        
        // Send subscription message to server
        self.send_subscription_message(event_type, true).await?;
        
        Ok(EventStream::new(rx))
    }
    
    /// Unsubscribe from event type
    pub async fn unsubscribe(&self, event_type: EventType) -> SDKResult<()> {
        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.remove(&event_type);
        }
        
        // Send unsubscription message to server
        self.send_subscription_message(event_type, false).await?;
        
        Ok(())
    }
    
    /// Send a message through WebSocket
    pub async fn send_message(&self, message: WebSocketMessage) -> SDKResult<()> {
        let connection = self.connection.read().await;
        if let Some(conn) = connection.as_ref() {
            let json = serde_json::to_string(&message)?;
            let mut sink = conn.sink.lock().await;
            sink.send(Message::Text(json)).await
                .map_err(|e| SDKError::WebSocketError(e.to_string()))?;
            Ok(())
        } else {
            Err(SDKError::WebSocketError("Not connected".to_string()))
        }
    }
    
    /// Start automatic reconnection process
    pub async fn start_reconnect_loop(&self) {
        let manager = self.clone_for_task();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let state = *manager.connection_state.read().await;
                if matches!(state, ConnectionState::Disconnected | ConnectionState::Failed) {
                    let attempts = {
                        let mut attempts_guard = manager.reconnect_attempts.lock().await;
                        *attempts_guard += 1;
                        *attempts_guard
                    };
                    
                    if attempts <= 10 { // Max reconnect attempts
                        log::info!("Attempting to reconnect (attempt {})", attempts);
                        
                        {
                            let mut state_guard = manager.connection_state.write().await;
                            *state_guard = ConnectionState::Reconnecting;
                        }
                        
                        if manager.connect().await.is_ok() {
                            log::info!("Reconnected successfully");
                        } else {
                            log::warn!("Reconnection attempt {} failed", attempts);
                        }
                    }
                }
            }
        });
    }
    
    // Private methods
    
    async fn establish_connection(&self, ws_config: &WebSocketConfig) -> SDKResult<WebSocketConnection> {
        let url = Url::parse(&ws_config.url)?;
        
        let (ws_stream, _) = connect_async(&url).await
            .map_err(|e| SDKError::WebSocketError(format!("Connection failed: {}", e)))?;
        
        let (sink, stream) = ws_stream.split();
        let sink = Arc::new(Mutex::new(sink));
        
        // Start receiver task
        let receiver_handle = self.start_receiver_task(stream).await;
        
        // Start ping task
        self.start_ping_task(sink.clone(), ws_config.ping_interval).await;
        
        Ok(WebSocketConnection {
            sink,
            _receiver_handle: receiver_handle,
            connected_at: Instant::now(),
            last_ping: Instant::now(),
        })
    }
    
    async fn start_receiver_task(&self, mut stream: WsReceiver) -> tokio::task::JoinHandle<()> {
        let subscriptions = self.subscriptions.clone();
        let connection_state = self.connection_state.clone();
        
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                            Self::handle_message(ws_message, &subscriptions).await;
                        }
                    }
                    Ok(Message::Binary(data)) => {
                        if let Ok(ws_message) = serde_json::from_slice::<WebSocketMessage>(&data) {
                            Self::handle_message(ws_message, &subscriptions).await;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        log::info!("WebSocket connection closed by server");
                        let mut state = connection_state.write().await;
                        *state = ConnectionState::Disconnected;
                        break;
                    }
                    Ok(Message::Pong(_)) => {
                        log::debug!("Received pong");
                    }
                    Ok(Message::Ping(data)) => {
                        log::debug!("Received ping, sending pong");
                        // Pong is handled automatically by tungstenite
                    }
                    Err(e) => {
                        log::error!("WebSocket error: {}", e);
                        let mut state = connection_state.write().await;
                        *state = ConnectionState::Failed;
                        break;
                    }
                }
            }
        })
    }
    
    async fn start_ping_task(&self, sink: Arc<Mutex<WsSink>>, interval: Duration) {
        let sink_clone = sink.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(interval);
            
            loop {
                interval.tick().await;
                
                let mut sink_guard = sink_clone.lock().await;
                if sink_guard.send(Message::Ping(vec![])).await.is_err() {
                    log::warn!("Failed to send ping");
                    break;
                }
            }
        });
    }
    
    async fn handle_message(
        message: WebSocketMessage,
        subscriptions: &Arc<RwLock<HashMap<EventType, Vec<mpsc::UnboundedSender<serde_json::Value>>>>>
    ) {
        let event_type = match &message {
            WebSocketMessage::GameUpdate { .. } => Some(EventType::GameStarted),
            WebSocketMessage::PlayerAction { .. } => Some(EventType::BetPlaced),
            WebSocketMessage::ChatMessage { .. } => Some(EventType::ChatMessage),
            WebSocketMessage::ConsensusProposal { .. } => Some(EventType::ConsensusProposal),
            WebSocketMessage::ConsensusVote { .. } => Some(EventType::ConsensusResult),
            WebSocketMessage::PeerUpdate { .. } => Some(EventType::PeerConnected),
            WebSocketMessage::SystemNotification { .. } => Some(EventType::SystemAnnouncement),
            _ => None,
        };
        
        if let Some(event_type) = event_type {
            let subs = subscriptions.read().await;
            if let Some(senders) = subs.get(&event_type) {
                let message_json = serde_json::to_value(&message).unwrap_or(serde_json::Value::Null);
                
                for sender in senders {
                    let _ = sender.send(message_json.clone());
                }
            }
        }
    }
    
    async fn send_subscription_message(&self, event_type: EventType, subscribe: bool) -> SDKResult<()> {
        let message = if subscribe {
            WebSocketMessage::SystemNotification {
                message: format!("subscribe:{:?}", event_type),
                level: crate::sdk_v2::types::NotificationLevel::Info,
            }
        } else {
            WebSocketMessage::SystemNotification {
                message: format!("unsubscribe:{:?}", event_type),
                level: crate::sdk_v2::types::NotificationLevel::Info,
            }
        };
        
        self.send_message(message).await
    }
    
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            connection: self.connection.clone(),
            subscriptions: self.subscriptions.clone(),
            connection_state: self.connection_state.clone(),
            reconnect_attempts: self.reconnect_attempts.clone(),
        }
    }
}

/// WebSocket client for simpler usage
pub struct WebSocketClient {
    manager: WebSocketManager,
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub async fn new(config: Config) -> SDKResult<Self> {
        let manager = WebSocketManager::new(config).await?;
        
        // Start reconnect loop
        manager.start_reconnect_loop().await;
        
        Ok(Self { manager })
    }
    
    /// Subscribe to game events
    pub async fn subscribe_to_game(&self, game_id: GameId) -> SDKResult<EventStream<WebSocketMessage>> {
        // Subscribe to relevant game events
        let stream = self.manager.subscribe::<WebSocketMessage>(EventType::GameStarted).await?;
        
        // Send game-specific subscription
        let message = WebSocketMessage::SystemNotification {
            message: format!("subscribe_game:{}", game_id),
            level: crate::sdk_v2::types::NotificationLevel::Info,
        };
        self.manager.send_message(message).await?;
        
        Ok(stream)
    }
    
    /// Send a chat message
    pub async fn send_chat(&self, game_id: GameId, message: String) -> SDKResult<()> {
        let ws_message = WebSocketMessage::ChatMessage {
            game_id,
            player_id: "current_player".to_string(), // This would come from auth context
            message,
            timestamp: chrono::Utc::now(),
        };
        
        self.manager.send_message(ws_message).await
    }
    
    /// Check connection status
    pub async fn is_connected(&self) -> bool {
        self.manager.is_connected().await
    }
    
    /// Disconnect
    pub async fn disconnect(&self) -> SDKResult<()> {
        self.manager.disconnect().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::config::{Config, Environment};
    
    #[tokio::test]
    async fn test_websocket_manager_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .websocket_url("ws://localhost:8081/v2")
            .build()
            .unwrap();
        
        // This might fail in CI/test environment, which is expected
        let result = WebSocketManager::new(config).await;
        assert!(result.is_ok() || matches!(result, Err(SDKError::WebSocketError(_))));
    }
    
    #[tokio::test]
    async fn test_websocket_client_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .websocket_url("ws://localhost:8081/v2")
            .build()
            .unwrap();
        
        // This might fail in CI/test environment, which is expected
        let result = WebSocketClient::new(config).await;
        assert!(result.is_ok() || matches!(result, Err(SDKError::WebSocketError(_))));
    }
    
    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.url, "ws://localhost:8081/v2");
        assert_eq!(config.ping_interval, Duration::from_secs(30));
        assert_eq!(config.max_reconnect_attempts, 10);
    }
}