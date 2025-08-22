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
#[derive(Debug, Default)]
struct PeerBehavior {
    packet_count: u64,
    last_packet_time: Option<Instant>,
    suspicious_patterns: u32,
    rapid_fire_count: u32,
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
        
        // Check for rapid-fire packets
        if let Some(last_time) = behavior.last_packet_time {
            if now.duration_since(last_time) < Duration::from_millis(10) {
                behavior.rapid_fire_count += 1;
                if behavior.rapid_fire_count > 100 {
                    return Some("Rapid-fire packet flooding detected".to_string());
                }
            } else {
                behavior.rapid_fire_count = 0;
            }
        }
        
        behavior.packet_count += 1;
        behavior.last_packet_time = Some(now);
        
        // Check for suspicious patterns in game packets
        if packet.packet_type as u8 >= 0x20 && packet.packet_type as u8 <= 0x27 {
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