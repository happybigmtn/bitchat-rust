//! Complete craps game implementation with all 64 bet types
//!
//! This module re-exports the refactored game implementation modules
//! for backward compatibility while providing a cleaner structure.
//!
//! ## Refactored Module Structure
//!
//! The large craps.rs file has been split into smaller, focused modules:
//! - `bet_types.rs` - Game phases, bet resolution types, validation
//! - `game_logic.rs` - Core game state and management
//! - `resolution.rs` - Bet resolution logic for all bet types
//! - `payouts.rs` - Payout calculations and special bet logic
//!
//! All functionality remains the same, but is now organized in a more
//! maintainable way. This module provides the same public API as before.

// Re-export all the refactored modules
pub use super::bet_types::{BetResolution, BetValidator, GamePhase};
pub use super::game_logic::{CrapsGame, GameState, GameStats};
pub use super::payouts::{utils as payout_utils, PayoutCalculator};
pub use super::resolution::{get_odds_multiplier, BetResolver};

// Re-export common types for convenience
pub use super::{Bet, BetType, CrapTokens, DiceRoll, GameId, PeerId};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_compatibility() {
        // Test that all the key types are still accessible
        let game_id = [1; 16];
        let shooter = [2; 32];
        let game = CrapsGame::new(game_id, shooter);

        assert_eq!(game.game_id, game_id);
        assert_eq!(game.shooter, shooter);
        assert_eq!(game.phase, GamePhase::ComeOut);
    }

    #[test]
    fn test_bet_resolution_types() {
        use super::BetResolution;

        let resolution = BetResolution::Won {
            player: [0; 32],
            bet_type: BetType::Pass,
            amount: CrapTokens::new_unchecked(100),
            payout: CrapTokens::new_unchecked(200),
        };

        assert!(resolution.is_win());
        assert!(!resolution.is_loss());
        assert!(!resolution.is_push());
    }
}
