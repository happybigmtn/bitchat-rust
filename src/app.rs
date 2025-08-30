//! Main BitCraps application coordinator
//!
//! This module provides the main application struct that coordinates all subsystems
//! including networking, consensus, gaming, and token management.

use std::sync::Arc;
use std::time::Duration;

use crate::{
    crypto::{BitchatIdentity, SecureKeystore},
    error::Result,
    gaming::{ConsensusGameConfig, ConsensusGameManager},
    mesh::{
        ConsensusMessageConfig, ConsensusMessageHandler, MeshConsensusIntegration, MeshService,
    },
    protocol::{BetType, CrapTokens, GameId, PeerId},
    token::TokenLedger,
    transport::TransportCoordinator,
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
        log::info!(
            "Starting BitCraps application on peer {:?}",
            self.identity.peer_id
        );

        self.initialize_token_ledger();
        let transport = self.setup_transport_layer().await?;
        let mesh_service = self.initialize_mesh_service(transport).await?;
        let consensus_handler = self.setup_consensus_handler(&mesh_service);
        self.initialize_game_manager(mesh_service.clone(), consensus_handler.clone())
            .await?;

        // Wire up consensus with mesh
        MeshConsensusIntegration::integrate(mesh_service, consensus_handler).await?;

        log::info!("BitCraps application started successfully");
        Ok(())
    }

    /// Initialize the token ledger
    fn initialize_token_ledger(&mut self) {
        let token_ledger = Arc::new(TokenLedger::new());
        self.token_ledger = Some(token_ledger);
    }

    /// Setup the transport layer with appropriate protocols
    async fn setup_transport_layer(&self) -> Result<Arc<TransportCoordinator>> {
        let mut coordinator = TransportCoordinator::new();

        // Enable TCP for desktop/server nodes
        if !self.config.mobile_mode {
            coordinator.enable_tcp(self.config.port).await?;
        }

        // Always enable Bluetooth for local mesh connectivity
        coordinator.init_bluetooth(self.identity.peer_id).await?;

        Ok(Arc::new(coordinator))
    }

    /// Initialize the mesh networking service
    async fn initialize_mesh_service(
        &self,
        transport: Arc<TransportCoordinator>,
    ) -> Result<Arc<MeshService>> {
        let mesh_service = Arc::new(MeshService::new(self.identity.clone(), transport));
        mesh_service.start().await?;
        Ok(mesh_service)
    }

    /// Setup the consensus message handler with encryption
    fn setup_consensus_handler(
        &self,
        mesh_service: &Arc<MeshService>,
    ) -> Arc<ConsensusMessageHandler> {
        let consensus_config = ConsensusMessageConfig {
            enable_encryption: true,
            ..Default::default()
        };

        Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            self.identity.clone(),
            consensus_config,
        ))
    }

    /// Initialize the consensus game manager
    async fn initialize_game_manager(
        &mut self,
        mesh_service: Arc<MeshService>,
        consensus_handler: Arc<ConsensusMessageHandler>,
    ) -> Result<()> {
        let game_config = ConsensusGameConfig::default();
        let game_manager = Arc::new(ConsensusGameManager::new(
            self.identity.clone(),
            mesh_service,
            consensus_handler,
            game_config,
        ));

        game_manager.start().await?;
        self.consensus_game_manager = Some(game_manager);
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
        let game_manager = self.get_game_manager()?;
        let participants = self.gather_participants(min_players).await;
        game_manager.create_game(participants).await
    }

    /// Gather participants for a new game
    async fn gather_participants(&self, min_players: u8) -> Vec<PeerId> {
        let mut participants = vec![self.identity.peer_id];

        // TODO: Replace with proper peer discovery and invitation system
        // Currently using placeholder peers for testing
        for _ in 1..min_players {
            participants.push(Self::generate_placeholder_peer());
        }

        participants
    }

    /// Generate a placeholder peer ID for testing
    fn generate_placeholder_peer() -> PeerId {
        let mut peer_id = [0u8; 32];
        use rand::{rngs::OsRng, RngCore};
        OsRng.fill_bytes(&mut peer_id);
        peer_id
    }

    /// Join an existing game
    pub async fn join_game(&self, game_id: GameId) -> Result<()> {
        let game_manager = self.get_game_manager()?;
        game_manager.join_game(game_id).await
    }

    /// Place a bet in a game
    pub async fn place_bet(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount: CrapTokens,
    ) -> Result<()> {
        let game_manager = self.get_game_manager()?;

        // Validate balance before placing bet
        if let Some(ledger) = &self.token_ledger {
            let balance =
                CrapTokens::new_unchecked(ledger.get_balance(&self.identity.peer_id).await);
            if balance < amount {
                return Err(crate::error::Error::InsufficientBalance(format!(
                    "Balance: {}, Required: {}",
                    balance.to_crap(),
                    amount.to_crap()
                )));
            }
        }

        // Place bet through consensus manager
        game_manager.place_bet(game_id, bet_type, amount).await
    }

    /// Get current balance
    pub async fn get_balance(&self) -> Result<CrapTokens> {
        let ledger = self.get_token_ledger()?;

        let balance = ledger.get_balance(&self.identity.peer_id).await;
        Ok(CrapTokens::new_unchecked(balance))
    }

    /// Get active games
    pub async fn get_active_games(&self) -> Result<Vec<GameId>> {
        let game_manager = self.get_game_manager()?;

        // Get list of active games from consensus manager
        let games = game_manager.get_active_games().await?;
        Ok(games)
    }

    /// Get peer ID
    pub fn peer_id(&self) -> PeerId {
        self.identity.peer_id
    }

    /// Get the game manager, returning an error if not initialized
    fn get_game_manager(&self) -> Result<&Arc<ConsensusGameManager>> {
        self.consensus_game_manager
            .as_ref()
            .ok_or_else(|| crate::error::Error::NotInitialized("Game manager not started".into()))
    }

    /// Get the token ledger, returning an error if not initialized
    fn get_token_ledger(&self) -> Result<&Arc<TokenLedger>> {
        self.token_ledger
            .as_ref()
            .ok_or_else(|| crate::error::Error::NotInitialized("Token ledger not started".into()))
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
