#[cfg(test)]
mod tests {
    use crate::protocol::*;
    
    #[test]
    fn test_peer_id_creation() {
        let bytes = [1u8; 32];
        let peer_id = PeerId::new(bytes);
        assert_eq!(peer_id.as_bytes(), &bytes);
    }
    
    #[test]
    fn test_message_id_uniqueness() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }
    
    #[test]
    fn test_packet_creation() {
        let sender = PeerId::new([2u8; 32]);
        let payload = b"Hello, BitChat!".to_vec();
        
        let packet = BitchatPacket::new(
            PACKET_TYPE_PUBLIC_MESSAGE,
            sender,
            payload.clone(),
        );
        
        assert_eq!(packet.packet_type, PACKET_TYPE_PUBLIC_MESSAGE);
        assert_eq!(packet.sender_id, sender);
        assert_eq!(packet.payload, payload);
        assert_eq!(packet.ttl, MAX_TTL);
        assert!(packet.recipient_id.is_none());
    }
    
    #[test]
    fn test_packet_with_recipient() {
        let sender = PeerId::new([2u8; 32]);
        let recipient = PeerId::new([3u8; 32]);
        let payload = b"Private message".to_vec();
        
        let packet = BitchatPacket::new(
            PACKET_TYPE_PRIVATE_MESSAGE,
            sender,
            payload,
        ).with_recipient(recipient);
        
        assert_eq!(packet.recipient_id, Some(recipient));
        assert!(packet.flags & FLAG_RECIPIENT_PRESENT != 0);
    }
    
    #[test]
    fn test_game_id_creation() {
        let id1 = GameId::new();
        let id2 = GameId::new();
        assert_ne!(id1, id2);
        assert_eq!(id1.as_bytes().len(), 16);
    }
    
    #[test]
    fn test_crap_tokens() {
        let mut tokens = CrapTokens::new(100);
        assert_eq!(tokens.amount(), 100);
        
        assert!(tokens.can_subtract(50));
        tokens.subtract(50).unwrap();
        assert_eq!(tokens.amount(), 50);
        
        assert!(!tokens.can_subtract(100));
        assert!(tokens.subtract(100).is_err());
        
        tokens.add(25);
        assert_eq!(tokens.amount(), 75);
    }
    
    #[test]
    fn test_dice_roll() {
        let roll = DiceRoll::new(3, 4);
        assert_eq!(roll.total(), 7);
        assert!(!roll.is_hard_way());
        assert!(roll.is_natural());
        assert!(!roll.is_craps());
        
        let hard_eight = DiceRoll::new(4, 4);
        assert_eq!(hard_eight.total(), 8);
        assert!(hard_eight.is_hard_way());
        assert!(!hard_eight.is_natural());
        assert!(!hard_eight.is_craps());
        
        let craps_roll = DiceRoll::new(1, 1);
        assert_eq!(craps_roll.total(), 2);
        assert!(!craps_roll.is_hard_way());
        assert!(!craps_roll.is_natural());
        assert!(craps_roll.is_craps());
    }
    
    #[test]
    fn test_randomness_commitment() {
        let player_id = PeerId::new([1u8; 32]);
        let game_id = GameId::new();
        let nonce = [42u8; NONCE_SIZE];
        
        let commitment = RandomnessCommitment::new(&nonce, player_id, game_id);
        
        // Should verify with correct nonce
        assert!(commitment.verify(&nonce));
        
        // Should fail with wrong nonce
        let wrong_nonce = [43u8; NONCE_SIZE];
        assert!(!commitment.verify(&wrong_nonce));
    }
    
    #[test]
    fn test_bet_types() {
        let bet = Bet {
            id: MessageId::new(),
            player_id: PeerId::new([1u8; 32]),
            game_id: GameId::new(),
            bet_type: BetType::Pass,
            amount: CrapTokens::new(10),
            timestamp: 1234567890,
        };
        
        assert_eq!(bet.amount.amount(), 10);
        assert!(matches!(bet.bet_type, BetType::Pass));
    }
    
    #[test]
    fn test_game_state_creation() {
        let game_id = GameId::new();
        let host_id = PeerId::new([1u8; 32]);
        
        let game_state = GameState {
            game_id,
            host_id,
            phase: GamePhase::WaitingForPlayers,
            point: None,
            players: vec![host_id],
            bets: Vec::new(),
            total_pot: CrapTokens::new(0),
            created_at: 1234567890,
            last_roll: None,
        };
        
        assert_eq!(game_state.game_id, game_id);
        assert_eq!(game_state.players.len(), 1);
        assert!(matches!(game_state.phase, GamePhase::WaitingForPlayers));
    }
}