use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, broadcast};
use serde::{Serialize, Deserialize};
use crate::protocol::{PeerId, GameId, CrapTokens, DiceRoll, new_game_id};
use crate::error::{Error, Result};
use super::craps::{CrapsGame, GamePhase};
use super::{Bet, BetType};

/// Gaming runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRuntimeConfig {
    pub max_concurrent_games: usize,
    pub min_players: usize,
    pub max_players: usize,
    pub game_timeout: Duration,
    pub roll_timeout: Duration,
    pub min_bet: u64,
    pub max_bet: u64,
    pub treasury_rake: f32,
    pub enable_anti_cheat: bool,
}

impl Default for GameRuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_games: 100,
            min_players: 2,
            max_players: 10,
            game_timeout: Duration::from_secs(3600),
            roll_timeout: Duration::from_secs(30),
            min_bet: 100,
            max_bet: 1_000_000,
            treasury_rake: 0.01, // 1% rake
            enable_anti_cheat: true,
        }
    }
}

/// Main gaming runtime orchestrator
/// 
/// Feynman: This is the "casino floor manager" - it oversees all games,
/// ensures fair play, manages the treasury, and coordinates between players.
/// Think of it as the conductor of an orchestra where each game is an instrument.
pub struct GameRuntime {
    config: GameRuntimeConfig,
    games: Arc<RwLock<HashMap<GameId, ActiveGame>>>,
    player_balances: Arc<RwLock<HashMap<PeerId, CrapTokens>>>,
    treasury_balance: Arc<RwLock<CrapTokens>>,
    
    // Event channels
    event_tx: broadcast::Sender<GameEvent>,
    command_rx: mpsc::Receiver<GameCommand>,
    
    // Statistics
    stats: Arc<RwLock<GameStats>>,
    is_running: Arc<RwLock<bool>>,
}

/// Active game wrapper with metadata
#[derive(Clone)]
pub struct ActiveGame {
    pub game: CrapsGame,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub total_pot: CrapTokens,
    pub rounds_played: u32,
    pub is_suspended: bool,
}

/// Events emitted by the gaming runtime
#[derive(Debug, Clone)]
pub enum GameEvent {
    GameCreated { game_id: GameId, creator: PeerId },
    PlayerJoined { game_id: GameId, player: PeerId },
    PlayerLeft { game_id: GameId, player: PeerId },
    BetPlaced { game_id: GameId, player: PeerId, bet: Bet },
    DiceRolled { game_id: GameId, roll: DiceRoll },
    RoundComplete { game_id: GameId, winners: Vec<(PeerId, u64)> },
    GameEnded { game_id: GameId, reason: String },
    TreasuryUpdate { balance: CrapTokens, rake_collected: u64 },
}

/// Commands that can be sent to the runtime
#[derive(Debug)]
pub enum GameCommand {
    CreateGame { creator: PeerId, config: GameConfig },
    JoinGame { game_id: GameId, player: PeerId, buy_in: CrapTokens },
    PlaceBet { game_id: GameId, player: PeerId, bet: Bet },
    RollDice { game_id: GameId, shooter: PeerId },
    LeaveGame { game_id: GameId, player: PeerId },
    SuspendGame { game_id: GameId, reason: String },
    ResumeGame { game_id: GameId },
}

/// Game-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub private: bool,
    pub password: Option<String>,
    pub allowed_bets: Vec<BetType>,
}

/// Runtime statistics
#[derive(Debug, Default, Clone)]
pub struct GameStats {
    pub total_games_created: u64,
    pub active_games: usize,
    pub total_bets_placed: u64,
    pub total_volume: u64,
    pub treasury_rake_collected: u64,
    pub largest_pot: u64,
    pub longest_game_rounds: u32,
}

impl GameRuntime {
    /// Create a new game runtime
    pub fn new(config: GameRuntimeConfig) -> (Self, mpsc::Sender<GameCommand>) {
        let (event_tx, _) = broadcast::channel(1000);
        let (command_tx, command_rx) = mpsc::channel(100);
        
        let runtime = Self {
            config,
            games: Arc::new(RwLock::new(HashMap::new())),
            player_balances: Arc::new(RwLock::new(HashMap::new())),
            treasury_balance: Arc::new(RwLock::new(CrapTokens::new_unchecked(1_000_000_000))), // 1B initial
            event_tx,
            command_rx,
            stats: Arc::new(RwLock::new(GameStats::default())),
            is_running: Arc::new(RwLock::new(false)),
        };
        
        (runtime, command_tx)
    }
    
    /// Start the runtime
    pub async fn start(&mut self) -> Result<()> {
        *self.is_running.write().await = true;
        
        // Start background tasks
        self.start_command_processor().await;
        self.start_timeout_checker().await;
        self.start_anti_cheat_monitor().await;
        
        Ok(())
    }
    
    /// Stop the runtime
    pub async fn stop(&mut self) -> Result<()> {
        *self.is_running.write().await = false;
        
        // Suspend all active games
        let games = self.games.read().await;
        for game_id in games.keys() {
            let _ = self.event_tx.send(GameEvent::GameEnded {
                game_id: *game_id,
                reason: "Runtime shutdown".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Process a game command
    async fn process_command(&self, command: GameCommand) -> Result<()> {
        match command {
            GameCommand::CreateGame { creator, config } => {
                self.create_game(creator, config).await?;
            }
            GameCommand::JoinGame { game_id, player, buy_in } => {
                self.join_game(game_id, player, buy_in).await?;
            }
            GameCommand::PlaceBet { game_id, player, bet } => {
                self.place_bet(game_id, player, bet).await?;
            }
            GameCommand::RollDice { game_id, shooter } => {
                self.roll_dice(game_id, shooter).await?;
            }
            GameCommand::LeaveGame { game_id, player } => {
                self.leave_game(game_id, player).await?;
            }
            GameCommand::SuspendGame { game_id, reason } => {
                self.suspend_game(game_id, reason).await?;
            }
            GameCommand::ResumeGame { game_id } => {
                self.resume_game(game_id).await?;
            }
        }
        Ok(())
    }
    
    /// Create a new game
    async fn create_game(&self, creator: PeerId, _config: GameConfig) -> Result<()> {
        let mut games = self.games.write().await;
        
        if games.len() >= self.config.max_concurrent_games {
            return Err(Error::GameError("Maximum concurrent games reached".into()));
        }
        
        let game_id = new_game_id();
        let mut game = CrapsGame::new(game_id, creator);
        
        // Add treasury as automatic participant
        let _ = game.add_player(crate::TREASURY_ADDRESS);
        
        let active_game = ActiveGame {
            game,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            total_pot: CrapTokens::new_unchecked(0),
            rounds_played: 0,
            is_suspended: false,
        };
        
        games.insert(game_id, active_game);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_games_created += 1;
        stats.active_games = games.len();
        
        // Emit event
        let _ = self.event_tx.send(GameEvent::GameCreated { game_id, creator });
        
        Ok(())
    }
    
    /// Join an existing game
    async fn join_game(&self, game_id: GameId, player: PeerId, buy_in: CrapTokens) -> Result<()> {
        // Check player balance
        let mut balances = self.player_balances.write().await;
        let balance = balances.entry(player).or_insert(CrapTokens::new_unchecked(0));
        
        if balance.amount() < buy_in.amount() {
            return Err(Error::InsufficientBalance);
        }
        
        // Deduct buy-in
        *balance = CrapTokens::new_unchecked(balance.amount() - buy_in.amount());
        
        // Add to game
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::GameNotFound)?;
        
        if game.game.participants.len() >= self.config.max_players {
            return Err(Error::GameError("Game is full".into()));
        }
        
        let _ = game.game.add_player(player);
        game.total_pot = CrapTokens::new_unchecked(game.total_pot.amount() + buy_in.amount());
        game.last_activity = Instant::now();
        
        // Emit event
        let _ = self.event_tx.send(GameEvent::PlayerJoined { game_id, player });
        
        Ok(())
    }
    
    /// Place a bet in a game
    async fn place_bet(&self, game_id: GameId, player: PeerId, bet: Bet) -> Result<()> {
        // Validate bet amount
        if bet.amount.amount() < self.config.min_bet || bet.amount.amount() > self.config.max_bet {
            return Err(Error::InvalidBet("Bet amount out of range".into()));
        }
        
        // Check player balance
        let mut balances = self.player_balances.write().await;
        let balance = balances.get_mut(&player)
            .ok_or(Error::InsufficientBalance)?;
        
        if balance.amount() < bet.amount.amount() {
            return Err(Error::InsufficientBalance);
        }
        
        // Deduct bet amount
        *balance = CrapTokens::new_unchecked(balance.amount() - bet.amount.amount());
        
        // Add bet to game
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::GameNotFound)?;
        
        game.game.place_bet(player, bet.clone()).map_err(|e| Error::ValidationError(e.to_string()))?;
        game.total_pot = CrapTokens::new_unchecked(game.total_pot.amount() + bet.amount.amount());
        game.last_activity = Instant::now();
        
        // Update stats
        self.stats.write().await.total_bets_placed += 1;
        self.stats.write().await.total_volume += bet.amount.amount();
        
        // Emit event
        let _ = self.event_tx.send(GameEvent::BetPlaced { game_id, player, bet });
        
        Ok(())
    }
    
    /// Process a dice roll
    async fn roll_dice(&self, game_id: GameId, _shooter: PeerId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::GameNotFound)?;
        
        // TODO: Add shooter validation when field is added
        
        // Generate secure random roll
        let roll = DiceRoll::new(
            (rand::random::<u8>() % 6) + 1,
            (rand::random::<u8>() % 6) + 1,
        );
        
        // Process roll
        // Extract the roll value for later use
        let dice_roll = roll?;
        let resolutions = game.game.process_roll(dice_roll);
        game.last_activity = Instant::now();
        game.rounds_played += 1;
        
        // Process payouts
        let mut total_payout = 0u64;
        let mut winners = Vec::new();
        
        for resolution in resolutions {
            match resolution {
                super::craps::BetResolution::Won { player, payout, .. } => {
                    // Add payout to player balance
                    let mut balances = self.player_balances.write().await;
                    let balance = balances.entry(player).or_insert(CrapTokens::new_unchecked(0));
                    *balance = CrapTokens::new_unchecked(balance.amount() + payout.amount());
                    
                    winners.push((player, payout.amount()));
                    total_payout += payout.amount();
                }
                super::craps::BetResolution::Lost { .. } => {
                    // Bet already deducted when placed
                }
                super::craps::BetResolution::Push { player, amount, .. } => {
                    // Return bet to player
                    let mut balances = self.player_balances.write().await;
                    let balance = balances.entry(player).or_insert(CrapTokens::new_unchecked(0));
                    *balance = CrapTokens::new_unchecked(balance.amount() + amount.amount());
                }
            }
        }
        
        // Collect treasury rake
        if total_payout > 0 {
            let rake = (total_payout as f32 * self.config.treasury_rake) as u64;
            let mut treasury = self.treasury_balance.write().await;
            *treasury = CrapTokens::new_unchecked(treasury.amount() + rake);
            
            // Update stats
            self.stats.write().await.treasury_rake_collected += rake;
            
            // Emit treasury event
            let _ = self.event_tx.send(GameEvent::TreasuryUpdate {
                balance: *treasury,
                rake_collected: rake,
            });
        }
        
        // Emit events
        let _ = self.event_tx.send(GameEvent::DiceRolled { game_id, roll: dice_roll });
        let _ = self.event_tx.send(GameEvent::RoundComplete { game_id, winners });
        
        // Check if game should end
        if game.game.current_phase == GamePhase::GameEnded {
            self.end_game(game_id, "Game completed".to_string()).await?;
        }
        
        Ok(())
    }
    
    /// Player leaves a game
    async fn leave_game(&self, game_id: GameId, player: PeerId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::GameNotFound)?;
        
        game.game.participants.retain(|&p| p != player);
        game.last_activity = Instant::now();
        
        // Cash out any remaining bets
        // In a real implementation, this would handle active bets
        
        // Emit event
        let _ = self.event_tx.send(GameEvent::PlayerLeft { game_id, player });
        
        // End game if not enough players
        if game.game.participants.len() < self.config.min_players {
            drop(games);
            self.end_game(game_id, "Not enough players".to_string()).await?;
        }
        
        Ok(())
    }
    
    /// Suspend a game
    async fn suspend_game(&self, game_id: GameId, reason: String) -> Result<()> {
        let mut games = self.games.write().await;
        if let Some(game) = games.get_mut(&game_id) {
            game.is_suspended = true;
            let _ = self.event_tx.send(GameEvent::GameEnded {
                game_id,
                reason: format!("Suspended: {}", reason),
            });
        }
        Ok(())
    }
    
    /// Resume a suspended game
    async fn resume_game(&self, game_id: GameId) -> Result<()> {
        let mut games = self.games.write().await;
        if let Some(game) = games.get_mut(&game_id) {
            game.is_suspended = false;
            game.last_activity = Instant::now();
        }
        Ok(())
    }
    
    /// End a game
    async fn end_game(&self, game_id: GameId, reason: String) -> Result<()> {
        let mut games = self.games.write().await;
        if let Some(game) = games.remove(&game_id) {
            // Update stats
            let mut stats = self.stats.write().await;
            stats.active_games = games.len();
            if game.total_pot.amount() > stats.largest_pot {
                stats.largest_pot = game.total_pot.amount();
            }
            if game.rounds_played > stats.longest_game_rounds {
                stats.longest_game_rounds = game.rounds_played;
            }
            
            // Emit event
            let _ = self.event_tx.send(GameEvent::GameEnded { game_id, reason });
        }
        Ok(())
    }
    
    /// Start command processor task
    async fn start_command_processor(&mut self) {
        let is_running = self.is_running.clone();
        
        while *is_running.read().await {
            if let Some(command) = self.command_rx.recv().await {
                let _ = self.process_command(command).await;
            }
        }
    }
    
    /// Start timeout checker task
    async fn start_timeout_checker(&self) {
        let is_running = self.is_running.clone();
        let games = self.games.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let now = Instant::now();
                let games_read = games.read().await;
                let expired: Vec<GameId> = games_read
                    .iter()
                    .filter(|(_, game)| {
                        now.duration_since(game.last_activity) > config.game_timeout
                    })
                    .map(|(id, _)| *id)
                    .collect();
                drop(games_read);
                
                for game_id in expired {
                    // End expired games
                    let mut games_write = games.write().await;
                    games_write.remove(&game_id);
                }
            }
        });
    }
    
    /// Start anti-cheat monitor task
    async fn start_anti_cheat_monitor(&self) {
        if !self.config.enable_anti_cheat {
            return;
        }
        
        let is_running = self.is_running.clone();
        let _games = self.games.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            while *is_running.read().await {
                interval.tick().await;
                
                // Anti-cheat checks would go here
                // - Detect impossible rolls
                // - Check for collusion patterns
                // - Monitor betting anomalies
            }
        });
    }
    
    /// Get a subscription to game events
    pub fn subscribe(&self) -> broadcast::Receiver<GameEvent> {
        self.event_tx.subscribe()
    }
    
    /// Get current statistics
    pub async fn get_stats(&self) -> GameStats {
        self.stats.read().await.clone()
    }
    
    /// Get player balance
    pub async fn get_balance(&self, player: &PeerId) -> CrapTokens {
        self.player_balances.read().await
            .get(player)
            .copied()
            .unwrap_or_else(|| CrapTokens::new_unchecked(0))
    }
}

/// Treasury participant that provides liquidity and takes opposite bets
pub struct TreasuryParticipant {
    balance: Arc<RwLock<u64>>, // CRAP token balance
    #[allow(dead_code)]
    game_participation: Arc<RwLock<HashMap<GameId, TreasuryPosition>>>,
    #[allow(dead_code)]
    strategy: TreasuryStrategy,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TreasuryPosition {
    game_id: GameId,
    total_exposure: u64,
    bets_placed: HashMap<BetType, u64>,
    profit_loss: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TreasuryStrategy {
    max_exposure_per_game: u64,
    preferred_bet_types: Vec<BetType>,
    risk_tolerance: f64,
}

impl Default for TreasuryStrategy {
    fn default() -> Self {
        Self {
            max_exposure_per_game: 10000,
            preferred_bet_types: vec![
                BetType::DontPass,
                BetType::DontCome,
            ],
            risk_tolerance: 0.5,
        }
    }
}

impl TreasuryParticipant {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: Arc::new(RwLock::new(initial_balance)),
            game_participation: Arc::new(RwLock::new(HashMap::new())),
            strategy: TreasuryStrategy::default(),
        }
    }
    
    pub async fn get_balance(&self) -> u64 {
        *self.balance.read().await
    }
}