use bitcraps::protocol::{BitchatPacket, PeerId, GameId, PacketUtils};
use bitcraps::BetType;
use bitcraps::CrapTokens;

#[tokio::test]
async fn test_packet_serialization() {
    let packet = PacketUtils::create_ping([1u8; 32]);
    
    // Test that packet has expected fields
    assert_eq!(packet.source, [1u8; 32]);
    assert_eq!(packet.packet_type as u8, 0x10); // PACKET_TYPE_PING
}

#[tokio::test]
async fn test_game_packet_creation() {
    let sender = [1u8; 32];
    let game_id = [2u8; 16];
    let packet = PacketUtils::create_game_create(sender, game_id, 8, CrapTokens::new(100));
    
    assert_eq!(packet.source, sender);
    assert_eq!(packet.packet_type as u8, 0x20); // PACKET_TYPE_GAME_CREATE
}

#[tokio::test]
async fn test_bet_packet_creation() {
    use bitcraps::protocol::Bet;
    
    let sender = [3u8; 32];
    let bet = Bet {
        id: [1u8; 16],
        game_id: [4u8; 16],
        player: sender,
        bet_type: BetType::Pass,
        amount: CrapTokens::new(50),
        timestamp: 0,
    };
    
    let packet = PacketUtils::create_bet_packet(sender, &bet).unwrap();
    assert_eq!(packet.source, sender);
    assert_eq!(packet.packet_type as u8, 0x22); // PACKET_TYPE_GAME_BET
}