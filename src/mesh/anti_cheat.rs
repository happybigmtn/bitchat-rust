use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use crate::protocol::{PeerId, BitchatPacket};

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
        let behavior = behaviors.entry(peer_id).or_insert_with(PeerBehavior::default);
        
        let now = Instant::now();
        
        // Check rate limiting using token bucket
        if !behavior.token_bucket.try_consume(1.0) {
            return Some("Rate limit exceeded - packet flooding detected".to_string());
        }
        
        behavior.packet_count += 1;
        behavior.last_packet_time = Some(now);
        
        // Check for suspicious patterns in game packets
        if packet.packet_type >= 0x20 && packet.packet_type <= 0x27 {
            // Game-specific anti-cheat would go here
            // For example: impossible dice rolls, betting patterns, etc.
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
}