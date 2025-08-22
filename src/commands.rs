//! Command implementations for BitCraps CLI
//! 
//! This module contains all the game command implementations,
//! including creating/joining games, betting, and utilities.

use std::time::Duration;
use tokio::time::sleep;
use log::info;

use bitcraps::{
    Result, Error, GameId, PeerId, CrapTokens, BetType,
    TREASURY_ADDRESS, PacketUtils, GameCrypto,
};

use crate::app_config::{parse_bet_type, parse_game_id, format_game_id};
use crate::app_state::{BitCrapsApp, GameInfo, AppStats};
use bitcraps::protocol::craps::{CrapsGame, BetValidator};

/// Command execution trait for BitCraps operations
pub trait CommandExecutor {
    /// Create a new craps game
    async fn create_game(&self, buy_in_crap: u64) -> Result<GameId>;
    
    /// Join an existing game
    async fn join_game(&self, game_id: GameId) -> Result<()>;
    
    /// Place a bet in a game
    async fn place_bet(&self, game_id: GameId, bet_type: BetType, amount_crap: u64) -> Result<()>;
    
    /// Get wallet balance
    async fn get_balance(&self) -> u64;
    
    /// List active games with basic info
    async fn list_games(&self) -> Vec<(GameId, GameInfo)>;
    
    /// Send discovery ping
    async fn send_ping(&self) -> Result<()>;
    
    /// Get network and application statistics
    async fn get_stats(&self) -> AppStats;
}

impl CommandExecutor for BitCrapsApp {
    /// Create a new craps game
    async fn create_game(&self, buy_in_crap: u64) -> Result<GameId> {
        info!("🎲 Creating new craps game with {} CRAP buy-in...", buy_in_crap);
        
        let game_id = GameCrypto::generate_game_id();
        let buy_in = CrapTokens::from_crap(buy_in_crap as f64)?;
        
        // Create game instance
        let mut game = CrapsGame::new(game_id, self.identity.peer_id);
        
        // Add treasury to game automatically if configured
        if self.config.enable_treasury {
            game.add_player(TREASURY_ADDRESS);
            info!("🏦 Treasury automatically joined game");
        }
        
        // Store game
        self.active_games.write().await.insert(game_id, game);
        
        // Broadcast game creation
        let packet = PacketUtils::create_game_create(
            self.identity.peer_id,
            game_id,
            8, // max players
            buy_in,
        );
        
        self.mesh_service.broadcast_packet(packet).await?;
        
        info!("✅ Game created: {:?}", game_id);
        Ok(game_id)
    }
    
    /// Join an existing game
    async fn join_game(&self, game_id: GameId) -> Result<()> {
        info!("🎯 Joining game: {:?}", game_id);
        
        let mut games = self.active_games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;
        
        if !game.add_player(self.identity.peer_id) {
            return Err(Error::GameError("Failed to join game - already a player or game full".to_string()));
        }
        
        info!("✅ Joined game: {:?}", game_id);
        Ok(())
    }
    
    /// Place a bet in a game
    async fn place_bet(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount_crap: u64,
    ) -> Result<()> {
        info!("💰 Placing bet: {:?} - {} CRAP on {:?}", 
             game_id, amount_crap, bet_type);
        
        let amount = CrapTokens::from_crap(amount_crap as f64)?;
        
        // Check balance first
        let balance = self.ledger.get_balance(&self.identity.peer_id).await;
        if balance < amount.amount() {
            return Err(Error::InvalidBet(
                format!("Insufficient balance: {} CRAP required, {} CRAP available",
                        amount.to_crap(), CrapTokens::new_unchecked(balance).to_crap())
            ));
        }
        
        // Process bet through ledger
        let bet_type_u8 = Self::bet_type_to_u8(&bet_type);
        
        self.ledger.process_game_bet(
            self.identity.peer_id,
            amount.amount(),
            game_id,
            bet_type_u8,
        ).await?;
        
        // Add bet to game
        let mut games = self.active_games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;
        
        // Generate bet ID with proper error handling
        let bet_id_bytes = GameCrypto::generate_random_bytes(16);
        let bet_id: [u8; 16] = bet_id_bytes.try_into()
            .map_err(|_| Error::Crypto("Failed to generate bet ID".to_string()))?;
        
        // Get timestamp with fallback
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        
        let bet = bitcraps::protocol::Bet {
            id: bet_id,
            game_id,
            player: self.identity.peer_id,
            bet_type,
            amount,
            timestamp,
        };
        
        game.place_bet(self.identity.peer_id, bet).map_err(|e| Error::InvalidBet(e.to_string()))?;
        
        info!("✅ Bet placed successfully");
        Ok(())
    }
    
    /// Get wallet balance
    async fn get_balance(&self) -> u64 {
        self.ledger.get_balance(&self.identity.peer_id).await
    }
    
    /// List active games with basic info
    async fn list_games(&self) -> Vec<(GameId, GameInfo)> {
        let games = self.active_games.read().await;
        games.iter()
            .map(|(id, game)| (*id, GameInfo {
                phase: format!("{:?}", game.phase),
                players: game.participants.len(),
                rolls: game.roll_count,
            }))
            .collect()
    }
    
    /// Send discovery ping
    async fn send_ping(&self) -> Result<()> {
        let packet = PacketUtils::create_ping(self.identity.peer_id);
        self.mesh_service.broadcast_packet(packet).await?;
        info!("📡 Ping sent to discover peers");
        Ok(())
    }
    
    /// Get network and application statistics
    async fn get_stats(&self) -> AppStats {
        self.get_stats().await
    }
}

impl BitCrapsApp {
    /// Convert BetType enum to u8 for ledger processing
    fn bet_type_to_u8(bet_type: &BetType) -> u8 {
        match bet_type {
            BetType::Pass => 0,
            BetType::DontPass => 1,
            BetType::Come => 2,
            BetType::DontCome => 3,
            BetType::Field => 4,
            BetType::OddsPass => 5,
            BetType::OddsDontPass => 6,
            
            // YES bets
            BetType::Yes2 => 10,
            BetType::Yes3 => 11,
            BetType::Yes4 => 12,
            BetType::Yes5 => 13,
            BetType::Yes6 => 14,
            BetType::Yes8 => 15,
            BetType::Yes9 => 16,
            BetType::Yes10 => 17,
            BetType::Yes11 => 18,
            BetType::Yes12 => 19,
            
            // NO bets
            BetType::No2 => 20,
            BetType::No3 => 21,
            BetType::No4 => 22,
            BetType::No5 => 23,
            BetType::No6 => 24,
            BetType::No8 => 25,
            BetType::No9 => 26,
            BetType::No10 => 27,
            BetType::No11 => 28,
            BetType::No12 => 29,
            
            // Hardway bets
            BetType::Hard4 => 30,
            BetType::Hard6 => 31,
            BetType::Hard8 => 32,
            BetType::Hard10 => 33,
            
            // NEXT bets
            BetType::Next2 => 40,
            BetType::Next3 => 41,
            BetType::Next4 => 42,
            BetType::Next5 => 43,
            BetType::Next6 => 44,
            BetType::Next7 => 45,
            BetType::Next8 => 46,
            BetType::Next9 => 47,
            BetType::Next10 => 48,
            BetType::Next11 => 49,
            BetType::Next12 => 50,
            
            // Special bets
            BetType::Fire => 60,
            BetType::BonusSmall => 61,
            BetType::BonusTall => 62,
            BetType::BonusAll => 63,
            BetType::HotRoller => 64,
            BetType::TwiceHard => 65,
            BetType::RideLine => 66,
            BetType::Muggsy => 67,
            BetType::Replay => 68,
            BetType::DifferentDoubles => 69,
            
            // Repeater bets
            BetType::Repeater2 => 70,
            BetType::Repeater3 => 71,
            BetType::Repeater4 => 72,
            BetType::Repeater5 => 73,
            BetType::Repeater6 => 74,
            BetType::Repeater8 => 75,
            BetType::Repeater9 => 76,
            BetType::Repeater10 => 77,
            BetType::Repeater11 => 78,
            BetType::Repeater12 => 79,
            
            // Missing odds bet types
            BetType::OddsCome => 80,
            BetType::OddsDontCome => 81,
        }
    }
}

/// High-level command processing functions
pub mod commands {
    use super::*;
    
    /// Execute the create game command
    pub async fn create_game_command(app: &BitCrapsApp, buy_in: u64) -> Result<()> {
        let game_id = app.create_game(buy_in).await?;
        println!("✅ Game created: {}", format_game_id(game_id));
        println!("📋 Share this Game ID with other players to join");
        Ok(())
    }
    
    /// Execute the join game command
    pub async fn join_game_command(app: &BitCrapsApp, game_id_str: &str) -> Result<()> {
        let game_id = parse_game_id(game_id_str)
            .map_err(|e| Error::Protocol(e))?;
        
        app.join_game(game_id).await?;
        println!("✅ Joined game: {}", format_game_id(game_id));
        Ok(())
    }
    
    /// Execute the place bet command
    pub async fn place_bet_command(
        app: &BitCrapsApp, 
        game_id_str: &str, 
        bet_type_str: &str, 
        amount: u64
    ) -> Result<()> {
        let game_id = parse_game_id(game_id_str)
            .map_err(|e| Error::Protocol(e))?;
        
        let bet_type = parse_bet_type(bet_type_str)
            .map_err(|e| Error::Protocol(e))?;
        
        app.place_bet(game_id, bet_type, amount).await?;
        println!("✅ Bet placed: {} CRAP on {:?}", amount, bet_type);
        Ok(())
    }
    
    /// Execute the balance command
    pub async fn balance_command(app: &BitCrapsApp) -> Result<()> {
        let balance = app.get_balance().await;
        println!("💰 Current balance: {} CRAP", CrapTokens::new_unchecked(balance).to_crap());
        Ok(())
    }
    
    /// Execute the list games command
    pub async fn list_games_command(app: &BitCrapsApp) -> Result<()> {
        let games = app.list_games().await;
        if games.is_empty() {
            println!("🎲 No active games found");
        } else {
            println!("🎲 Active games:");
            for (game_id, stats) in games {
                println!("  📋 Game: {}", format_game_id(game_id));
                println!("    👥 Players: {}", stats.players);
                println!("    🎯 Phase: {}", stats.phase);
                println!("    🎲 Rolls: {}", stats.rolls);
                println!();
            }
        }
        Ok(())
    }
    
    /// Execute the stats command
    pub async fn stats_command(app: &BitCrapsApp) -> Result<()> {
        let stats = app.get_stats().await;
        println!("📊 BitCraps Node Statistics:");
        println!("  🆔 Peer ID: {:?}", stats.peer_id);
        println!("  🔗 Connected Peers: {}", stats.connected_peers);
        println!("  🔐 Active Sessions: {}", stats.active_sessions);
        println!("  💰 Balance: {} CRAP", CrapTokens::new_unchecked(stats.balance).to_crap());
        println!("  🎲 Active Games: {}", stats.active_games);
        println!("  🪙 Total Supply: {} CRAP", CrapTokens::new_unchecked(stats.total_supply).to_crap());
        println!("  📡 Total Relays: {}", stats.total_relays);
        Ok(())
    }
    
    /// Execute the ping command
    pub async fn ping_command(app: &BitCrapsApp) -> Result<()> {
        app.send_ping().await?;
        println!("📡 Discovery ping sent - listening for peers...");
        
        // Wait and show discovered peers
        sleep(Duration::from_secs(5)).await;
        
        let stats = app.get_stats().await;
        println!("🔍 Discovered {} peers", stats.connected_peers);
        Ok(())
    }
}

/// Validation utilities for commands
pub mod validation {
    use super::*;
    
    /// Validate bet amount
    pub fn validate_bet_amount(amount: u64, min_bet: u64, max_bet: u64) -> Result<()> {
        if amount < min_bet {
            return Err(Error::InvalidBet(
                format!("Minimum bet is {} CRAP", min_bet)
            ));
        }
        
        if amount > max_bet {
            return Err(Error::InvalidBet(
                format!("Maximum bet is {} CRAP", max_bet)
            ));
        }
        
        Ok(())
    }
    
    /// Validate game ID format
    pub fn validate_game_id(game_id_str: &str) -> Result<GameId> {
        parse_game_id(game_id_str).map_err(|e| Error::Protocol(e))
    }
    
    /// Validate bet type for current game phase
    pub fn validate_bet_for_phase(bet_type: &BetType, game: &CrapsGame) -> Result<()> {
        
        if !bet_type.is_valid_for_phase(&game.phase) {
            return Err(Error::InvalidBet(
                format!("Bet type {:?} not allowed in phase {:?}", 
                        bet_type, game.phase)
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bet_type_conversion() {
        assert_eq!(BitCrapsApp::bet_type_to_u8(&BetType::Pass), 0);
        assert_eq!(BitCrapsApp::bet_type_to_u8(&BetType::DontPass), 1);
        assert_eq!(BitCrapsApp::bet_type_to_u8(&BetType::Field), 4);
        assert_eq!(BitCrapsApp::bet_type_to_u8(&BetType::Yes4), 12);
        assert_eq!(BitCrapsApp::bet_type_to_u8(&BetType::Fire), 60);
    }
    
    #[test]
    fn test_bet_amount_validation() {
        assert!(validation::validate_bet_amount(10, 1, 100).is_ok());
        assert!(validation::validate_bet_amount(0, 1, 100).is_err());
        assert!(validation::validate_bet_amount(200, 1, 100).is_err());
    }
    
    #[test]
    fn test_game_id_validation() {
        let valid_id = "0123456789abcdef0123456789abcdef";
        assert!(validation::validate_game_id(valid_id).is_ok());
        
        assert!(validation::validate_game_id("invalid").is_err());
        assert!(validation::validate_game_id("").is_err());
    }
}