//! Consensus Coordinator
//!
//! Manages consensus engines for active games.

use crate::error::{Error, Result};
use crate::protocol::consensus::{ConsensusConfig, ConsensusEngine, GameOperation};
use crate::protocol::{GameId, PeerId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Coordinates consensus across games
pub struct ConsensusCoordinator {
    /// Consensus configuration
    config: ConsensusConfig,

    /// Local peer ID
    local_peer_id: PeerId,

    /// Consensus engines per game
    engines: Arc<RwLock<HashMap<GameId, ConsensusEngine>>>,

    /// Pending operations per game
    pending_operations: Arc<RwLock<HashMap<GameId, Vec<GameOperation>>>>,
}

impl ConsensusCoordinator {
    /// Create a new consensus coordinator
    pub fn new(config: ConsensusConfig, local_peer_id: PeerId) -> Self {
        Self {
            config,
            local_peer_id,
            engines: Arc::new(RwLock::new(HashMap::new())),
            pending_operations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize consensus for a game
    pub async fn initialize_game(&self, game_id: GameId, participants: Vec<PeerId>) -> Result<()> {
        let engine = ConsensusEngine::new(
            game_id,
            participants,
            self.local_peer_id,
            self.config.clone(),
        )?;

        let mut engines = self.engines.write().await;
        engines.insert(game_id, engine);

        Ok(())
    }

    /// Add a participant to consensus
    pub async fn add_participant(&self, game_id: GameId, participant: PeerId) -> Result<()> {
        let mut engines = self.engines.write().await;
        let engine = engines.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        engine.add_participant(participant)?;
        Ok(())
    }

    /// Remove a participant from consensus
    pub async fn remove_participant(&self, game_id: GameId, participant: PeerId) -> Result<()> {
        let mut engines = self.engines.write().await;
        let engine = engines.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        engine.remove_participant(participant)?;
        Ok(())
    }

    /// Submit an operation for consensus
    pub async fn submit_operation(&self, game_id: GameId, operation: GameOperation) -> Result<()> {
        let mut engines = self.engines.write().await;
        let engine = engines.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        engine.propose_operation(operation)?;
        Ok(())
    }

    /// Queue an operation for later consensus
    pub async fn queue_operation(&self, game_id: GameId, operation: GameOperation) -> Result<()> {
        let mut pending = self.pending_operations.write().await;
        pending
            .entry(game_id)
            .or_insert_with(Vec::new)
            .push(operation);
        Ok(())
    }

    /// Process pending operations for a game
    pub async fn process_pending(&self, game_id: GameId) -> Result<()> {
        let mut pending = self.pending_operations.write().await;
        if let Some(operations) = pending.remove(&game_id) {
            for operation in operations {
                self.submit_operation(game_id, operation).await?;
            }
        }
        Ok(())
    }

    /// Check if consensus is reached for a game
    pub async fn check_consensus(&self, game_id: GameId) -> Result<bool> {
        let engines = self.engines.read().await;
        let engine = engines.get(&game_id).ok_or(Error::GameNotFound)?;

        Ok(engine.has_consensus())
    }

    /// Get consensus state for a game
    pub async fn get_consensus_state(&self, game_id: GameId) -> Result<Vec<u8>> {
        let engines = self.engines.read().await;
        let engine = engines.get(&game_id).ok_or(Error::GameNotFound)?;

        engine.get_consensus_state()
    }

    /// Handle consensus timeout
    pub async fn handle_timeout(&self, game_id: GameId) -> Result<()> {
        let mut engines = self.engines.write().await;
        let engine = engines.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        engine.handle_timeout()?;
        Ok(())
    }

    /// Start consensus processor
    pub async fn start_consensus_processor(&self) {
        let engines = self.engines.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                interval.tick().await;

                // Process consensus for all games
                let engines_map = engines.read().await;
                for (game_id, engine) in engines_map.iter() {
                    if let Err(e) = engine.process_round() {
                        log::error!("Consensus processing error for game {:?}: {:?}", game_id, e);
                    }
                }
            }
        });
    }

    /// Shutdown consensus for a game
    pub async fn shutdown_game(&self, game_id: GameId) -> Result<()> {
        let mut engines = self.engines.write().await;
        engines.remove(&game_id);

        let mut pending = self.pending_operations.write().await;
        pending.remove(&game_id);

        Ok(())
    }

    /// Shutdown all consensus engines
    pub async fn shutdown(&self) -> Result<()> {
        let mut engines = self.engines.write().await;
        engines.clear();

        let mut pending = self.pending_operations.write().await;
        pending.clear();

        Ok(())
    }
}
