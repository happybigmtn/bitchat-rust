//! Refactored Gaming Runtime with Separated Concerns
//!
//! This module orchestrates the gaming runtime by delegating
//! responsibilities to specialized sub-modules.

pub mod config;
pub mod consensus_coordinator;
pub mod game_lifecycle;
pub mod player_manager;
pub mod statistics;
pub mod treasury_manager;

use crate::error::Result;
use crate::mesh::MeshService;
use crate::protocol::{GameId, PeerId};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

pub use config::GameRuntimeConfig;
pub use consensus_coordinator::ConsensusCoordinator;
pub use game_lifecycle::{ActiveGame, GameCommand, GameLifecycleManager};
pub use player_manager::PlayerManager;
pub use statistics::{GameStats, StatisticsTracker};
pub use treasury_manager::TreasuryManager;

/// Refactored Game Runtime - Orchestrator Pattern
///
/// This runtime delegates responsibilities to specialized managers:
/// - GameLifecycleManager: Handles game creation, joining, leaving
/// - TreasuryManager: Manages treasury and rake collection
/// - PlayerManager: Tracks player balances and sessions
/// - ConsensusCoordinator: Manages consensus engines
/// - StatisticsTracker: Collects and reports statistics
pub struct GameRuntime {
    /// Configuration
    config: Arc<GameRuntimeConfig>,

    /// Specialized managers
    game_manager: Arc<GameLifecycleManager>,
    treasury: Arc<TreasuryManager>,
    player_manager: Arc<PlayerManager>,
    consensus_coordinator: Arc<ConsensusCoordinator>,
    statistics: Arc<StatisticsTracker>,

    /// Local peer identity
    local_peer_id: PeerId,

    /// Event broadcasting
    event_tx: broadcast::Sender<GameEvent>,

    /// Command processing
    command_rx: mpsc::Receiver<GameCommand>,

    /// Runtime state
    is_running: Arc<RwLock<bool>>,
}

/// Events emitted by the gaming runtime
#[derive(Debug, Clone)]
pub enum GameEvent {
    GameCreated {
        game_id: GameId,
        creator: PeerId,
    },
    PlayerJoined {
        game_id: GameId,
        player: PeerId,
    },
    PlayerLeft {
        game_id: GameId,
        player: PeerId,
    },
    BetPlaced {
        game_id: GameId,
        player: PeerId,
        amount: u64,
    },
    DiceRolled {
        game_id: GameId,
        roll: (u8, u8),
    },
    RoundComplete {
        game_id: GameId,
        winners: Vec<(PeerId, u64)>,
    },
    GameEnded {
        game_id: GameId,
        reason: String,
    },
    ConsensusReached {
        game_id: GameId,
    },
    TreasuryUpdate {
        balance: u64,
        rake_collected: u64,
    },
}

impl GameRuntime {
    /// Create a new refactored game runtime
    pub fn new(
        config: GameRuntimeConfig,
        local_peer_id: PeerId,
        mesh_service: Arc<MeshService>,
    ) -> (Self, mpsc::Sender<GameCommand>) {
        let (event_tx, _) = broadcast::channel(1000);
        let (command_tx, command_rx) = mpsc::channel(100);

        let config = Arc::new(config);

        // Initialize specialized managers
        let game_manager = Arc::new(GameLifecycleManager::new(config.clone(), mesh_service));
        let treasury = Arc::new(TreasuryManager::new(config.treasury_rake));
        let player_manager = Arc::new(PlayerManager::new());
        let consensus_coordinator = Arc::new(ConsensusCoordinator::new(
            config.consensus_config.clone(),
            local_peer_id,
        ));
        let statistics = Arc::new(StatisticsTracker::new());

        let runtime = Self {
            config,
            game_manager,
            treasury,
            player_manager,
            consensus_coordinator,
            statistics,
            local_peer_id,
            event_tx,
            command_rx,
            is_running: Arc::new(RwLock::new(false)),
        };

        (runtime, command_tx)
    }

    /// Start the runtime
    pub async fn start(&mut self) -> Result<()> {
        *self.is_running.write().await = true;

        // Start command processor
        let runtime_handle = self.clone_for_task();
        tokio::spawn(async move {
            runtime_handle.process_commands().await;
        });

        // Start background tasks in each manager
        self.game_manager.start_timeout_monitor().await;
        self.consensus_coordinator.start_consensus_processor().await;
        self.statistics.start_metrics_collector().await;

        Ok(())
    }

    /// Process commands
    async fn process_commands(mut self) {
        while let Some(command) = self.command_rx.recv().await {
            if let Err(e) = self.handle_command(command).await {
                log::error!("Command processing error: {:?}", e);
            }
        }
    }

    /// Handle a single command by delegating to appropriate manager
    async fn handle_command(&self, command: GameCommand) -> Result<()> {
        match command {
            GameCommand::CreateGame { creator, config } => {
                let game_id = self.game_manager.create_game(creator, config).await?;

                // Initialize consensus if enabled
                if self.config.enable_consensus {
                    self.consensus_coordinator
                        .initialize_game(game_id, vec![creator])
                        .await?;
                }

                // Update statistics
                self.statistics.record_game_created().await;

                // Emit event
                let _ = self
                    .event_tx
                    .send(GameEvent::GameCreated { game_id, creator });
            }

            GameCommand::JoinGame {
                game_id,
                player,
                buy_in,
            } => {
                // Check and deduct player balance
                self.player_manager.deduct_balance(player, buy_in).await?;

                // Add player to game with security
                use std::net::IpAddr;
                let client_ip = IpAddr::from([127, 0, 0, 1]); // Default localhost for now
                self.game_manager
                    .add_player_to_game_with_security(game_id, player, buy_in, client_ip)
                    .await?;

                // Add to treasury pot
                self.treasury.add_to_pot(game_id, buy_in).await?;

                // Update consensus participants
                if self.config.enable_consensus {
                    self.consensus_coordinator
                        .add_participant(game_id, player)
                        .await?;
                }

                // Emit event
                let _ = self
                    .event_tx
                    .send(GameEvent::PlayerJoined { game_id, player });
            }

            GameCommand::PlaceBet {
                game_id,
                player,
                bet,
            } => {
                // Store bet amount before moving bet
                let bet_amount = bet.amount.amount();

                // Validate with player manager
                self.player_manager.validate_bet(player, bet_amount).await?;

                // Process bet in game
                self.game_manager.process_bet(game_id, player, bet).await?;

                // Update statistics
                self.statistics.record_bet(bet_amount).await;

                // Emit event
                let _ = self.event_tx.send(GameEvent::BetPlaced {
                    game_id,
                    player,
                    amount: bet_amount,
                });
            }

            _ => {
                // Handle other commands similarly
            }
        }

        Ok(())
    }

    /// Clone runtime handle for async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            game_manager: self.game_manager.clone(),
            treasury: self.treasury.clone(),
            player_manager: self.player_manager.clone(),
            consensus_coordinator: self.consensus_coordinator.clone(),
            statistics: self.statistics.clone(),
            local_peer_id: self.local_peer_id,
            event_tx: self.event_tx.clone(),
            command_rx: mpsc::channel(1).1, // Dummy receiver
            is_running: self.is_running.clone(),
        }
    }

    /// Stop the runtime
    pub async fn stop(&mut self) -> Result<()> {
        *self.is_running.write().await = false;

        // Stop all managers
        self.game_manager.stop_all_games().await?;
        self.consensus_coordinator.shutdown().await?;

        Ok(())
    }
}
