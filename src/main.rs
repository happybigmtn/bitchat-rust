use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use clap::{Parser, Subcommand};
use log::{info, warn};

use bitcraps::{
    BitchatIdentity, ProofOfWork, BluetoothTransport, TransportCoordinator,
    MeshService, SessionManager as BitchatSessionManager, TokenLedger, ProofOfRelay,
    TreasuryParticipant, CrapsGame, BluetoothDiscovery, DhtDiscovery,
    PersistenceManager, GameRuntime,
    AppConfig, Result, Error, GameId, PeerId, CrapTokens, BetType,
    TREASURY_ADDRESS, PacketUtils, GameCrypto, BitchatPacket,
};

/// Simple struct for game info display
#[derive(Debug, Clone)]
pub struct GameInfo {
    phase: String,
    players: usize,
    rolls: u64,
}

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

/// Main BitCraps application
/// 
/// Feynman: This is the master conductor that brings the whole
/// orchestra together. Each component is like a different section
/// (strings, brass, percussion), and the conductor ensures they
/// all play in harmony to create the complete casino experience.
pub struct BitCrapsApp {
    identity: Arc<BitchatIdentity>,
    transport_coordinator: Arc<TransportCoordinator>,
    mesh_service: Arc<MeshService>,
    session_manager: Arc<BitchatSessionManager>,
    ledger: Arc<TokenLedger>,
    game_runtime: Arc<GameRuntime>,
    discovery: Arc<BluetoothDiscovery>,
    persistence: Arc<PersistenceManager>,
    proof_of_relay: Arc<ProofOfRelay>,
    // ui: Option<TerminalUI>, // Removed until UI is implemented
    config: AppConfig,
    active_games: Arc<tokio::sync::RwLock<std::collections::HashMap<GameId, CrapsGame>>>,
}

impl BitCrapsApp {
    pub async fn new(config: AppConfig) -> Result<Self> {
        println!("🎲 Initializing BitCraps...");
        
        // Step 1: Generate or load identity with PoW
        println!("⛏️ Generating identity with proof-of-work (difficulty: {})...", 
                 config.pow_difficulty);
        let identity = Arc::new(
            BitchatIdentity::generate_with_pow(config.pow_difficulty)
        );
        println!("✅ Identity generated: {:?}", identity.peer_id);
        
        // Step 2: Initialize persistence
        println!("💾 Initializing persistence layer...");
        let persistence = Arc::new(
            PersistenceManager::new(&config.data_dir).await?
        );
        
        // Step 3: Initialize token ledger with treasury
        println!("💰 Initializing token ledger and treasury...");
        let ledger = Arc::new(TokenLedger::new());
        let treasury_balance = ledger.get_balance(&TREASURY_ADDRESS).await;
        println!("✅ Treasury initialized with {} CRAP tokens", 
                 treasury_balance / 1_000_000);
        
        // Step 4: Setup transport layer
        println!("📡 Setting up Bluetooth transport...");
        let mut transport_coordinator = TransportCoordinator::new();
        transport_coordinator.init_bluetooth(identity.peer_id).await
            .map_err(|e| Error::Network(format!("Failed to initialize Bluetooth: {}", e)))?;
        let transport_coordinator = Arc::new(transport_coordinator);
        
        // Step 5: Initialize mesh service
        println!("🕸️ Starting mesh networking service...");
        let session_manager = BitchatSessionManager::new(Default::default());
        let mesh_service = Arc::new(
            MeshService::new(identity.clone(), transport_coordinator.clone())
        );
        
        // Step 6: Setup discovery
        println!("🔍 Starting peer discovery...");
        let discovery = Arc::new(
            BluetoothDiscovery::new(identity.clone()).await
                .map_err(|e| Error::Network(e.to_string()))?
        );
        
        // Step 7: Initialize game runtime with treasury
        println!("🎰 Starting game runtime with treasury participant...");
        let (game_runtime, _game_sender) = GameRuntime::new(Default::default());
        let game_runtime = Arc::new(game_runtime);
        
        // Step 8: Setup proof-of-relay consensus
        println!("⚡ Initializing proof-of-relay consensus...");
        let proof_of_relay = Arc::new(ProofOfRelay::new(ledger.clone()));
        
        // Step 9: Start mesh service
        mesh_service.start().await?;
        
        println!("🚀 BitCraps node ready!");
        println!("📱 Peer ID: {:?}", identity.peer_id);
        if let Some(nick) = &config.nickname {
            println!("👤 Nickname: {}", nick);
        }
        
        Ok(Self {
            identity: identity.clone(),
            transport_coordinator,
            mesh_service,
            session_manager: Arc::new(session_manager),
            ledger,
            game_runtime,
            discovery,
            persistence,
            proof_of_relay,
            // ui: None, // Removed until UI is implemented
            config,
            active_games: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }
    
    /// Start the main application loop
    /// 
    /// Feynman: Like opening the casino doors - all systems are go,
    /// the dealers are at their tables, the lights are on, and we're
    /// ready for players. The main loop keeps everything running smoothly.
    pub async fn start(&mut self) -> Result<()> {
        // Start relay reward timer
        self.start_mining_rewards().await?;
        
        // Start UI if in interactive mode (TODO: implement UI)
        // if self.ui.is_none() {
        //     let ui = TerminalUI::new(
        //         self.identity.clone(),
        //         self.ledger.clone(),
        //         self.game_runtime.clone(),
        //     );
        //     self.ui = Some(ui);
        // }
        
        // Start background tasks
        self.start_heartbeat().await;
        self.start_game_coordinator().await;
        
        info!("✅ BitCraps node started successfully!");
        info!("📡 Peer ID: {:?}", self.identity.peer_id);
        info!("💼 Balance: {} CRAP", 
             CrapTokens::new(self.ledger.get_balance(&self.identity.peer_id).await).to_crap());
        info!("🎲 Ready to play craps!");
        
        // Keep running until shutdown
        loop {
            sleep(Duration::from_secs(1)).await;
            self.periodic_tasks().await?;
        }
        
        println!("👋 Shutting down BitCraps...");
        self.shutdown().await?;
        
        Ok(())
    }
    
    /// Create a new craps game
    pub async fn create_game(&self, buy_in_crap: u64) -> Result<GameId> {
        info!("🎲 Creating new craps game with {} CRAP buy-in...", buy_in_crap);
        
        let game_id = bitcraps::GameCrypto::generate_game_id();
        let buy_in = CrapTokens::from_crap(buy_in_crap as f64);
        
        // Create game instance
        let mut game = CrapsGame::new(game_id, self.identity.peer_id);
        // Buy-in is managed at the runtime level, not directly on the game
        
        // Add treasury to game automatically
        // Add treasury if configured
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
    pub async fn join_game(&self, game_id: GameId) -> Result<()> {
        info!("🎯 Joining game: {:?}", game_id);
        
        let mut games = self.active_games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or_else(|| Error::Protocol("Game not found".to_string()))?;
        
        if !game.add_player(self.identity.peer_id) {
            return Err(Error::GameError("Failed to join game".to_string()));
        }
        
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
        
        // Generate bet ID with proper error handling
        let bet_id_bytes = bitcraps::GameCrypto::generate_random_bytes(16);
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
        
        game.place_bet(self.identity.peer_id, bet).map_err(|e| Error::InvalidBet(e))?;
        
        info!("✅ Bet placed successfully");
        Ok(())
    }
    
    /// Get wallet balance
    pub async fn get_balance(&self) -> u64 {
        self.ledger.get_balance(&self.identity.peer_id).await
    }
    
    /// List active games with basic info
    pub async fn list_games(&self) -> Vec<(GameId, GameInfo)> {
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
    pub async fn send_ping(&self) -> Result<()> {
        let packet = PacketUtils::create_ping(self.identity.peer_id);
        self.mesh_service.broadcast_packet(packet).await?;
        info!("📡 Ping sent to discover peers");
        Ok(())
    }
    
    /// Get network and application statistics
    pub async fn get_stats(&self) -> AppStats {
        let mesh_stats = self.mesh_service.get_stats().await;
        let session_stats = self.session_manager.get_stats().await;
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
    
    
    /// Start mining rewards for network participation
    /// 
    /// Feynman: Like getting paid for being a good citizen - the more
    /// you help the network (relay messages, store data, host games),
    /// the more tokens you earn. It's capitalism for routers!
    async fn start_mining_rewards(&self) -> Result<()> {
        let ledger = self.ledger.clone();
        let peer_id = self.identity.peer_id;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Process staking rewards (existing method)
                if let Err(e) = ledger.distribute_staking_rewards().await {
                    warn!("Failed to distribute mining rewards: {}", e);
                } else {
                    println!("⛏️ Processed mining rewards for network participation");
                }
            }
        });
        
        Ok(())
    }
    
    /// Start heartbeat task for periodic maintenance
    async fn start_heartbeat(&self) {
        let ledger = self.ledger.clone();
        
        tokio::spawn(async move {
            let mut heartbeat = interval(Duration::from_secs(30));
            
            loop {
                heartbeat.tick().await;
                
                // Distribute staking rewards
                if let Err(e) = ledger.distribute_staking_rewards().await {
                    warn!("Failed to distribute staking rewards: {}", e);
                }
            }
        });
    }
    
    /// Start game coordinator for managing active games
    async fn start_game_coordinator(&self) {
        let game_runtime = self.game_runtime.clone();
        
        tokio::spawn(async move {
            let mut coordinator = interval(Duration::from_secs(10));
            
            loop {
                coordinator.tick().await;
                
                // Game management logic would go here
                info!("🎲 Game coordinator running...");
            }
        });
    }
    
    // async fn handle_game_event(&self, _event: crate::gaming::GameEvent) -> Result<()> {
    //     // Handle game runtime events
    //     Ok(())
    // }
    // 
    // async fn handle_discovery_event(&self, _event: crate::discovery::DiscoveryEvent) -> Result<()> {
    //     // Handle peer discovery events  
    //     Ok(())
    // }
    
    // async fn handle_ui_event(&self, event: UIEvent) -> Result<()> {
    //     // Handle terminal UI events
    //     Ok(())
    // }
    
    async fn periodic_tasks(&self) -> Result<()> {
        // Perform periodic maintenance tasks
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<()> {
        // Save state
        self.persistence.flush().await?;
        
        // Services will be stopped automatically when dropped
        println!("✅ BitCraps shutdown complete");
        
        Ok(())
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
            
            let mut app = BitCrapsApp::new(config).await?;
            app.start().await?;
        }
        
        Commands::CreateGame { buy_in } => {
            let mut app = BitCrapsApp::new(config).await?;
            
            let game_id = app.create_game(buy_in).await?;
            println!("✅ Game created: {:?}", game_id);
            println!("📋 Share this Game ID with other players to join");
            
            // Start the main loop
            app.start().await?;
        }
        
        Commands::JoinGame { game_id } => {
            let game_id_bytes = hex::decode(&game_id)
                .map_err(|_| Error::Protocol("Invalid game ID format".to_string()))?;
            
            if game_id_bytes.len() != 16 {
                return Err(Error::Protocol("Game ID must be 16 bytes".to_string()));
            }
            
            let mut game_id_array = [0u8; 16];
            game_id_array.copy_from_slice(&game_id_bytes);
            
            let mut app = BitCrapsApp::new(config).await?;
            
            // Join the game
            app.join_game(game_id_array).await?;
            println!("✅ Joined game: {:?}", game_id);
            
            // Start the main loop
            app.start().await?;
        }
        
        Commands::Balance => {
            let app = BitCrapsApp::new(config).await?;
            let balance = app.get_balance().await;
            println!("💰 Current balance: {} CRAP", CrapTokens::new(balance).to_crap());
        }
        
        Commands::Games => {
            let app = BitCrapsApp::new(config).await?;
            
            let games = app.list_games().await;
            if games.is_empty() {
                println!("🎲 No active games found");
            } else {
                println!("🎲 Active games:");
                for (game_id, stats) in games {
                    println!("  📋 Game: {:?}", game_id);
                    println!("    👥 Players: {}", stats.players);
                    println!("    🎯 Phase: {:?}", stats.phase);
                    println!("    🎲 Rolls: {}", stats.rolls);
                    println!();
                }
            }
        }
        
        Commands::Bet { game_id, bet_type, amount } => {
            let game_id_bytes = hex::decode(&game_id)
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
            
            let mut app = BitCrapsApp::new(config).await?;
            
            app.place_bet(game_id_array, bet_type_enum, amount).await?;
            println!("✅ Bet placed: {} CRAP on {:?}", amount, bet_type_enum);
            
            app.start().await?;
        }
        
        Commands::Stats => {
            let app = BitCrapsApp::new(config).await?;
            
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