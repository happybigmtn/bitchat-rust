//! Main BitCraps application coordinator
//! 
//! This module provides the main application struct that coordinates all subsystems
//! including networking, consensus, gaming, and token management.

use std::sync::Arc;
use std::time::Duration;

use crate::{
    protocol::{PeerId, GameId, BetType, CrapTokens},
    error::Result,
    crypto::{BitchatIdentity, SecureKeystore},
    gaming::{ConsensusGameManager, ConsensusGameConfig},
    mesh::{MeshService, ConsensusMessageHandler, ConsensusMessageConfig, MeshConsensusIntegration},
    transport::TransportCoordinator,
    token::TokenLedger,
};

/// Main BitCraps application coordinator
pub struct BitCrapsApp {
    /// Peer identity
    pub identity: Arc<BitchatIdentity>,
    
    /// Application configuration
    pub config: ApplicationConfig,
    
    /// Consensus game manager for distributed game coordination
    pub consensus_game_manager: Option<Arc<ConsensusGameManager>>,
    
    /// Token ledger for managing CRAP tokens
    pub token_ledger: Option<Arc<TokenLedger>>,
    
    /// Keystore for persistent identity
    pub keystore: Option<Arc<SecureKeystore>>,
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct ApplicationConfig {
    /// Network listen port
    pub port: u16,
    
    /// Enable debug logging
    pub debug: bool,
    
    /// Database path
    pub db_path: String,
    
    /// Maximum concurrent games
    pub max_games: usize,
    
    /// Session timeout
    pub session_timeout: Duration,
    
    /// Enable mobile optimizations
    pub mobile_mode: bool,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            port: 8989,
            debug: false,
            db_path: "bitcraps.db".to_string(),
            max_games: 100,
            session_timeout: Duration::from_secs(3600),
            mobile_mode: false,
        }
    }
}

impl BitCrapsApp {
    /// Create a new BitCraps application instance
    pub async fn new(config: ApplicationConfig) -> Result<Self> {
        // Initialize secure keystore
        let keystore = Arc::new(SecureKeystore::new()?);
        
        // Generate identity with proof-of-work
        // In production, this would load from persistent storage if available
        let identity = Arc::new(BitchatIdentity::generate_with_pow(16));
        
        Ok(Self {
            identity,
            config,
            consensus_game_manager: None,
            token_ledger: None,
            keystore: Some(keystore),
        })
    }
    
    /// Start the application
    pub async fn start(&mut self) -> Result<()> {
        log::info!("Starting BitCraps application on peer {:?}", self.identity.peer_id);
        
        // Initialize token ledger
        let token_ledger = Arc::new(TokenLedger::new());
        self.token_ledger = Some(token_ledger.clone());
        
        // Initialize transport coordinator
        let mut transport_coordinator = TransportCoordinator::new();
        
        // Enable multiple transports based on configuration
        if !self.config.mobile_mode {
            // Add TCP transport for desktop/server nodes
            transport_coordinator.enable_tcp(self.config.port).await?;
        }
        
        // Always enable Bluetooth for local mesh
        transport_coordinator.init_bluetooth(self.identity.peer_id).await?;
        
        let transport_coordinator = Arc::new(transport_coordinator);
        
        // Initialize mesh service
        let mesh_service = Arc::new(MeshService::new(self.identity.clone(), transport_coordinator));
        mesh_service.start().await?;
        
        // Setup consensus message handler
        let consensus_config = ConsensusMessageConfig {
            enable_encryption: true,  // Enable transport encryption by default
            ..Default::default()
        };
        
        let consensus_handler = Arc::new(
            ConsensusMessageHandler::new(
                mesh_service.clone(),
                self.identity.clone(),
                consensus_config,
            )
        );
        
        // Initialize consensus game manager
        let game_config = ConsensusGameConfig::default();
        let consensus_game_manager = Arc::new(
            ConsensusGameManager::new(
                self.identity.clone(),
                mesh_service.clone(),
                consensus_handler.clone(),
                game_config,
            )
        );
        
        // Wire up consensus with mesh
        MeshConsensusIntegration::integrate(
            mesh_service.clone(),
            consensus_handler,
        ).await?;
        
        // Start game manager
        consensus_game_manager.start().await?;
        self.consensus_game_manager = Some(consensus_game_manager);
        
        log::info!("BitCraps application started successfully");
        Ok(())
    }
    
    /// Stop the application
    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping BitCraps application");
        
        // Services stop automatically when dropped
        
        log::info!("BitCraps application stopped");
        Ok(())
    }
    
    /// Create a new game
    pub async fn create_game(&self, min_players: u8, _ante: CrapTokens) -> Result<GameId> {
        let game_manager = self.consensus_game_manager.as_ref()
            .ok_or(crate::error::Error::NotInitialized("Game manager not started".into()))?;
        
        // Create participants list starting with self
        let mut participants = vec![self.identity.peer_id];
        
        // Wait for minimum players to be available (simplified for now)
        // In production, this would use peer discovery
        for _ in 1..min_players {
            // For now, just use placeholder peers
            // TODO: Implement proper peer discovery and invitation
            let mut placeholder_peer = [0u8; 32];
            use rand::{RngCore, rngs::OsRng};
            OsRng.fill_bytes(&mut placeholder_peer);
            participants.push(placeholder_peer);
        }
        
        // Create game through consensus manager
        game_manager.create_game(participants).await
    }
    
    /// Join an existing game
    pub async fn join_game(&self, game_id: GameId) -> Result<()> {
        let game_manager = self.consensus_game_manager.as_ref()
            .ok_or(crate::error::Error::NotInitialized("Game manager not started".into()))?;
        
        // Join game through consensus manager with full state sync
        game_manager.join_game(game_id).await
    }
    
    /// Place a bet in a game
    pub async fn place_bet(&self, game_id: GameId, bet_type: BetType, amount: CrapTokens) -> Result<()> {
        let game_manager = self.consensus_game_manager.as_ref()
            .ok_or(crate::error::Error::NotInitialized("Game manager not started".into()))?;
        
        // Validate balance before placing bet
        if let Some(ledger) = &self.token_ledger {
            let balance = CrapTokens::new_unchecked(ledger.get_balance(&self.identity.peer_id).await);
            if balance < amount {
                return Err(crate::error::Error::InsufficientBalance(
                    format!("Balance: {}, Required: {}", balance.to_crap(), amount.to_crap())
                ));
            }
        }
        
        // Place bet through consensus manager
        game_manager.place_bet(game_id, bet_type, amount).await
    }
    
    /// Get current balance
    pub async fn get_balance(&self) -> Result<CrapTokens> {
        let ledger = self.token_ledger.as_ref()
            .ok_or(crate::error::Error::NotInitialized("Token ledger not started".into()))?;
        
        let balance = ledger.get_balance(&self.identity.peer_id).await;
        Ok(CrapTokens::new_unchecked(balance))
    }
    
    /// Get active games
    pub async fn get_active_games(&self) -> Result<Vec<GameId>> {
        let game_manager = self.consensus_game_manager.as_ref()
            .ok_or(crate::error::Error::NotInitialized("Game manager not started".into()))?;
        
        // Get list of active games from consensus manager
        let games = game_manager.get_active_games().await?;
        Ok(games)
    }
    
    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.identity.peer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_app_creation() {
        let config = ApplicationConfig::default();
        let app = BitCrapsApp::new(config).await;
        assert!(app.is_ok());
    }
}