// src/protocol/utils.rs
use super::{BitchatPacket, PeerId, GameId, MessageId, CrapTokens, BetType, Bet, RandomnessCommitment, RandomnessReveal, GameResult};
use super::constants::*;

pub struct PacketUtils;

impl PacketUtils {
    /// Create an announcement packet
    pub fn create_announcement(
        sender_id: PeerId,
        nickname: &str,
        public_key: &[u8; 32],
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // TLV encoding: Type(1) + Length(2) + Value(N)
        // Nickname TLV (type = 0x01)
        tlv_data.push(0x01);
        tlv_data.extend_from_slice(&(nickname.len() as u16).to_be_bytes());
        tlv_data.extend_from_slice(nickname.as_bytes());
        
        // Public Key TLV (type = 0x02)
        tlv_data.push(0x02);
        tlv_data.extend_from_slice(&(32u16).to_be_bytes());
        tlv_data.extend_from_slice(public_key);
        
        BitchatPacket::new(
            PACKET_TYPE_ANNOUNCEMENT,
            sender_id,
            tlv_data,
        )
    }
    
    /// Create a public message packet
    pub fn create_public_message(
        sender_id: PeerId,
        message: &str,
    ) -> BitchatPacket {
        BitchatPacket::new(
            PACKET_TYPE_PUBLIC_MESSAGE,
            sender_id,
            message.as_bytes().to_vec(),
        )
    }
    
    /// Create a private message packet
    pub fn create_private_message(
        sender_id: PeerId,
        recipient_id: PeerId,
        encrypted_message: Vec<u8>,
    ) -> BitchatPacket {
        BitchatPacket::new(
            PACKET_TYPE_PRIVATE_MESSAGE,
            sender_id,
            encrypted_message,
        ).with_recipient(recipient_id)
    }
    
    /// Parse TLV data from announcement payload
    pub fn parse_announcement_tlv(payload: &[u8]) -> Result<(String, [u8; 32]), String> {
        let mut cursor = 0;
        let mut nickname = None;
        let mut public_key = None;
        
        while cursor < payload.len() {
            if cursor + 3 > payload.len() {
                break; // Not enough data for TLV header
            }
            
            let tlv_type = payload[cursor];
            let tlv_length = u16::from_be_bytes([
                payload[cursor + 1],
                payload[cursor + 2],
            ]) as usize;
            cursor += 3;
            
            if cursor + tlv_length > payload.len() {
                return Err("Invalid TLV length".to_string());
            }
            
            match tlv_type {
                0x01 => { // Nickname
                    nickname = Some(String::from_utf8_lossy(
                        &payload[cursor..cursor + tlv_length]
                    ).to_string());
                }
                0x02 => { // Public Key
                    if tlv_length == 32 {
                        let mut key_bytes = [0u8; 32];
                        key_bytes.copy_from_slice(&payload[cursor..cursor + tlv_length]);
                        public_key = Some(key_bytes);
                    }
                }
                _ => {} // Ignore unknown TLV types
            }
            
            cursor += tlv_length;
        }
        
        match (nickname, public_key) {
            (Some(nick), Some(key)) => Ok((nick, key)),
            _ => Err("Missing required fields in announcement".to_string()),
        }
    }
    
    /// Create a game creation packet
    pub fn create_game_create(
        sender_id: PeerId,
        game_id: GameId,
        max_players: u8,
        buy_in: CrapTokens,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(game_id.as_bytes());
        
        // Max Players TLV (type = 0x11)
        tlv_data.push(0x11);
        tlv_data.extend_from_slice(&(1u16).to_be_bytes());
        tlv_data.push(max_players);
        
        // Buy-in TLV (type = 0x12)
        tlv_data.push(0x12);
        tlv_data.extend_from_slice(&(8u16).to_be_bytes());
        tlv_data.extend_from_slice(&buy_in.amount().to_be_bytes());
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_CREATE,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a game join packet
    pub fn create_game_join(
        sender_id: PeerId,
        game_id: GameId,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(game_id.as_bytes());
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_JOIN,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a bet packet
    pub fn create_game_bet(
        sender_id: PeerId,
        bet: &Bet,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(bet.game_id.as_bytes());
        
        // Bet ID TLV (type = 0x13)
        tlv_data.push(0x13);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(bet.id.as_bytes());
        
        // Bet Type TLV (type = 0x14)
        let bet_type_data = Self::serialize_bet_type(&bet.bet_type);
        tlv_data.push(0x14);
        tlv_data.extend_from_slice(&(bet_type_data.len() as u16).to_be_bytes());
        tlv_data.extend_from_slice(&bet_type_data);
        
        // Bet Amount TLV (type = 0x15)
        tlv_data.push(0x15);
        tlv_data.extend_from_slice(&(8u16).to_be_bytes());
        tlv_data.extend_from_slice(&bet.amount.amount().to_be_bytes());
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_BET,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a randomness commitment packet
    pub fn create_roll_commit(
        sender_id: PeerId,
        commitment: &RandomnessCommitment,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(commitment.game_id.as_bytes());
        
        // Commitment TLV (type = 0x20)
        tlv_data.push(0x20);
        tlv_data.extend_from_slice(&(COMMITMENT_SIZE as u16).to_be_bytes());
        tlv_data.extend_from_slice(&commitment.commitment);
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_ROLL_COMMIT,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a randomness reveal packet
    pub fn create_roll_reveal(
        sender_id: PeerId,
        reveal: &RandomnessReveal,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(reveal.game_id.as_bytes());
        
        // Nonce TLV (type = 0x21)
        tlv_data.push(0x21);
        tlv_data.extend_from_slice(&(NONCE_SIZE as u16).to_be_bytes());
        tlv_data.extend_from_slice(&reveal.nonce);
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_ROLL_REVEAL,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a game result packet
    pub fn create_game_result(
        sender_id: PeerId,
        result: &GameResult,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Game ID TLV (type = 0x10)
        tlv_data.push(0x10);
        tlv_data.extend_from_slice(&(16u16).to_be_bytes());
        tlv_data.extend_from_slice(result.game_id.as_bytes());
        
        // Final Roll TLV (type = 0x22)
        tlv_data.push(0x22);
        tlv_data.extend_from_slice(&(2u16).to_be_bytes());
        tlv_data.push(result.final_roll.die1);
        tlv_data.push(result.final_roll.die2);
        
        // Serialize payouts - this would be more complex in practice
        // For now, just include the number of winning bets
        tlv_data.push(0x23);
        tlv_data.extend_from_slice(&(1u16).to_be_bytes());
        tlv_data.push(result.winning_bets.len() as u8);
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_GAME_RESULT,
            sender_id,
            tlv_data,
        );
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Create a CRAP token transfer packet
    pub fn create_token_transfer(
        sender_id: PeerId,
        recipient_id: PeerId,
        amount: CrapTokens,
        memo: &str,
    ) -> BitchatPacket {
        let mut tlv_data = Vec::new();
        
        // Amount TLV (type = 0x30)
        tlv_data.push(0x30);
        tlv_data.extend_from_slice(&(8u16).to_be_bytes());
        tlv_data.extend_from_slice(&amount.amount().to_be_bytes());
        
        // Memo TLV (type = 0x31)
        if !memo.is_empty() {
            tlv_data.push(0x31);
            tlv_data.extend_from_slice(&(memo.len() as u16).to_be_bytes());
            tlv_data.extend_from_slice(memo.as_bytes());
        }
        
        let mut packet = BitchatPacket::new(
            PACKET_TYPE_CRAP_TOKEN_TRANSFER,
            sender_id,
            tlv_data,
        ).with_recipient(recipient_id);
        packet.flags |= FLAG_GAMING_MESSAGE;
        packet
    }
    
    /// Serialize bet type to bytes
    fn serialize_bet_type(bet_type: &BetType) -> Vec<u8> {
        match bet_type {
            BetType::Pass => vec![0x01],
            BetType::DontPass => vec![0x02],
            BetType::Come => vec![0x03],
            BetType::DontCome => vec![0x04],
            BetType::Field => vec![0x05],
            BetType::Any7 => vec![0x06],
            BetType::Any11 => vec![0x07],
            BetType::AnyCraps => vec![0x08],
            BetType::Hardway(num) => vec![0x10, *num],
            BetType::Place(num) => vec![0x20, *num],
        }
    }
    
    /// Parse game creation TLV data
    pub fn parse_game_create_tlv(payload: &[u8]) -> Result<(GameId, u8, CrapTokens), String> {
        let mut cursor = 0;
        let mut game_id = None;
        let mut max_players = None;
        let mut buy_in = None;
        
        while cursor < payload.len() {
            if cursor + 3 > payload.len() {
                break;
            }
            
            let tlv_type = payload[cursor];
            let tlv_length = u16::from_be_bytes([
                payload[cursor + 1],
                payload[cursor + 2],
            ]) as usize;
            cursor += 3;
            
            if cursor + tlv_length > payload.len() {
                return Err("Invalid TLV length".to_string());
            }
            
            match tlv_type {
                0x10 => { // Game ID
                    if tlv_length == 16 {
                        let mut id_bytes = [0u8; 16];
                        id_bytes.copy_from_slice(&payload[cursor..cursor + tlv_length]);
                        game_id = Some(GameId::from_bytes(id_bytes));
                    }
                }
                0x11 => { // Max Players
                    if tlv_length == 1 {
                        max_players = Some(payload[cursor]);
                    }
                }
                0x12 => { // Buy-in
                    if tlv_length == 8 {
                        let amount = u64::from_be_bytes([
                            payload[cursor], payload[cursor + 1], payload[cursor + 2], payload[cursor + 3],
                            payload[cursor + 4], payload[cursor + 5], payload[cursor + 6], payload[cursor + 7],
                        ]);
                        buy_in = Some(CrapTokens::new(amount));
                    }
                }
                _ => {} // Ignore unknown TLV types
            }
            
            cursor += tlv_length;
        }
        
        match (game_id, max_players, buy_in) {
            (Some(id), Some(players), Some(amount)) => Ok((id, players, amount)),
            _ => Err("Missing required fields in game creation".to_string()),
        }
    }
    
    /// Parse bet TLV data
    pub fn parse_bet_tlv(payload: &[u8]) -> Result<(GameId, MessageId, BetType, CrapTokens), String> {
        let mut cursor = 0;
        let mut game_id = None;
        let mut bet_id = None;
        let mut bet_type = None;
        let mut amount = None;
        
        while cursor < payload.len() {
            if cursor + 3 > payload.len() {
                break;
            }
            
            let tlv_type = payload[cursor];
            let tlv_length = u16::from_be_bytes([
                payload[cursor + 1],
                payload[cursor + 2],
            ]) as usize;
            cursor += 3;
            
            if cursor + tlv_length > payload.len() {
                return Err("Invalid TLV length".to_string());
            }
            
            match tlv_type {
                0x10 => { // Game ID
                    if tlv_length == 16 {
                        let mut id_bytes = [0u8; 16];
                        id_bytes.copy_from_slice(&payload[cursor..cursor + tlv_length]);
                        game_id = Some(GameId::from_bytes(id_bytes));
                    }
                }
                0x13 => { // Bet ID
                    if tlv_length == 16 {
                        let mut id_bytes = [0u8; 16];
                        id_bytes.copy_from_slice(&payload[cursor..cursor + tlv_length]);
                        bet_id = Some(MessageId::from_bytes(id_bytes));
                    }
                }
                0x14 => { // Bet Type
                    bet_type = Self::parse_bet_type(&payload[cursor..cursor + tlv_length]);
                }
                0x15 => { // Amount
                    if tlv_length == 8 {
                        let amt = u64::from_be_bytes([
                            payload[cursor], payload[cursor + 1], payload[cursor + 2], payload[cursor + 3],
                            payload[cursor + 4], payload[cursor + 5], payload[cursor + 6], payload[cursor + 7],
                        ]);
                        amount = Some(CrapTokens::new(amt));
                    }
                }
                _ => {} // Ignore unknown TLV types
            }
            
            cursor += tlv_length;
        }
        
        match (game_id, bet_id, bet_type, amount) {
            (Some(gid), Some(bid), Some(bt), Some(amt)) => Ok((gid, bid, bt, amt)),
            _ => Err("Missing required fields in bet".to_string()),
        }
    }
    
    /// Parse bet type from bytes
    fn parse_bet_type(data: &[u8]) -> Option<BetType> {
        if data.is_empty() {
            return None;
        }
        
        match data[0] {
            0x01 => Some(BetType::Pass),
            0x02 => Some(BetType::DontPass),
            0x03 => Some(BetType::Come),
            0x04 => Some(BetType::DontCome),
            0x05 => Some(BetType::Field),
            0x06 => Some(BetType::Any7),
            0x07 => Some(BetType::Any11),
            0x08 => Some(BetType::AnyCraps),
            0x10 if data.len() >= 2 => Some(BetType::Hardway(data[1])),
            0x20 if data.len() >= 2 => Some(BetType::Place(data[1])),
            _ => None,
        }
    }
}