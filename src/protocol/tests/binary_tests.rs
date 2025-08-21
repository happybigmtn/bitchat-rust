#[cfg(test)]
mod tests {
    use crate::protocol::*;
    
    #[test]
    fn test_encode_decode_roundtrip() {
        let sender = PeerId::new([1u8; 32]);
        let payload = b"Test message for encoding".to_vec();
        
        let original = BitchatPacket::new(
            PACKET_TYPE_PUBLIC_MESSAGE,
            sender,
            payload,
        );
        
        let encoded = BinaryProtocol::encode(&original).unwrap();
        let decoded = BinaryProtocol::decode(&encoded).unwrap();
        
        assert_eq!(decoded.packet_type, original.packet_type);
        assert_eq!(decoded.sender_id, original.sender_id);
        assert_eq!(decoded.payload, original.payload);
    }
    
    #[test]
    fn test_encode_decode_with_recipient() {
        let sender = PeerId::new([1u8; 32]);
        let recipient = PeerId::new([2u8; 32]);
        let payload = b"Private message".to_vec();
        
        let original = BitchatPacket::new(
            PACKET_TYPE_PRIVATE_MESSAGE,
            sender,
            payload,
        ).with_recipient(recipient);
        
        let encoded = BinaryProtocol::encode(&original).unwrap();
        let decoded = BinaryProtocol::decode(&encoded).unwrap();
        
        assert_eq!(decoded.recipient_id, Some(recipient));
        assert!(decoded.flags & FLAG_RECIPIENT_PRESENT != 0);
    }
    
    #[test]
    fn test_compression() {
        let sender = PeerId::new([1u8; 32]);
        // Create a large, compressible payload
        let payload = "A".repeat(1000).into_bytes();
        
        let packet = BitchatPacket::new(
            PACKET_TYPE_PUBLIC_MESSAGE,
            sender,
            payload.clone(),
        );
        
        let encoded = BinaryProtocol::encode(&packet).unwrap();
        let decoded = BinaryProtocol::decode(&encoded).unwrap();
        
        assert_eq!(decoded.payload, payload);
        // Should be compressed due to size and repetitive content
    }
    
    #[test]
    fn test_announcement_tlv() {
        let sender = PeerId::new([1u8; 32]);
        let nickname = "Alice";
        let public_key = [42u8; 32];
        
        let packet = PacketUtils::create_announcement(
            sender,
            nickname,
            &public_key,
        );
        
        let (parsed_nick, parsed_key) = PacketUtils::parse_announcement_tlv(&packet.payload)
            .unwrap();
        
        assert_eq!(parsed_nick, nickname);
        assert_eq!(parsed_key, public_key);
    }
    
    #[test]
    fn test_gaming_packet_creation() {
        let sender = PeerId::new([1u8; 32]);
        let game_id = GameId::new();
        let buy_in = CrapTokens::new(50);
        
        let packet = PacketUtils::create_game_create(sender, game_id, 6, buy_in);
        
        assert_eq!(packet.packet_type, PACKET_TYPE_GAME_CREATE);
        assert_eq!(packet.sender_id, sender);
        assert!(packet.flags & FLAG_GAMING_MESSAGE != 0);
        
        // Test parsing
        let (parsed_id, max_players, parsed_buy_in) = 
            PacketUtils::parse_game_create_tlv(&packet.payload).unwrap();
        assert_eq!(parsed_id, game_id);
        assert_eq!(max_players, 6);
        assert_eq!(parsed_buy_in.amount(), 50);
    }
    
    #[test]
    fn test_bet_packet_creation() {
        let sender = PeerId::new([1u8; 32]);
        let bet = Bet {
            id: MessageId::new(),
            player_id: sender,
            game_id: GameId::new(),
            bet_type: BetType::Pass,
            amount: CrapTokens::new(25),
            timestamp: 1234567890,
        };
        
        let packet = PacketUtils::create_game_bet(sender, &bet);
        
        assert_eq!(packet.packet_type, PACKET_TYPE_GAME_BET);
        assert!(packet.flags & FLAG_GAMING_MESSAGE != 0);
        
        // Test parsing
        let (game_id, bet_id, bet_type, amount) = 
            PacketUtils::parse_bet_tlv(&packet.payload).unwrap();
        assert_eq!(game_id, bet.game_id);
        assert_eq!(bet_id, bet.id);
        assert!(matches!(bet_type, BetType::Pass));
        assert_eq!(amount.amount(), 25);
    }
    
    #[test]
    fn test_randomness_commitment_packet() {
        let sender = PeerId::new([1u8; 32]);
        let game_id = GameId::new();
        let nonce = [42u8; NONCE_SIZE];
        let commitment = RandomnessCommitment::new(&nonce, sender, game_id);
        
        let packet = PacketUtils::create_roll_commit(sender, &commitment);
        
        assert_eq!(packet.packet_type, PACKET_TYPE_GAME_ROLL_COMMIT);
        assert!(packet.flags & FLAG_GAMING_MESSAGE != 0);
        assert!(!packet.payload.is_empty());
    }
    
    #[test]
    fn test_token_transfer_packet() {
        let sender = PeerId::new([1u8; 32]);
        let recipient = PeerId::new([2u8; 32]);
        let amount = CrapTokens::new(100);
        let memo = "Payment for game";
        
        let packet = PacketUtils::create_token_transfer(sender, recipient, amount, memo);
        
        assert_eq!(packet.packet_type, PACKET_TYPE_CRAP_TOKEN_TRANSFER);
        assert_eq!(packet.recipient_id, Some(recipient));
        assert!(packet.flags & FLAG_GAMING_MESSAGE != 0);
        assert!(packet.flags & FLAG_RECIPIENT_PRESENT != 0);
    }
    
    #[test]
    fn test_invalid_packet_size() {
        let data = vec![1, 2, 3]; // Too small
        let result = BinaryProtocol::decode(&data);
        assert!(matches!(result, Err(ProtocolError::PacketTooSmall { .. })));
    }
    
    #[test]
    fn test_invalid_version() {
        let mut data = vec![0u8; HEADER_SIZE + 32]; // Minimum valid size
        data[0] = 99; // Invalid version
        let result = BinaryProtocol::decode(&data);
        assert!(matches!(result, Err(ProtocolError::InvalidVersion { .. })));
    }
}