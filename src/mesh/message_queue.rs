use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::protocol::BitchatPacket;

/// Priority message queue for the mesh service
pub struct MessageQueue {
    high_priority: Arc<RwLock<VecDeque<BitchatPacket>>>,
    normal_priority: Arc<RwLock<VecDeque<BitchatPacket>>>,
    low_priority: Arc<RwLock<VecDeque<BitchatPacket>>>,
    max_size: usize,
}

impl MessageQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            high_priority: Arc::new(RwLock::new(VecDeque::new())),
            normal_priority: Arc::new(RwLock::new(VecDeque::new())),
            low_priority: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
        }
    }
    
    pub async fn enqueue(&self, packet: BitchatPacket) {
        // Determine priority based on packet type
        let queue = match packet.packet_type {
            p if p as u8 >= 0x20 && p as u8 <= 0x27 => self.high_priority.clone(), // Game packets
            p if p as u8 >= 0x10 && p as u8 <= 0x17 => self.normal_priority.clone(), // Regular messages
            _ => self.low_priority.clone(),
        };
        
        let mut q = queue.write().await;
        if q.len() < self.max_size {
            q.push_back(packet);
        }
    }
    
    pub async fn dequeue(&self) -> Option<BitchatPacket> {
        // Check high priority first
        if let Some(packet) = self.high_priority.write().await.pop_front() {
            return Some(packet);
        }
        
        // Then normal priority
        if let Some(packet) = self.normal_priority.write().await.pop_front() {
            return Some(packet);
        }
        
        // Finally low priority
        self.low_priority.write().await.pop_front()
    }
}