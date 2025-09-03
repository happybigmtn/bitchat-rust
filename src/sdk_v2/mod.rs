//! BitCraps Developer SDK v2.0
//!
//! ## Overview
//! The BitCraps SDK v2.0 provides developers with production-ready tools to build
//! games, integrations, and applications on the BitCraps platform. This SDK features:
//!
//! - **Type-safe APIs** with comprehensive error handling
//! - **Multi-language support** with code generation
//! - **Real-time communication** via WebSocket and REST APIs
//! - **Testing framework** with mock environments
//! - **Interactive playground** for rapid development
//! - **Comprehensive documentation** with examples
//!
//! ## Quick Start
//!
//! ```rust
//! use bitcraps_sdk_v2::{BitCrapsSDK, Config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::builder()
//!         .api_key("your-api-key")
//!         .environment(Environment::Sandbox)
//!         .build()?;
//!         
//!     let sdk = BitCrapsSDK::new(config).await?;
//!     
//!     // Create a new game
//!     let game = sdk.games()
//!         .create("My Craps Game")
//!         .with_max_players(8)
//!         .with_betting_limits(1, 1000)
//!         .build()
//!         .await?;
//!     
//!     println!("Game created: {}", game.id);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod types;
pub mod builder;
pub mod testing;
pub mod playground;
pub mod codegen;
pub mod docs;

// API modules
pub mod rest;
pub mod websocket;
// Note: GraphQL support planned for future release

// Game development
pub mod games;
pub mod consensus;
pub mod networking;

// Developer tools
pub mod cli;
// Note: Advanced monitoring and debugging tools planned for future release

// Re-exports for convenience
pub use client::BitCrapsSDK;
pub use config::{Config, ConfigBuilder, Environment};
pub use error::{SDKError, SDKResult};
pub use types::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main SDK version
pub const SDK_VERSION: &str = "2.0.0";

/// SDK-wide configuration and state
#[derive(Debug)]
pub struct SDKContext {
    pub config: Config,
    pub client: Arc<reqwest::Client>,
    pub websocket_manager: Arc<RwLock<websocket::WebSocketManager>>,
    pub metrics: Arc<RwLock<SDKMetrics>>,
}

/// SDK usage metrics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SDKMetrics {
    pub requests_made: u64,
    pub games_created: u64,
    pub consensus_operations: u64,
    pub websocket_connections: u64,
    pub errors_encountered: u64,
    pub average_response_time_ms: f64,
}

impl SDKMetrics {
    pub fn record_request(&mut self, response_time_ms: f64, success: bool) {
        self.requests_made += 1;
        if success {
            // Update average using exponential moving average
            let alpha = 0.1;
            self.average_response_time_ms = 
                alpha * response_time_ms + (1.0 - alpha) * self.average_response_time_ms;
        } else {
            self.errors_encountered += 1;
        }
    }
}

/// Initialize SDK with configuration
pub async fn init(config: Config) -> SDKResult<Arc<SDKContext>> {
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(config.request_timeout)
            .user_agent(&format!("BitCraps-SDK/{}", SDK_VERSION))
            .build()
            .map_err(|e| SDKError::ConfigurationError(e.to_string()))?
    );
    
    let websocket_manager = Arc::new(RwLock::new(
        websocket::WebSocketManager::new(config.clone()).await?
    ));
    
    let metrics = Arc::new(RwLock::new(SDKMetrics::default()));
    
    Ok(Arc::new(SDKContext {
        config,
        client,
        websocket_manager,
        metrics,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::testing::MockEnvironment;
    
    #[tokio::test]
    async fn test_sdk_initialization() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build()
            .unwrap();
            
        let context = init(config).await.unwrap();
        assert_eq!(context.config.environment, Environment::Testing);
    }
    
    #[tokio::test]
    async fn test_sdk_with_mock_environment() {
        let mut mock_env = MockEnvironment::new();
        mock_env.expect_game_creation().returning(|_| Ok(()));
        
        // Test SDK operations in mock environment
        let sdk = BitCrapsSDK::with_mock(mock_env).await.unwrap();
        let result = sdk.games().create("Test Game").build().await;
        assert!(result.is_ok());
    }
}