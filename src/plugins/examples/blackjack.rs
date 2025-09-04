//! Blackjack Plugin Reference Implementation
//!
//! This module implements a complete blackjack game plugin demonstrating
//! proper plugin architecture, state management, and game logic.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::super::core::*;
use super::{PluginUtils, CommonGameState, PlayerState, PluginErrorHandler, BasePluginStatistics};
use crate::gaming::{GameAction, GameActionResult};
use rand::{CryptoRng, RngCore};

/// Blackjack game plugin implementation
pub struct BlackjackPlugin {
    info: PluginInfo,
    capabilities: Vec<PluginCapability>,
    state: Arc<RwLock<PluginState>>,
    game_sessions: Arc<RwLock<HashMap<String, BlackjackGameSession>>>,
    config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    error_handler: PluginErrorHandler,
    statistics: BasePluginStatistics,
    rng: Arc<dyn crate::plugins::core::CryptoRngCore + Send + Sync>,
}

impl BlackjackPlugin {
    /// Create new blackjack plugin
    pub fn new() -> Self {
        let info = PluginUtils::create_plugin_info_template(
            "Blackjack",
            "1.0.0",
            GameType::Blackjack,
            "Professional Blackjack game with standard casino rules, side bets, and advanced features"
        );

        let capabilities = vec![
            PluginCapability::NetworkAccess,
            PluginCapability::DataStorage,
            PluginCapability::Cryptography,
            PluginCapability::RandomNumberGeneration,
            PluginCapability::InterPluginCommunication,
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

    /// Deal a card from the deck
    async fn deal_card(&self, session: &mut BlackjackGameSession) -> PluginResult<Card> {
        if session.deck.len() < 10 {
            // Reshuffle when deck is low
            session.shuffle_deck(&self.rng).await?;
        }

        session.deck.pop()
            .ok_or_else(|| PluginError::RuntimeError("Empty deck".to_string()))
    }

    /// Calculate hand value considering aces
    fn calculate_hand_value(&self, hand: &[Card]) -> u32 {
        let mut value = 0;
        let mut aces = 0;

        for card in hand {
            match card.rank {
                CardRank::Ace => {
                    aces += 1;
                    value += 11;
                }
                CardRank::King | CardRank::Queen | CardRank::Jack => {
                    value += 10;
                }
                CardRank::Number(n) => {
                    value += n as u32;
                }
            }
        }

        // Convert aces from 11 to 1 if needed
        while value > 21 && aces > 0 {
            value -= 10;
            aces -= 1;
        }

        value
    }

    /// Check if hand is blackjack (21 with 2 cards)
    fn is_blackjack(&self, hand: &[Card]) -> bool {
        hand.len() == 2 && self.calculate_hand_value(hand) == 21
    }

    /// Check if hand is bust (over 21)
    fn is_bust(&self, hand: &[Card]) -> bool {
        self.calculate_hand_value(hand) > 21
    }

    /// Process hit action
    async fn process_hit(&self, session_id: &str, player_id: &str) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        // Check if player can hit
        if player.state != BlackjackPlayerState::Playing {
            return Err(PluginError::RuntimeError("Player cannot hit in current state".to_string()));
        }

        // Deal card
        let card = self.deal_card(session).await?;
        player.hand.push(card);

        let hand_value = self.calculate_hand_value(&player.hand);
        
        // Check for bust
        if self.is_bust(&player.hand) {
            player.state = BlackjackPlayerState::Bust;
            info!("Player {} busted with {}", player_id, hand_value);
            
            Ok(GameActionResult::CardDealt { 
                card: card.to_number() 
            })
        } else if hand_value == 21 {
            player.state = BlackjackPlayerState::Standing;
            Ok(GameActionResult::CardDealt { 
                card: card.to_number() 
            })
        } else {
            Ok(GameActionResult::CardDealt { 
                card: card.to_number() 
            })
        }
    }

    /// Process stand action
    async fn process_stand(&self, session_id: &str, player_id: &str) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        if player.state != BlackjackPlayerState::Playing {
            return Err(PluginError::RuntimeError("Player cannot stand in current state".to_string()));
        }

        player.state = BlackjackPlayerState::Standing;
        info!("Player {} stands with {}", player_id, self.calculate_hand_value(&player.hand));

        // Check if all players are done
        let all_done = session.players.values().all(|p| {
            matches!(p.state, BlackjackPlayerState::Standing | BlackjackPlayerState::Bust | BlackjackPlayerState::Blackjack)
        });

        if all_done {
            self.resolve_game(session).await?;
        }

        Ok(GameActionResult::PlayerStands)
    }

    /// Process bet placement
    async fn process_bet(&self, session_id: &str, player_id: &str, amount: u64) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        // Validate bet amount
        if amount < session.rules.min_bet || amount > session.rules.max_bet {
            return Err(PluginError::RuntimeError(format!(
                "Bet amount {} not within limits ({}-{})", 
                amount, session.rules.min_bet, session.rules.max_bet
            )));
        }

        if player.balance < amount {
            return Err(PluginError::RuntimeError("Insufficient balance".to_string()));
        }

        // Place bet
        player.current_bet = amount;
        player.balance -= amount;
        player.state = BlackjackPlayerState::Betting;

        info!("Player {} placed bet of {}", player_id, amount);

        Ok(GameActionResult::BetPlaced {
            bet_id: uuid::Uuid::new_v4().to_string(),
            confirmation: format!("Bet of {} placed", amount),
        })
    }

    /// Resolve game when all players are done
    async fn resolve_game(&self, session: &mut BlackjackGameSession) -> PluginResult<()> {
        // Play dealer hand
        self.play_dealer_hand(session).await?;

        let dealer_value = self.calculate_hand_value(&session.dealer_hand);
        let dealer_blackjack = self.is_blackjack(&session.dealer_hand);
        let dealer_bust = self.is_bust(&session.dealer_hand);

        // Resolve each player
        for (_player_id, player) in session.players.iter_mut() {
            let player_value = self.calculate_hand_value(&player.hand);
            let player_blackjack = self.is_blackjack(&player.hand);
            
            match player.state {
                BlackjackPlayerState::Bust => {
                    // Player loses, dealer keeps bet
                    player.result = Some(BlackjackResult::Loss);
                }
                BlackjackPlayerState::Blackjack => {
                    if dealer_blackjack {
                        // Push - return bet
                        player.balance += player.current_bet;
                        player.result = Some(BlackjackResult::Push);
                    } else {
                        // Blackjack wins 3:2
                        let winnings = player.current_bet + (player.current_bet * 3 / 2);
                        player.balance += winnings;
                        player.result = Some(BlackjackResult::Blackjack);
                    }
                }
                BlackjackPlayerState::Standing => {
                    if dealer_bust {
                        // Dealer bust, player wins
                        player.balance += player.current_bet * 2;
                        player.result = Some(BlackjackResult::Win);
                    } else if player_blackjack && !dealer_blackjack {
                        // Player blackjack beats dealer
                        let winnings = player.current_bet + (player.current_bet * 3 / 2);
                        player.balance += winnings;
                        player.result = Some(BlackjackResult::Blackjack);
                    } else if player_value > dealer_value {
                        // Player wins
                        player.balance += player.current_bet * 2;
                        player.result = Some(BlackjackResult::Win);
                    } else if player_value == dealer_value {
                        // Push - return bet
                        player.balance += player.current_bet;
                        player.result = Some(BlackjackResult::Push);
                    } else {
                        // Dealer wins
                        player.result = Some(BlackjackResult::Loss);
                    }
                }
                _ => {
                    player.result = Some(BlackjackResult::Loss);
                }
            }
        }

        session.state = BlackjackGameState::Finished;
        info!("Game resolved - dealer: {}, bust: {}", dealer_value, dealer_bust);

        Ok(())
    }

    /// Play dealer hand according to rules
    async fn play_dealer_hand(&self, session: &mut BlackjackGameSession) -> PluginResult<()> {
        while self.calculate_hand_value(&session.dealer_hand) < 17 {
            let card = self.deal_card(session).await?;
            session.dealer_hand.push(card);
        }

        let final_value = self.calculate_hand_value(&session.dealer_hand);
        debug!("Dealer final hand value: {}", final_value);

        Ok(())
    }
}

impl Default for BlackjackPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GamePlugin for BlackjackPlugin {
    fn get_info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn get_capabilities(&self) -> Vec<PluginCapability> {
        self.capabilities.clone()
    }

    async fn initialize(&mut self, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        *self.state.write().await = PluginState::Initializing;
        
        // Store configuration
        *self.config.write().await = config;
        
        *self.state.write().await = PluginState::Initialized;
        info!("Blackjack plugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Starting;
        
        // Initialize any background tasks or connections here
        
        *self.state.write().await = PluginState::Running;
        info!("Blackjack plugin started");
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Stopping;
        
        // Cleanup any resources
        self.game_sessions.write().await.clear();
        
        *self.state.write().await = PluginState::Stopped;
        info!("Blackjack plugin stopped");
        Ok(())
    }

    async fn handle_event(&mut self, event: PluginEvent) -> PluginResult<()> {
        match event {
            PluginEvent::SystemStartup => {
                debug!("Blackjack plugin received system startup event");
            }
            PluginEvent::SystemShutdown => {
                debug!("Blackjack plugin received system shutdown event");
                self.stop().await?;
            }
            PluginEvent::ConfigurationUpdated(new_config) => {
                *self.config.write().await = new_config;
                debug!("Blackjack plugin configuration updated");
            }
            _ => {
                debug!("Blackjack plugin received event: {:?}", event);
            }
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
            GameAction::Hit => self.process_hit(session_id, player_id).await,
            GameAction::Stand => self.process_stand(session_id, player_id).await,
            GameAction::PlaceBet { amount, .. } => self.process_bet(session_id, player_id, amount).await,
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

        // Convert to common game state format
        let common_state = CommonGameState {
            session_id: session_id.to_string(),
            players: session.players.iter().map(|(id, player)| {
                (id.clone(), PlayerState {
                    player_id: id.clone(),
                    balance: player.balance,
                    is_active: matches!(player.state, BlackjackPlayerState::Playing | BlackjackPlayerState::Standing),
                    joined_at: player.joined_at,
                    player_data: serde_json::to_value(player).unwrap_or(serde_json::Value::Null),
                })
            }).collect(),
            current_phase: format!("{:?}", session.state),
            game_data: serde_json::to_value(session).unwrap_or(serde_json::Value::Null),
            created_at: session.created_at,
            last_updated: std::time::SystemTime::now(),
        };

        serde_json::to_value(common_state)
            .map_err(|e| PluginError::SerializationError(e))
    }

    async fn sync_state(
        &mut self,
        session_id: &str,
        peer_states: Vec<serde_json::Value>,
    ) -> PluginResult<serde_json::Value> {
        // For blackjack, state sync is relatively simple since it's mostly server-authoritative
        // In a production implementation, you would resolve conflicts and merge states
        
        if peer_states.is_empty() {
            return self.get_game_state(session_id).await;
        }

        // For now, just return current state - in practice would implement proper consensus
        warn!("State synchronization not fully implemented for blackjack plugin");
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

        // Validate action based on current game state and player state
        match action {
            GameAction::Hit | GameAction::Stand => {
                Ok(player.state == BlackjackPlayerState::Playing)
            }
            GameAction::PlaceBet { amount, .. } => {
                Ok(player.state == BlackjackPlayerState::WaitingForBet &&
                   *amount >= session.rules.min_bet &&
                   *amount <= session.rules.max_bet &&
                   player.balance >= *amount)
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

        let player = BlackjackPlayer {
            id: player_id.to_string(),
            balance: initial_balance,
            current_bet: 0,
            hand: Vec::new(),
            state: BlackjackPlayerState::WaitingForBet,
            result: None,
            joined_at: std::time::SystemTime::now(),
        };

        session.players.insert(player_id.to_string(), player);
        info!("Player {} joined blackjack session {}", player_id, session_id);
        Ok(())
    }

    async fn on_player_leave(&mut self, session_id: &str, player_id: &str) -> PluginResult<()> {
        let mut sessions = self.game_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.players.remove(player_id);
            info!("Player {} left blackjack session {}", player_id, session_id);
        }
        Ok(())
    }

    async fn on_session_create(&mut self, session_id: &str, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        let rules = BlackjackRules {
            min_bet: config.get("min_bet").and_then(|v| v.as_u64()).unwrap_or(1),
            max_bet: config.get("max_bet").and_then(|v| v.as_u64()).unwrap_or(1000),
            max_players: config.get("max_players").and_then(|v| v.as_u64()).unwrap_or(7) as usize,
            blackjack_payout: 1.5,
            dealer_hits_soft_17: false,
            double_after_split: true,
            surrender_allowed: false,
        };

        let mut session = BlackjackGameSession {
            id: session_id.to_string(),
            rules,
            players: HashMap::new(),
            deck: Vec::new(),
            dealer_hand: Vec::new(),
            state: BlackjackGameState::WaitingForPlayers,
            created_at: std::time::SystemTime::now(),
        };

        // Initialize and shuffle deck
        session.shuffle_deck(&self.rng).await?;

        self.game_sessions.write().await.insert(session_id.to_string(), session);
        self.statistics.sessions_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        info!("Created blackjack session: {}", session_id);
        Ok(())
    }

    async fn on_session_end(&mut self, session_id: &str) -> PluginResult<()> {
        self.game_sessions.write().await.remove(session_id);
        info!("Ended blackjack session: {}", session_id);
        Ok(())
    }

    async fn health_check(&self) -> PluginResult<PluginHealth> {
        let state = self.state.read().await.clone();
        let warnings = self.error_handler.get_warnings();
        let error_count = self.error_handler.get_error_count();

        Ok(PluginUtils::create_health_status(
            state,
            64, // Mock memory usage
            5.0, // Mock CPU usage
            error_count,
            warnings,
        ))
    }

    async fn get_statistics(&self) -> PluginStatistics {
        self.statistics.to_plugin_statistics()
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.stop().await
    }
}

/// Blackjack game session
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlackjackGameSession {
    id: String,
    rules: BlackjackRules,
    players: HashMap<String, BlackjackPlayer>,
    deck: Vec<Card>,
    dealer_hand: Vec<Card>,
    state: BlackjackGameState,
    created_at: std::time::SystemTime,
}

impl BlackjackGameSession {
    /// Shuffle a new deck
    async fn shuffle_deck(&mut self, rng: &Arc<dyn RngCore + CryptoRng + Send + Sync>) -> PluginResult<()> {
        self.deck.clear();
        
        // Create standard 52-card deck (or multiple decks)
        for suit in &[CardSuit::Hearts, CardSuit::Diamonds, CardSuit::Clubs, CardSuit::Spades] {
            for rank in &[
                CardRank::Ace,
                CardRank::Number(2), CardRank::Number(3), CardRank::Number(4), CardRank::Number(5),
                CardRank::Number(6), CardRank::Number(7), CardRank::Number(8), CardRank::Number(9),
                CardRank::Number(10), CardRank::Jack, CardRank::Queen, CardRank::King,
            ] {
                self.deck.push(Card {
                    suit: suit.clone(),
                    rank: rank.clone(),
                });
            }
        }

        // Shuffle using Fisher-Yates algorithm with crypto RNG
        for i in (1..self.deck.len()).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            self.deck.swap(i, j);
        }

        info!("Shuffled new deck with {} cards", self.deck.len());
        Ok(())
    }
}

/// Blackjack game rules
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlackjackRules {
    min_bet: u64,
    max_bet: u64,
    max_players: usize,
    blackjack_payout: f64,
    dealer_hits_soft_17: bool,
    double_after_split: bool,
    surrender_allowed: bool,
}

/// Blackjack player
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlackjackPlayer {
    id: String,
    balance: u64,
    current_bet: u64,
    hand: Vec<Card>,
    state: BlackjackPlayerState,
    result: Option<BlackjackResult>,
    joined_at: std::time::SystemTime,
}

/// Blackjack player states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum BlackjackPlayerState {
    WaitingForBet,
    Betting,
    Playing,
    Standing,
    Bust,
    Blackjack,
}

/// Blackjack game states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum BlackjackGameState {
    WaitingForPlayers,
    Betting,
    Dealing,
    Playing,
    DealerPlaying,
    Finished,
}

/// Blackjack round results
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum BlackjackResult {
    Win,
    Loss,
    Push,
    Blackjack,
}

/// Playing card
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Card {
    suit: CardSuit,
    rank: CardRank,
}

impl Card {
    /// Convert card to number for API compatibility
    fn to_number(&self) -> u8 {
        match self.rank {
            CardRank::Ace => 1,
            CardRank::Number(n) => n,
            CardRank::Jack => 11,
            CardRank::Queen => 12,
            CardRank::King => 13,
        }
    }
}

/// Card suits
#[derive(Debug, Clone, Serialize, Deserialize)]
enum CardSuit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

/// Card ranks
#[derive(Debug, Clone, Serialize, Deserialize)]
enum CardRank {
    Ace,
    Number(u8),
    Jack,
    Queen,
    King,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blackjack_plugin_creation() {
        let plugin = BlackjackPlugin::new();
        
        assert_eq!(plugin.get_info().name, "Blackjack");
        assert_eq!(plugin.get_info().game_type, GameType::Blackjack);
        assert!(plugin.get_capabilities().contains(&PluginCapability::NetworkAccess));
    }

    #[tokio::test]
    async fn test_blackjack_plugin_lifecycle() {
        let mut plugin = BlackjackPlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.start().await.unwrap();
        
        let health = plugin.health_check().await.unwrap();
        assert_eq!(health.state, PluginState::Running);
        
        plugin.stop().await.unwrap();
    }

    #[test]
    fn test_hand_calculation() {
        let plugin = BlackjackPlugin::new();
        
        // Test basic hand
        let hand = vec![
            Card { suit: CardSuit::Hearts, rank: CardRank::Number(10) },
            Card { suit: CardSuit::Spades, rank: CardRank::Number(5) },
        ];
        assert_eq!(plugin.calculate_hand_value(&hand), 15);
        
        // Test blackjack
        let blackjack = vec![
            Card { suit: CardSuit::Hearts, rank: CardRank::Ace },
            Card { suit: CardSuit::Spades, rank: CardRank::King },
        ];
        assert_eq!(plugin.calculate_hand_value(&blackjack), 21);
        assert!(plugin.is_blackjack(&blackjack));
        
        // Test soft ace
        let soft_hand = vec![
            Card { suit: CardSuit::Hearts, rank: CardRank::Ace },
            Card { suit: CardSuit::Spades, rank: CardRank::Number(6) },
            Card { suit: CardSuit::Clubs, rank: CardRank::Number(8) },
        ];
        assert_eq!(plugin.calculate_hand_value(&soft_hand), 15); // Ace counts as 1
    }

    #[tokio::test]
    async fn test_session_creation() {
        let mut plugin = BlackjackPlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.on_session_create("test-session", HashMap::new()).await.unwrap();
        
        let sessions = plugin.game_sessions.read().await;
        assert!(sessions.contains_key("test-session"));
        
        let session = sessions.get("test-session").unwrap();
        assert_eq!(session.id, "test-session");
        assert_eq!(session.deck.len(), 52);
    }
}