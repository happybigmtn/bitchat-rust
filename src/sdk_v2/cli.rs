//! CLI Tool for SDK Interaction
//!
//! Command-line interface for interacting with the BitCraps SDK,
//! including game management, testing, and development tools.

use crate::sdk_v2::{
    client::BitCrapsSDK,
    config::{Config, Environment},
    error::{SDKError, SDKResult},
    types::*,
    testing::{TestFramework, TestScenarios},
    playground::APIPlayground,
    codegen::CodeGenerator,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json;
use std::path::PathBuf;
use tokio::fs;

/// BitCraps SDK Command Line Interface
#[derive(Parser, Debug)]
#[command(name = "bitcraps-sdk")]
#[command(about = "BitCraps SDK - Developer tools and API client")]
#[command(version = "2.0.0")]
pub struct CliApp {
    /// API key for authentication
    #[arg(long, env = "BITCRAPS_API_KEY")]
    pub api_key: Option<String>,
    
    /// Environment to connect to
    #[arg(long, short = 'e', value_enum, default_value_t = Environment::Development)]
    pub environment: Environment,
    
    /// Configuration file path
    #[arg(long, short = 'c')]
    pub config: Option<PathBuf>,
    
    /// Enable verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,
    
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Game management commands
    Games {
        #[command(subcommand)]
        action: GameCommands,
    },
    /// Testing commands
    Test {
        #[command(subcommand)]
        action: TestCommands,
    },
    /// Code generation commands
    Codegen {
        #[command(subcommand)]
        action: CodegenCommands,
    },
    /// API playground commands
    Playground {
        #[command(subcommand)]
        action: PlaygroundCommands,
    },
    /// Network management commands
    Network {
        #[command(subcommand)]
        action: NetworkCommands,
    },
    /// SDK configuration commands
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    /// Interactive mode
    Interactive,
}

/// Game management commands
#[derive(Subcommand, Debug)]
pub enum GameCommands {
    /// List available games
    List {
        /// Filter by game status
        #[arg(long)]
        status: Option<GameStatus>,
        /// Maximum number of results
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Create a new game
    Create {
        /// Game name
        name: String,
        /// Game type
        #[arg(long, value_enum, default_value_t = GameType::Craps)]
        game_type: GameType,
        /// Maximum players
        #[arg(long, default_value_t = 8)]
        max_players: u32,
        /// Minimum bet amount
        #[arg(long, default_value_t = 1)]
        min_bet: u64,
        /// Maximum bet amount
        #[arg(long, default_value_t = 1000)]
        max_bet: u64,
        /// Make the game private
        #[arg(long)]
        private: bool,
    },
    /// Join an existing game
    Join {
        /// Game ID to join
        game_id: String,
    },
    /// Get game details
    Info {
        /// Game ID
        game_id: String,
    },
    /// Leave a game
    Leave {
        /// Game ID to leave
        game_id: String,
    },
    /// Place a bet in a game
    Bet {
        /// Game ID
        game_id: String,
        /// Bet type
        bet_type: String,
        /// Bet amount
        amount: u64,
    },
}

/// Testing commands
#[derive(Subcommand, Debug)]
pub enum TestCommands {
    /// Run predefined test scenarios
    Run {
        /// Scenario name (or 'all' for all scenarios)
        scenario: String,
        /// Output test report to file
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// List available test scenarios
    List,
    /// Create a custom test scenario
    Create {
        /// Test scenario file path
        file: PathBuf,
    },
    /// Validate SDK configuration
    Validate,
}

/// Code generation commands
#[derive(Subcommand, Debug)]
pub enum CodegenCommands {
    /// Generate client code for a specific language
    Generate {
        /// Target language
        #[arg(value_enum)]
        language: TargetLanguage,
        /// Output directory
        #[arg(long, short = 'o')]
        output: PathBuf,
        /// Include examples
        #[arg(long)]
        examples: bool,
    },
    /// List supported languages
    Languages,
    /// Generate OpenAPI specification
    Openapi {
        /// Output file path
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// API playground commands
#[derive(Subcommand, Debug)]
pub enum PlaygroundCommands {
    /// Start interactive API playground
    Start {
        /// Port to run playground on
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// Open browser automatically
        #[arg(long)]
        open: bool,
    },
    /// Generate playground configuration
    Config {
        /// Output file path
        output: PathBuf,
    },
}

/// Network management commands
#[derive(Subcommand, Debug)]
pub enum NetworkCommands {
    /// List connected peers
    Peers,
    /// Connect to a peer
    Connect {
        /// Peer address
        address: String,
    },
    /// Disconnect from a peer
    Disconnect {
        /// Peer ID
        peer_id: String,
    },
    /// Show network statistics
    Stats,
    /// Test network connectivity
    Ping {
        /// Peer ID to ping
        peer_id: String,
    },
}

/// Configuration commands
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set configuration values
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Initialize configuration file
    Init {
        /// Output configuration file path
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// Output formats
#[derive(ValueEnum, Debug, Clone)]
pub enum OutputFormat {
    Json,
    Yaml,
    Table,
    Pretty,
}

/// Target languages for code generation
#[derive(ValueEnum, Debug, Clone)]
pub enum TargetLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Swift,
}

/// CLI application implementation
impl CliApp {
    /// Run the CLI application
    pub async fn run(&self) -> SDKResult<()> {
        let config = self.load_config().await?;
        let sdk = BitCrapsSDK::new(config).await?;
        
        match &self.command {
            Commands::Games { action } => self.handle_game_commands(&sdk, action).await,
            Commands::Test { action } => self.handle_test_commands(action).await,
            Commands::Codegen { action } => self.handle_codegen_commands(action).await,
            Commands::Playground { action } => self.handle_playground_commands(action).await,
            Commands::Network { action } => self.handle_network_commands(&sdk, action).await,
            Commands::Config { action } => self.handle_config_commands(action).await,
            Commands::Interactive => self.start_interactive_mode(&sdk).await,
        }
    }
    
    /// Load SDK configuration
    async fn load_config(&self) -> SDKResult<Config> {
        let mut config_builder = Config::builder()
            .environment(self.environment);
        
        // Load from config file if provided
        if let Some(config_path) = &self.config {
            let config_content = fs::read_to_string(config_path).await
                .map_err(|e| SDKError::ConfigurationError(format!("Failed to read config file: {}", e)))?;
            
            let file_config: serde_json::Value = serde_json::from_str(&config_content)
                .map_err(|e| SDKError::ConfigurationError(format!("Invalid config file: {}", e)))?;
            
            if let Some(api_key) = file_config.get("api_key").and_then(|v| v.as_str()) {
                config_builder = config_builder.api_key(api_key);
            }
        }
        
        // Override with command line API key if provided
        if let Some(api_key) = &self.api_key {
            config_builder = config_builder.api_key(api_key);
        }
        
        config_builder.build()
    }
    
    /// Handle game management commands
    async fn handle_game_commands(&self, sdk: &BitCrapsSDK, action: &GameCommands) -> SDKResult<()> {
        match action {
            GameCommands::List { status, limit } => {
                let filters = if let Some(status) = status {
                    Some(GameFilters {
                        status: Some(*status),
                        ..Default::default()
                    })
                } else {
                    None
                };
                
                let games = sdk.list_games(filters).await?;
                let limited_games: Vec<_> = games.into_iter().take(*limit as usize).collect();
                
                self.print_output(&limited_games)?;
            }
            GameCommands::Create { 
                name, 
                game_type, 
                max_players, 
                min_bet, 
                max_bet, 
                private 
            } => {
                let mut builder = sdk.create_game(name)
                    .game_type(*game_type)
                    .with_max_players(*max_players)
                    .with_betting_limits(*min_bet, *max_bet);
                
                if *private {
                    builder = builder.private();
                }
                
                let game = builder.build().await?;
                self.print_output(&game)?;
            }
            GameCommands::Join { game_id } => {
                let session = sdk.join_game(game_id).await?;
                self.print_output(&session)?;
            }
            GameCommands::Info { game_id } => {
                let game = sdk.get_game(game_id).await?;
                self.print_output(&game)?;
            }
            GameCommands::Leave { game_id } => {
                sdk.games().leave(game_id).await?;
                println!("Successfully left game {}", game_id);
            }
            GameCommands::Bet { game_id, bet_type, amount } => {
                let result = sdk.games().place_bet(game_id, bet_type.clone(), *amount).await?;
                self.print_output(&result)?;
            }
        }
        Ok(())
    }
    
    /// Handle testing commands
    async fn handle_test_commands(&self, action: &TestCommands) -> SDKResult<()> {
        match action {
            TestCommands::Run { scenario, output } => {
                let mut framework = TestFramework::new();
                
                let test_result = if scenario == "all" {
                    // Run all predefined scenarios
                    framework
                        .add_scenario(TestScenarios::basic_game_flow())
                        .add_scenario(TestScenarios::multi_player_scenario())
                        .run_all_tests()
                        .await
                } else {
                    // Run specific scenario
                    let test_scenario = match scenario.as_str() {
                        "basic" => TestScenarios::basic_game_flow(),
                        "multiplayer" => TestScenarios::multi_player_scenario(),
                        _ => return Err(SDKError::ValidationError {
                            message: format!("Unknown test scenario: {}", scenario),
                            field: Some("scenario".to_string()),
                            invalid_value: Some(scenario.clone()),
                        }),
                    };
                    
                    framework.add_scenario(test_scenario);
                    framework.run_all_tests().await
                };
                
                if let Some(output_path) = output {
                    let json_output = serde_json::to_string_pretty(&test_result)?;
                    fs::write(output_path, json_output).await
                        .map_err(|e| SDKError::ConfigurationError(format!("Failed to write test report: {}", e)))?;
                    println!("Test report written to {:?}", output_path);
                } else {
                    self.print_output(&test_result)?;
                }
            }
            TestCommands::List => {
                let scenarios = vec![
                    "basic - Basic game creation and betting flow",
                    "multiplayer - Multi-player interaction scenarios",
                ];
                
                for scenario in scenarios {
                    println!("{}", scenario);
                }
            }
            TestCommands::Create { file: _ } => {
                println!("Custom test scenario creation not yet implemented");
            }
            TestCommands::Validate => {
                let config = self.load_config().await?;
                match config.validate() {
                    Ok(_) => println!("✅ SDK configuration is valid"),
                    Err(e) => {
                        println!("❌ SDK configuration is invalid: {}", e);
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Handle code generation commands
    async fn handle_codegen_commands(&self, action: &CodegenCommands) -> SDKResult<()> {
        match action {
            CodegenCommands::Generate { language, output, examples } => {
                let generator = CodeGenerator::new();
                generator.generate_client_code(*language, output.clone(), *examples).await?;
                println!("Generated {} client code in {:?}", format!("{:?}", language).to_lowercase(), output);
            }
            CodegenCommands::Languages => {
                let languages = vec![
                    "rust - Rust client library with async/await support",
                    "python - Python client with asyncio support", 
                    "javascript - JavaScript/Node.js client",
                    "typescript - TypeScript client with full type definitions",
                    "go - Go client library",
                    "java - Java client with Spring Boot integration",
                    "csharp - C# client for .NET",
                    "swift - Swift client for iOS/macOS",
                ];
                
                for lang in languages {
                    println!("{}", lang);
                }
            }
            CodegenCommands::Openapi { output } => {
                let spec = crate::sdk_v2::rest::generate_openapi_spec();
                let yaml_content = serde_yaml::to_string(&spec)
                    .map_err(|e| SDKError::SerializationError(e.to_string()))?;
                
                if let Some(output_path) = output {
                    fs::write(output_path, yaml_content).await
                        .map_err(|e| SDKError::ConfigurationError(format!("Failed to write OpenAPI spec: {}", e)))?;
                    println!("OpenAPI specification written to {:?}", output_path);
                } else {
                    println!("{}", yaml_content);
                }
            }
        }
        Ok(())
    }
    
    /// Handle API playground commands
    async fn handle_playground_commands(&self, action: &PlaygroundCommands) -> SDKResult<()> {
        match action {
            PlaygroundCommands::Start { port, open } => {
                let playground = APIPlayground::new(*port);
                println!("Starting API playground on port {}", port);
                
                if *open {
                    println!("Opening browser...");
                    // Would open browser in real implementation
                }
                
                playground.start().await?;
            }
            PlaygroundCommands::Config { output } => {
                let config = serde_json::json!({
                    "playground": {
                        "title": "BitCraps API Playground",
                        "description": "Interactive API testing environment",
                        "theme": "dark"
                    }
                });
                
                let json_content = serde_json::to_string_pretty(&config)?;
                fs::write(output, json_content).await
                    .map_err(|e| SDKError::ConfigurationError(format!("Failed to write config: {}", e)))?;
                println!("Playground configuration written to {:?}", output);
            }
        }
        Ok(())
    }
    
    /// Handle network management commands
    async fn handle_network_commands(&self, sdk: &BitCrapsSDK, action: &NetworkCommands) -> SDKResult<()> {
        match action {
            NetworkCommands::Peers => {
                let peers = sdk.network().get_connected_peers().await;
                self.print_output(&peers)?;
            }
            NetworkCommands::Connect { address } => {
                let peer_id = sdk.network().connect(address).await?;
                println!("Connected to peer: {}", peer_id);
            }
            NetworkCommands::Disconnect { peer_id } => {
                sdk.network().disconnect(peer_id).await?;
                println!("Disconnected from peer: {}", peer_id);
            }
            NetworkCommands::Stats => {
                let stats = sdk.network().get_network_statistics().await;
                self.print_output(&stats)?;
            }
            NetworkCommands::Ping { peer_id } => {
                let result = sdk.network().ping_peer(peer_id).await?;
                self.print_output(&result)?;
            }
        }
        Ok(())
    }
    
    /// Handle configuration commands
    async fn handle_config_commands(&self, action: &ConfigCommands) -> SDKResult<()> {
        match action {
            ConfigCommands::Show => {
                let config = self.load_config().await?;
                self.print_output(&config)?;
            }
            ConfigCommands::Set { key: _, value: _ } => {
                println!("Configuration setting not yet implemented");
            }
            ConfigCommands::Init { output } => {
                let default_config = serde_json::json!({
                    "api_key": "your-api-key-here",
                    "environment": "Development",
                    "debug_logging": true,
                    "request_timeout": 30
                });
                
                let output_path = output.as_ref()
                    .map(|p| p.clone())
                    .unwrap_or_else(|| PathBuf::from("bitcraps-config.json"));
                
                let json_content = serde_json::to_string_pretty(&default_config)?;
                fs::write(&output_path, json_content).await
                    .map_err(|e| SDKError::ConfigurationError(format!("Failed to write config: {}", e)))?;
                
                println!("Configuration file created at {:?}", output_path);
                println!("Please edit the file to set your API key and preferences.");
            }
        }
        Ok(())
    }
    
    /// Start interactive mode
    async fn start_interactive_mode(&self, _sdk: &BitCrapsSDK) -> SDKResult<()> {
        println!("🎲 BitCraps SDK Interactive Mode");
        println!("Type 'help' for available commands, 'exit' to quit");
        
        loop {
            print!("> ");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)
                .map_err(|e| SDKError::InternalError(format!("Failed to read input: {}", e)))?;
            
            let input = input.trim();
            
            match input {
                "exit" | "quit" => break,
                "help" => self.print_interactive_help(),
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", input);
                }
            }
        }
        
        println!("Goodbye!");
        Ok(())
    }
    
    /// Print interactive mode help
    fn print_interactive_help(&self) {
        println!("Available commands:");
        println!("  help     - Show this help message");
        println!("  exit     - Exit interactive mode");
        println!("  quit     - Exit interactive mode");
        // More commands would be added in a full implementation
    }
    
    /// Print output in the specified format
    fn print_output<T>(&self, data: &T) -> SDKResult<()>
    where
        T: serde::Serialize,
    {
        match self.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(data)?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(data)
                    .map_err(|e| SDKError::SerializationError(e.to_string()))?;
                println!("{}", yaml);
            }
            OutputFormat::Table | OutputFormat::Pretty => {
                // For simplicity, fall back to JSON for now
                // In a real implementation, we'd format as tables
                let json = serde_json::to_string_pretty(data)?;
                println!("{}", json);
            }
        }
        Ok(())
    }
}

/// CLI entry point
pub async fn run_cli() -> SDKResult<()> {
    let app = CliApp::parse();
    app.run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    
    #[test]
    fn verify_cli() {
        CliApp::command().debug_assert();
    }
    
    #[test]
    fn test_output_formats() {
        let formats = vec![
            OutputFormat::Json,
            OutputFormat::Yaml,
            OutputFormat::Table,
            OutputFormat::Pretty,
        ];
        
        assert_eq!(formats.len(), 4);
    }
    
    #[test]
    fn test_target_languages() {
        let languages = vec![
            TargetLanguage::Rust,
            TargetLanguage::Python,
            TargetLanguage::JavaScript,
            TargetLanguage::TypeScript,
            TargetLanguage::Go,
            TargetLanguage::Java,
            TargetLanguage::CSharp,
            TargetLanguage::Swift,
        ];
        
        assert_eq!(languages.len(), 8);
    }
}