//! Bet types and game state definitions for craps
//! 
//! This module contains the core data structures that define
//! the game phases, bet resolutions, and validation logic.

use super::{PeerId, BetType, CrapTokens};
use serde::{Serialize, Deserialize};

/// Game phase in craps
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    ComeOut,
    Point,
    Ended,
    GameEnded,  // Alias for compatibility
}

/// Result of bet resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BetResolution {
    Won {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
        payout: CrapTokens,
    },
    Lost {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
    },
    Push {
        player: PeerId,
        bet_type: BetType,
        amount: CrapTokens,
    },
}

impl BetResolution {
    /// Get the player involved in this resolution
    pub fn player(&self) -> PeerId {
        match self {
            BetResolution::Won { player, .. } => *player,
            BetResolution::Lost { player, .. } => *player,
            BetResolution::Push { player, .. } => *player,
        }
    }
    
    /// Get the bet type involved in this resolution
    pub fn bet_type(&self) -> &BetType {
        match self {
            BetResolution::Won { bet_type, .. } => bet_type,
            BetResolution::Lost { bet_type, .. } => bet_type,
            BetResolution::Push { bet_type, .. } => bet_type,
        }
    }
    
    /// Get the original bet amount
    pub fn amount(&self) -> CrapTokens {
        match self {
            BetResolution::Won { amount, .. } => *amount,
            BetResolution::Lost { amount, .. } => *amount,
            BetResolution::Push { amount, .. } => *amount,
        }
    }
    
    /// Get the payout if this is a winning bet
    pub fn payout(&self) -> Option<CrapTokens> {
        match self {
            BetResolution::Won { payout, .. } => Some(*payout),
            _ => None,
        }
    }
    
    /// Check if this resolution is a win
    pub fn is_win(&self) -> bool {
        matches!(self, BetResolution::Won { .. })
    }
    
    /// Check if this resolution is a loss
    pub fn is_loss(&self) -> bool {
        matches!(self, BetResolution::Lost { .. })
    }
    
    /// Check if this resolution is a push
    pub fn is_push(&self) -> bool {
        matches!(self, BetResolution::Push { .. })
    }
}

/// Bet validation trait for checking if bets are valid in specific game phases
pub trait BetValidator {
    /// Check if a bet type is valid for the current game phase
    fn is_valid_for_phase(&self, phase: &GamePhase) -> bool;
}

impl BetValidator for BetType {
    fn is_valid_for_phase(&self, phase: &GamePhase) -> bool {
        match (self, phase) {
            // Pass/Don't Pass bets can only be placed on come-out
            (BetType::Pass | BetType::DontPass, GamePhase::ComeOut) => true,
            
            // Come/Don't Come bets can only be placed after point is established  
            (BetType::Come | BetType::DontCome, GamePhase::Point) => true,
            
            // Odds bets can only be placed after point is established
            (BetType::OddsPass | BetType::OddsDontPass, GamePhase::Point) => true,
            
            // Field bets and proposition bets can be placed anytime
            (BetType::Field, _) => true,
            
            // YES/NO bets can be placed anytime during active play
            (BetType::Yes2 | BetType::Yes3 | BetType::Yes4 | BetType::Yes5 |
             BetType::Yes6 | BetType::Yes8 | BetType::Yes9 | BetType::Yes10 |
             BetType::Yes11 | BetType::Yes12, GamePhase::ComeOut | GamePhase::Point) => true,
            
            (BetType::No2 | BetType::No3 | BetType::No4 | BetType::No5 |
             BetType::No6 | BetType::No8 | BetType::No9 | BetType::No10 |
             BetType::No11 | BetType::No12, GamePhase::ComeOut | GamePhase::Point) => true,
            
            // Hardway bets can be placed anytime
            (BetType::Hard4 | BetType::Hard6 | BetType::Hard8 | BetType::Hard10, _) => true,
            
            // NEXT bets (one-roll) can be placed anytime
            (BetType::Next2 | BetType::Next3 | BetType::Next4 | BetType::Next5 |
             BetType::Next6 | BetType::Next7 | BetType::Next8 | BetType::Next9 |
             BetType::Next10 | BetType::Next11 | BetType::Next12, _) => true,
            
            // Special bets can typically be placed at the start of a series
            (BetType::Fire | BetType::BonusSmall | BetType::BonusTall | BetType::BonusAll |
             BetType::HotRoller | BetType::TwiceHard | BetType::RideLine |
             BetType::Muggsy | BetType::Replay | BetType::DifferentDoubles, GamePhase::ComeOut) => true,
            
            // Repeater bets can be placed at start of series
            (BetType::Repeater2 | BetType::Repeater3 | BetType::Repeater4 | BetType::Repeater5 |
             BetType::Repeater6 | BetType::Repeater8 | BetType::Repeater9 | BetType::Repeater10 |
             BetType::Repeater11 | BetType::Repeater12, GamePhase::ComeOut) => true,
            
            // Game ended - no bets allowed
            (_, GamePhase::Ended | GamePhase::GameEnded) => false,
            
            // Default case - bet not allowed in this phase
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bet_validation() {
        // Pass line bets only on come-out
        assert!(BetType::Pass.is_valid_for_phase(&GamePhase::ComeOut));
        assert!(!BetType::Pass.is_valid_for_phase(&GamePhase::Point));
        assert!(!BetType::Pass.is_valid_for_phase(&GamePhase::Ended));
        
        // Come bets only after point established
        assert!(!BetType::Come.is_valid_for_phase(&GamePhase::ComeOut));
        assert!(BetType::Come.is_valid_for_phase(&GamePhase::Point));
        
        // Field bets anytime
        assert!(BetType::Field.is_valid_for_phase(&GamePhase::ComeOut));
        assert!(BetType::Field.is_valid_for_phase(&GamePhase::Point));
        assert!(!BetType::Field.is_valid_for_phase(&GamePhase::Ended));
        
        // Special bets only on come-out
        assert!(BetType::Fire.is_valid_for_phase(&GamePhase::ComeOut));
        assert!(!BetType::Fire.is_valid_for_phase(&GamePhase::Point));
    }
    
    #[test]
    fn test_bet_resolution_methods() {
        let resolution = BetResolution::Won {
            player: [0; 32],
            bet_type: BetType::Pass,
            amount: CrapTokens::new_unchecked(100),
            payout: CrapTokens::new_unchecked(200),
        };
        
        assert!(resolution.is_win());
        assert!(!resolution.is_loss());
        assert!(!resolution.is_push());
        assert_eq!(resolution.payout(), Some(CrapTokens::new_unchecked(200)));
    }
}