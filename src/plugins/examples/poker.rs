//! Poker Plugin with Multi-Player Support
//!
//! This module implements a Texas Hold'em poker game plugin with
//! comprehensive multi-player support, betting rounds, and tournament features.

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

/// Texas Hold'em Poker plugin implementation
pub struct PokerPlugin {
    info: PluginInfo,
    capabilities: Vec<PluginCapability>,
    state: Arc<RwLock<PluginState>>,
    game_sessions: Arc<RwLock<HashMap<String, PokerGameSession>>>,
    config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    error_handler: PluginErrorHandler,
    statistics: BasePluginStatistics,
    rng: Arc<dyn RngCore + Send + Sync>,
}

impl PokerPlugin {
    /// Create new poker plugin
    pub fn new() -> Self {
        let info = PluginUtils::create_plugin_info_template(
            "Texas Hold'em Poker",
            "1.0.0",
            GameType::Poker,
            "Professional Texas Hold'em poker with multi-player support, tournaments, and advanced betting mechanics"
        );

        let mut capabilities = vec![
            PluginCapability::NetworkAccess,
            PluginCapability::DataStorage,
            PluginCapability::Cryptography,
            PluginCapability::RandomNumberGeneration,
            PluginCapability::InterPluginCommunication,
        ];
        
        // Poker often involves real money, so include capability
        capabilities.push(PluginCapability::RealMoneyGaming);

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

    /// Deal cards to players
    async fn deal_cards(&self, session: &mut PokerGameSession) -> PluginResult<()> {
        // Clear existing hands
        for player in session.players.values_mut() {
            player.hand.clear();
        }
        session.community_cards.clear();

        // Deal 2 cards to each player
        let active_players: Vec<String> = session.players
            .iter()
            .filter(|(_, p)| p.is_active && !p.is_folded)
            .map(|(id, _)| id.clone())
            .collect();

        for _ in 0..2 {
            for player_id in &active_players {
                let card = self.deal_card(session).await?;
                if let Some(player) = session.players.get_mut(player_id) {
                    player.hand.push(card);
                }
            }
        }

        info!("Dealt cards to {} players in session {}", active_players.len(), session.id);
        Ok(())
    }

    /// Deal a single card from the deck
    async fn deal_card(&self, session: &mut PokerGameSession) -> PluginResult<PokerCard> {
        if session.deck.len() < 10 {
            return Err(PluginError::RuntimeError("Insufficient cards in deck".to_string()));
        }

        session.deck.pop()
            .ok_or_else(|| PluginError::RuntimeError("Empty deck".to_string()))
    }

    /// Deal community cards (flop, turn, river)
    async fn deal_community_cards(&self, session: &mut PokerGameSession, count: usize) -> PluginResult<()> {
        for _ in 0..count {
            let card = self.deal_card(session).await?;
            session.community_cards.push(card);
        }
        
        info!("Dealt {} community cards, total: {}", count, session.community_cards.len());
        Ok(())
    }

    /// Process bet/raise action
    async fn process_bet(&self, session_id: &str, player_id: &str, amount: u64) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        // Validate it's player's turn
        if session.current_player != Some(player_id.to_string()) {
            return Err(PluginError::RuntimeError("Not player's turn".to_string()));
        }

        // Validate player state
        if player.is_folded || !player.is_active {
            return Err(PluginError::RuntimeError("Player cannot bet in current state".to_string()));
        }

        // Calculate required amount
        let current_bet = session.current_bet;
        let player_bet_this_round = session.get_player_bet_this_round(player_id);
        let required_to_call = current_bet.saturating_sub(player_bet_this_round);

        // Validate bet amount
        if amount < required_to_call {
            return Err(PluginError::RuntimeError(
                format!("Bet {} is less than required call amount {}", amount, required_to_call)
            ));
        }

        if player.chips < amount {
            return Err(PluginError::RuntimeError("Insufficient chips".to_string()));
        }

        // Process bet
        player.chips -= amount;
        player.total_bet_this_hand += amount;
        session.pot += amount;
        
        // Update betting round state
        session.add_bet(player_id, amount);

        // Determine action type
        let action_type = if amount == required_to_call {
            if required_to_call == 0 {
                "check"
            } else {
                "call"
            }
        } else {
            "raise"
        };

        // Update current bet if this is a raise
        if amount > required_to_call {
            session.current_bet = player_bet_this_round + amount;
            session.last_raiser = Some(player_id.to_string());
        }

        self.advance_to_next_player(session).await?;

        info!("Player {} {} {}", player_id, action_type, amount);

        Ok(GameActionResult::BetPlaced {
            bet_id: uuid::Uuid::new_v4().to_string(),
            confirmation: format!("{} {}", action_type, amount),
        })
    }

    /// Process fold action
    async fn process_fold(&self, session_id: &str, player_id: &str) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        if session.current_player != Some(player_id.to_string()) {
            return Err(PluginError::RuntimeError("Not player's turn".to_string()));
        }

        player.is_folded = true;
        session.folded_players.insert(player_id.to_string());

        self.advance_to_next_player(session).await?;

        info!("Player {} folded", player_id);

        Ok(GameActionResult::PlayerFolds)
    }

    /// Process check action
    async fn process_check(&self, session_id: &str, player_id: &str) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        // Can only check if no bet has been made
        if session.current_bet > 0 {
            return Err(PluginError::RuntimeError("Cannot check when there is a bet".to_string()));
        }

        if session.current_player != Some(player_id.to_string()) {
            return Err(PluginError::RuntimeError("Not player's turn".to_string()));
        }

        self.advance_to_next_player(session).await?;

        info!("Player {} checks", player_id);

        Ok(GameActionResult::PlayerChecks)
    }

    /// Advance to the next player
    async fn advance_to_next_player(&self, session: &mut PokerGameSession) -> PluginResult<()> {
        let active_players: Vec<String> = session.players
            .iter()
            .filter(|(_, p)| p.is_active && !p.is_folded)
            .map(|(id, _)| id.clone())
            .collect();

        if active_players.len() <= 1 {
            // Hand is over
            self.resolve_hand(session).await?;
            return Ok(());
        }

        // Find next player
        if let Some(current) = &session.current_player {
            if let Some(current_index) = active_players.iter().position(|p| p == current) {
                let next_index = (current_index + 1) % active_players.len();
                session.current_player = Some(active_players[next_index].clone());
            }
        } else {
            session.current_player = active_players.first().cloned();
        }

        // Check if betting round is complete
        if self.is_betting_round_complete(session) {
            self.advance_to_next_betting_round(session).await?;
        }

        Ok(())
    }

    /// Check if current betting round is complete
    fn is_betting_round_complete(&self, session: &PokerGameSession) -> bool {
        let active_players: Vec<_> = session.players
            .iter()
            .filter(|(_, p)| p.is_active && !p.is_folded)
            .collect();

        if active_players.len() <= 1 {
            return true;
        }

        // Check if all active players have acted and met the current bet
        for (player_id, player) in &active_players {
            let player_bet_this_round = session.get_player_bet_this_round(player_id);
            if player_bet_this_round < session.current_bet {
                // Player still needs to act
                return false;
            }
        }

        true
    }

    /// Advance to the next betting round
    async fn advance_to_next_betting_round(&self, session: &mut PokerGameSession) -> PluginResult<()> {
        // Reset betting round state
        session.current_bet = 0;
        session.last_raiser = None;
        session.betting_round_bets.clear();

        // Advance game phase
        match session.game_phase {
            PokerGamePhase::PreFlop => {
                // Deal the flop (3 cards)
                self.deal_community_cards(session, 3).await?;
                session.game_phase = PokerGamePhase::Flop;
            }
            PokerGamePhase::Flop => {
                // Deal the turn (1 card)
                self.deal_community_cards(session, 1).await?;
                session.game_phase = PokerGamePhase::Turn;
            }
            PokerGamePhase::Turn => {
                // Deal the river (1 card)
                self.deal_community_cards(session, 1).await?;
                session.game_phase = PokerGamePhase::River;
            }
            PokerGamePhase::River => {
                // Showdown
                self.resolve_hand(session).await?;
                return Ok(());
            }
            _ => {}
        }

        // Set first active player as current player
        let active_players: Vec<String> = session.players
            .iter()
            .filter(|(_, p)| p.is_active && !p.is_folded)
            .map(|(id, _)| id.clone())
            .collect();

        session.current_player = active_players.first().cloned();

        info!("Advanced to {:?} phase", session.game_phase);
        Ok(())
    }

    /// Resolve hand and determine winners
    async fn resolve_hand(&self, session: &mut PokerGameSession) -> PluginResult<()> {
        let active_players: Vec<String> = session.players
            .iter()
            .filter(|(_, p)| p.is_active && !p.is_folded)
            .map(|(id, _)| id.clone())
            .collect();

        if active_players.len() == 1 {
            // Only one player left, they win
            if let Some(winner_id) = active_players.first() {
                if let Some(winner) = session.players.get_mut(winner_id) {
                    winner.chips += session.pot;
                    info!("Player {} wins pot of {} (all others folded)", winner_id, session.pot);
                }
            }
        } else if active_players.len() > 1 {
            // Showdown - evaluate hands
            let mut hand_rankings = Vec::new();
            
            for player_id in &active_players {
                if let Some(player) = session.players.get(player_id) {
                    let hand_strength = self.evaluate_hand(&player.hand, &session.community_cards);
                    hand_rankings.push((player_id.clone(), hand_strength));
                }
            }

            // Sort by hand strength (higher is better)
            hand_rankings.sort_by(|a, b| b.1.cmp(&a.1));

            // Determine winners (handle ties)
            let best_hand = hand_rankings[0].1;
            let winners: Vec<String> = hand_rankings
                .iter()
                .take_while(|(_, strength)| *strength == best_hand)
                .map(|(id, _)| id.clone())
                .collect();

            // Split pot among winners
            let pot_share = session.pot / winners.len() as u64;
            for winner_id in &winners {
                if let Some(winner) = session.players.get_mut(winner_id) {
                    winner.chips += pot_share;
                }
            }

            info!("Hand resolved - {} winners, {} each", winners.len(), pot_share);
        }

        session.pot = 0;
        session.game_phase = PokerGamePhase::Finished;
        
        Ok(())
    }

    /// Evaluate poker hand strength (simplified)
    fn evaluate_hand(&self, hole_cards: &[PokerCard], community_cards: &[PokerCard]) -> u32 {
        // This is a simplified hand evaluation
        // In a production system, you'd use a proper poker hand evaluator
        
        let mut all_cards = hole_cards.to_vec();
        all_cards.extend(community_cards.iter().cloned());
        
        // Count ranks for pair/trips detection
        let mut rank_counts = HashMap::new();
        for card in &all_cards {
            *rank_counts.entry(card.rank.clone()).or_insert(0) += 1;
        }

        // Check for pairs, trips, etc.
        let mut counts: Vec<u32> = rank_counts.values().cloned().collect();
        counts.sort_by(|a, b| b.cmp(a));

        // Simplified ranking (higher is better)
        match counts.as_slice() {
            [4, ..] => 800, // Four of a kind
            [3, 2, ..] => 700, // Full house  
            [3, ..] => 400, // Three of a kind
            [2, 2, ..] => 300, // Two pair
            [2, ..] => 200, // One pair
            _ => 100, // High card
        }
    }
}

impl Default for PokerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GamePlugin for PokerPlugin {
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
        info!("Poker plugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Starting;
        *self.state.write().await = PluginState::Running;
        info!("Poker plugin started");
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Stopping;
        self.game_sessions.write().await.clear();
        *self.state.write().await = PluginState::Stopped;
        info!("Poker plugin stopped");
        Ok(())
    }

    async fn handle_event(&mut self, event: PluginEvent) -> PluginResult<()> {
        match event {
            PluginEvent::SystemStartup => debug!("Poker plugin received system startup"),
            PluginEvent::SystemShutdown => self.stop().await?,
            PluginEvent::ConfigurationUpdated(new_config) => {
                *self.config.write().await = new_config;
            }
            _ => debug!("Poker plugin received event: {:?}", event),
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
            GameAction::PlaceBet { amount, .. } | GameAction::Raise { amount } => {
                self.process_bet(session_id, player_id, amount).await
            }
            GameAction::Call => {
                // Calculate call amount
                let sessions = self.game_sessions.read().await;
                let session = sessions.get(session_id)
                    .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;
                let call_amount = session.current_bet - session.get_player_bet_this_round(player_id);
                drop(sessions);
                
                self.process_bet(session_id, player_id, call_amount).await
            }
            GameAction::Fold => self.process_fold(session_id, player_id).await,
            GameAction::Check => self.process_check(session_id, player_id).await,
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

        let common_state = CommonGameState {
            session_id: session_id.to_string(),
            players: session.players.iter().map(|(id, player)| {
                (id.clone(), PlayerState {
                    player_id: id.clone(),
                    balance: player.chips,
                    is_active: player.is_active && !player.is_folded,
                    joined_at: player.joined_at,
                    player_data: serde_json::to_value(player).unwrap_or(serde_json::Value::Null),
                })
            }).collect(),
            current_phase: format!("{:?}", session.game_phase),
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
        _peer_states: Vec<serde_json::Value>,
    ) -> PluginResult<serde_json::Value> {
        // Poker state synchronization is complex due to hidden information
        // For now, return current state - production would implement proper sync
        warn!("Poker state synchronization not fully implemented");
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

        // Basic validation
        if session.current_player != Some(player_id.to_string()) {
            return Ok(false);
        }

        if player.is_folded || !player.is_active {
            return Ok(false);
        }

        match action {
            GameAction::Fold => Ok(true),
            GameAction::Check => Ok(session.current_bet == 0),
            GameAction::Call => Ok(session.current_bet > 0),
            GameAction::PlaceBet { amount, .. } | GameAction::Raise { amount } => {
                let required_to_call = session.current_bet - session.get_player_bet_this_round(player_id);
                Ok(*amount >= required_to_call && player.chips >= *amount)
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

        if session.players.len() >= session.max_players {
            return Err(PluginError::RuntimeError("Session is full".to_string()));
        }

        let player = PokerPlayer {
            id: player_id.to_string(),
            chips: initial_balance,
            hand: Vec::new(),
            is_active: true,
            is_folded: false,
            total_bet_this_hand: 0,
            position: session.players.len(),
            joined_at: std::time::SystemTime::now(),
        };

        session.players.insert(player_id.to_string(), player);
        info!("Player {} joined poker session {}", player_id, session_id);
        Ok(())
    }

    async fn on_player_leave(&mut self, session_id: &str, player_id: &str) -> PluginResult<()> {
        let mut sessions = self.game_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(player) = session.players.get_mut(player_id) {
                player.is_active = false;
            }
            info!("Player {} left poker session {}", player_id, session_id);
        }
        Ok(())
    }

    async fn on_session_create(&mut self, session_id: &str, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        let small_blind = config.get("small_blind").and_then(|v| v.as_u64()).unwrap_or(1);
        let big_blind = config.get("big_blind").and_then(|v| v.as_u64()).unwrap_or(2);
        let max_players = config.get("max_players").and_then(|v| v.as_u64()).unwrap_or(9) as usize;

        let mut session = PokerGameSession {
            id: session_id.to_string(),
            players: HashMap::new(),
            deck: Vec::new(),
            community_cards: Vec::new(),
            pot: 0,
            small_blind,
            big_blind,
            current_bet: 0,
            current_player: None,
            last_raiser: None,
            game_phase: PokerGamePhase::WaitingForPlayers,
            max_players,
            dealer_position: 0,
            betting_round_bets: HashMap::new(),
            folded_players: std::collections::HashSet::new(),
            created_at: std::time::SystemTime::now(),
        };

        // Initialize deck
        session.shuffle_deck(&self.rng).await?;

        self.game_sessions.write().await.insert(session_id.to_string(), session);
        self.statistics.sessions_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        info!("Created poker session: {}", session_id);
        Ok(())
    }

    async fn on_session_end(&mut self, session_id: &str) -> PluginResult<()> {
        self.game_sessions.write().await.remove(session_id);
        info!("Ended poker session: {}", session_id);
        Ok(())
    }

    async fn health_check(&self) -> PluginResult<PluginHealth> {
        let state = self.state.read().await.clone();
        let warnings = self.error_handler.get_warnings();
        let error_count = self.error_handler.get_error_count();

        Ok(PluginUtils::create_health_status(
            state, 128, 10.0, error_count, warnings,
        ))
    }

    async fn get_statistics(&self) -> PluginStatistics {
        self.statistics.to_plugin_statistics()
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.stop().await
    }
}

/// Poker game session
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PokerGameSession {
    id: String,
    players: HashMap<String, PokerPlayer>,
    deck: Vec<PokerCard>,
    community_cards: Vec<PokerCard>,
    pot: u64,
    small_blind: u64,
    big_blind: u64,
    current_bet: u64,
    current_player: Option<String>,
    last_raiser: Option<String>,
    game_phase: PokerGamePhase,
    max_players: usize,
    dealer_position: usize,
    betting_round_bets: HashMap<String, u64>,
    folded_players: std::collections::HashSet<String>,
    created_at: std::time::SystemTime,
}

impl PokerGameSession {
    /// Shuffle a new deck
    async fn shuffle_deck(&mut self, rng: &Arc<dyn RngCore + CryptoRng + Send + Sync>) -> PluginResult<()> {
        self.deck.clear();
        
        // Create standard 52-card deck
        for suit in &[PokerSuit::Hearts, PokerSuit::Diamonds, PokerSuit::Clubs, PokerSuit::Spades] {
            for rank in &[
                PokerRank::Two, PokerRank::Three, PokerRank::Four, PokerRank::Five,
                PokerRank::Six, PokerRank::Seven, PokerRank::Eight, PokerRank::Nine,
                PokerRank::Ten, PokerRank::Jack, PokerRank::Queen, PokerRank::King, PokerRank::Ace,
            ] {
                self.deck.push(PokerCard {
                    suit: suit.clone(),
                    rank: rank.clone(),
                });
            }
        }

        // Shuffle using Fisher-Yates algorithm
        for i in (1..self.deck.len()).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            self.deck.swap(i, j);
        }

        Ok(())
    }

    /// Get player's bet amount in current round
    fn get_player_bet_this_round(&self, player_id: &str) -> u64 {
        self.betting_round_bets.get(player_id).copied().unwrap_or(0)
    }

    /// Add bet to current round tracking
    fn add_bet(&mut self, player_id: &str, amount: u64) {
        *self.betting_round_bets.entry(player_id.to_string()).or_insert(0) += amount;
    }
}

/// Poker player
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PokerPlayer {
    id: String,
    chips: u64,
    hand: Vec<PokerCard>,
    is_active: bool,
    is_folded: bool,
    total_bet_this_hand: u64,
    position: usize,
    joined_at: std::time::SystemTime,
}

/// Poker game phases
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum PokerGamePhase {
    WaitingForPlayers,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    Finished,
}

/// Playing card for poker
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PokerCard {
    suit: PokerSuit,
    rank: PokerRank,
}

/// Poker card suits
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum PokerSuit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

/// Poker card ranks
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum PokerRank {
    Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten,
    Jack, Queen, King, Ace,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_poker_plugin_creation() {
        let plugin = PokerPlugin::new();
        
        assert_eq!(plugin.get_info().name, "Texas Hold'em Poker");
        assert_eq!(plugin.get_info().game_type, GameType::Poker);
        assert!(plugin.get_capabilities().contains(&PluginCapability::RealMoneyGaming));
    }

    #[tokio::test]
    async fn test_poker_session_creation() {
        let mut plugin = PokerPlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.on_session_create("test-session", HashMap::new()).await.unwrap();
        
        let sessions = plugin.game_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        
        assert_eq!(session.id, "test-session");
        assert_eq!(session.deck.len(), 52);
        assert_eq!(session.small_blind, 1);
        assert_eq!(session.big_blind, 2);
    }

    #[tokio::test]
    async fn test_player_join_poker() {
        let mut plugin = PokerPlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.on_session_create("test-session", HashMap::new()).await.unwrap();
        
        plugin.on_player_join("test-session", "player1", 1000).await.unwrap();
        plugin.on_player_join("test-session", "player2", 2000).await.unwrap();
        
        let sessions = plugin.game_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        
        assert_eq!(session.players.len(), 2);
        assert_eq!(session.players.get("player1").unwrap().chips, 1000);
        assert_eq!(session.players.get("player2").unwrap().chips, 2000);
    }

    #[test]
    fn test_hand_evaluation() {
        let plugin = PokerPlugin::new();
        
        // Test hand evaluation with simple hands
        let hole_cards = vec![
            PokerCard { suit: PokerSuit::Hearts, rank: PokerRank::Ace },
            PokerCard { suit: PokerSuit::Spades, rank: PokerRank::Ace },
        ];
        
        let community_cards = vec![
            PokerCard { suit: PokerSuit::Clubs, rank: PokerRank::Two },
            PokerCard { suit: PokerSuit::Diamonds, rank: PokerRank::Three },
            PokerCard { suit: PokerSuit::Hearts, rank: PokerRank::Four },
        ];
        
        let strength = plugin.evaluate_hand(&hole_cards, &community_cards);
        assert!(strength >= 200); // At least a pair
    }
}