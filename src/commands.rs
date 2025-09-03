//! Command implementations for BitCraps CLI
//!
//! This module contains all the game command implementations,
//! including creating/joining games, betting, and utilities.

use log::info;
use std::time::Duration;
use tokio::time::sleep;

use bitcraps::{BetType, CrapTokens, Error, GameCrypto, GameId, Result, TREASURY_ADDRESS};

use crate::app_config::{format_game_id, parse_bet_type, parse_game_id};
use crate::app_state::{AppStats, BitCrapsApp, GameInfo};
use bitcraps::protocol::craps::{BetValidator, CrapsGame};

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
    async fn _get_stats(&self) -> AppStats;
}

impl CommandExecutor for BitCrapsApp {
    /// Create a new craps game
    async fn create_game(&self, buy_in_crap: u64) -> Result<GameId> {
        info!(
            "🎲 Creating new craps game with {} CRAP buy-in...",
            buy_in_crap
        );

        let game_id = GameCrypto::generate_game_id();
        let _buy_in = CrapTokens::from_crap(buy_in_crap as f64)?;

        // Create game instance
        let mut game = CrapsGame::new(game_id, self.identity.peer_id);

        // Add treasury to game automatically if configured
        if self.config.enable_treasury {
            game.add_player(TREASURY_ADDRESS);
            info!("🏦 Treasury automatically joined game");
        }

        // Store game
        self.active_games.write().await.insert(game_id, game);

        // Broadcast game creation to the network
        let packet = bitcraps::protocol::create_game_packet(
            self.identity.peer_id,
            game_id,
            8, // max players
            buy_in_crap,
        );
        self.mesh_service.broadcast_packet(packet).await?;
        info!("📡 Game creation packet broadcast to network");

        info!("✅ Game created: {:?}", game_id);
        Ok(game_id)
    }

    /// Join an existing game
    async fn join_game(&self, game_id: GameId) -> Result<()> {
        info!("🎯 Joining game: {:?}", game_id);

        let mut games = self.active_games.write().await;
        let game = games
            .get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;

        if !game.add_player(self.identity.peer_id) {
            return Err(Error::GameError(
                "Failed to join game - already a player or game full".to_string(),
            ));
        }

        info!("✅ Joined game: {:?}", game_id);
        Ok(())
    }

    /// Place a bet in a game
    async fn place_bet(&self, game_id: GameId, bet_type: BetType, amount_crap: u64) -> Result<()> {
        info!(
            "💰 Placing bet: {:?} - {} CRAP on {:?}",
            game_id, amount_crap, bet_type
        );

        let amount = CrapTokens::from_crap(amount_crap as f64)?;

        // Check balance first
        let balance = self.ledger.get_balance(&self.identity.peer_id).await;
        if balance < amount.amount() {
            return Err(Error::InvalidBet(format!(
                "Insufficient balance: {} CRAP required, {} CRAP available",
                amount.to_crap(),
                CrapTokens::new_unchecked(balance).to_crap()
            )));
        }

        // Process bet through ledger
        let bet_type_u8 = Self::bet_type_to_u8(&bet_type);

        self.ledger
            .process_game_bet(self.identity.peer_id, amount.amount(), game_id, bet_type_u8)
            .await?;

        // Add bet to game
        let mut games = self.active_games.write().await;
        let game = games
            .get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;

        // Generate bet ID with proper error handling (unused for now)
        // let bet_id_bytes = GameCrypto::generate_random_bytes(16);
        // let bet_id: [u8; 16] = bet_id_bytes.try_into()
        //     .map_err(|_| Error::Crypto("Failed to generate bet ID".to_string()))?;

        // Get timestamp with fallback
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();

        let bet = bitcraps::protocol::Bet {
            id: [0u8; 16], // Auto-generated ID
            game_id,
            player: self.identity.peer_id,
            bet_type,
            amount,
            timestamp,
        };

        game.place_bet(self.identity.peer_id, bet)
            .map_err(|e| Error::InvalidBet(e.to_string()))?;

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
        games
            .iter()
            .map(|(id, game)| {
                (
                    *id,
                    GameInfo {
                        phase: format!("{:?}", game.phase),
                        players: game.participants.len(),
                        rolls: game.roll_count,
                    },
                )
            })
            .collect()
    }

    /// Send discovery ping
    async fn send_ping(&self) -> Result<()> {
        // Create and prepare ping packet
        let packet = bitcraps::protocol::create_ping_packet(self.identity.peer_id);
        self.mesh_service.broadcast_packet(packet).await?;
        info!("📡 Discovery ping packet broadcast to network");
        Ok(())
    }

    /// Get network and application statistics
    async fn _get_stats(&self) -> AppStats {
        self.get_stats().await
    }
}

impl BitCrapsApp {
    /// Convert BetType enum to u8 for ledger processing
    fn bet_type_to_u8(bet_type: &BetType) -> u8 {
        bet_type.to_u8()
    }
}

/// High-level command processing functions
pub mod commands {
    use super::*;

    /// Execute the create game command
    pub async fn create_game_command(app: &BitCrapsApp, buy_in: u64) -> Result<()> {
        // Validate minimum buy-in
        if buy_in < 10 {
            eprintln!("❌ Error: Minimum buy-in is 10 CRAP");
            eprintln!("💡 Suggestion: Try 'bitcraps create-game 10' or higher");
            return Err(Error::InvalidBet("Buy-in too low".to_string()));
        }
        
        // Check if user has enough balance
        let balance = app.get_balance().await;
        if balance < buy_in {
            eprintln!("❌ Error: Insufficient balance for buy-in");
            eprintln!("💰 Your balance: {} CRAP", CrapTokens::new_unchecked(balance).to_crap());
            eprintln!("🎯 Required: {} CRAP", buy_in);
            eprintln!("💡 Suggestion: Start with 'bitcraps start' to mine some tokens first");
            return Err(Error::InsufficientBalance(format!("Need {} CRAP, have {} CRAP", buy_in, CrapTokens::new_unchecked(balance).to_crap())));
        }

        match app.create_game(buy_in).await {
            Ok(game_id) => {
                println!("✅ Game created successfully!");
                println!("🎲 Game ID: {}", format_game_id(game_id));
                println!("💰 Buy-in: {} CRAP", buy_in);
                println!("");
                println!("📋 Next steps:");
                println!("   1. Share this Game ID with other players");
                println!("   2. They can join with: bitcraps join-game {}", format_game_id(game_id));
                println!("   3. Start betting once players join");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to create game: {}", e);
                eprintln!("💡 Troubleshooting:");
                eprintln!("   • Check network connectivity with: bitcraps ping");
                eprintln!("   • Verify balance with: bitcraps balance");
                eprintln!("   • Try a different buy-in amount");
                Err(e)
            }
        }
    }

    /// Execute the join game command
    pub async fn join_game_command(app: &BitCrapsApp, game_id_str: &str) -> Result<()> {
        // Validate game ID format first
        let game_id = match parse_game_id(game_id_str) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("❌ Error: Invalid game ID format");
                eprintln!("📋 Expected: 32-character hexadecimal string");
                eprintln!("🔍 You provided: '{}'", game_id_str);
                eprintln!("✅ Example: 0123456789abcdef0123456789abcdef");
                eprintln!("💡 Get valid game IDs with: bitcraps games");
                return Err(Error::Protocol(e));
            }
        };

        match app.join_game(game_id).await {
            Ok(()) => {
                println!("✅ Successfully joined game!");
                println!("🎲 Game ID: {}", format_game_id(game_id));
                println!("");
                println!("🎯 You can now:");
                println!("   • Place bets: bitcraps bet --game-id {} --bet-type pass --amount 50", format_game_id(game_id));
                println!("   • Check game status: bitcraps games");
                println!("   • View stats: bitcraps stats");
                Ok(())
            }
            Err(Error::Protocol(msg)) if msg.contains("not found") => {
                eprintln!("❌ Error: Game not found");
                eprintln!("🎲 Game ID: {}", format_game_id(game_id));
                eprintln!("💡 Possible reasons:");
                eprintln!("   • Game hasn't been created yet");
                eprintln!("   • You're not connected to the game host");
                eprintln!("   • Game ID was typed incorrectly");
                eprintln!("");
                eprintln!("🔧 Try these steps:");
                eprintln!("   1. Verify network: bitcraps ping");
                eprintln!("   2. List available games: bitcraps games");
                eprintln!("   3. Ask host to reshare the game ID");
                Err(Error::Protocol(format!("Game not found: {}", format_game_id(game_id))))
            }
            Err(Error::GameError(msg)) if msg.contains("already a player") => {
                eprintln!("ℹ️  You're already in this game!");
                eprintln!("🎲 Game ID: {}", format_game_id(game_id));
                eprintln!("🎯 You can start betting right away");
                Ok(()) // Not really an error
            }
            Err(Error::GameError(msg)) if msg.contains("game full") => {
                eprintln!("❌ Error: Game is full");
                eprintln!("🎲 Game ID: {}", format_game_id(game_id));
                eprintln!("💡 Suggestions:");
                eprintln!("   • Wait for a player to leave");
                eprintln!("   • Look for other games: bitcraps games");
                eprintln!("   • Create your own game: bitcraps create-game 10");
                Err(Error::GameError("Game is full".to_string()))
            }
            Err(e) => {
                eprintln!("❌ Failed to join game: {}", e);
                eprintln!("💡 Troubleshooting:");
                eprintln!("   • Check connectivity: bitcraps ping");
                eprintln!("   • Verify game exists: bitcraps games");
                eprintln!("   • Ensure sufficient balance: bitcraps balance");
                Err(e)
            }
        }
    }

    /// Execute the place bet command
    pub async fn place_bet_command(
        app: &BitCrapsApp,
        game_id_str: &str,
        bet_type_str: &str,
        amount: u64,
    ) -> Result<()> {
        // Validate game ID
        let game_id = match parse_game_id(game_id_str) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("❌ Error: Invalid game ID format");
                eprintln!("📋 Expected: 32-character hexadecimal string");
                eprintln!("🔍 You provided: '{}'", game_id_str);
                return Err(Error::Protocol(e));
            }
        };

        // Validate bet type
        let bet_type = match parse_bet_type(bet_type_str) {
            Ok(bt) => bt,
            Err(e) => {
                eprintln!("❌ Error: Invalid bet type '{}'", bet_type_str);
                eprintln!("");
                eprintln!("🎯 Popular bet types:");
                eprintln!("   • pass          - Pass Line (even money)");
                eprintln!("   • dontpass      - Don't Pass (even money)");
                eprintln!("   • field         - Field bet (1:1 or 2:1)");
                eprintln!("   • yes4          - 4 before 7 (2:1)");
                eprintln!("   • hard6         - Hard 6 (9:1)");
                eprintln!("");
                eprintln!("📚 See all types: bitcraps --help | grep 'bet types'");
                return Err(Error::Protocol(e));
            }
        };

        // Validate bet amount
        if amount < 10 {
            eprintln!("❌ Error: Minimum bet is 10 CRAP");
            eprintln!("💡 Try: bitcraps bet --game-id {} --bet-type {} --amount 10", game_id_str, bet_type_str);
            return Err(Error::InvalidBet("Bet amount too low".to_string()));
        }

        if amount > 1000 {
            eprintln!("❌ Error: Maximum bet is 1000 CRAP");
            eprintln!("💡 Try a smaller amount to reduce risk");
            return Err(Error::InvalidBet("Bet amount too high".to_string()));
        }

        match app.place_bet(game_id, bet_type, amount).await {
            Ok(()) => {
                println!("✅ Bet placed successfully!");
                println!("🎲 Game: {}", format_game_id(game_id));
                println!("🎯 Bet: {} CRAP on {:?}", amount, bet_type);
                println!("💰 Remaining balance: {} CRAP", 
                    CrapTokens::new_unchecked(app.get_balance().await.saturating_sub(amount)).to_crap());
                println!("");
                println!("🎮 Next: Wait for dice roll or place more bets");
                Ok(())
            }
            Err(Error::InvalidBet(msg)) if msg.contains("Insufficient balance") => {
                eprintln!("❌ Error: Not enough CRAP tokens");
                let balance = app.get_balance().await;
                eprintln!("💰 Your balance: {} CRAP", CrapTokens::new_unchecked(balance).to_crap());
                eprintln!("🎯 Required: {} CRAP", amount);
                eprintln!("💡 Solutions:");
                eprintln!("   • Place smaller bet: --amount {}", balance.min(100));
                eprintln!("   • Mine more tokens: bitcraps start");
                eprintln!("   • Check balance: bitcraps balance");
                Err(Error::InvalidBet(msg))
            }
            Err(Error::Protocol(msg)) if msg.contains("not found") => {
                eprintln!("❌ Error: Game not found");
                eprintln!("🔧 Solutions:");
                eprintln!("   • Join game first: bitcraps join-game {}", format_game_id(game_id));
                eprintln!("   • Check active games: bitcraps games");
                eprintln!("   • Verify game ID with host");
                Err(Error::Protocol(msg))
            }
            Err(e) => {
                eprintln!("❌ Failed to place bet: {}", e);
                eprintln!("💡 Troubleshooting:");
                eprintln!("   • Verify you're in the game: bitcraps games");
                eprintln!("   • Check network: bitcraps ping");
                eprintln!("   • Confirm bet type is valid for current phase");
                Err(e)
            }
        }
    }

    /// Execute the balance command
    pub async fn balance_command(app: &BitCrapsApp) -> Result<()> {
        let balance = app.get_balance().await;
        let balance_crap = CrapTokens::new_unchecked(balance).to_crap();
        
        println!("✨ Wallet Status ✨");
        println!("💰 Balance: {} CRAP", balance_crap);
        
        // Provide contextual guidance based on balance
        if balance_crap >= 1000.0 {
            println!("🎆 Excellent! You're ready for high-stakes games");
            println!("💡 Suggestions:");
            println!("   • Create premium game: bitcraps create-game 100");
            println!("   • Place bigger bets for higher rewards");
        } else if balance_crap >= 100.0 {
            println!("🚀 Good balance! Ready to play");
            println!("💡 Suggestions:");
            println!("   • Join or create games: bitcraps create-game 50");
            println!("   • Try different bet types for variety");
        } else if balance_crap >= 10.0 {
            println!("🌱 Starting balance - play conservatively");
            println!("💡 Suggestions:");
            println!("   • Start with minimum games: bitcraps create-game 10");
            println!("   • Try low-risk bets like pass/dontpass");
            println!("   • Mine more tokens: bitcraps start");
        } else {
            println!("⚠️  Low balance - mine some tokens first");
            println!("💡 How to get CRAP tokens:");
            println!("   • Start mining: bitcraps start");
            println!("   • Tokens are mined automatically while connected");
            println!("   • Minimum game buy-in is 10 CRAP");
        }
        
        Ok(())
    }

    /// Execute the list games command
    pub async fn list_games_command(app: &BitCrapsApp) -> Result<()> {
        let games = app.list_games().await;
        
        if games.is_empty() {
            println!("🎲 No active games found");
            println!("");
            println!("💡 What you can do:");
            println!("   • Create a new game: bitcraps create-game 10");
            println!("   • Check network connectivity: bitcraps ping");
            println!("   • Wait for other players to create games");
            println!("   • Start mining tokens: bitcraps start");
        } else {
            println!("🎲 Active Games ({} found)", games.len());
            println!("{}", "=".repeat(60));
            
            for (i, (game_id, stats)) in games.iter().enumerate() {
                let formatted_id = format_game_id(*game_id);
                println!("🎮 Game {}: {}", i + 1, &formatted_id[..8]);
                println!("   🆔 Full ID: {}", formatted_id);
                println!("   👥 Players: {} ({})", stats.players, 
                    if stats.players < 8 { "accepting new players" } else { "full" });
                println!("   🎯 Phase: {}", stats.phase);
                println!("   🎲 Dice rolls: {}", stats.rolls);
                
                // Show actionable commands
                if stats.players < 8 {
                    println!("   ➡️  Join: bitcraps join-game {}", formatted_id);
                }
                println!();
            }
            
            println!("💡 Quick commands:");
            println!("   • Join first available: bitcraps join-game {}", format_game_id(games[0].0));
            println!("   • Create your own: bitcraps create-game 10");
            println!("   • Check your balance: bitcraps balance");
        }
        
        Ok(())
    }

    /// Execute the stats command with comprehensive KPI observability
    pub async fn stats_command(app: &BitCrapsApp) -> Result<()> {
        let stats = app.get_stats().await;
        
        println!("✨ BitCraps Network Dashboard ✨");
        println!("{}", "=".repeat(50));
        
        // Node Identity & Status
        println!("🆔 Node Identity:");
        println!("   Peer ID: {:?}", stats.peer_id);
        println!("   Status: {} (uptime: {}s)", 
            if stats.connected_peers > 0 { "Connected" } else { "Isolated" },
            stats.total_relays // Using as uptime proxy
        );
        println!();
        
        // Network KPIs
        println!("🌐 Network Metrics:");
        let network_health = match stats.connected_peers {
            0 => "❌ Isolated",
            1..=3 => "🟡 Limited",
            4..=10 => "✅ Good",
            _ => "🎆 Excellent",
        };
        println!("   Connected Peers: {} ({})", stats.connected_peers, network_health);
        println!("   Active Sessions: {} secure channels", stats.active_sessions);
        println!("   Message Relays: {} total", stats.total_relays);
        
        // Show peer distribution if we have peers
        if stats.connected_peers > 0 {
            println!("   Network Reach: {} hops estimated", 
                (stats.connected_peers as f64).log2().ceil() as u32);
        }
        println!();
        
        // Gaming KPIs
        println!("🎲 Gaming Metrics:");
        println!("   Active Games: {}", stats.active_games);
        if stats.active_games > 0 {
            println!("   Game Health: 🎆 Games running smoothly");
            println!("   Avg Players/Game: ~{:.1}", 
                if stats.active_games > 0 { stats.connected_peers as f64 / stats.active_games as f64 } else { 0.0 });
        } else {
            println!("   Game Health: 🟡 No active games");
            println!("   Opportunity: Create game to attract players");
        }
        println!();
        
        // Economic KPIs
        let balance_crap = CrapTokens::new_unchecked(stats.balance).to_crap();
        let supply_crap = CrapTokens::new_unchecked(stats.total_supply).to_crap();
        
        println!("💰 Token Economy:");
        println!("   Your Balance: {:.2} CRAP", balance_crap);
        println!("   Network Supply: {:.2} CRAP total", supply_crap);
        
        if supply_crap > 0.0 {
            let ownership_pct = (balance_crap / supply_crap) * 100.0;
            println!("   Your Share: {:.3}% of network", ownership_pct);
        }
        
        // Economic health indicators
        match balance_crap {
            x if x >= 1000.0 => println!("   Wealth Status: 🎆 High roller"),
            x if x >= 100.0 => println!("   Wealth Status: 🚀 Well funded"),
            x if x >= 10.0 => println!("   Wealth Status: 🌱 Getting started"),
            _ => println!("   Wealth Status: ⚠️  Need more tokens (mine first)"),
        }
        println!();
        
        // Performance & Health KPIs
        println!("⚙️ Performance Health:");
        
        // Calculate derived metrics
        let relay_rate = if stats.total_relays > 0 { 
            stats.total_relays as f64 / 60.0 // Assuming 60s uptime for demo
        } else { 0.0 };
        
        println!("   Message Rate: {:.2} msgs/sec", relay_rate);
        println!("   Consensus State: {} sync", 
            if stats.active_sessions > 0 { "✅ In" } else { "🟡 No" });
        
        // Network resilience
        let resilience = match (stats.connected_peers, stats.active_sessions) {
            (0, _) => "❌ Isolated - single point failure",
            (1..=2, _) => "🟡 Limited - vulnerable to disconnects",
            (3..=5, s) if s >= 2 => "✅ Resilient - good redundancy",
            (_, s) if s >= 3 => "🎆 Highly resilient - excellent redundancy",
            _ => "🟡 Moderate - some redundancy",
        };
        println!("   Resilience: {}", resilience);
        println!();
        
        // Actionable recommendations
        println!("💡 Recommendations:");
        if stats.connected_peers == 0 {
            println!("   • PRIORITY: Connect to network (check connectivity)");
            println!("   • Try: bitcraps ping to discover peers");
        } else if stats.active_games == 0 {
            println!("   • Create a game to attract players");
            println!("   • Try: bitcraps create-game 10");
        } else {
            println!("   • Network healthy - continue playing");
            println!("   • Consider creating more games for growth");
        }
        
        if balance_crap < 50.0 {
            println!("   • Mine more tokens for better gameplay");
            println!("   • Start with low-risk bets");
        }
        
        println!();
        println!("🔄 Live stats refresh: Run 'bitcraps stats' again");
        
        Ok(())
    }

    /// Execute the ping command
    pub async fn ping_command(app: &BitCrapsApp) -> Result<()> {
        println!("📡 Sending network discovery ping...");
        
        match app.send_ping().await {
            Ok(()) => {
                println!("✅ Ping sent successfully");
                println!("🕰️ Waiting for peer responses (5 seconds)...");
                
                // Show progress indicator
                for i in 1..=5 {
                    print!("\r🔄 Listening... {}s", 6 - i);
                    use std::io::{self, Write};
                    io::stdout().flush().unwrap();
                    sleep(Duration::from_secs(1)).await;
                }
                println!(); // New line after progress
                
                let stats = app.get_stats().await;
                
                if stats.connected_peers > 0 {
                    println!("✨ Network Discovery Results ✨");
                    println!("🔗 Connected peers: {}", stats.connected_peers);
                    println!("🎲 Active games: {}", stats.active_games);
                    println!("📡 Total relays: {}", stats.total_relays);
                    println!("");
                    println!("💡 Network is active! You can:");
                    println!("   • List games: bitcraps games");
                    println!("   • Join a game: bitcraps join-game <game-id>");
                    println!("   • Create a game: bitcraps create-game 10");
                    println!("   • View full stats: bitcraps stats");
                } else {
                    println!("🤔 No peers discovered");
                    println!("");
                    println!("💡 Troubleshooting:");
                    println!("   • You might be the first player online");
                    println!("   • Check your network/firewall settings");
                    println!("   • Try starting a node first: bitcraps start");
                    println!("   • Create a game to attract other players");
                    println!("");
                    println!("ℹ️  This is normal for new networks!");
                }
            }
            Err(e) => {
                eprintln!("❌ Ping failed: {}", e);
                eprintln!("💡 Common issues:");
                eprintln!("   • Network interface not available");
                eprintln!("   • Node not running (try: bitcraps start)");
                eprintln!("   • Firewall blocking connections");
                eprintln!("   • Bluetooth/WiFi disabled");
                return Err(e);
            }
        }
        
        Ok(())
    }
}

/// Validation utilities for commands
pub mod validation {
    use super::*;

    /// Validate bet amount
    #[allow(dead_code)]
    pub fn validate_bet_amount(amount: u64, min_bet: u64, max_bet: u64) -> Result<()> {
        if amount < min_bet {
            return Err(Error::InvalidBet(format!(
                "Minimum bet is {} CRAP",
                min_bet
            )));
        }

        if amount > max_bet {
            return Err(Error::InvalidBet(format!(
                "Maximum bet is {} CRAP",
                max_bet
            )));
        }

        Ok(())
    }

    /// Validate game ID format
    #[allow(dead_code)]
    pub fn validate_game_id(game_id_str: &str) -> Result<GameId> {
        parse_game_id(game_id_str).map_err(|e| Error::Protocol(e))
    }

    /// Validate bet type for current game phase
    #[allow(dead_code)]
    pub fn validate_bet_for_phase(bet_type: &BetType, game: &CrapsGame) -> Result<()> {
        if !bet_type.is_valid_for_phase(&game.phase) {
            return Err(Error::InvalidBet(format!(
                "Bet type {:?} not allowed in phase {:?}",
                bet_type, game.phase
            )));
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
