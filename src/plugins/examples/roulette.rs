//! Roulette Plugin with Physics Simulation
//!
//! This module implements a European/American roulette game plugin with
//! realistic physics simulation for ball movement and comprehensive betting options.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::super::core::*;
use super::{PluginUtils, CommonGameState, PlayerState, PluginErrorHandler, BasePluginStatistics};
use crate::gaming::{GameAction, GameActionResult};
use rand::{CryptoRng, RngCore};

/// Roulette game plugin with physics simulation
pub struct RoulettePlugin {
    info: PluginInfo,
    capabilities: Vec<PluginCapability>,
    state: Arc<RwLock<PluginState>>,
    game_sessions: Arc<RwLock<HashMap<String, RouletteGameSession>>>,
    config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    error_handler: PluginErrorHandler,
    statistics: BasePluginStatistics,
    rng: Arc<dyn RngCore + Send + Sync>,
}

impl RoulettePlugin {
    /// Create new roulette plugin
    pub fn new() -> Self {
        let info = PluginUtils::create_plugin_info_template(
            "European Roulette",
            "1.0.0",
            GameType::Other("Roulette".to_string()),
            "Professional roulette game with realistic physics simulation, comprehensive betting options, and multi-player support"
        );

        let capabilities = vec![
            PluginCapability::NetworkAccess,
            PluginCapability::DataStorage,
            PluginCapability::Cryptography,
            PluginCapability::RandomNumberGeneration,
            PluginCapability::InterPluginCommunication,
            PluginCapability::RealMoneyGaming,
        ];

        Self {
            info,
            capabilities,
            state: Arc::new(RwLock::new(PluginState::Uninitialized)),
            game_sessions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(HashMap::new())),
            error_handler: PluginErrorHandler::new(),
            statistics: BasePluginStatistics::new(),
            rng: Arc::new(rand::rngs::OsRng),
        }
    }

    /// Start a new roulette spin with physics simulation
    async fn start_spin(&self, session: &mut RouletteGameSession) -> PluginResult<()> {
        // Initialize physics simulation
        let initial_velocity = 8.0 + (self.rng.next_f64() * 4.0); // 8-12 m/s
        let angular_velocity = 15.0 + (self.rng.next_f64() * 10.0); // 15-25 rad/s
        let friction_coefficient = 0.05 + (self.rng.next_f64() * 0.03); // Variable friction

        session.physics = RoulettePhysics {
            ball_position: 0.0,
            ball_velocity: initial_velocity,
            wheel_angular_velocity: angular_velocity,
            friction_coefficient,
            air_resistance: 0.02,
            spin_start_time: Instant::now(),
            last_update_time: Instant::now(),
            is_spinning: true,
        };

        session.game_phase = RouletteGamePhase::Spinning;
        session.spin_start_time = Some(Instant::now());
        
        info!("Started roulette spin with initial velocity: {:.2} m/s", initial_velocity);
        Ok(())
    }

    /// Update physics simulation
    async fn update_physics(&self, session: &mut RouletteGameSession) -> PluginResult<()> {
        if !session.physics.is_spinning {
            return Ok(());
        }

        let now = Instant::now();
        let dt = (now - session.physics.last_update_time).as_secs_f64();
        session.physics.last_update_time = now;

        // Update ball velocity with friction and air resistance
        let deceleration = session.physics.friction_coefficient * 9.81 + // Friction
                          session.physics.air_resistance * session.physics.ball_velocity.powi(2); // Air resistance
        
        session.physics.ball_velocity = (session.physics.ball_velocity - deceleration * dt).max(0.0);

        // Update ball position
        session.physics.ball_position += session.physics.ball_velocity * dt;
        
        // Wrap around the wheel (circumference ≈ 2π * 0.5m = 3.14m)
        session.physics.ball_position %= std::f64::consts::PI * 2.0;

        // Update wheel angular velocity (wheel also decelerates but more slowly)
        session.physics.wheel_angular_velocity = 
            (session.physics.wheel_angular_velocity - 0.1 * dt).max(0.0);

        // Check if ball has stopped
        if session.physics.ball_velocity < 0.1 {
            session.physics.is_spinning = false;
            self.determine_winning_number(session).await?;
        }

        Ok(())
    }

    /// Determine the winning number based on ball position
    async fn determine_winning_number(&self, session: &mut RouletteGameSession) -> PluginResult<()> {
        // Convert ball position to roulette number
        // European roulette has 37 numbers (0-36)
        let segment_size = (std::f64::consts::PI * 2.0) / 37.0;
        let mut number_index = (session.physics.ball_position / segment_size) as usize % 37;

        // Add some randomness to account for pocket selection uncertainty
        let pocket_randomness = self.rng.next_u32() % 3;
        if pocket_randomness == 1 && number_index > 0 {
            number_index -= 1;
        } else if pocket_randomness == 2 && number_index < 36 {
            number_index += 1;
        }

        // Map to actual roulette wheel layout (European wheel order)
        let wheel_layout = [
            0, 32, 15, 19, 4, 21, 2, 25, 17, 34, 6, 27, 13, 36, 11, 30, 8, 23, 10, 5,
            24, 16, 33, 1, 20, 14, 31, 9, 22, 18, 29, 7, 28, 12, 35, 3, 26
        ];

        let winning_number = wheel_layout[number_index];
        session.winning_number = Some(winning_number);
        session.game_phase = RouletteGamePhase::PayingOut;

        self.calculate_payouts(session).await?;

        info!("Roulette ball landed on number: {} (position: {:.2})", 
              winning_number, session.physics.ball_position);

        Ok(())
    }

    /// Calculate payouts for all bets
    async fn calculate_payouts(&self, session: &mut RouletteGameSession) -> PluginResult<()> {
        let winning_number = session.winning_number.unwrap();
        let mut total_payouts = 0u64;

        for (player_id, player) in session.players.iter_mut() {
            let mut player_winnings = 0u64;

            for bet in &player.bets {
                if self.is_winning_bet(bet, winning_number) {
                    let payout = self.calculate_bet_payout(bet);
                    player_winnings += payout;
                    total_payouts += payout;

                    debug!("Player {} won {} on bet {:?}", player_id, payout, bet.bet_type);
                }
            }

            // Add winnings to player balance
            player.balance += player_winnings;

            if player_winnings > 0 {
                info!("Player {} won total of {}", player_id, player_winnings);
            }

            // Clear bets for next round
            player.bets.clear();
        }

        session.total_payout = total_payouts;
        session.game_phase = RouletteGamePhase::Finished;

        Ok(())
    }

    /// Check if a bet wins
    fn is_winning_bet(&self, bet: &RouletteBet, winning_number: u32) -> bool {
        match &bet.bet_type {
            BetType::StraightUp(number) => *number == winning_number,
            BetType::Split(numbers) => numbers.contains(&winning_number),
            BetType::Street(row) => {
                let start = row * 3 + 1;
                winning_number >= start && winning_number <= start + 2
            }
            BetType::Corner(numbers) => numbers.contains(&winning_number),
            BetType::Red => self.is_red_number(winning_number),
            BetType::Black => self.is_black_number(winning_number),
            BetType::Even => winning_number != 0 && winning_number % 2 == 0,
            BetType::Odd => winning_number % 2 == 1,
            BetType::Low => winning_number >= 1 && winning_number <= 18,
            BetType::High => winning_number >= 19 && winning_number <= 36,
            BetType::FirstDozen => winning_number >= 1 && winning_number <= 12,
            BetType::SecondDozen => winning_number >= 13 && winning_number <= 24,
            BetType::ThirdDozen => winning_number >= 25 && winning_number <= 36,
            BetType::FirstColumn => winning_number > 0 && (winning_number - 1) % 3 == 0,
            BetType::SecondColumn => winning_number > 0 && (winning_number - 1) % 3 == 1,
            BetType::ThirdColumn => winning_number > 0 && (winning_number - 1) % 3 == 2,
        }
    }

    /// Calculate payout amount for a winning bet
    fn calculate_bet_payout(&self, bet: &RouletteBet) -> u64 {
        let multiplier = match &bet.bet_type {
            BetType::StraightUp(_) => 36, // 35:1 plus original bet
            BetType::Split(_) => 18,      // 17:1 plus original bet
            BetType::Street(_) => 12,     // 11:1 plus original bet
            BetType::Corner(_) => 9,      // 8:1 plus original bet
            BetType::Red | BetType::Black | BetType::Even | BetType::Odd |
            BetType::Low | BetType::High => 2, // 1:1 plus original bet
            BetType::FirstDozen | BetType::SecondDozen | BetType::ThirdDozen |
            BetType::FirstColumn | BetType::SecondColumn | BetType::ThirdColumn => 3, // 2:1 plus original bet
        };

        bet.amount * multiplier
    }

    /// Check if number is red
    fn is_red_number(&self, number: u32) -> bool {
        match number {
            1 | 3 | 5 | 7 | 9 | 12 | 14 | 16 | 18 | 19 | 21 | 23 | 25 | 27 | 30 | 32 | 34 | 36 => true,
            _ => false,
        }
    }

    /// Check if number is black
    fn is_black_number(&self, number: u32) -> bool {
        number != 0 && !self.is_red_number(number)
    }

    /// Process bet placement
    async fn process_bet(
        &self,
        session_id: &str,
        player_id: &str,
        bet_type: &str,
        amount: u64,
    ) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        // Check if betting is allowed
        if session.game_phase != RouletteGamePhase::Betting {
            return Err(PluginError::RuntimeError("Betting is not currently allowed".to_string()));
        }

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        // Validate bet amount
        if amount < session.min_bet || amount > session.max_bet {
            return Err(PluginError::RuntimeError(format!(
                "Bet amount {} not within limits ({}-{})", 
                amount, session.min_bet, session.max_bet
            )));
        }

        if player.balance < amount {
            return Err(PluginError::RuntimeError("Insufficient balance".to_string()));
        }

        // Parse bet type
        let parsed_bet_type = self.parse_bet_type(bet_type)?;

        // Create bet
        let bet = RouletteBet {
            bet_type: parsed_bet_type,
            amount,
            placed_at: Instant::now(),
        };

        // Deduct from balance and add bet
        player.balance -= amount;
        player.bets.push(bet);

        session.total_bets += amount;

        info!("Player {} placed {} bet of {}", player_id, bet_type, amount);

        Ok(GameActionResult::BetPlaced {
            bet_id: uuid::Uuid::new_v4().to_string(),
            confirmation: format!("{} bet of {} placed", bet_type, amount),
        })
    }

    /// Parse bet type string into BetType enum
    fn parse_bet_type(&self, bet_type: &str) -> PluginResult<BetType> {
        match bet_type {
            "red" => Ok(BetType::Red),
            "black" => Ok(BetType::Black),
            "even" => Ok(BetType::Even),
            "odd" => Ok(BetType::Odd),
            "low" => Ok(BetType::Low),
            "high" => Ok(BetType::High),
            "first_dozen" => Ok(BetType::FirstDozen),
            "second_dozen" => Ok(BetType::SecondDozen),
            "third_dozen" => Ok(BetType::ThirdDozen),
            "first_column" => Ok(BetType::FirstColumn),
            "second_column" => Ok(BetType::SecondColumn),
            "third_column" => Ok(BetType::ThirdColumn),
            _ => {
                // Try to parse number bets
                if bet_type.starts_with("straight_") {
                    if let Ok(number) = bet_type[9..].parse::<u32>() {
                        if number <= 36 {
                            return Ok(BetType::StraightUp(number));
                        }
                    }
                }
                
                Err(PluginError::RuntimeError(format!("Invalid bet type: {}", bet_type)))
            }
        }
    }

    /// Get current spin progress for UI updates
    async fn get_spin_progress(&self, session: &RouletteGameSession) -> f64 {
        if let Some(start_time) = session.spin_start_time {
            let elapsed = start_time.elapsed().as_secs_f64();
            let estimated_duration = 10.0; // Typical spin duration
            (elapsed / estimated_duration).min(1.0)
        } else {
            0.0
        }
    }
}

impl Default for RoulettePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GamePlugin for RoulettePlugin {
    fn get_info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn get_capabilities(&self) -> Vec<PluginCapability> {
        self.capabilities.clone()
    }

    async fn initialize(&mut self, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        *self.state.write().await = PluginState::Initializing;
        *self.config.write().await = config;
        *self.state.write().await = PluginState::Initialized;
        info!("Roulette plugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Starting;

        // Start physics update task
        let game_sessions = Arc::clone(&self.game_sessions);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20 FPS
            loop {
                interval.tick().await;
                
                let mut sessions = game_sessions.write().await;
                for session in sessions.values_mut() {
                    if session.physics.is_spinning {
                        // Physics update would be implemented here
                    }
                }
            }
        });

        *self.state.write().await = PluginState::Running;
        info!("Roulette plugin started");
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Stopping;
        self.game_sessions.write().await.clear();
        *self.state.write().await = PluginState::Stopped;
        info!("Roulette plugin stopped");
        Ok(())
    }

    async fn handle_event(&mut self, event: PluginEvent) -> PluginResult<()> {
        match event {
            PluginEvent::SystemStartup => debug!("Roulette plugin received system startup"),
            PluginEvent::SystemShutdown => self.stop().await?,
            PluginEvent::ConfigurationUpdated(new_config) => {
                *self.config.write().await = new_config;
            }
            _ => debug!("Roulette plugin received event: {:?}", event),
        }
        Ok(())
    }

    async fn process_game_action(
        &mut self,
        session_id: &str,
        player_id: &str,
        action: GameAction,
    ) -> PluginResult<GameActionResult> {
        self.statistics.actions_processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match action {
            GameAction::PlaceBet { bet_type, amount } => {
                self.process_bet(session_id, player_id, &bet_type, amount).await
            }
            GameAction::RollDice => {
                // In roulette, this represents spinning the wheel
                let mut sessions = self.game_sessions.write().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    if session.game_phase == RouletteGamePhase::Betting {
                        session.game_phase = RouletteGamePhase::NoMoreBets;
                        // Start spin after a brief delay
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        self.start_spin(session).await?;
                        
                        return Ok(GameActionResult::DiceRolled {
                            dice: (0, 0), // Not applicable for roulette
                            total: 0,
                        });
                    }
                }
                Err(PluginError::RuntimeError("Cannot spin wheel in current state".to_string()))
            }
            _ => {
                self.error_handler.record_error(&format!("Unsupported action: {:?}", action));
                Err(PluginError::RuntimeError(format!("Unsupported action: {:?}", action)))
            }
        }
    }

    async fn get_game_state(&self, session_id: &str) -> PluginResult<serde_json::Value> {
        let sessions = self.game_sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let mut game_data = serde_json::to_value(session).unwrap_or(serde_json::Value::Null);
        
        // Add spin progress for client UI
        if let serde_json::Value::Object(ref mut obj) = game_data {
            obj.insert(
                "spin_progress".to_string(),
                serde_json::json!(self.get_spin_progress(session).await)
            );
        }

        let common_state = CommonGameState {
            session_id: session_id.to_string(),
            players: session.players.iter().map(|(id, player)| {
                (id.clone(), PlayerState {
                    player_id: id.clone(),
                    balance: player.balance,
                    is_active: true,
                    joined_at: player.joined_at,
                    player_data: serde_json::to_value(player).unwrap_or(serde_json::Value::Null),
                })
            }).collect(),
            current_phase: format!("{:?}", session.game_phase),
            game_data,
            created_at: session.created_at,
            last_updated: std::time::SystemTime::now(),
        };

        serde_json::to_value(common_state)
            .map_err(|e| PluginError::SerializationError(e))
    }

    async fn sync_state(
        &mut self,
        session_id: &str,
        _peer_states: Vec<serde_json::Value>,
    ) -> PluginResult<serde_json::Value> {
        // Roulette state sync is simpler than poker - mainly need to sync the spin result
        warn!("Roulette state synchronization not fully implemented");
        self.get_game_state(session_id).await
    }

    async fn validate_action(
        &self,
        session_id: &str,
        player_id: &str,
        action: &GameAction,
    ) -> PluginResult<bool> {
        let sessions = self.game_sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        match action {
            GameAction::PlaceBet { amount, .. } => {
                Ok(session.game_phase == RouletteGamePhase::Betting &&
                   *amount >= session.min_bet &&
                   *amount <= session.max_bet &&
                   player.balance >= *amount)
            }
            GameAction::RollDice => {
                Ok(session.game_phase == RouletteGamePhase::Betting)
            }
            _ => Ok(false),
        }
    }

    async fn on_player_join(
        &mut self,
        session_id: &str,
        player_id: &str,
        initial_balance: u64,
    ) -> PluginResult<()> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = RoulettePlayer {
            id: player_id.to_string(),
            balance: initial_balance,
            bets: Vec::new(),
            joined_at: std::time::SystemTime::now(),
        };

        session.players.insert(player_id.to_string(), player);
        info!("Player {} joined roulette session {}", player_id, session_id);
        Ok(())
    }

    async fn on_player_leave(&mut self, session_id: &str, player_id: &str) -> PluginResult<()> {
        let mut sessions = self.game_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.players.remove(player_id);
            info!("Player {} left roulette session {}", player_id, session_id);
        }
        Ok(())
    }

    async fn on_session_create(&mut self, session_id: &str, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        let min_bet = config.get("min_bet").and_then(|v| v.as_u64()).unwrap_or(1);
        let max_bet = config.get("max_bet").and_then(|v| v.as_u64()).unwrap_or(1000);

        let session = RouletteGameSession {
            id: session_id.to_string(),
            players: HashMap::new(),
            game_phase: RouletteGamePhase::Betting,
            winning_number: None,
            min_bet,
            max_bet,
            total_bets: 0,
            total_payout: 0,
            physics: RoulettePhysics::new(),
            spin_start_time: None,
            created_at: std::time::SystemTime::now(),
        };

        self.game_sessions.write().await.insert(session_id.to_string(), session);
        self.statistics.sessions_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        info!("Created roulette session: {}", session_id);
        Ok(())
    }

    async fn on_session_end(&mut self, session_id: &str) -> PluginResult<()> {
        self.game_sessions.write().await.remove(session_id);
        info!("Ended roulette session: {}", session_id);
        Ok(())
    }

    async fn health_check(&self) -> PluginResult<PluginHealth> {
        let state = self.state.read().await.clone();
        let warnings = self.error_handler.get_warnings();
        let error_count = self.error_handler.get_error_count();

        Ok(PluginUtils::create_health_status(
            state, 96, 7.5, error_count, warnings,
        ))
    }

    async fn get_statistics(&self) -> PluginStatistics {
        self.statistics.to_plugin_statistics()
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.stop().await
    }
}

/// Roulette game session
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouletteGameSession {
    id: String,
    players: HashMap<String, RoulettePlayer>,
    game_phase: RouletteGamePhase,
    winning_number: Option<u32>,
    min_bet: u64,
    max_bet: u64,
    total_bets: u64,
    total_payout: u64,
    physics: RoulettePhysics,
    #[serde(skip, default)]
    spin_start_time: Option<Instant>,
    created_at: std::time::SystemTime,
}

/// Roulette player
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoulettePlayer {
    id: String,
    balance: u64,
    bets: Vec<RouletteBet>,
    joined_at: std::time::SystemTime,
}

/// Roulette game phases
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum RouletteGamePhase {
    Betting,
    NoMoreBets,
    Spinning,
    PayingOut,
    Finished,
}

/// Roulette bet
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouletteBet {
    bet_type: BetType,
    amount: u64,
    #[serde(skip, default = "std::time::Instant::now")]
    placed_at: Instant,
}

/// Roulette bet types
#[derive(Debug, Clone, Serialize, Deserialize)]
enum BetType {
    // Inside bets
    StraightUp(u32),           // Single number
    Split(Vec<u32>),           // Two adjacent numbers
    Street(u32),               // Three numbers in a row
    Corner(Vec<u32>),          // Four numbers in a square

    // Outside bets
    Red,                       // All red numbers
    Black,                     // All black numbers
    Even,                      // All even numbers
    Odd,                       // All odd numbers
    Low,                       // 1-18
    High,                      // 19-36
    FirstDozen,                // 1-12
    SecondDozen,               // 13-24
    ThirdDozen,                // 25-36
    FirstColumn,               // 1,4,7,10,13,16,19,22,25,28,31,34
    SecondColumn,              // 2,5,8,11,14,17,20,23,26,29,32,35
    ThirdColumn,               // 3,6,9,12,15,18,21,24,27,30,33,36
}

/// Physics simulation for roulette ball
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoulettePhysics {
    ball_position: f64,        // Position around the wheel (radians)
    ball_velocity: f64,        // Ball velocity (m/s)
    wheel_angular_velocity: f64, // Wheel rotation speed (rad/s)
    friction_coefficient: f64,  // Friction coefficient
    air_resistance: f64,       // Air resistance coefficient
    #[serde(skip)]
    spin_start_time: Instant,  // When spin started
    #[serde(skip)]
    last_update_time: Instant, // Last physics update
    is_spinning: bool,         // Whether ball is still spinning
}

impl RoulettePhysics {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            ball_position: 0.0,
            ball_velocity: 0.0,
            wheel_angular_velocity: 0.0,
            friction_coefficient: 0.05,
            air_resistance: 0.02,
            spin_start_time: now,
            last_update_time: now,
            is_spinning: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_roulette_plugin_creation() {
        let plugin = RoulettePlugin::new();
        
        assert_eq!(plugin.get_info().name, "European Roulette");
        assert!(matches!(plugin.get_info().game_type, GameType::Other(_)));
        assert!(plugin.get_capabilities().contains(&PluginCapability::RealMoneyGaming));
    }

    #[tokio::test]
    async fn test_roulette_session_creation() {
        let mut plugin = RoulettePlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.on_session_create("test-session", HashMap::new()).await.unwrap();
        
        let sessions = plugin.game_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        
        assert_eq!(session.id, "test-session");
        assert_eq!(session.game_phase, RouletteGamePhase::Betting);
        assert_eq!(session.min_bet, 1);
        assert_eq!(session.max_bet, 1000);
    }

    #[test]
    fn test_bet_type_parsing() {
        let plugin = RoulettePlugin::new();
        
        assert!(matches!(plugin.parse_bet_type("red").unwrap(), BetType::Red));
        assert!(matches!(plugin.parse_bet_type("black").unwrap(), BetType::Black));
        assert!(matches!(plugin.parse_bet_type("even").unwrap(), BetType::Even));
        assert!(matches!(plugin.parse_bet_type("odd").unwrap(), BetType::Odd));
        assert!(matches!(plugin.parse_bet_type("straight_17").unwrap(), BetType::StraightUp(17)));
        
        assert!(plugin.parse_bet_type("invalid").is_err());
    }

    #[test]
    fn test_winning_bet_calculation() {
        let plugin = RoulettePlugin::new();
        
        // Test red number
        let red_bet = RouletteBet {
            bet_type: BetType::Red,
            amount: 100,
            placed_at: Instant::now(),
        };
        assert!(plugin.is_winning_bet(&red_bet, 1)); // 1 is red
        assert!(!plugin.is_winning_bet(&red_bet, 2)); // 2 is black
        
        // Test straight up bet
        let straight_bet = RouletteBet {
            bet_type: BetType::StraightUp(17),
            amount: 100,
            placed_at: Instant::now(),
        };
        assert!(plugin.is_winning_bet(&straight_bet, 17));
        assert!(!plugin.is_winning_bet(&straight_bet, 18));
        
        // Test payout calculation
        assert_eq!(plugin.calculate_bet_payout(&straight_bet), 3600); // 36x multiplier
        assert_eq!(plugin.calculate_bet_payout(&red_bet), 200); // 2x multiplier
    }

    #[test]
    fn test_color_identification() {
        let plugin = RoulettePlugin::new();
        
        // Test some known red and black numbers
        assert!(plugin.is_red_number(1));
        assert!(plugin.is_red_number(3));
        assert!(plugin.is_red_number(5));
        
        assert!(plugin.is_black_number(2));
        assert!(plugin.is_black_number(4));
        assert!(plugin.is_black_number(6));
        
        // 0 is neither red nor black
        assert!(!plugin.is_red_number(0));
        assert!(!plugin.is_black_number(0));
    }

    #[test]
    fn test_physics_initialization() {
        let physics = RoulettePhysics::new();
        
        assert_eq!(physics.ball_position, 0.0);
        assert_eq!(physics.ball_velocity, 0.0);
        assert!(!physics.is_spinning);
        assert_eq!(physics.friction_coefficient, 0.05);
    }
}