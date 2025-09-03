use crate::protocol::{BitchatPacket, PeerId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Anti-cheat monitor for detecting suspicious behavior
pub struct AntiCheatMonitor {
    enabled: bool,
    peer_behavior: Arc<RwLock<HashMap<PeerId, PeerBehavior>>>,
    ban_list: Arc<RwLock<HashMap<PeerId, Instant>>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct PeerBehavior {
    packet_count: u64,
    last_packet_time: Option<Instant>,
    suspicious_patterns: u32,
    // Token bucket for rate limiting
    token_bucket: TokenBucket,
}

impl Default for PeerBehavior {
    fn default() -> Self {
        Self {
            packet_count: 0,
            last_packet_time: None,
            suspicious_patterns: 0,
            token_bucket: TokenBucket::new(50, Duration::from_secs(1)), // 50 tokens per second
        }
    }
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_interval: Duration) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate: capacity as f64 / refill_interval.as_secs_f64(),
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }
}

impl AntiCheatMonitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            peer_behavior: Arc::new(RwLock::new(HashMap::new())),
            ban_list: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn analyze_packet(&self, packet: &BitchatPacket, peer_id: PeerId) -> Option<String> {
        if !self.enabled {
            return None;
        }

        // Check if peer is banned
        if self.is_banned(peer_id).await {
            return Some("Peer is banned".to_string());
        }

        let mut behaviors = self.peer_behavior.write().await;
        let behavior = behaviors
            .entry(peer_id)
            .or_insert_with(PeerBehavior::default);

        let now = Instant::now();

        // Check rate limiting using token bucket
        if !behavior.token_bucket.try_consume(1.0) {
            return Some("Rate limit exceeded - packet flooding detected".to_string());
        }

        behavior.packet_count += 1;
        behavior.last_packet_time = Some(now);

        // Check for suspicious patterns in game packets
        if packet.packet_type >= 0x20 && packet.packet_type <= 0x27 {
            // Game-specific anti-cheat validation
            if let Some(violation) = self.validate_game_packet(packet, peer_id).await {
                behavior.suspicious_patterns += 1;

                // Ban peer if too many violations
                if behavior.suspicious_patterns >= 3 {
                    self.ban_peer(peer_id, Duration::from_secs(300)).await; // 5 minute ban
                    return Some(format!("Multiple game violations: {}", violation));
                }

                return Some(violation);
            }
        }

        // Check for consensus message violations
        if packet.packet_type == crate::protocol::PACKET_TYPE_CONSENSUS_VOTE {
            if let Some(violation) = self.validate_consensus_packet(packet, peer_id).await {
                behavior.suspicious_patterns += 1;
                return Some(violation);
            }
        }

        None
    }

    pub async fn ban_peer(&self, peer_id: PeerId, duration: Duration) {
        let ban_until = Instant::now() + duration;
        self.ban_list.write().await.insert(peer_id, ban_until);
    }

    async fn is_banned(&self, peer_id: PeerId) -> bool {
        let mut ban_list = self.ban_list.write().await;

        if let Some(&ban_until) = ban_list.get(&peer_id) {
            if Instant::now() < ban_until {
                return true;
            } else {
                ban_list.remove(&peer_id);
            }
        }

        false
    }

    /// Validate game-specific packets for cheating
    async fn validate_game_packet(
        &self,
        packet: &BitchatPacket,
        _peer_id: PeerId,
    ) -> Option<String> {
        // Deserialize payload to analyze game content
        if let Some(ref payload) = packet.payload {
            // Check for impossible dice rolls
            if packet.packet_type == 0x21 {
                // Dice roll packet type
                if let Ok(dice_data) = bincode::deserialize::<(u8, u8)>(payload) {
                    let (die1, die2) = dice_data;
                    if die1 < 1 || die1 > 6 || die2 < 1 || die2 > 6 {
                        return Some("Invalid dice values".to_string());
                    }
                }
            }

            // Check for suspicious betting patterns
            if packet.packet_type == 0x22 {
                // Bet placement packet
                if let Ok(bet_data) = bincode::deserialize::<crate::protocol::Bet>(payload) {
                    // Check for impossible bet amounts (negative or zero)
                    if bet_data.amount.0 == 0 {
                        return Some("Invalid bet amount".to_string());
                    }

                    // Check for unreasonably large bets that might indicate manipulation
                    if bet_data.amount.0 > 1_000_000 {
                        return Some("Bet amount too large".to_string());
                    }
                }
            }
        }

        None
    }

    /// Validate consensus-specific packets for cheating
    async fn validate_consensus_packet(
        &self,
        packet: &BitchatPacket,
        peer_id: PeerId,
    ) -> Option<String> {
        if let Some(ref payload) = packet.payload {
            // Check consensus message structure
            if let Ok(consensus_msg) =
                bincode::deserialize::<crate::protocol::p2p_messages::ConsensusMessage>(payload)
            {
                // Check for timestamp manipulation
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Allow 30 second clock skew
                if consensus_msg.timestamp > current_time + 30
                    || consensus_msg.timestamp + 300 < current_time
                {
                    // 5 minutes in the past
                    return Some("Suspicious timestamp in consensus message".to_string());
                }

                // Check for message sender spoofing
                if consensus_msg.sender != peer_id {
                    return Some("Sender ID mismatch in consensus message".to_string());
                }

                // Check for malformed signatures
                if consensus_msg.signature.0.len() != 64 {
                    return Some("Invalid signature length".to_string());
                }

                // Check for consensus round manipulation
                if consensus_msg.round > 1_000_000 {
                    return Some("Suspicious consensus round number".to_string());
                }
            }
        }

        None
    }
}
