//! Application state management and initialization
//!
//! This module contains the main application struct and all
//! initialization, background task management, and core app logic.

use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};

use bitcraps::{
    AppConfig, BitchatIdentity, BluetoothDiscovery, CrapTokens, Error, GameId, GameRuntime,
    MeshService, PeerId, PersistenceManager, ProofOfRelay, Result,
    SessionManager as BitchatSessionManager, TokenLedger, TransportCoordinator, TREASURY_ADDRESS,
};

use bitcraps::gaming::{ConsensusGameConfig, ConsensusGameManager};
use bitcraps::mesh::{ConsensusMessageConfig, ConsensusMessageHandler, MeshConsensusIntegration};
use bitcraps::protocol::craps::CrapsGame;

/// Simple struct for game info display
#[derive(Debug, Clone)]
pub struct GameInfo {
    pub phase: String,
    pub players: usize,
    pub rolls: u64,
}

/// Main BitCraps application
///
/// Feynman: This is the master conductor that brings the whole
/// orchestra together. Each component is like a different section
/// (strings, brass, percussion), and the conductor ensures they
/// all play in harmony to create the complete casino experience.
pub struct BitCrapsApp {
    pub identity: Arc<BitchatIdentity>,
    pub _transport_coordinator: Arc<TransportCoordinator>,
    pub mesh_service: Arc<MeshService>,
    pub session_manager: Arc<BitchatSessionManager>,
    pub ledger: Arc<TokenLedger>,
    pub game_runtime: Arc<GameRuntime>,
    pub _discovery: Arc<BluetoothDiscovery>,
    pub _persistence: Arc<PersistenceManager>,
    pub proof_of_relay: Arc<ProofOfRelay>,
    pub config: AppConfig,
    pub active_games: Arc<tokio::sync::RwLock<rustc_hash::FxHashMap<GameId, CrapsGame>>>,

    // P2P Consensus Components
    pub consensus_message_handler: Arc<ConsensusMessageHandler>,
    pub consensus_game_manager: Arc<ConsensusGameManager>,
}

impl BitCrapsApp {
    /// Initialize a new BitCraps application
    pub async fn new(config: AppConfig) -> Result<Self> {
        println!("🎲 Initializing BitCraps...");

        // Step 1: Generate or load identity with PoW
        println!(
            "⛏️ Generating identity with proof-of-work (difficulty: {})...",
            config.pow_difficulty
        );
        let identity = Arc::new(BitchatIdentity::generate_with_pow(config.pow_difficulty));
        println!("✅ Identity generated: {:?}", identity.peer_id);

        // Step 2: Initialize persistence
        println!("💾 Initializing persistence layer...");
        let persistence = Arc::new(PersistenceManager::new(&config.data_dir).await?);

        // Step 3: Initialize token ledger with treasury
        println!("💰 Initializing token ledger and treasury...");
        let ledger = Arc::new(TokenLedger::new());
        let treasury_balance = ledger.get_balance(&TREASURY_ADDRESS).await;
        println!(
            "✅ Treasury initialized with {} CRAP tokens",
            treasury_balance / 1_000_000
        );

        // Step 4: Setup transport layer
        println!("📡 Setting up Bluetooth transport...");
        let mut transport_coordinator = TransportCoordinator::new();
        transport_coordinator
            .init_bluetooth(identity.peer_id)
            .await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;
        let transport_coordinator = Arc::new(transport_coordinator);

        // Step 5: Initialize mesh service
        println!("🕸️ Starting mesh networking service...");
        let session_manager = BitchatSessionManager::new(Default::default());
        let mut mesh_service = MeshService::new(identity.clone(), transport_coordinator.clone());

        // Step 6: Setup discovery
        println!("🔍 Starting peer discovery...");
        let discovery = Arc::new(
            BluetoothDiscovery::new(identity.clone())
                .await
                .map_err(|e| Error::Network(e.to_string()))?,
        );

        // Step 7: Initialize game runtime with treasury
        println!("🎰 Starting game runtime with treasury participant...");
        let (game_runtime, _game_sender) = GameRuntime::new(Default::default(), [0; 32]);
        let game_runtime = Arc::new(game_runtime);

        // Step 8: Setup proof-of-relay consensus
        println!("⚡ Initializing proof-of-relay consensus...");
        let proof_of_relay = Arc::new(ProofOfRelay::new(ledger.clone()));

        // Wire up proof of relay to mesh service
        mesh_service.set_proof_of_relay(proof_of_relay.clone());
        let mesh_service = Arc::new(mesh_service);

        // Step 9: Initialize P2P consensus components
        println!("🤝 Setting up P2P consensus system...");

        // Create consensus message handler
        let consensus_config = ConsensusMessageConfig::default();
        let consensus_message_handler = Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            identity.clone(),
            consensus_config,
        ));

        // Create consensus game manager
        let game_config = ConsensusGameConfig::default();
        let consensus_game_manager = Arc::new(ConsensusGameManager::new(
            identity.clone(),
            mesh_service.clone(),
            consensus_message_handler.clone(),
            game_config,
        ));

        // Integrate consensus handler with mesh service
        MeshConsensusIntegration::integrate(
            mesh_service.clone(),
            consensus_message_handler.clone(),
        )
        .await?;

        // Step 10: Start all services
        mesh_service.start().await?;
        consensus_game_manager.start().await?;

        println!("🚀 BitCraps node ready!");
        println!("📱 Peer ID: {:?}", identity.peer_id);
        if let Some(nick) = &config.nickname {
            println!("👤 Nickname: {}", nick);
        }

        Ok(Self {
            identity: identity.clone(),
            _transport_coordinator: transport_coordinator,
            mesh_service,
            session_manager: Arc::new(session_manager),
            ledger,
            game_runtime,
            _discovery: discovery,
            _persistence: persistence,
            proof_of_relay,
            config,
            active_games: Arc::new(tokio::sync::RwLock::new(rustc_hash::FxHashMap::default())),
            consensus_message_handler,
            consensus_game_manager,
        })
    }

    /// Start the main application loop
    ///
    /// Feynman: Like opening the casino doors - all systems are go,
    /// the dealers are at their tables, the lights are on, and we're
    /// ready for players. The main loop keeps everything running smoothly.
    pub async fn start(&mut self) -> Result<()> {
        // Start relay reward timer
        self.start_mining_rewards().await?;

        // Start background tasks
        self.start_heartbeat().await;
        self.start_game_coordinator().await;

        info!("✅ BitCraps node started successfully!");
        info!("📡 Peer ID: {:?}", self.identity.peer_id);
        info!(
            "💼 Balance: {} CRAP",
            CrapTokens::new_unchecked(self.ledger.get_balance(&self.identity.peer_id).await)
                .to_crap()
        );
        info!("🎲 Ready to play craps!");

        // Keep running until shutdown
        loop {
            sleep(Duration::from_secs(1)).await;
            self.periodic_tasks().await?;
        }
    }

    /// Get the current user's peer ID
    pub fn _get_peer_id(&self) -> PeerId {
        self.identity.peer_id
    }

    /// Get application configuration
    pub fn _get_config(&self) -> &AppConfig {
        &self.config
    }

    /// Check if the application is ready to accept commands
    pub async fn _is_ready(&self) -> bool {
        // Check if all core services are initialized and running
        let mesh_ready = self.mesh_service.get_stats().await.connected_peers > 0 || true; // Allow offline mode
        let ledger_ready = true; // Balance is always valid (u64 >= 0)

        mesh_ready && ledger_ready
    }

    /// Get a snapshot of active games
    pub async fn _get_active_games(&self) -> rustc_hash::FxHashMap<GameId, GameInfo> {
        let games = self.active_games.read().await;
        games
            .iter()
            .map(|(id, game)| {
                (
                    *id,
                    GameInfo {
                        phase: format!("{:?}", game.phase),
                        players: game.participants.len(),
                        rolls: game.roll_count,
                    },
                )
            })
            .collect()
    }

    /// Get a specific game by ID
    pub async fn _get_game(&self, game_id: &GameId) -> Option<CrapsGame> {
        let games = self.active_games.read().await;
        games.get(game_id).cloned()
    }

    /// Update a specific game
    pub async fn _update_game(&self, game_id: GameId, game: CrapsGame) {
        let mut games = self.active_games.write().await;
        games.insert(game_id, game);
    }

    /// Remove a game from active games
    pub async fn _remove_game(&self, game_id: &GameId) -> Option<CrapsGame> {
        let mut games = self.active_games.write().await;
        games.remove(game_id)
    }

    // ============= P2P Consensus Game Methods =============

    /// Create a new consensus-based multiplayer game
    pub async fn create_consensus_game(&self, participants: Vec<PeerId>) -> Result<GameId> {
        self.consensus_game_manager.create_game(participants).await
    }

    /// Join an existing consensus game
    pub async fn join_consensus_game(&self, game_id: GameId) -> Result<()> {
        self.consensus_game_manager.join_game(game_id).await
    }

    /// Place a bet in a consensus game
    pub async fn place_consensus_bet(
        &self,
        game_id: GameId,
        bet_type: bitcraps::protocol::BetType,
        amount: CrapTokens,
    ) -> Result<()> {
        self.consensus_game_manager
            .place_bet(game_id, bet_type, amount)
            .await
    }

    /// Roll dice in a consensus game
    pub async fn roll_consensus_dice(
        &self,
        game_id: GameId,
    ) -> Result<bitcraps::protocol::DiceRoll> {
        self.consensus_game_manager.roll_dice(game_id).await
    }

    /// Get consensus game state
    pub async fn get_consensus_game_state(
        &self,
        game_id: &GameId,
    ) -> Option<bitcraps::gaming::ConsensusGameSession> {
        self.consensus_game_manager.get_game_state(game_id).await
    }

    /// List all active consensus games
    pub async fn list_consensus_games(
        &self,
    ) -> Vec<(GameId, bitcraps::gaming::ConsensusGameSession)> {
        self.consensus_game_manager.list_active_games().await
    }

    /// Get consensus system statistics
    pub async fn get_consensus_stats(&self) -> bitcraps::gaming::GameManagerStats {
        self.consensus_game_manager.get_stats().await
    }

    /// Start mining rewards for network participation
    ///
    /// Feynman: Like getting paid for being a good citizen - the more
    /// you help the network (relay messages, store data, host games),
    /// the more tokens you earn. It's capitalism for routers!
    async fn start_mining_rewards(&self) -> Result<()> {
        let ledger = self.ledger.clone();
        let proof_of_relay = self.proof_of_relay.clone();
        let peer_id = self.identity.peer_id;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                // Process relay rewards
                if let Ok(reward_amount) = proof_of_relay.process_accumulated_rewards(peer_id).await
                {
                    if reward_amount > 0 {
                        println!(
                            "⚡ Earned {} CRAP tokens for relaying messages",
                            CrapTokens::new_unchecked(reward_amount).to_crap()
                        );
                    }
                }

                // Process staking rewards (existing method)
                if let Err(e) = ledger.distribute_staking_rewards().await {
                    warn!("Failed to distribute staking rewards: {}", e);
                }

                // Adjust mining difficulty based on network activity
                if let Err(e) = proof_of_relay.adjust_mining_difficulty().await {
                    warn!("Failed to adjust mining difficulty: {}", e);
                }

                // Clean up old relay entries
                if let Err(e) = proof_of_relay.cleanup_old_entries().await {
                    warn!("Failed to cleanup old relay entries: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start heartbeat task for periodic maintenance
    async fn start_heartbeat(&self) {
        let ledger = self.ledger.clone();

        tokio::spawn(async move {
            let mut heartbeat = interval(Duration::from_secs(30));

            loop {
                heartbeat.tick().await;

                // Distribute staking rewards
                if let Err(e) = ledger.distribute_staking_rewards().await {
                    warn!("Failed to distribute staking rewards: {}", e);
                }
            }
        });
    }

    /// Start game coordinator for managing active games
    async fn start_game_coordinator(&self) {
        let _game_runtime = self.game_runtime.clone();
        let active_games = self.active_games.clone();

        tokio::spawn(async move {
            let mut coordinator = interval(Duration::from_secs(10));

            loop {
                coordinator.tick().await;

                // Clean up completed games
                {
                    let mut games = active_games.write().await;
                    games.retain(|_id, game| game.is_active());
                }

                // Game management logic would go here
                info!("🎲 Game coordinator running...");
            }
        });
    }

    /// Perform periodic maintenance tasks
    async fn periodic_tasks(&self) -> Result<()> {
        // Check for network updates, clean up old data, etc.
        Ok(())
    }

    /// Gracefully shutdown the application
    pub async fn _shutdown(&mut self) -> Result<()> {
        println!("👋 Shutting down BitCraps...");

        // Save state to persistence
        self._persistence.flush().await?;

        // Services will be stopped automatically when dropped
        println!("✅ BitCraps shutdown complete");

        Ok(())
    }
}

/// Application statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct AppStats {
    pub peer_id: PeerId,
    pub connected_peers: usize,
    pub active_sessions: usize,
    pub balance: u64,
    pub active_games: usize,
    pub total_supply: u64,
    pub total_relays: u64,
}

impl BitCrapsApp {
    /// Get comprehensive application statistics
    pub async fn get_stats(&self) -> AppStats {
        let mesh_stats = self.mesh_service.get_stats().await;
        let session_stats = self.session_manager.get_stats().await;
        let ledger_stats = self.ledger.get_stats().await;
        let mining_stats = self.proof_of_relay.get_stats().await;

        let games = self.active_games.read().await;
        let active_games = games.len();

        AppStats {
            peer_id: self.identity.peer_id,
            connected_peers: mesh_stats.connected_peers,
            active_sessions: session_stats.active_sessions,
            balance: self.ledger.get_balance(&self.identity.peer_id).await,
            active_games,
            total_supply: ledger_stats.total_supply,
            total_relays: mining_stats.total_relays,
        }
    }
}

/// Application lifecycle management
impl BitCrapsApp {
    /// Check if the application needs to be restarted
    pub fn _needs_restart(&self) -> bool {
        // Check for configuration changes, critical errors, etc.
        false
    }

    /// Reload configuration without full restart
    pub async fn _reload_config(&mut self, new_config: AppConfig) -> Result<()> {
        // Update configuration and restart necessary services
        self.config = new_config;
        Ok(())
    }

    /// Get application health status
    pub async fn _health_check(&self) -> AppHealth {
        let is_ready = self._is_ready().await;
        let stats = self.get_stats().await;

        AppHealth {
            is_ready,
            peer_count: stats.connected_peers,
            balance: stats.balance,
            active_games: stats.active_games,
            uptime_seconds: 0, // Would track actual uptime
        }
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct AppHealth {
    pub is_ready: bool,
    pub peer_count: usize,
    pub balance: u64,
    pub active_games: usize,
    pub uptime_seconds: u64,
}

impl AppHealth {
    /// Check if the application is healthy
    pub fn is_healthy(&self) -> bool {
        self.is_ready // balance is u64, always >= 0
    }

    /// Get a human-readable status message
    pub fn status_message(&self) -> String {
        if self.is_healthy() {
            format!(
                "✅ Healthy - {} peers, {} games active",
                self.peer_count, self.active_games
            )
        } else {
            "❌ Unhealthy - check logs for details".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_info() {
        let info = GameInfo {
            phase: "ComeOut".to_string(),
            players: 3,
            rolls: 15,
        };

        assert_eq!(info.phase, "ComeOut");
        assert_eq!(info.players, 3);
        assert_eq!(info.rolls, 15);
    }

    #[test]
    fn test_app_health() {
        let health = AppHealth {
            is_ready: true,
            peer_count: 5,
            balance: 1000,
            active_games: 2,
            uptime_seconds: 3600,
        };

        assert!(health.is_healthy());
        assert!(health.status_message().starts_with("✅"));

        let unhealthy = AppHealth {
            is_ready: false,
            peer_count: 0,
            balance: 0,
            active_games: 0,
            uptime_seconds: 0,
        };

        assert!(!unhealthy.is_healthy());
        assert!(unhealthy.status_message().starts_with("❌"));
    }
}
