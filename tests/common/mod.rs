use bitcraps::protocol::{PeerId, BitchatPacket};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

/// Mock network for testing
pub struct MockNetwork {
    pub sent_packets: Arc<Mutex<Vec<BitchatPacket>>>,
    pub peers: Arc<Mutex<HashMap<PeerId, String>>>, // peer_id -> address
}

impl MockNetwork {
    pub fn new() -> Self {
        Self {
            sent_packets: Arc::new(Mutex::new(Vec::new())),
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    pub async fn add_peer(&self, peer_id: PeerId, address: String) {
        self.peers.lock().await.insert(peer_id, address);
    }
    
    pub async fn send_packet(&self, packet: BitchatPacket) {
        self.sent_packets.lock().await.push(packet);
    }
    
    pub async fn get_sent_count(&self) -> usize {
        self.sent_packets.lock().await.len()
    }
}