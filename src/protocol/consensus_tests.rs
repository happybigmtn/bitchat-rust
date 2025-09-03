//! Unit tests for consensus module
#[cfg(test)]
mod tests {
    use super::consensus::*;
    use super::craps::CrapsGame;
    use super::{PeerId, GameId};

    #[test]
    fn test_consensus_config_default() {
        let config = ConsensusConfig::default();
        assert_eq!(config.min_confirmations, 2);
        assert!(config.enable_fork_recovery);
        assert!(config.require_unanimous_bets);
    }

    #[test]
    fn test_consensus_engine_creation() {
        let config = ConsensusConfig::default();
        let game_id: GameId = [1u8; 16];
        let player1: PeerId = [1u8; 32];
        let player2: PeerId = [2u8; 32];
        let participants = vec![player1, player2];

        let initial_game = CrapsGame::new(game_id, player1);

        let result = ConsensusEngine::new(
            config,
            game_id,
            participants,
            player1,
            initial_game,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_consensus_metrics() {
        let metrics = ConsensusMetrics::default();
        assert_eq!(metrics.total_proposals, 0);
        assert_eq!(metrics.successful_consensus, 0);
        assert_eq!(metrics.forks_resolved, 0);
    }
}