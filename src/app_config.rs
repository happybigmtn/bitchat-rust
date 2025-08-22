//! Application configuration and CLI argument parsing
//! 
//! This module handles all command-line interface definitions,
//! argument parsing, and application configuration.

use clap::{Parser, Subcommand};

/// Command-line interface definition for BitCraps
#[derive(Parser)]
#[command(name = "bitcraps")]
#[command(about = "Decentralized craps casino over Bluetooth mesh")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    #[arg(short, long, default_value = "~/.bitcraps")]
    pub data_dir: String,
    
    #[arg(short, long)]
    pub nickname: Option<String>,
    
    #[arg(long, default_value = "16")]
    pub pow_difficulty: u32,
    
    #[arg(short, long)]
    pub verbose: bool,
}

/// Available commands for the BitCraps CLI
#[derive(Subcommand)]
pub enum Commands {
    /// Start the BitCraps node
    Start,
    
    /// Create a new game
    CreateGame { 
        #[arg(default_value = "10")]
        buy_in: u64 
    },
    
    /// Join an existing game by ID
    JoinGame { 
        game_id: String 
    },
    
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

impl Commands {
    /// Check if this command requires a running node
    pub fn requires_node(&self) -> bool {
        matches!(self, 
            Commands::Start | 
            Commands::CreateGame { .. } | 
            Commands::JoinGame { .. } | 
            Commands::Bet { .. } |
            Commands::Ping
        )
    }
    
    /// Check if this command is a quick query that doesn't need full initialization
    pub fn is_query_only(&self) -> bool {
        matches!(self, 
            Commands::Balance | 
            Commands::Games | 
            Commands::Stats
        )
    }
    
    /// Get the command name as a string
    pub fn name(&self) -> &'static str {
        match self {
            Commands::Start => "start",
            Commands::CreateGame { .. } => "create-game",
            Commands::JoinGame { .. } => "join-game",
            Commands::Balance => "balance",
            Commands::Games => "games",
            Commands::Bet { .. } => "bet",
            Commands::Stats => "stats",
            Commands::Ping => "ping",
        }
    }
}

/// Parse bet type string to BetType enum
pub fn parse_bet_type(bet_type_str: &str) -> Result<bitcraps::BetType, String> {
    use bitcraps::BetType;
    
    match bet_type_str.to_lowercase().as_str() {
        // Main line bets
        "pass" | "passline" | "pass-line" => Ok(BetType::Pass),
        "dontpass" | "dont-pass" | "don't-pass" => Ok(BetType::DontPass),
        "come" => Ok(BetType::Come),
        "dontcome" | "dont-come" | "don't-come" => Ok(BetType::DontCome),
        
        // Odds bets
        "oddspass" | "odds-pass" | "pass-odds" => Ok(BetType::OddsPass),
        "oddsdontpass" | "odds-dont-pass" | "dont-pass-odds" => Ok(BetType::OddsDontPass),
        
        // Field bet
        "field" => Ok(BetType::Field),
        
        // YES bets (number comes before 7)
        "yes2" | "yes-2" => Ok(BetType::Yes2),
        "yes3" | "yes-3" => Ok(BetType::Yes3),
        "yes4" | "yes-4" => Ok(BetType::Yes4),
        "yes5" | "yes-5" => Ok(BetType::Yes5),
        "yes6" | "yes-6" => Ok(BetType::Yes6),
        "yes8" | "yes-8" => Ok(BetType::Yes8),
        "yes9" | "yes-9" => Ok(BetType::Yes9),
        "yes10" | "yes-10" => Ok(BetType::Yes10),
        "yes11" | "yes-11" => Ok(BetType::Yes11),
        "yes12" | "yes-12" => Ok(BetType::Yes12),
        
        // NO bets (7 comes before number)
        "no2" | "no-2" => Ok(BetType::No2),
        "no3" | "no-3" => Ok(BetType::No3),
        "no4" | "no-4" => Ok(BetType::No4),
        "no5" | "no-5" => Ok(BetType::No5),
        "no6" | "no-6" => Ok(BetType::No6),
        "no8" | "no-8" => Ok(BetType::No8),
        "no9" | "no-9" => Ok(BetType::No9),
        "no10" | "no-10" => Ok(BetType::No10),
        "no11" | "no-11" => Ok(BetType::No11),
        "no12" | "no-12" => Ok(BetType::No12),
        
        // Hardway bets
        "hard4" | "hard-4" | "hardway4" => Ok(BetType::Hard4),
        "hard6" | "hard-6" | "hardway6" => Ok(BetType::Hard6),
        "hard8" | "hard-8" | "hardway8" => Ok(BetType::Hard8),
        "hard10" | "hard-10" | "hardway10" => Ok(BetType::Hard10),
        
        // NEXT bets (one-roll)
        "next2" | "next-2" => Ok(BetType::Next2),
        "next3" | "next-3" => Ok(BetType::Next3),
        "next4" | "next-4" => Ok(BetType::Next4),
        "next5" | "next-5" => Ok(BetType::Next5),
        "next6" | "next-6" => Ok(BetType::Next6),
        "next7" | "next-7" => Ok(BetType::Next7),
        "next8" | "next-8" => Ok(BetType::Next8),
        "next9" | "next-9" => Ok(BetType::Next9),
        "next10" | "next-10" => Ok(BetType::Next10),
        "next11" | "next-11" => Ok(BetType::Next11),
        "next12" | "next-12" => Ok(BetType::Next12),
        
        // Special bets
        "fire" => Ok(BetType::Fire),
        "bonussmall" | "bonus-small" | "small" => Ok(BetType::BonusSmall),
        "bonustall" | "bonus-tall" | "tall" => Ok(BetType::BonusTall),
        "bonusall" | "bonus-all" | "all" => Ok(BetType::BonusAll),
        "hotroller" | "hot-roller" => Ok(BetType::HotRoller),
        "twicehard" | "twice-hard" => Ok(BetType::TwiceHard),
        "rideline" | "ride-line" | "ride-the-line" => Ok(BetType::RideLine),
        "muggsy" => Ok(BetType::Muggsy),
        "replay" => Ok(BetType::Replay),
        "differentdoubles" | "different-doubles" => Ok(BetType::DifferentDoubles),
        
        // Repeater bets
        "repeater2" | "repeater-2" => Ok(BetType::Repeater2),
        "repeater3" | "repeater-3" => Ok(BetType::Repeater3),
        "repeater4" | "repeater-4" => Ok(BetType::Repeater4),
        "repeater5" | "repeater-5" => Ok(BetType::Repeater5),
        "repeater6" | "repeater-6" => Ok(BetType::Repeater6),
        "repeater8" | "repeater-8" => Ok(BetType::Repeater8),
        "repeater9" | "repeater-9" => Ok(BetType::Repeater9),
        "repeater10" | "repeater-10" => Ok(BetType::Repeater10),
        "repeater11" | "repeater-11" => Ok(BetType::Repeater11),
        "repeater12" | "repeater-12" => Ok(BetType::Repeater12),
        
        _ => Err(format!("Invalid bet type: '{}'. Use 'bitcraps --help' to see available bet types.", bet_type_str)),
    }
}

/// Parse game ID string (hex format) to GameId array
pub fn parse_game_id(game_id_str: &str) -> Result<bitcraps::GameId, String> {
    let game_id_bytes = hex::decode(game_id_str)
        .map_err(|_| "Invalid game ID format - must be hexadecimal".to_string())?;
    
    if game_id_bytes.len() != 16 {
        return Err("Game ID must be exactly 16 bytes (32 hex characters)".to_string());
    }
    
    let mut game_id_array = [0u8; 16];
    game_id_array.copy_from_slice(&game_id_bytes);
    Ok(game_id_array)
}

/// Format GameId as hex string for display
pub fn format_game_id(game_id: bitcraps::GameId) -> String {
    hex::encode(game_id)
}

/// Validate and expand data directory path
pub fn resolve_data_dir(data_dir: &str) -> Result<String, String> {
    if data_dir.starts_with("~/") {
        if let Some(home) = std::env::var("HOME").ok() {
            Ok(data_dir.replacen("~", &home, 1))
        } else {
            Err("Cannot resolve ~ in data directory path - HOME environment variable not set".to_string())
        }
    } else {
        Ok(data_dir.to_string())
    }
}

/// Get available bet types with descriptions
pub fn get_bet_type_help() -> &'static str {
    r#"Available bet types:

MAIN LINE BETS:
  pass, passline          - Pass Line bet (1:1 payout)
  dontpass, dont-pass     - Don't Pass Line bet (1:1 payout)  
  come                    - Come bet (1:1 payout)
  dontcome, dont-come     - Don't Come bet (1:1 payout)
  field                   - Field bet (1:1 or 2:1 payout)

ODDS BETS:
  oddspass, pass-odds     - Pass Line odds (true odds)
  oddsdontpass, dont-pass-odds - Don't Pass odds (true odds)

YES BETS (number before 7):
  yes2, yes3, yes4, yes5, yes6, yes8, yes9, yes10, yes11, yes12

NO BETS (7 before number):  
  no2, no3, no4, no5, no6, no8, no9, no10, no11, no12

HARDWAY BETS:
  hard4, hard6, hard8, hard10 - Number must come as doubles

ONE-ROLL BETS:
  next2, next3, next4, next5, next6, next7, next8, next9, next10, next11, next12

SPECIAL BETS:
  fire                    - Fire bet (multiple points)
  bonussmall, small       - Bonus Small (2-6 before 7)
  bonustall, tall         - Bonus Tall (8-12 before 7) 
  bonusall, all           - Bonus All (2-12 except 7)
  hotroller, hot-roller   - Hot Roller progressive
  twicehard, twice-hard   - Same hardway twice
  rideline, ride-line     - Pass line win streak
  muggsy                  - Specific 7-point-7 pattern
  replay                  - Same point repeated 3+ times
  differentdoubles        - Multiple unique doubles

REPEATER BETS:
  repeater2, repeater3, repeater4, repeater5, repeater6,
  repeater8, repeater9, repeater10, repeater11, repeater12
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bet_type_parsing() {
        // Test main line bets
        assert!(matches!(parse_bet_type("pass"), Ok(bitcraps::BetType::Pass)));
        assert!(matches!(parse_bet_type("dontpass"), Ok(bitcraps::BetType::DontPass)));
        assert!(matches!(parse_bet_type("field"), Ok(bitcraps::BetType::Field)));
        
        // Test case insensitive
        assert!(matches!(parse_bet_type("PASS"), Ok(bitcraps::BetType::Pass)));
        assert!(matches!(parse_bet_type("Pass"), Ok(bitcraps::BetType::Pass)));
        
        // Test with hyphens
        assert!(matches!(parse_bet_type("dont-pass"), Ok(bitcraps::BetType::DontPass)));
        assert!(matches!(parse_bet_type("pass-line"), Ok(bitcraps::BetType::Pass)));
        
        // Test YES/NO bets
        assert!(matches!(parse_bet_type("yes4"), Ok(bitcraps::BetType::Yes4)));
        assert!(matches!(parse_bet_type("no6"), Ok(bitcraps::BetType::No6)));
        
        // Test invalid bet type
        assert!(parse_bet_type("invalid").is_err());
    }
    
    #[test] 
    fn test_game_id_parsing() {
        // Valid game ID (32 hex characters = 16 bytes)
        let valid_id = "0123456789abcdef0123456789abcdef";
        assert!(parse_game_id(valid_id).is_ok());
        
        // Invalid length
        assert!(parse_game_id("short").is_err());
        assert!(parse_game_id("toolongtobeagameid123456789abcdef").is_err());
        
        // Invalid hex
        assert!(parse_game_id("gggggggggggggggggggggggggggggggg").is_err());
    }
    
    #[test]
    fn test_data_dir_resolution() {
        // Test home directory expansion
        if std::env::var("HOME").is_ok() {
            let result = resolve_data_dir("~/test");
            assert!(result.is_ok());
            assert!(!result.unwrap().starts_with("~"));
        }
        
        // Test absolute path (no change)
        let abs_path = "/absolute/path";
        assert_eq!(resolve_data_dir(abs_path).unwrap(), abs_path);
        
        // Test relative path (no change)
        let rel_path = "relative/path";
        assert_eq!(resolve_data_dir(rel_path).unwrap(), rel_path);
    }
    
    #[test]
    fn test_command_classification() {
        assert!(Commands::Start.requires_node());
        assert!(!Commands::Balance.requires_node());
        
        assert!(Commands::Balance.is_query_only());
        assert!(!Commands::Start.is_query_only());
        
        assert_eq!(Commands::Start.name(), "start");
        assert_eq!(Commands::Balance.name(), "balance");
    }
}