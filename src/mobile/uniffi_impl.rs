//! UniFFI implementation for BitCrapsNode and GameHandle

use super::*;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

/// Game configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub player_limit: usize,
    pub timeout_seconds: u32,
    pub allow_spectators: bool,
}

// TODO: Fix UniFFI configuration issues
//#[uniffi::export]
impl BitCrapsNode {
    /// Start Bluetooth discovery for nearby peers
    pub async fn start_discovery(&self) -> Result<(), BitCrapsError> {
        // Update status
        if let Ok(mut status) = self.status.lock() {
            status.discovery_active = true;
            status.state = NodeState::Discovering;
            status.bluetooth_enabled = true;
        }

        // Configure power management for discovery
        self.power_manager.configure_discovery(&self.config.platform_config).await?;

        // Send discovery started event
        let _ = self.event_sender.send(GameEvent::NetworkStateChanged { 
            new_state: NetworkState::Scanning 
        });

        // TODO: Start actual Bluetooth discovery using mesh service
        log::info!("Started Bluetooth discovery");
        
        Ok(())
    }

    /// Stop Bluetooth discovery
    pub async fn stop_discovery(&self) -> Result<(), BitCrapsError> {
        // Update status
        if let Ok(mut status) = self.status.lock() {
            status.discovery_active = false;
            status.state = NodeState::Ready;
        }

        // TODO: Stop actual Bluetooth discovery
        log::info!("Stopped Bluetooth discovery");

        // Send discovery stopped event
        let _ = self.event_sender.send(GameEvent::NetworkStateChanged { 
            new_state: NetworkState::Offline 
        });

        Ok(())
    }

    /// Create a new game with the given configuration
    pub async fn create_game(&self, config: GameConfig) -> Result<Arc<GameHandle>, BitCrapsError> {
        let game_id = Uuid::new_v4().to_string();
        
        // Use config to create game with appropriate settings
        tracing::info!("Creating game with config: min_bet={}, max_bet={}, player_limit={}", 
                      config.min_bet, config.max_bet, config.player_limit);
        
        // Create game handle
        let game_handle = Arc::new(GameHandle {
            game_id: game_id.clone(),
            node: Arc::downgrade(&Arc::new(self.clone())).upgrade().unwrap(),
            state: Arc::new(Mutex::new(GameState::Waiting)),
            history: Arc::new(Mutex::new(Vec::new())),
            last_roll: Arc::new(Mutex::new(None)),
        });

        // Update node status
        if let Ok(mut status) = self.status.lock() {
            status.current_game_id = Some(game_id.clone());
            status.state = NodeState::InGame;
        }

        // Set current game
        if let Ok(mut current_game) = self.current_game.lock() {
            *current_game = Some(Arc::clone(&game_handle));
        }

        // Send game created event
        let _ = self.event_sender.send(GameEvent::GameCreated { game_id: game_id.clone() });

        log::info!("Created game: {}", game_id);
        Ok(game_handle)
    }

    /// Join an existing game by ID
    pub async fn join_game(&self, game_id: String) -> Result<Arc<GameHandle>, BitCrapsError> {
        // TODO: Implement actual game joining logic
        // For now, create a game handle for the existing game
        let game_handle = Arc::new(GameHandle {
            game_id: game_id.clone(),
            node: Arc::downgrade(&Arc::new(self.clone())).upgrade().unwrap(),
            state: Arc::new(Mutex::new(GameState::Waiting)),
            history: Arc::new(Mutex::new(Vec::new())),
            last_roll: Arc::new(Mutex::new(None)),
        });

        // Update node status
        if let Ok(mut status) = self.status.lock() {
            status.current_game_id = Some(game_id.clone());
            status.state = NodeState::InGame;
        }

        // Set current game
        if let Ok(mut current_game) = self.current_game.lock() {
            *current_game = Some(Arc::clone(&game_handle));
        }

        // Send game joined event
        let _ = self.event_sender.send(GameEvent::GameJoined { 
            game_id: game_id.clone(), 
            peer_id: "self".to_string() // TODO: Use actual peer ID
        });

        log::info!("Joined game: {}", game_id);
        Ok(game_handle)
    }

    /// Leave the current game
    pub async fn leave_game(&self) -> Result<(), BitCrapsError> {
        let game_id = if let Ok(mut status) = self.status.lock() {
            let game_id = status.current_game_id.take();
            status.state = NodeState::Ready;
            game_id
        } else {
            None
        };

        // Clear current game
        if let Ok(mut current_game) = self.current_game.lock() {
            *current_game = None;
        }

        if let Some(game_id) = game_id {
            // Send game left event
            let _ = self.event_sender.send(GameEvent::GameLeft { 
                game_id, 
                peer_id: "self".to_string() // TODO: Use actual peer ID
            });
            log::info!("Left game");
        }

        Ok(())
    }

    /// Poll for the next event (non-blocking)
    pub async fn poll_event(&self) -> Option<GameEvent> {
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.pop_front()
        } else {
            None
        }
    }

    /// Drain all pending events
    pub async fn drain_events(&self) -> Vec<GameEvent> {
        if let Ok(mut queue) = self.event_queue.lock() {
            let events: Vec<GameEvent> = queue.drain(..).collect();
            events
        } else {
            Vec::new()
        }
    }

    /// Get current node status
    pub fn get_status(&self) -> NodeStatus {
        if let Ok(status) = self.status.lock() {
            status.clone()
        } else {
            NodeStatus {
                state: NodeState::Error { reason: "Failed to get status".to_string() },
                bluetooth_enabled: false,
                discovery_active: false,
                current_game_id: None,
                active_connections: 0,
                current_power_mode: PowerMode::Balanced,
            }
        }
    }

    /// Get list of connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        // TODO: Get actual peer list from mesh service
        vec![]
    }

    /// Get network statistics
    pub fn get_network_stats(&self) -> NetworkStats {
        // TODO: Get actual network stats
        NetworkStats {
            peers_discovered: 0,
            active_connections: 0,
            bytes_sent: 0,
            bytes_received: 0,
            packets_dropped: 0,
            average_latency_ms: 0.0,
        }
    }

    /// Set power management mode
    pub fn set_power_mode(&self, mode: PowerMode) -> Result<(), BitCrapsError> {
        self.power_manager.set_mode(mode)?;
        
        // Update status
        if let Ok(mut status) = self.status.lock() {
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
}

// We need to implement Clone for BitCrapsNode to work with Arc
impl Clone for BitCrapsNode {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            event_queue: Arc::clone(&self.event_queue),
            event_sender: self.event_sender.clone(),
            power_manager: Arc::clone(&self.power_manager),
            config: self.config.clone(),
            status: Arc::clone(&self.status),
            current_game: Arc::clone(&self.current_game),
        }
    }
}

// TODO: Fix UniFFI configuration issues
//#[uniffi::export]
impl GameHandle {
    /// Get the game ID
    pub fn get_game_id(&self) -> String {
        self.game_id.clone()
    }

    /// Get current game state
    pub fn get_state(&self) -> GameState {
        if let Ok(state) = self.state.lock() {
            state.clone()
        } else {
            GameState::Error { reason: "Failed to get state".to_string() }
        }
    }

    /// Place a bet in the game
    pub async fn place_bet(&self, bet_type: BetType, amount: u64) -> Result<(), BitCrapsError> {
        // Validate bet amount
        // TODO: Get actual game config for validation
        if amount == 0 {
            return Err(BitCrapsError::InvalidInput { 
                reason: "Bet amount must be greater than zero".to_string() 
            });
        }

        // TODO: Implement actual bet placement logic
        log::info!("Placed bet: {:?} for {}", bet_type, amount);

        // Send bet placed event
        let _ = self.node.event_sender.send(GameEvent::BetPlaced {
            peer_id: "self".to_string(), // TODO: Use actual peer ID
            bet_type,
            amount,
        });

        // Add to game history
        if let Ok(mut history) = self.history.lock() {
            history.push(GameEvent::BetPlaced {
                peer_id: "self".to_string(),
                bet_type: bet_type,
                amount,
            });
        }

        Ok(())
    }

    /// Roll the dice (if it's the player's turn)
    pub async fn roll_dice(&self) -> Result<(), BitCrapsError> {
        // TODO: Implement proper turn management
        
        // Simulate dice roll using cryptographically secure RNG
        use rand::{Rng, rngs::OsRng};
        let mut rng = OsRng;
        let die1 = rng.gen_range(1..=6);
        let die2 = rng.gen_range(1..=6);
        
        let roll = DiceRoll {
            die1,
            die2,
            roll_time: current_timestamp(),
            roller_peer_id: "self".to_string(), // TODO: Use actual peer ID
        };

        // Update last roll
        if let Ok(mut last_roll) = self.last_roll.lock() {
            *last_roll = Some(roll.clone());
        }

        // Update game state based on roll
        let total = die1 + die2;
        if let Ok(mut state) = self.state.lock() {
            match &*state {
                GameState::ComeOut => {
                    match total {
                        7 | 11 => *state = GameState::Resolved,
                        2 | 3 | 12 => *state = GameState::Resolved,
                        point => *state = GameState::Point { point },
                    }
                },
                GameState::Point { point } => {
                    if total == *point || total == 7 {
                        *state = GameState::Resolved;
                    }
                },
                _ => {},
            }
        }

        // Send dice rolled event
        let _ = self.node.event_sender.send(GameEvent::DiceRolled { roll: roll.clone() });

        // Add to game history
        if let Ok(mut history) = self.history.lock() {
            history.push(GameEvent::DiceRolled { roll });
        }

        log::info!("Rolled dice: {} + {} = {}", die1, die2, total);
        Ok(())
    }

    /// Get the last dice roll result
    pub async fn get_last_roll(&self) -> Option<DiceRoll> {
        if let Ok(last_roll) = self.last_roll.lock() {
            last_roll.clone()
        } else {
            None
        }
    }

    /// Get game history
    pub fn get_game_history(&self) -> Vec<GameEvent> {
        if let Ok(history) = self.history.lock() {
            history.clone()
        } else {
            vec![]
        }
    }
}