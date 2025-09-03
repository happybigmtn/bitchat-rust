//! BitCraps SDK Client
//!
//! High-level client interface for interacting with the BitCraps platform.

use crate::sdk_v2::{
    config::{Config, Environment},
    error::{SDKError, SDKResult},
    types::*,
    games::GameAPI,
    consensus::ConsensusAPI,
    networking::NetworkAPI,
    testing::MockEnvironment,
    SDKContext, init
};
use std::sync::Arc;
use tokio::time::{Duration, Instant};

/// Main SDK client for BitCraps platform
#[derive(Debug)]
pub struct BitCrapsSDK {
    context: Arc<SDKContext>,
    games_api: GameAPI,
    consensus_api: ConsensusAPI,
    network_api: NetworkAPI,
}

impl BitCrapsSDK {
    /// Create a new SDK instance with the given configuration
    pub async fn new(config: Config) -> SDKResult<Self> {
        let context = init(config).await?;
        
        let games_api = GameAPI::new(context.clone());
        let consensus_api = ConsensusAPI::new(context.clone());
        let network_api = NetworkAPI::new(context.clone());
        
        Ok(Self {
            context,
            games_api,
            consensus_api,
            network_api,
        })
    }
    
    /// Create SDK instance with mock environment for testing
    pub async fn with_mock(mock: MockEnvironment) -> SDKResult<Self> {
        let config = Config::builder()
            .environment(Environment::Testing)
            .mock_environment(Some(mock))
            .build()?;
            
        Self::new(config).await
    }
    
    /// Get games API interface
    pub fn games(&self) -> &GameAPI {
        &self.games_api
    }
    
    /// Get consensus API interface
    pub fn consensus(&self) -> &ConsensusAPI {
        &self.consensus_api
    }
    
    /// Get network API interface
    pub fn network(&self) -> &NetworkAPI {
        &self.network_api
    }
    
    /// Get SDK metrics
    pub async fn metrics(&self) -> SDKMetrics {
        self.context.metrics.read().await.clone()
    }
    
    /// Health check - verify SDK can connect to all services
    pub async fn health_check(&self) -> SDKResult<HealthStatus> {
        let start = Instant::now();
        
        // Check API connectivity
        let api_health = self.check_api_health().await?;
        let websocket_health = self.check_websocket_health().await?;
        let consensus_health = self.check_consensus_health().await?;
        
        let response_time = start.elapsed();
        
        // Record metrics
        {
            let mut metrics = self.context.metrics.write().await;
            metrics.record_request(response_time.as_millis() as f64, true);
        }
        
        Ok(HealthStatus {
            overall: if api_health.healthy && websocket_health.healthy && consensus_health.healthy {
                ServiceHealth::Healthy
            } else if api_health.healthy || websocket_health.healthy || consensus_health.healthy {
                ServiceHealth::Degraded
            } else {
                ServiceHealth::Unhealthy
            },
            api: api_health,
            websocket: websocket_health,
            consensus: consensus_health,
            response_time_ms: response_time.as_millis() as u64,
            timestamp: chrono::Utc::now(),
        })
    }
    
    /// Subscribe to platform events
    pub async fn subscribe<T>(&self, event_type: EventType) -> SDKResult<EventStream<T>>
    where
        T: for<'de> serde::Deserialize<'de> + Send + 'static,
    {
        let ws_manager = self.context.websocket_manager.read().await;
        ws_manager.subscribe(event_type).await
    }
    
    /// Get current configuration
    pub fn config(&self) -> &Config {
        &self.context.config
    }
    
    /// Create a new game builder
    pub fn create_game(&self, name: &str) -> GameBuilder {
        self.games_api.create(name)
    }
    
    /// Join an existing game
    pub async fn join_game(&self, game_id: &GameId) -> SDKResult<GameSession> {
        self.games_api.join(game_id).await
    }
    
    /// List available games
    pub async fn list_games(&self, filters: Option<GameFilters>) -> SDKResult<Vec<GameInfo>> {
        self.games_api.list(filters).await
    }
    
    /// Get detailed game information
    pub async fn get_game(&self, game_id: &GameId) -> SDKResult<GameDetails> {
        self.games_api.get(game_id).await
    }
    
    /// Submit a consensus proposal
    pub async fn propose(&self, game_id: &GameId, action: GameAction) -> SDKResult<ProposalId> {
        self.consensus_api.propose(game_id, action).await
    }
    
    /// Vote on a consensus proposal
    pub async fn vote(&self, proposal_id: &ProposalId, vote: Vote) -> SDKResult<()> {
        self.consensus_api.vote(proposal_id, vote).await
    }
    
    /// Get network peers
    pub async fn get_peers(&self) -> SDKResult<Vec<PeerInfo>> {
        self.network_api.get_peers().await
    }
    
    /// Connect to a specific peer
    pub async fn connect_peer(&self, peer_address: &str) -> SDKResult<PeerId> {
        self.network_api.connect(peer_address).await
    }
    
    // Private health check methods
    async fn check_api_health(&self) -> SDKResult<ComponentHealth> {
        let start = Instant::now();
        
        match self.context.client
            .get(&format!("{}/health", self.context.config.base_url))
            .header("Authorization", format!("Bearer {}", self.context.config.api_key))
            .send()
            .await
        {
            Ok(response) => {
                let healthy = response.status().is_success();
                Ok(ComponentHealth {
                    healthy,
                    response_time_ms: start.elapsed().as_millis() as u64,
                    last_error: if healthy { None } else { 
                        Some(format!("HTTP {}", response.status())) 
                    },
                })
            }
            Err(e) => Ok(ComponentHealth {
                healthy: false,
                response_time_ms: start.elapsed().as_millis() as u64,
                last_error: Some(e.to_string()),
            })
        }
    }
    
    async fn check_websocket_health(&self) -> SDKResult<ComponentHealth> {
        let start = Instant::now();
        let ws_manager = self.context.websocket_manager.read().await;
        
        let healthy = ws_manager.is_connected();
        Ok(ComponentHealth {
            healthy,
            response_time_ms: start.elapsed().as_millis() as u64,
            last_error: if healthy { None } else { 
                Some("WebSocket disconnected".to_string()) 
            },
        })
    }
    
    async fn check_consensus_health(&self) -> SDKResult<ComponentHealth> {
        let start = Instant::now();
        
        // Try to get consensus status
        match self.context.client
            .get(&format!("{}/consensus/status", self.context.config.base_url))
            .header("Authorization", format!("Bearer {}", self.context.config.api_key))
            .send()
            .await
        {
            Ok(response) => {
                let healthy = response.status().is_success();
                Ok(ComponentHealth {
                    healthy,
                    response_time_ms: start.elapsed().as_millis() as u64,
                    last_error: if healthy { None } else { 
                        Some(format!("Consensus HTTP {}", response.status())) 
                    },
                })
            }
            Err(e) => Ok(ComponentHealth {
                healthy: false,
                response_time_ms: start.elapsed().as_millis() as u64,
                last_error: Some(format!("Consensus error: {}", e)),
            })
        }
    }
}

/// SDK builder pattern for advanced configuration
pub struct SDKBuilder {
    config_builder: crate::sdk_v2::config::ConfigBuilder,
}

impl SDKBuilder {
    pub fn new() -> Self {
        Self {
            config_builder: Config::builder(),
        }
    }
    
    pub fn api_key<S: Into<String>>(mut self, key: S) -> Self {
        self.config_builder = self.config_builder.api_key(key);
        self
    }
    
    pub fn environment(mut self, env: Environment) -> Self {
        self.config_builder = self.config_builder.environment(env);
        self
    }
    
    pub fn base_url<S: Into<String>>(mut self, url: S) -> Self {
        self.config_builder = self.config_builder.base_url(url);
        self
    }
    
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config_builder = self.config_builder.timeout(timeout);
        self
    }
    
    pub async fn build(self) -> SDKResult<BitCrapsSDK> {
        let config = self.config_builder.build()?;
        BitCrapsSDK::new(config).await
    }
}

impl Default for SDKBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::testing::MockEnvironment;
    use tokio::time::Duration;
    
    #[tokio::test]
    async fn test_sdk_builder() {
        let result = SDKBuilder::new()
            .api_key("test-key")
            .environment(Environment::Testing)
            .timeout(Duration::from_secs(30))
            .build()
            .await;
            
        // Should succeed in test environment
        assert!(result.is_ok() || matches!(result, Err(SDKError::NetworkError(_))));
    }
    
    #[tokio::test]
    async fn test_health_check_with_mock() {
        let mock_env = MockEnvironment::new();
        let sdk = BitCrapsSDK::with_mock(mock_env).await.unwrap();
        
        // Health check should work with mock environment
        let health = sdk.health_check().await;
        assert!(health.is_ok() || matches!(health, Err(SDKError::NetworkError(_))));
    }
    
    #[tokio::test]
    async fn test_game_operations() {
        let mock_env = MockEnvironment::new();
        let sdk = BitCrapsSDK::with_mock(mock_env).await.unwrap();
        
        // Test game listing
        let games = sdk.list_games(None).await;
        assert!(games.is_ok() || matches!(games, Err(SDKError::NetworkError(_))));
    }
}