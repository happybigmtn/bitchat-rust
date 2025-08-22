use std::collections::HashMap;
use std::time::Instant;
use crate::protocol::{PeerId, GameId, BitchatPacket};
use crate::error::Result;

/// Game session state
#[derive(Debug, Clone)]
pub enum SessionState {
    Waiting,
    Active,
    Paused,
    Completed,
}

/// Individual game session
#[derive(Debug, Clone)]
pub struct GameSession {
    pub game_id: GameId,
    pub participants: Vec<PeerId>,
    pub state: SessionState,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub treasury_joined: bool,
}

/// Manager for all game sessions
#[allow(dead_code)]
pub struct GameSessionManager {
    local_peer_id: PeerId,
    sessions: HashMap<GameId, GameSession>,
    player_sessions: HashMap<PeerId, Vec<GameId>>,
    treasury_enabled: bool,
}

impl GameSessionManager {
    pub fn new(local_peer_id: PeerId, treasury_enabled: bool) -> Self {
        Self {
            local_peer_id,
            sessions: HashMap::new(),
            player_sessions: HashMap::new(),
            treasury_enabled,
        }
    }
    
    pub async fn create_session(&mut self, game_id: GameId, mut participants: Vec<PeerId>) {
        // Add treasury as participant if enabled
        if self.treasury_enabled && !participants.contains(&crate::gaming::TREASURY_ADDRESS) {
            participants.push(crate::gaming::TREASURY_ADDRESS);
        }
        
        let session = GameSession {
            game_id,
            participants: participants.clone(),
            state: SessionState::Waiting,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            treasury_joined: self.treasury_enabled,
        };
        
        // Track session
        self.sessions.insert(game_id, session);
        
        // Track player participation
        for participant in participants {
            self.player_sessions
                .entry(participant)
                .or_insert_with(Vec::new)
                .push(game_id);
        }
    }
    
    pub async fn end_session(&mut self, game_id: GameId) {
        if let Some(session) = self.sessions.remove(&game_id) {
            // Clean up player tracking
            for participant in session.participants {
                if let Some(games) = self.player_sessions.get_mut(&participant) {
                    games.retain(|&g| g != game_id);
                }
            }
        }
    }
    
    pub async fn process_game_packet(&mut self, from: PeerId, packet: BitchatPacket) -> Result<()> {
        // Update last activity
        if let Some(game_id) = self.find_player_game(from) {
            if let Some(session) = self.sessions.get_mut(&game_id) {
                session.last_activity = Instant::now();
                
                // Update state based on packet type
                match packet.packet_type {
                    p if p as u8 == 0x21 => { // GameStart
                        session.state = SessionState::Active;
                    }
                    p if p as u8 == 0x27 => { // GameEnd
                        session.state = SessionState::Completed;
                    }
                    _ => {}
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn handle_player_disconnect(&mut self, game_id: GameId, peer_id: PeerId) {
        if let Some(session) = self.sessions.get_mut(&game_id) {
            session.participants.retain(|&p| p != peer_id);
            
            // End session if not enough players
            if session.participants.len() < 2 {
                session.state = SessionState::Completed;
            }
        }
    }
    
    pub async fn cleanup_all_sessions(&mut self) {
        self.sessions.clear();
        self.player_sessions.clear();
    }
    
    pub async fn start_treasury_bot(&mut self) -> Result<()> {
        // Treasury bot logic would go here
        Ok(())
    }
    
    fn find_player_game(&self, peer_id: PeerId) -> Option<GameId> {
        self.player_sessions
            .get(&peer_id)
            .and_then(|games| games.first())
            .copied()
    }
}