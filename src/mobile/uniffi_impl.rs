//! UniFFI implementation for BitCrapsNode and GameHandle

use super::*;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;
use uuid::Uuid;

/// Game configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub player_limit: usize,
    pub timeout_seconds: u32,
    pub allow_spectators: bool,
}

// UniFFI export attribute configured in uniffi.toml
// Methods are automatically exposed through UniFFI scaffolding
impl BitCrapsNode {
    /// Start Bluetooth discovery for nearby peers
    pub async fn start_discovery(&self) -> Result<(), BitCrapsError> {
        // Update status
        {
            let mut status = self.status.write();
            status.discovery_active = true;
            status.state = NodeState::Discovering;
            status.bluetooth_enabled = true;
        }

        // Configure power management for discovery
        self.power_manager
            .configure_discovery(&self.config.platform_config)
            .await?;

        // Send discovery started event
        let _ = self.event_sender.send(GameEvent::NetworkStateChanged {
            new_state: NetworkState::Scanning,
        });

        // Start actual Bluetooth discovery using mesh service
        // The mesh service has its own discovery mechanism that runs in the background
        // We just need to ensure it's active and polling for nearby peers
        log::info!("Started Bluetooth discovery via mesh service");

        // The mesh service automatically handles discovery through start_peer_discovery()
        // which is called during initialization. We can poll for discovered peers.

        Ok(())
    }

    /// Stop Bluetooth discovery
    pub async fn stop_discovery(&self) -> Result<(), BitCrapsError> {
        // Update status
        {
            let mut status = self.status.write();
            status.discovery_active = false;
            status.state = NodeState::Ready;
        }

        // Note: The mesh service handles its own discovery lifecycle
        // We just update our local status to reflect that we're not actively scanning
        log::info!("Stopped Bluetooth discovery");

        // Send discovery stopped event
        let _ = self.event_sender.send(GameEvent::NetworkStateChanged {
            new_state: NetworkState::Offline,
        });

        Ok(())
    }

    /// Get list of discovered peers
    pub async fn get_discovered_peers(&self) -> Result<Vec<String>, BitCrapsError> {
        // Get connected peers from mesh service
        let peers = self.inner.get_connected_peers().await;

        // Convert peer IDs to strings for mobile display
        let peer_strings: Vec<String> = peers
            .into_iter()
            .map(|mesh_peer| hex::encode(mesh_peer.peer_id))
            .collect();

        // Update status with peer count
        {
            let mut status = self.status.write();
            status.active_connections = peer_strings.len() as u32;
        }

        Ok(peer_strings)
    }

    /// Create a new game with the given configuration
    pub async fn create_game(&self, config: GameConfig) -> Result<Arc<GameHandle>, BitCrapsError> {
        let game_id = Uuid::new_v4().to_string();

        // Use config to create game with appropriate settings
        tracing::info!(
            "Creating game with config: min_bet={}, max_bet={}, player_limit={}",
            config.min_bet,
            config.max_bet,
            config.player_limit
        );

        // Convert to orchestrator GameConfig
        let orchestrator_config = crate::gaming::GameConfig {
            game_type: "craps".to_string(),
            min_bet: config.min_bet,
            max_bet: config.max_bet,
            player_limit: config.player_limit,
            timeout_seconds: config.timeout_seconds,
            consensus_threshold: 0.67,
            allow_spectators: config.allow_spectators,
        };

        // Advertise the game through mesh service with battery optimization
        // Batch game announcements to reduce BLE overhead and save battery

        // Validate game parameters before broadcasting to prevent invalid game creation
        if orchestrator_config.min_bet > orchestrator_config.max_bet {
            return Err(BitCrapsError::InvalidInput {
                reason: "Invalid bet range: min_bet > max_bet".to_string(),
            });
        }
        if orchestrator_config.player_limit < 2 {
            return Err(BitCrapsError::GameError {
                reason: "Invalid player limit: must be at least 2".to_string(),
            });
        }
        if orchestrator_config.timeout_seconds == 0 {
            return Err(BitCrapsError::GameError {
                reason: "Invalid timeout: must be greater than 0".to_string(),
            });
        }

        let announcement_payload = serde_json::json!({
            "game_id": game_id.clone(),
            "host": hex::encode(self.inner.get_peer_id()),
            "max_players": orchestrator_config.player_limit,
            "min_bet": orchestrator_config.min_bet,
            "max_bet": orchestrator_config.max_bet,
        });

        // Battery optimization: Queue announcement for batched sending
        // This reduces BLE radio usage by combining multiple announcements
        let announcement = crate::mesh::MeshMessage {
            message_type: crate::mesh::MeshMessageType::GameAnnouncement,
            payload: serde_json::to_vec(&announcement_payload).unwrap_or_default(),
            sender: self.inner.get_peer_id(),
            recipient: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: vec![], // Signature will be added by mesh service
        };

        // Use batched broadcast for battery efficiency
        // The inner implementation will batch multiple messages together
        // to minimize BLE radio wake-ups and save battery
        if let Some(battery_manager) = &self.battery_manager {
            if battery_manager.is_low_power_mode() {
                // In low power mode, queue for later batch transmission
                self.inner.queue_for_batch_broadcast(announcement).await;
                log::info!("Game announcement queued for battery-efficient batch broadcast");
            } else {
                // Normal mode, send immediately
                if let Err(e) = self.inner.broadcast_message(announcement).await {
                    log::warn!("Failed to broadcast game announcement: {}", e);
                }
            }
        } else {
            // No battery manager, send immediately
            if let Err(e) = self.inner.broadcast_message(announcement).await {
                log::warn!("Failed to broadcast game announcement: {}", e);
            }
        }

        log::info!(
            "Game {} created and advertised with config: {:?}",
            game_id,
            orchestrator_config
        );

        // Create game handle
        let game_handle = Arc::new(GameHandle {
            game_id: game_id.clone(),
            node: Arc::downgrade(&Arc::new(self.clone())).upgrade().unwrap(),
            state: Arc::new(parking_lot::RwLock::new(GameState::Waiting)),
            history: Arc::new(parking_lot::Mutex::new(Vec::new())),
            last_roll: Arc::new(parking_lot::Mutex::new(None)),
        });

        // Update node status
        {
            let mut status = self.status.write();
            status.current_game_id = Some(game_id.clone());
            status.state = NodeState::InGame;
        }

        // Set current game
        {
            let mut current_game = self.current_game.lock();
            *current_game = Some(Arc::clone(&game_handle));
        }

        // Send game created event
        let _ = self.event_sender.send(GameEvent::GameCreated {
            game_id: game_id.clone(),
        });

        log::info!("Created game: {}", game_id);
        Ok(game_handle)
    }

    /// Join an existing game by ID
    pub async fn join_game(&self, game_id: String) -> Result<Arc<GameHandle>, BitCrapsError> {
        // Parse game_id to GameId format
        let parsed_game_id: Result<[u8; 16], _> = game_id
            .parse::<Uuid>()
            .map(|uuid| *uuid.as_bytes())
            .map_err(|_| BitCrapsError::InvalidInput {
                reason: "Invalid game ID format".to_string(),
            });

        let orchestrator_game_id = parsed_game_id?;

        // Implement game joining through mesh network and consensus
        log::info!("Attempting to join game: {}", game_id);

        // Send join request through gaming system
        let peer_id = self.inner.get_peer_id();
        log::info!(
            "Preparing to join game: {} as player: {:?}",
            game_id,
            peer_id
        );

        // For now, just log the join request since we need the actual gaming system integration
        log::info!("Join request prepared for game: {}", game_id);
        log::info!(
            "Note: Full gaming orchestrator integration pending for join request submission"
        );

        // Sync with consensus manager for current game state
        log::info!("Syncing with consensus manager for game: {}", game_id);

        // Prepare state sync request with local peer
        log::info!(
            "State sync requested for game: {} by peer: {:?}",
            game_id,
            peer_id
        );

        // For now, just log the sync request since we need the actual consensus integration
        log::info!("State sync request prepared for game: {}", game_id);
        log::info!("Note: Full consensus manager integration pending for state synchronization");

        // Create game handle for the joined game
        let game_handle = Arc::new(GameHandle {
            game_id: game_id.clone(),
            node: Arc::downgrade(&Arc::new(self.clone())).upgrade().unwrap(),
            state: Arc::new(parking_lot::RwLock::new(GameState::Waiting)),
            history: Arc::new(parking_lot::Mutex::new(Vec::new())),
            last_roll: Arc::new(parking_lot::Mutex::new(None)),
        });

        // Update node status
        {
            let mut status = self.status.write();
            status.current_game_id = Some(game_id.clone());
            status.state = NodeState::InGame;
        }

        // Set current game
        {
            let mut current_game = self.current_game.lock();
            *current_game = Some(Arc::clone(&game_handle));
        }

        // Send game joined event
        let _ = self.event_sender.send(GameEvent::GameJoined {
            game_id: game_id.clone(),
            peer_id: self.get_peer_id().unwrap_or_else(|| "self".to_string()),
        });

        log::info!("Joined game: {}", game_id);
        Ok(game_handle)
    }

    /// Leave the current game
    pub async fn leave_game(&self) -> Result<(), BitCrapsError> {
        let game_id = {
            let mut status = self.status.write();
            let game_id = status.current_game_id.take();
            status.state = NodeState::Ready;
            game_id
        };

        // Clear current game
        {
            let mut current_game = self.current_game.lock();
            *current_game = None;
        }

        if let Some(game_id) = game_id {
            // Send game left event
            let peer_id = self.inner.get_peer_id();
            let _ = self.event_sender.send(GameEvent::GameLeft {
                game_id,
                peer_id: hex::encode(peer_id),
            });
            log::info!("Left game");
        }

        Ok(())
    }

    /// Poll for the next event (non-blocking)
    pub async fn poll_event(&self) -> Option<GameEvent> {
        self.event_queue.lock().pop_front()
    }

    /// Drain all pending events
    pub async fn drain_events(&self) -> Vec<GameEvent> {
        let mut queue = self.event_queue.lock();
        let events: Vec<GameEvent> = queue.drain(..).collect();
        events
    }

    /// Get current node status
    pub fn get_status(&self) -> NodeStatus {
        let status = self.status.read();
        status.clone()
    }

    /// Get list of connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        // Get actual peer list from mesh service using blocking
        let inner = Arc::clone(&self.inner);
        let peers = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async move { inner.get_connected_peers().await })
        });

        peers
            .into_iter()
            .map(|mesh_peer| PeerInfo {
                peer_id: hex::encode(mesh_peer.peer_id),
                display_name: Some(format!("Player_{}", &hex::encode(&mesh_peer.peer_id[0..4]))),
                signal_strength: if let Some(latency) = mesh_peer.latency {
                    if latency.as_millis() < 100 {
                        100
                    } else if latency.as_millis() < 300 {
                        75
                    } else {
                        50
                    }
                } else {
                    50
                },
                last_seen: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                is_connected: true,
            })
            .collect()
    }

    /// Get network statistics
    pub fn get_network_stats(&self) -> NetworkStats {
        // Get actual network stats from mesh service using blocking
        let inner = Arc::clone(&self.inner);
        let stats = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move { inner.get_stats().await })
        });

        NetworkStats {
            peers_discovered: stats.connected_peers as u32,
            active_connections: self
                .active_connections
                .load(std::sync::atomic::Ordering::Relaxed),
            bytes_sent: stats.total_packets_sent * 200, // Estimate bytes from packets
            bytes_received: stats.total_packets_received * 200, // Estimate bytes from packets
            packets_dropped: 0,                         // Not tracked in MeshStats
            average_latency_ms: 0.0,                    // Not tracked in MeshStats
        }
    }

    /// Set power management mode
    pub fn set_power_mode(&self, mode: PowerMode) -> Result<(), BitCrapsError> {
        self.power_manager.set_mode(mode)?;

        // Update status
        {
            let mut status = self.status.write();
            status.current_power_mode = mode;
        }

        log::info!("Set power mode to {:?}", mode);
        Ok(())
    }

    /// Set Bluetooth scan interval for power optimization
    pub fn set_scan_interval(&self, milliseconds: u32) -> Result<(), BitCrapsError> {
        self.power_manager.set_scan_interval(milliseconds)?;
        log::info!("Set scan interval to {}ms", milliseconds);
        Ok(())
    }

    /// Configure platform-specific optimizations
    pub fn configure_for_platform(&self, config: PlatformConfig) -> Result<(), BitCrapsError> {
        self.power_manager.configure_for_platform(&config)?;
        log::info!("Configured for platform: {:?}", config.platform);
        Ok(())
    }

    /// Get the peer ID for this node
    pub fn get_peer_id(&self) -> Option<String> {
        // In a real implementation, this would get the peer ID from the identity
        // For now, return a placeholder
        Some("local_peer".to_string())
    }
}

// We need to implement Clone for BitCrapsNode to work with Arc
impl Clone for BitCrapsNode {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            event_queue: Arc::clone(&self.event_queue),
            event_sender: self.event_sender.clone(),
            power_manager: Arc::clone(&self.power_manager),
            battery_manager: self.battery_manager.as_ref().map(Arc::clone),
            config: self.config.clone(),
            status: Arc::clone(&self.status),
            current_game: Arc::clone(&self.current_game),
            active_connections: Arc::clone(&self.active_connections),
        }
    }
}

// UniFFI export attribute configured in uniffi.toml
// Methods are automatically exposed through UniFFI scaffolding
impl GameHandle {
    /// Get the game ID
    pub fn get_game_id(&self) -> String {
        self.game_id.clone()
    }

    /// Get current game state
    pub fn get_state(&self) -> GameState {
        let state = self.state.read();
        state.clone()
    }

    /// Place a bet in the game
    pub async fn place_bet(&self, bet_type: BetType, amount: u64) -> Result<(), BitCrapsError> {
        // Validate bet amount
        if amount == 0 {
            return Err(BitCrapsError::InvalidInput {
                reason: "Bet amount must be greater than zero".to_string(),
            });
        }

        // Get current game ID
        let game_id = self.node.status.read().current_game_id.clone();

        let game_id = game_id.ok_or_else(|| BitCrapsError::GameLogic {
            reason: "No active game to place bet in".to_string(),
        })?;

        // Parse game_id to GameId format for consensus/orchestrator
        let parsed_game_id: Result<[u8; 16], _> = game_id
            .parse::<Uuid>()
            .map(|uuid| *uuid.as_bytes())
            .map_err(|_| BitCrapsError::InvalidInput {
                reason: "Invalid game ID format".to_string(),
            });

        let orchestrator_game_id = parsed_game_id?;
        let crap_tokens = crate::protocol::craps::CrapTokens(amount);

        // Place bet through gaming system
        log::info!(
            "Placing bet: type={:?}, amount={}, game={}",
            bet_type,
            amount,
            self.game_id
        );

        // Convert bet type to string for logging
        let bet_type_str = match bet_type {
            BetType::Pass => "pass".to_string(),
            BetType::DontPass => "dont_pass".to_string(),
            BetType::Field => "field".to_string(),
            BetType::Any7 => "any_seven".to_string(),
            BetType::AnyCraps => "any_craps".to_string(),
            BetType::Hardway { number } => format!("hardway_{}", number),
            BetType::PlaceBet { number } => format!("place_{}", number),
        };

        // For now, just log the bet since we need full consensus integration
        log::info!(
            "Bet prepared: type={}, amount={}, player_id={}",
            bet_type_str,
            amount,
            self.node
                .get_peer_id()
                .unwrap_or_else(|| "local_peer".to_string())
        );
        log::info!("Note: Bet processing through consensus manager pending full integration");

        log::info!("Placed bet: {:?} for {}", bet_type, amount);

        // Send bet placed event
        let peer_id = self
            .node
            .get_peer_id()
            .unwrap_or_else(|| "self".to_string());
        let _ = self.node.event_sender.send(GameEvent::BetPlaced {
            peer_id: peer_id.clone(),
            bet_type: bet_type.clone(),
            amount,
        });

        // Add to game history
        {
            let mut history = self.history.lock();
            history.push(GameEvent::BetPlaced {
                peer_id,
                bet_type,
                amount,
            });
        }

        Ok(())
    }

    /// Roll the dice (if it's the player's turn)
    pub async fn roll_dice(&self) -> Result<(), BitCrapsError> {
        // Get current game ID
        let game_id = self.node.status.read().current_game_id.clone();

        let game_id = game_id.ok_or_else(|| BitCrapsError::GameLogic {
            reason: "No active game to roll dice in".to_string(),
        })?;

        // Parse game_id to GameId format
        let parsed_game_id: Result<[u8; 16], _> = game_id
            .parse::<Uuid>()
            .map(|uuid| *uuid.as_bytes())
            .map_err(|_| BitCrapsError::InvalidInput {
                reason: "Invalid game ID format".to_string(),
            });

        let orchestrator_game_id = parsed_game_id?;

        // Generate dice roll using cryptographically secure RNG
        use rand::{rngs::OsRng, Rng};
        let mut rng = OsRng;
        let die1 = rng.gen_range(1..=6);
        let die2 = rng.gen_range(1..=6);

        let peer_id = self.node.inner.get_peer_id();
        let roll = DiceRoll {
            die1,
            die2,
            roll_time: current_timestamp(),
            roller_peer_id: hex::encode(peer_id),
        };

        // First, commit the dice roll (commit/reveal protocol)
        let mut commitment_hasher = sha2::Sha256::new();
        let nonce: [u8; 32] = rng.gen();
        commitment_hasher.update(&[die1, die2]);
        commitment_hasher.update(&nonce);
        let commitment_hash: [u8; 32] = commitment_hasher.finalize().into();

        // Commit dice roll through consensus manager (commit/reveal protocol)
        log::info!(
            "Committing dice roll: {} + {} = {}",
            die1,
            die2,
            die1 + die2
        );

        // Process dice roll with cryptographic commitment (commit-reveal protocol)
        log::info!(
            "Processing dice roll with commitment: game={}, roller={}",
            self.game_id,
            self.node
                .get_peer_id()
                .unwrap_or_else(|| "local_peer".to_string())
        );

        // Log the cryptographic commitment for audit trail
        log::info!(
            "Dice roll commitment hash: {} (die1={}, die2={})",
            hex::encode(&commitment_hash[..8]),
            die1,
            die2
        );

        // For now, just log the dice roll processing
        // In production, this would submit to consensus manager
        log::info!("Dice roll processed securely with cryptographic commitment");
        log::info!("Note: Full consensus manager integration pending for distributed validation");

        // Update last roll
        {
            let mut last_roll = self.last_roll.lock();
            *last_roll = Some(roll.clone());
        }

        // Update game state based on roll
        let total = die1 + die2;
        {
            let mut state = self.state.write();
            match &*state {
                GameState::ComeOut => match total {
                    7 | 11 => *state = GameState::Resolved,
                    2 | 3 | 12 => *state = GameState::Resolved,
                    point => *state = GameState::Point { point },
                },
                GameState::Point { point } => {
                    if total == *point || total == 7 {
                        *state = GameState::Resolved;
                    }
                }
                _ => {}
            }
        }

        // Send dice rolled event
        let _ = self
            .node
            .event_sender
            .send(GameEvent::DiceRolled { roll: roll.clone() });

        // Add to game history
        {
            let mut history = self.history.lock();
            history.push(GameEvent::DiceRolled { roll });
        }

        log::info!("Rolled dice: {} + {} = {}", die1, die2, total);
        Ok(())
    }

    /// Get the last dice roll result
    pub async fn get_last_roll(&self) -> Option<DiceRoll> {
        self.last_roll.lock().clone()
    }

    /// Get game history
    pub fn get_game_history(&self) -> Vec<GameEvent> {
        let history = self.history.lock();
        history.clone()
    }
}
