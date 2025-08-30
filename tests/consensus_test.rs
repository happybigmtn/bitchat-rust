//! Integration tests for BitCraps consensus mechanism

use bitcraps::protocol::consensus::{ConsensusConfig, ConsensusEngine, GameOperation};
use bitcraps::protocol::craps::CrapsGame;
use bitcraps::protocol::{Bet, BetType, CrapTokens, GameId, PeerId};

#[tokio::test]
async fn test_consensus_engine_creation() {
    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let player1: PeerId = [1u8; 32];
    let player2: PeerId = [2u8; 32];
    let participants = vec![player1, player2];

    let result = ConsensusEngine::new(game_id, participants, player1, config);

    assert!(
        result.is_ok(),
        "Consensus engine should be created successfully"
    );
}

#[tokio::test]
async fn test_bet_proposal() {
    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let player1: PeerId = [1u8; 32];
    let player2: PeerId = [2u8; 32];
    let participants = vec![player1, player2];

    let mut consensus_engine =
        ConsensusEngine::new(game_id, participants, player1, config).unwrap();

    let bet = Bet::new(
        player1, // player (PeerId)
        game_id, // game_id (GameId)
        BetType::Pass,
        CrapTokens::new(100),
    );

    let bet_operation = GameOperation::PlaceBet {
        player: player1,
        bet,
        nonce: 12345,
    };

    let result = consensus_engine.propose_operation(bet_operation);
    assert!(result.is_ok(), "Bet proposal should succeed");
}

#[tokio::test]
async fn test_dice_commit_reveal() {
    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let player1: PeerId = [1u8; 32];
    let player2: PeerId = [2u8; 32];
    let participants = vec![player1, player2];

    let mut consensus_engine =
        ConsensusEngine::new(game_id, participants, player1, config).unwrap();

    let round_id = 1;
    let commitment_result = consensus_engine.start_dice_commit_phase(round_id);
    assert!(
        commitment_result.is_ok(),
        "Dice commit phase should start successfully"
    );
}

#[tokio::test]
async fn test_consensus_health() {
    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let player1: PeerId = [1u8; 32];
    let player2: PeerId = [2u8; 32];
    let participants = vec![player1, player2];

    let initial_game = CrapsGame::new(game_id, player1);

    let consensus_engine = ConsensusEngine::new(game_id, participants, player1, config).unwrap();

    let is_healthy = consensus_engine.is_consensus_healthy();
    assert!(is_healthy, "Consensus should be healthy initially");
}
