use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use clap::{Parser, Subcommand};
use log::{info, warn, error};

use bitcraps::{
    BitchatIdentity, TransportCoordinator, MeshService, SessionManager, 
    TokenLedger, ProofOfRelay, CrapsGame, TreasuryParticipant, 
    AppConfig, Result, Error, GameId, PeerId, CrapTokens, BetType,
    TREASURY_ADDRESS, PacketUtils, PACKET_TYPE_GAME_CREATE,
};

#[derive(Parser)]
#[command(name = "bitcraps")]
#[command(about = "Decentralized craps casino over Bluetooth mesh")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, default_value = "~/.bitcraps")]
    data_dir: String,
    
    #[arg(short, long)]
    nickname: Option<String>,
    
    #[arg(long, default_value = "16")]
    pow_difficulty: u32,
    
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the BitCraps node
    Start,
    /// Create a new game
    CreateGame { 
        #[arg(default_value = "10")]
        buy_in: u64 
    },
    /// Join an existing game by ID
    JoinGame { game_id: String },
    /// Show wallet balance
    Balance,
    /// List active games
    Games,
    /// Place a bet in active game
    Bet {
        #[arg(long)]
        game_id: String,
        #[arg(long)]
        bet_type: String,
        #[arg(long)]
        amount: u64,
    },
    /// Show network statistics
    Stats,
    /// Send test ping to discover peers
    Ping,
}

/// Main BitCraps application bringing all components together
pub struct BitCrapsApp {
    identity: Arc<BitchatIdentity>,
    transport: Arc<TransportCoordinator>,
    mesh: Arc<MeshService>,
    sessions: Arc<SessionManager>,
    ledger: Arc<TokenLedger>,
    proof_of_relay: Arc<ProofOfRelay>,
    treasury: Option<Arc<TreasuryParticipant>>,
    config: AppConfig,
    active_games: Arc<tokio::sync::RwLock<std::collections::HashMap<GameId, CrapsGame>>>,
}

impl BitCrapsApp {
    /// Initialize the complete BitCraps application
    pub async fn new(config: AppConfig) -> Result<Self> {
        info!("🎲 Initializing BitCraps Casino...");
        
        // Step 1: Generate or load identity with proof-of-work
        info!("⛏️  Generating identity with proof-of-work (difficulty: {})...", config.pow_difficulty);
        let identity = Arc::new(BitchatIdentity::generate_with_pow(config.pow_difficulty));
        info!("✅ Identity generated: {:?}", identity.peer_id);
        
        // Step 2: Initialize transport layer
        info!("🔗 Initializing Bluetooth mesh transport...");
        let mut transport = TransportCoordinator::new();
        transport.init_bluetooth(identity.peer_id).await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;
        let transport = Arc::new(transport);
        info!("✅ Transport layer ready");
        
        // Step 3: Initialize mesh networking
        info!("🕸️  Setting up mesh networking...");
        let mesh = Arc::new(MeshService::new(identity.clone(), transport.clone()));
        info!("✅ Mesh service ready");
        
        // Step 4: Initialize session management
        info!("🔐 Setting up encrypted sessions...");
        let sessions = Arc::new(SessionManager::new(
            bitcraps::SessionLimits::default()
        ));
        info!("✅ Session manager ready");
        
        // Step 5: Initialize token ledger
        info!("💰 Initializing CRAP token ledger...");
        let ledger = Arc::new(TokenLedger::new());
        
        // Wait for treasury initialization
        sleep(Duration::from_millis(100)).await;
        
        let treasury_balance = ledger.get_balance(&TREASURY_ADDRESS).await;
        info!("✅ Token ledger ready - Treasury: {} CRAP", 
             CrapTokens::new(treasury_balance).to_crap());
        
        // Step 6: Initialize proof-of-relay mining
        info!("⛏️  Setting up proof-of-relay mining...");
        let proof_of_relay = Arc::new(ProofOfRelay::new(ledger.clone()));
        info!("✅ Proof-of-relay ready");
        
        // Step 7: Initialize treasury participant (if enabled)
        let treasury = if config.enable_treasury {
            info!("🏦 Initializing treasury participant...");
            let treasury_participant = Arc::new(TreasuryParticipant::new(treasury_balance));
            info!("✅ Treasury participant ready");
            Some(treasury_participant)
        } else {
            None
        };
        
        Ok(Self {
            identity,
            transport,
            mesh,
            sessions,
            ledger,
            proof_of_relay,
            treasury,
            config,
            active_games: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }
    
    /// Start the BitCraps application
    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting BitCraps node...");
        
        // Start all services
        self.mesh.start().await?;
        
        // Start background tasks
        self.start_heartbeat().await;
        self.start_game_coordinator().await;
        
        info!("✅ BitCraps node started successfully!");
        info!("📡 Peer ID: {:?}", self.identity.peer_id);
        info!("💼 Balance: {} CRAP", 
             CrapTokens::new(self.ledger.get_balance(&self.identity.peer_id).await).to_crap());
        info!("🎲 Ready to play craps!");
        
        Ok(())
    }
    
    /// Create a new craps game
    pub async fn create_game(&self, buy_in_crap: u64) -> Result<GameId> {
        info!("🎲 Creating new craps game with {} CRAP buy-in...", buy_in_crap);
        
        let game_id = bitcraps::GameCrypto::generate_game_id();
        let buy_in = CrapTokens::from_crap(buy_in_crap as f64);
        
        // Create game instance
        let mut game = CrapsGame::new(game_id, self.identity.peer_id);
        game.buy_in = buy_in;
        
        // Add treasury to game automatically
        if self.treasury.is_some() {
            game.add_player(TREASURY_ADDRESS)?;
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
        
        self.mesh.broadcast_packet(packet).await?;
        
        info!("✅ Game created: {:?}", game_id);
        Ok(game_id)
    }
    
    /// Join an existing game
    pub async fn join_game(&self, game_id: GameId) -> Result<()> {
        info!("🎯 Joining game: {:?}", game_id);
        
        let mut games = self.active_games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;
        
        game.add_player(self.identity.peer_id)?;
        
        info!("✅ Joined game: {:?}", game_id);
        Ok(())
    }
    
    /// Place a bet in a game
    pub async fn place_bet(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount_crap: u64,
    ) -> Result<()> {
        info!("💰 Placing bet: {:?} - {} CRAP on {:?}", 
             game_id, amount_crap, bet_type);
        
        let amount = CrapTokens::from_crap(amount_crap as f64);
        
        // Process bet through ledger
        let bet_type_u8 = match bet_type {
            BetType::Pass => 0,
            BetType::DontPass => 1,
            BetType::Field => 2,
            _ => 0,
        };
        
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
        
        let bet = bitcraps::protocol::Bet {
            id: bitcraps::GameCrypto::generate_random_bytes(16).try_into().unwrap(),
            game_id,
            player: self.identity.peer_id,
            bet_type,
            amount,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        game.place_bet(bet)?;
        
        info!("✅ Bet placed successfully");
        Ok(())
    }
    
    /// Get wallet balance
    pub async fn get_balance(&self) -> u64 {
        self.ledger.get_balance(&self.identity.peer_id).await
    }
    
    /// List active games
    pub async fn list_games(&self) -> Vec<(GameId, bitcraps::gaming::GameStats)> {
        let games = self.active_games.read().await;
        games.iter()
            .map(|(id, game)| (*id, game.get_stats()))
            .collect()
    }
    
    /// Send discovery ping
    pub async fn send_ping(&self) -> Result<()> {
        let packet = PacketUtils::create_ping(self.identity.peer_id);
        self.mesh.broadcast_packet(packet).await?;
        info!("📡 Ping sent to discover peers");
        Ok(())
    }
    
    /// Get network and application statistics
    pub async fn get_stats(&self) -> AppStats {
        let mesh_stats = self.mesh.get_stats().await;
        let session_stats = self.sessions.get_stats().await;
        let ledger_stats = self.ledger.get_stats().await;
        let mining_stats = self.proof_of_relay.get_stats().await;
        
        let games = self.active_games.read().await;
        let active_games = games.len();
        
        AppStats {
            peer_id: self.identity.peer_id,
            connected_peers: mesh_stats.connected_peers,
            active_sessions: session_stats.active_sessions,
            balance: self.ledger.get_balance(&self.identity.peer_id).await,
            active_games,
            total_supply: ledger_stats.total_supply,
            total_relays: mining_stats.total_relays,
        }
    }
    
    /// Start heartbeat task for periodic maintenance
    async fn start_heartbeat(&self) {
        let sessions = self.sessions.clone();
        let ledger = self.ledger.clone();
        
        tokio::spawn(async move {
            let mut heartbeat = interval(Duration::from_secs(30));
            
            loop {
                heartbeat.tick().await;
                
                // Check session health
                sessions.check_session_health().await;
                
                // Distribute staking rewards
                if let Err(e) = ledger.distribute_staking_rewards().await {
                    warn!("Failed to distribute staking rewards: {}", e);
                }
            }
        });
    }
    
    /// Start game coordinator for managing active games
    async fn start_game_coordinator(&self) {
        let games = self.active_games.clone();
        
        tokio::spawn(async move {
            let mut coordinator = interval(Duration::from_secs(10));
            
            loop {
                coordinator.tick().await;
                
                // Game management logic would go here
                // For now, just log active games
                let game_count = games.read().await.len();
                if game_count > 0 {
                    info!("🎲 Managing {} active games", game_count);
                }
            }
        });
    }
}

/// Application statistics
#[derive(Debug, Clone)]
pub struct AppStats {
    pub peer_id: PeerId,
    pub connected_peers: usize,
    pub active_sessions: usize,
    pub balance: u64,
    pub active_games: usize,
    pub total_supply: u64,
    pub total_relays: u64,
}

/// Main entry point for BitCraps
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }
    
    println!("🎲 BitCraps - Decentralized Casino Protocol");
    println!("⚡ Real-time craps over Bluetooth mesh with CRAP tokens");
    println!();
    
    let config = AppConfig {
        data_dir: cli.data_dir,
        nickname: cli.nickname,
        pow_difficulty: cli.pow_difficulty,
        ..AppConfig::default()
    };
    
    match cli.command {
        Commands::Start => {
            info!("Starting BitCraps node...");
            
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            // Keep running
            loop {
                sleep(Duration::from_secs(1)).await;
            }
        }
        
        Commands::CreateGame { buy_in } => {
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            let game_id = app.create_game(buy_in).await?;
            println!("✅ Game created: {:?}", game_id);
            println!("📋 Share this Game ID with other players to join");
            
            // Keep node running
            loop {
                sleep(Duration::from_secs(1)).await;
            }
        }
        
        Commands::JoinGame { game_id } => {
            let game_id_bytes = hex::decode(game_id)
                .map_err(|_| Error::Protocol("Invalid game ID format".to_string()))?;
            
            if game_id_bytes.len() != 16 {
                return Err(Error::Protocol("Game ID must be 16 bytes".to_string()));
            }
            
            let mut game_id_array = [0u8; 16];
            game_id_array.copy_from_slice(&game_id_bytes);
            
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            sleep(Duration::from_secs(2)).await; // Wait for peer discovery
            app.join_game(game_id_array).await?;
            
            // Keep node running
            loop {
                sleep(Duration::from_secs(1)).await;
            }
        }
        
        Commands::Balance => {
            let app = BitCrapsApp::new(config).await?;
            let balance = app.get_balance().await;
            println!("💰 Current balance: {} CRAP", CrapTokens::new(balance).to_crap());
        }
        
        Commands::Games => {
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            sleep(Duration::from_secs(2)).await; // Wait for discovery
            
            let games = app.list_games().await;
            if games.is_empty() {
                println!("🎲 No active games found");
            } else {
                println!("🎲 Active games:");
                for (game_id, stats) in games {
                    println!("  📋 Game: {:?}", game_id);
                    println!("    👥 Players: {}", stats.player_count);
                    println!("    🎯 Phase: {:?}", stats.phase);
                    println!("    🎲 Rolls: {}", stats.roll_count);
                    println!();
                }
            }
        }
        
        Commands::Bet { game_id, bet_type, amount } => {
            let game_id_bytes = hex::decode(game_id)
                .map_err(|_| Error::Protocol("Invalid game ID format".to_string()))?;
            
            if game_id_bytes.len() != 16 {
                return Err(Error::Protocol("Game ID must be 16 bytes".to_string()));
            }
            
            let mut game_id_array = [0u8; 16];
            game_id_array.copy_from_slice(&game_id_bytes);
            
            let bet_type_enum = match bet_type.as_str() {
                "pass" => BetType::Pass,
                "dontpass" => BetType::DontPass,
                "field" => BetType::Field,
                _ => return Err(Error::Protocol("Invalid bet type".to_string())),
            };
            
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            app.place_bet(game_id_array, bet_type_enum, amount).await?;
            println!("✅ Bet placed: {} CRAP on {:?}", amount, bet_type_enum);
        }
        
        Commands::Stats => {
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            sleep(Duration::from_secs(2)).await; // Wait for initialization
            
            let stats = app.get_stats().await;
            println!("📊 BitCraps Node Statistics:");
            println!("  🆔 Peer ID: {:?}", stats.peer_id);
            println!("  🔗 Connected Peers: {}", stats.connected_peers);
            println!("  🔐 Active Sessions: {}", stats.active_sessions);
            println!("  💰 Balance: {} CRAP", CrapTokens::new(stats.balance).to_crap());
            println!("  🎲 Active Games: {}", stats.active_games);
            println!("  🪙 Total Supply: {} CRAP", CrapTokens::new(stats.total_supply).to_crap());
            println!("  📡 Total Relays: {}", stats.total_relays);
        }
        
        Commands::Ping => {
            let app = BitCrapsApp::new(config).await?;
            app.start().await?;
            
            app.send_ping().await?;
            println!("📡 Discovery ping sent - listening for peers...");
            
            // Wait and show discovered peers
            sleep(Duration::from_secs(5)).await;
            
            let stats = app.get_stats().await;
            println!("🔍 Discovered {} peers", stats.connected_peers);
        }
    }
    
    Ok(())
}