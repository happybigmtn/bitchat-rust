//! Slot Machine Plugin Implementation
//!
//! This module implements a multi-reel slot machine game plugin with
//! configurable paylines, bonus features, and progressive jackpots.

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

/// Slot machine game plugin
pub struct SlotMachinePlugin {
    info: PluginInfo,
    capabilities: Vec<PluginCapability>,
    state: Arc<RwLock<PluginState>>,
    game_sessions: Arc<RwLock<HashMap<String, SlotGameSession>>>,
    config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    error_handler: PluginErrorHandler,
    statistics: BasePluginStatistics,
    rng: Arc<dyn CryptoRng + RngCore + Send + Sync>,
}

impl SlotMachinePlugin {
    /// Create new slot machine plugin
    pub fn new() -> Self {
        let info = PluginUtils::create_plugin_info_template(
            "Classic Slot Machine",
            "1.0.0",
            GameType::Other("Slots".to_string()),
            "Classic 5-reel slot machine with multiple paylines, bonus features, and progressive jackpots"
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

    /// Spin the slot machine reels
    async fn spin_reels(&self, session: &mut SlotGameSession, bet_amount: u64) -> PluginResult<SpinResult> {
        // Generate random symbols for each reel
        let mut reels = Vec::new();
        
        for reel_index in 0..session.machine_config.reel_count {
            let mut reel_symbols = Vec::new();
            let reel_symbols_config = &session.machine_config.reel_symbols[reel_index];
            
            for _position in 0..session.machine_config.visible_symbols_per_reel {
                let symbol_index = self.rng.next_u32() as usize % reel_symbols_config.len();
                reel_symbols.push(reel_symbols_config[symbol_index].clone());
            }
            reels.push(reel_symbols);
        }

        // Calculate winning combinations
        let winning_lines = self.calculate_winning_lines(&reels, &session.machine_config).await?;
        
        // Calculate total payout
        let mut total_payout = 0u64;
        for line in &winning_lines {
            total_payout += line.payout;
        }

        // Check for bonus features
        let bonus_triggered = self.check_bonus_trigger(&reels, &session.machine_config);
        
        if bonus_triggered {
            total_payout += self.calculate_bonus_payout(bet_amount, &session.machine_config);
        }

        // Check for progressive jackpot
        let jackpot_won = self.check_jackpot(&reels, &session.machine_config);
        if jackpot_won {
            total_payout += session.progressive_jackpot;
            session.progressive_jackpot = session.machine_config.base_jackpot; // Reset jackpot
        } else {
            // Contribute to progressive jackpot
            let jackpot_contribution = bet_amount * session.machine_config.jackpot_contribution_percent / 100;
            session.progressive_jackpot += jackpot_contribution;
        }

        let spin_result = SpinResult {
            reels,
            winning_lines,
            total_payout,
            bonus_triggered,
            jackpot_won,
            progressive_jackpot: session.progressive_jackpot,
        };

        session.total_spins += 1;
        session.total_wagered += bet_amount;
        session.total_paid_out += total_payout;

        info!("Slot spin completed - bet: {}, payout: {}, jackpot: {}", 
              bet_amount, total_payout, session.progressive_jackpot);

        Ok(spin_result)
    }

    /// Calculate winning paylines
    async fn calculate_winning_lines(
        &self,
        reels: &[Vec<SlotSymbol>],
        config: &SlotMachineConfig,
    ) -> PluginResult<Vec<WinningLine>> {
        let mut winning_lines = Vec::new();

        for (line_index, payline) in config.paylines.iter().enumerate() {
            if let Some(win) = self.check_payline_win(reels, payline, config) {
                winning_lines.push(WinningLine {
                    line_number: line_index + 1,
                    symbol: win.symbol,
                    count: win.count,
                    payout: win.payout,
                    positions: payline.clone(),
                });
            }
        }

        Ok(winning_lines)
    }

    /// Check if a payline has winning symbols
    fn check_payline_win(
        &self,
        reels: &[Vec<SlotSymbol>],
        payline: &[(usize, usize)], // (reel, position)
        config: &SlotMachineConfig,
    ) -> Option<PaylineWin> {
        if payline.is_empty() || reels.is_empty() {
            return None;
        }

        // Get the first symbol to check for matches
        let (first_reel, first_pos) = payline[0];
        if first_reel >= reels.len() || first_pos >= reels[first_reel].len() {
            return None;
        }

        let first_symbol = &reels[first_reel][first_pos];
        let mut consecutive_count = 1;

        // Count consecutive matching symbols from left to right
        for &(reel, pos) in &payline[1..] {
            if reel >= reels.len() || pos >= reels[reel].len() {
                break;
            }

            let symbol = &reels[reel][pos];
            if *symbol == *first_symbol || *symbol == SlotSymbol::Wild || *first_symbol == SlotSymbol::Wild {
                consecutive_count += 1;
            } else {
                break;
            }
        }

        // Check if we have enough symbols for a win
        let min_symbols = config.min_symbols_for_win.get(first_symbol).copied().unwrap_or(3);
        if consecutive_count >= min_symbols {
            let payout = self.calculate_symbol_payout(first_symbol, consecutive_count, config);
            Some(PaylineWin {
                symbol: first_symbol.clone(),
                count: consecutive_count,
                payout,
            })
        } else {
            None
        }
    }

    /// Calculate payout for symbol combination
    fn calculate_symbol_payout(
        &self,
        symbol: &SlotSymbol,
        count: usize,
        config: &SlotMachineConfig,
    ) -> u64 {
        if let Some(symbol_payout) = config.symbol_payouts.get(symbol) {
            match count {
                3 => symbol_payout.three_of_a_kind,
                4 => symbol_payout.four_of_a_kind,
                5 => symbol_payout.five_of_a_kind,
                _ => 0,
            }
        } else {
            0
        }
    }

    /// Check if bonus feature is triggered
    fn check_bonus_trigger(&self, reels: &[Vec<SlotSymbol>], _config: &SlotMachineConfig) -> bool {
        // Simple bonus trigger: 3+ scatter symbols anywhere
        let mut scatter_count = 0;
        for reel in reels {
            for symbol in reel {
                if *symbol == SlotSymbol::Scatter {
                    scatter_count += 1;
                }
            }
        }
        scatter_count >= 3
    }

    /// Calculate bonus feature payout
    fn calculate_bonus_payout(&self, bet_amount: u64, _config: &SlotMachineConfig) -> u64 {
        // Simple bonus: 10x bet amount
        bet_amount * 10
    }

    /// Check if progressive jackpot is won
    fn check_jackpot(&self, reels: &[Vec<SlotSymbol>], _config: &SlotMachineConfig) -> bool {
        // Jackpot triggered by 5 Wild symbols on middle payline
        if reels.len() >= 5 {
            let middle_position = reels[0].len() / 2;
            for reel in reels.iter().take(5) {
                if reel.get(middle_position) != Some(&SlotSymbol::Wild) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// Process spin action
    async fn process_spin(
        &self,
        session_id: &str,
        player_id: &str,
        bet_amount: u64,
    ) -> PluginResult<GameActionResult> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| PluginError::RuntimeError("Session not found".to_string()))?;

        let player = session.players.get_mut(player_id)
            .ok_or_else(|| PluginError::RuntimeError("Player not found".to_string()))?;

        // Validate bet
        if bet_amount < session.machine_config.min_bet || bet_amount > session.machine_config.max_bet {
            return Err(PluginError::RuntimeError(format!(
                "Bet amount {} not within limits ({}-{})", 
                bet_amount, session.machine_config.min_bet, session.machine_config.max_bet
            )));
        }

        if player.balance < bet_amount {
            return Err(PluginError::RuntimeError("Insufficient balance".to_string()));
        }

        // Deduct bet
        player.balance -= bet_amount;

        // Spin reels
        let spin_result = self.spin_reels(session, bet_amount).await?;

        // Add winnings to player balance
        player.balance += spin_result.total_payout;
        player.total_spins += 1;
        player.total_wagered += bet_amount;
        player.total_won += spin_result.total_payout;

        // Create result message
        let mut result_messages = Vec::new();
        
        if !spin_result.winning_lines.is_empty() {
            result_messages.push(format!("{} winning line(s)", spin_result.winning_lines.len()));
        }
        
        if spin_result.bonus_triggered {
            result_messages.push("Bonus triggered!".to_string());
        }
        
        if spin_result.jackpot_won {
            result_messages.push(format!("JACKPOT WON: {}", spin_result.progressive_jackpot));
        }

        let confirmation = if result_messages.is_empty() {
            "No win this spin".to_string()
        } else {
            result_messages.join(", ")
        };

        info!("Player {} spun for {} and won {}", player_id, bet_amount, spin_result.total_payout);

        Ok(GameActionResult::BetAccepted {
            new_balance: player.balance,
        })
    }
}

impl Default for SlotMachinePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GamePlugin for SlotMachinePlugin {
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
        info!("Slot machine plugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Starting;
        *self.state.write().await = PluginState::Running;
        info!("Slot machine plugin started");
        Ok(())
    }

    async fn stop(&mut self) -> PluginResult<()> {
        *self.state.write().await = PluginState::Stopping;
        self.game_sessions.write().await.clear();
        *self.state.write().await = PluginState::Stopped;
        info!("Slot machine plugin stopped");
        Ok(())
    }

    async fn handle_event(&mut self, event: PluginEvent) -> PluginResult<()> {
        match event {
            PluginEvent::SystemStartup => debug!("Slot machine plugin received system startup"),
            PluginEvent::SystemShutdown => self.stop().await?,
            PluginEvent::ConfigurationUpdated(new_config) => {
                *self.config.write().await = new_config;
            }
            _ => debug!("Slot machine plugin received event: {:?}", event),
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
            GameAction::PlaceBet { amount, .. } => {
                // In slot machines, placing a bet triggers a spin
                self.process_spin(session_id, player_id, amount).await
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
            current_phase: "Playing".to_string(),
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
        // Slot machines are typically single-player, so sync is simpler
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
                Ok(*amount >= session.machine_config.min_bet &&
                   *amount <= session.machine_config.max_bet &&
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

        let player = SlotPlayer {
            id: player_id.to_string(),
            balance: initial_balance,
            total_spins: 0,
            total_wagered: 0,
            total_won: 0,
            joined_at: std::time::SystemTime::now(),
        };

        session.players.insert(player_id.to_string(), player);
        info!("Player {} joined slot machine session {}", player_id, session_id);
        Ok(())
    }

    async fn on_player_leave(&mut self, session_id: &str, player_id: &str) -> PluginResult<()> {
        let mut sessions = self.game_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.players.remove(player_id);
            info!("Player {} left slot machine session {}", player_id, session_id);
        }
        Ok(())
    }

    async fn on_session_create(&mut self, session_id: &str, config: HashMap<String, serde_json::Value>) -> PluginResult<()> {
        // Create default slot machine configuration
        let machine_config = SlotMachineConfig::default();

        let session = SlotGameSession {
            id: session_id.to_string(),
            players: HashMap::new(),
            machine_config,
            progressive_jackpot: 10000, // Starting jackpot
            total_spins: 0,
            total_wagered: 0,
            total_paid_out: 0,
            created_at: std::time::SystemTime::now(),
        };

        self.game_sessions.write().await.insert(session_id.to_string(), session);
        self.statistics.sessions_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        info!("Created slot machine session: {}", session_id);
        Ok(())
    }

    async fn on_session_end(&mut self, session_id: &str) -> PluginResult<()> {
        self.game_sessions.write().await.remove(session_id);
        info!("Ended slot machine session: {}", session_id);
        Ok(())
    }

    async fn health_check(&self) -> PluginResult<PluginHealth> {
        let state = self.state.read().await.clone();
        let warnings = self.error_handler.get_warnings();
        let error_count = self.error_handler.get_error_count();

        Ok(PluginUtils::create_health_status(
            state, 80, 6.0, error_count, warnings,
        ))
    }

    async fn get_statistics(&self) -> PluginStatistics {
        self.statistics.to_plugin_statistics()
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.stop().await
    }
}

// Slot machine types and structures

/// Slot machine game session
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlotGameSession {
    id: String,
    players: HashMap<String, SlotPlayer>,
    machine_config: SlotMachineConfig,
    progressive_jackpot: u64,
    total_spins: u64,
    total_wagered: u64,
    total_paid_out: u64,
    created_at: std::time::SystemTime,
}

/// Slot machine player
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlotPlayer {
    id: String,
    balance: u64,
    total_spins: u64,
    total_wagered: u64,
    total_won: u64,
    joined_at: std::time::SystemTime,
}

/// Slot machine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlotMachineConfig {
    reel_count: usize,
    visible_symbols_per_reel: usize,
    min_bet: u64,
    max_bet: u64,
    base_jackpot: u64,
    jackpot_contribution_percent: u64,
    reel_symbols: Vec<Vec<SlotSymbol>>,
    paylines: Vec<Vec<(usize, usize)>>, // (reel_index, symbol_position)
    symbol_payouts: HashMap<SlotSymbol, SymbolPayout>,
    min_symbols_for_win: HashMap<SlotSymbol, usize>,
}

impl Default for SlotMachineConfig {
    fn default() -> Self {
        // Create default 5-reel, 3-row slot machine
        let mut reel_symbols = Vec::new();
        for _ in 0..5 {
            reel_symbols.push(vec![
                SlotSymbol::Cherry, SlotSymbol::Cherry, SlotSymbol::Cherry,
                SlotSymbol::Lemon, SlotSymbol::Lemon,
                SlotSymbol::Orange, SlotSymbol::Orange,
                SlotSymbol::Plum, SlotSymbol::Plum,
                SlotSymbol::Bell, SlotSymbol::Bell,
                SlotSymbol::Seven,
                SlotSymbol::Wild,
                SlotSymbol::Scatter,
            ]);
        }

        // Create standard paylines (simplified)
        let paylines = vec![
            vec![(0, 1), (1, 1), (2, 1), (3, 1), (4, 1)], // Middle row
            vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)], // Top row
            vec![(0, 2), (1, 2), (2, 2), (3, 2), (4, 2)], // Bottom row
        ];

        // Symbol payouts
        let mut symbol_payouts = HashMap::new();
        symbol_payouts.insert(SlotSymbol::Cherry, SymbolPayout { three_of_a_kind: 5, four_of_a_kind: 25, five_of_a_kind: 100 });
        symbol_payouts.insert(SlotSymbol::Lemon, SymbolPayout { three_of_a_kind: 10, four_of_a_kind: 50, five_of_a_kind: 200 });
        symbol_payouts.insert(SlotSymbol::Orange, SymbolPayout { three_of_a_kind: 15, four_of_a_kind: 75, five_of_a_kind: 300 });
        symbol_payouts.insert(SlotSymbol::Plum, SymbolPayout { three_of_a_kind: 20, four_of_a_kind: 100, five_of_a_kind: 400 });
        symbol_payouts.insert(SlotSymbol::Bell, SymbolPayout { three_of_a_kind: 50, four_of_a_kind: 250, five_of_a_kind: 1000 });
        symbol_payouts.insert(SlotSymbol::Seven, SymbolPayout { three_of_a_kind: 100, four_of_a_kind: 500, five_of_a_kind: 2000 });
        symbol_payouts.insert(SlotSymbol::Wild, SymbolPayout { three_of_a_kind: 200, four_of_a_kind: 1000, five_of_a_kind: 5000 });

        // Minimum symbols for win
        let mut min_symbols_for_win = HashMap::new();
        for symbol in &[SlotSymbol::Cherry, SlotSymbol::Lemon, SlotSymbol::Orange, 
                       SlotSymbol::Plum, SlotSymbol::Bell, SlotSymbol::Seven, SlotSymbol::Wild] {
            min_symbols_for_win.insert(symbol.clone(), 3);
        }

        Self {
            reel_count: 5,
            visible_symbols_per_reel: 3,
            min_bet: 1,
            max_bet: 100,
            base_jackpot: 10000,
            jackpot_contribution_percent: 1, // 1% goes to progressive jackpot
            reel_symbols,
            paylines,
            symbol_payouts,
            min_symbols_for_win,
        }
    }
}

/// Slot machine symbols
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum SlotSymbol {
    Cherry,
    Lemon,
    Orange,
    Plum,
    Bell,
    Seven,
    Wild,
    Scatter,
}

/// Payout amounts for symbol combinations
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SymbolPayout {
    three_of_a_kind: u64,
    four_of_a_kind: u64,
    five_of_a_kind: u64,
}

/// Result of a slot machine spin
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpinResult {
    reels: Vec<Vec<SlotSymbol>>,
    winning_lines: Vec<WinningLine>,
    total_payout: u64,
    bonus_triggered: bool,
    jackpot_won: bool,
    progressive_jackpot: u64,
}

/// Winning payline information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WinningLine {
    line_number: usize,
    symbol: SlotSymbol,
    count: usize,
    payout: u64,
    positions: Vec<(usize, usize)>,
}

/// Internal payline win calculation
struct PaylineWin {
    symbol: SlotSymbol,
    count: usize,
    payout: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_slot_machine_plugin_creation() {
        let plugin = SlotMachinePlugin::new();
        
        assert_eq!(plugin.get_info().name, "Classic Slot Machine");
        assert!(matches!(plugin.get_info().game_type, GameType::Other(_)));
        assert!(plugin.get_capabilities().contains(&PluginCapability::RealMoneyGaming));
    }

    #[tokio::test]
    async fn test_slot_machine_session_creation() {
        let mut plugin = SlotMachinePlugin::new();
        let config = HashMap::new();
        
        plugin.initialize(config).await.unwrap();
        plugin.on_session_create("test-session", HashMap::new()).await.unwrap();
        
        let sessions = plugin.game_sessions.read().await;
        let session = sessions.get("test-session").unwrap();
        
        assert_eq!(session.id, "test-session");
        assert_eq!(session.machine_config.reel_count, 5);
        assert_eq!(session.machine_config.visible_symbols_per_reel, 3);
        assert_eq!(session.progressive_jackpot, 10000);
    }

    #[test]
    fn test_symbol_payout_calculation() {
        let plugin = SlotMachinePlugin::new();
        let config = SlotMachineConfig::default();
        
        // Test cherry payout
        let cherry_payout = plugin.calculate_symbol_payout(&SlotSymbol::Cherry, 3, &config);
        assert_eq!(cherry_payout, 5);
        
        let cherry_payout_4 = plugin.calculate_symbol_payout(&SlotSymbol::Cherry, 4, &config);
        assert_eq!(cherry_payout_4, 25);
        
        let cherry_payout_5 = plugin.calculate_symbol_payout(&SlotSymbol::Cherry, 5, &config);
        assert_eq!(cherry_payout_5, 100);
        
        // Test seven payout
        let seven_payout = plugin.calculate_symbol_payout(&SlotSymbol::Seven, 5, &config);
        assert_eq!(seven_payout, 2000);
    }

    #[test]
    fn test_payline_win_detection() {
        let plugin = SlotMachinePlugin::new();
        let config = SlotMachineConfig::default();
        
        // Create winning reels (3 cherries in a row)
        let reels = vec![
            vec![SlotSymbol::Cherry, SlotSymbol::Lemon, SlotSymbol::Orange],
            vec![SlotSymbol::Cherry, SlotSymbol::Bell, SlotSymbol::Wild],
            vec![SlotSymbol::Cherry, SlotSymbol::Seven, SlotSymbol::Plum],
            vec![SlotSymbol::Bell, SlotSymbol::Orange, SlotSymbol::Cherry],
            vec![SlotSymbol::Wild, SlotSymbol::Seven, SlotSymbol::Bell],
        ];
        
        // Test top row payline
        let payline = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)];
        let win = plugin.check_payline_win(&reels, &payline, &config);
        
        assert!(win.is_some());
        let win = win.unwrap();
        assert_eq!(win.symbol, SlotSymbol::Cherry);
        assert_eq!(win.count, 3);
        assert_eq!(win.payout, 5); // 3 of a kind cherry payout
    }

    #[test]
    fn test_jackpot_detection() {
        let plugin = SlotMachinePlugin::new();
        let config = SlotMachineConfig::default();
        
        // Create jackpot-winning reels (5 wilds in middle row)
        let reels = vec![
            vec![SlotSymbol::Cherry, SlotSymbol::Wild, SlotSymbol::Orange],
            vec![SlotSymbol::Bell, SlotSymbol::Wild, SlotSymbol::Plum],
            vec![SlotSymbol::Lemon, SlotSymbol::Wild, SlotSymbol::Seven],
            vec![SlotSymbol::Orange, SlotSymbol::Wild, SlotSymbol::Cherry],
            vec![SlotSymbol::Plum, SlotSymbol::Wild, SlotSymbol::Bell],
        ];
        
        assert!(plugin.check_jackpot(&reels, &config));
        
        // Test non-jackpot reels
        let non_jackpot_reels = vec![
            vec![SlotSymbol::Cherry, SlotSymbol::Cherry, SlotSymbol::Orange],
            vec![SlotSymbol::Bell, SlotSymbol::Wild, SlotSymbol::Plum],
            vec![SlotSymbol::Lemon, SlotSymbol::Wild, SlotSymbol::Seven],
            vec![SlotSymbol::Orange, SlotSymbol::Wild, SlotSymbol::Cherry],
            vec![SlotSymbol::Plum, SlotSymbol::Wild, SlotSymbol::Bell],
        ];
        
        assert!(!plugin.check_jackpot(&non_jackpot_reels, &config));
    }

    #[test]
    fn test_bonus_trigger() {
        let plugin = SlotMachinePlugin::new();
        let config = SlotMachineConfig::default();
        
        // Create reels with 3 scatter symbols
        let reels = vec![
            vec![SlotSymbol::Scatter, SlotSymbol::Cherry, SlotSymbol::Orange],
            vec![SlotSymbol::Bell, SlotSymbol::Scatter, SlotSymbol::Plum],
            vec![SlotSymbol::Lemon, SlotSymbol::Wild, SlotSymbol::Scatter],
            vec![SlotSymbol::Orange, SlotSymbol::Bell, SlotSymbol::Cherry],
            vec![SlotSymbol::Plum, SlotSymbol::Seven, SlotSymbol::Bell],
        ];
        
        assert!(plugin.check_bonus_trigger(&reels, &config));
        
        // Test reels without enough scatters
        let no_bonus_reels = vec![
            vec![SlotSymbol::Cherry, SlotSymbol::Cherry, SlotSymbol::Orange],
            vec![SlotSymbol::Bell, SlotSymbol::Bell, SlotSymbol::Plum],
            vec![SlotSymbol::Lemon, SlotSymbol::Wild, SlotSymbol::Seven],
            vec![SlotSymbol::Orange, SlotSymbol::Bell, SlotSymbol::Cherry],
            vec![SlotSymbol::Plum, SlotSymbol::Seven, SlotSymbol::Bell],
        ];
        
        assert!(!plugin.check_bonus_trigger(&no_bonus_reels, &config));
    }
}