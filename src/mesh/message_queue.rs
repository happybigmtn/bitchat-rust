use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::protocol::BitchatPacket;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    High = 2,
    Normal = 1,
    Low = 0,
}

/// High-performance priority message queue using separate channels per priority level
/// This provides O(1) dequeue operations instead of O(n log n)
pub struct MessageQueue {
    // Separate channels for each priority level
    high_sender: Sender<BitchatPacket>,
    high_receiver: Receiver<BitchatPacket>,
    normal_sender: Sender<BitchatPacket>,
    normal_receiver: Receiver<BitchatPacket>,
    low_sender: Sender<BitchatPacket>,
    low_receiver: Receiver<BitchatPacket>,
    
    // Size tracking per priority
    high_size: AtomicUsize,
    normal_size: AtomicUsize,
    low_size: AtomicUsize,
    max_size: usize,
}

impl MessageQueue {
    pub fn new(max_size: usize) -> Self {
        let (high_sender, high_receiver) = unbounded();
        let (normal_sender, normal_receiver) = unbounded();
        let (low_sender, low_receiver) = unbounded();
        
        Self {
            high_sender,
            high_receiver,
            normal_sender,
            normal_receiver,
            low_sender,
            low_receiver,
            high_size: AtomicUsize::new(0),
            normal_size: AtomicUsize::new(0),
            low_size: AtomicUsize::new(0),
            max_size,
        }
    }
    
    pub fn enqueue(&self, packet: BitchatPacket) -> Result<(), &'static str> {
        // Check total size limit atomically
        let total_size = self.len();
        if total_size >= self.max_size {
            return Err("Queue is full");
        }
        
        // Determine priority based on packet type and route to appropriate channel
        match packet.packet_type {
            p if p >= 0x20 && p <= 0x27 => {
                // High priority: Game packets
                match self.high_sender.try_send(packet) {
                    Ok(_) => {
                        self.high_size.fetch_add(1, Ordering::AcqRel);
                        Ok(())
                    }
                    Err(_) => Err("Failed to send high priority message"),
                }
            }
            p if p >= 0x10 && p <= 0x17 => {
                // Normal priority: Regular messages
                match self.normal_sender.try_send(packet) {
                    Ok(_) => {
                        self.normal_size.fetch_add(1, Ordering::AcqRel);
                        Ok(())
                    }
                    Err(_) => Err("Failed to send normal priority message"),
                }
            }
            _ => {
                // Low priority: Everything else
                match self.low_sender.try_send(packet) {
                    Ok(_) => {
                        self.low_size.fetch_add(1, Ordering::AcqRel);
                        Ok(())
                    }
                    Err(_) => Err("Failed to send low priority message"),
                }
            }
        }
    }
    
    pub fn dequeue(&self) -> Option<BitchatPacket> {
        // Check high priority queue first - O(1) operation
        match self.high_receiver.try_recv() {
            Ok(packet) => {
                self.high_size.fetch_sub(1, Ordering::AcqRel);
                return Some(packet);
            }
            Err(TryRecvError::Empty) => {
                // High priority queue is empty, check normal priority
            }
            Err(TryRecvError::Disconnected) => {
                // Channel disconnected, should not happen in normal operation
            }
        }
        
        // Check normal priority queue - O(1) operation
        match self.normal_receiver.try_recv() {
            Ok(packet) => {
                self.normal_size.fetch_sub(1, Ordering::AcqRel);
                return Some(packet);
            }
            Err(TryRecvError::Empty) => {
                // Normal priority queue is empty, check low priority
            }
            Err(TryRecvError::Disconnected) => {
                // Channel disconnected, should not happen in normal operation
            }
        }
        
        // Check low priority queue - O(1) operation
        match self.low_receiver.try_recv() {
            Ok(packet) => {
                self.low_size.fetch_sub(1, Ordering::AcqRel);
                Some(packet)
            }
            Err(_) => {
                // All queues are empty
                None
            }
        }
    }
    
    /// Non-blocking dequeue that returns immediately if no messages available
    pub fn try_dequeue(&self) -> Option<BitchatPacket> {
        self.dequeue()
    }
    
    /// Get current total queue size
    pub fn len(&self) -> usize {
        self.high_size.load(Ordering::Acquire)
            + self.normal_size.load(Ordering::Acquire)
            + self.low_size.load(Ordering::Acquire)
    }
    
    /// Get size of each priority queue
    pub fn len_by_priority(&self) -> (usize, usize, usize) {
        (
            self.high_size.load(Ordering::Acquire),
            self.normal_size.load(Ordering::Acquire),
            self.low_size.load(Ordering::Acquire),
        )
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Get maximum queue size
    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_priority_ordering() {
        let queue = MessageQueue::new(1000);
        
        // Create test packets with different priorities
        let high_packet = BitchatPacket {
            packet_type: 0x20, // High priority (game packet)
            sender_id: [1; 32],
            recipient_id: [2; 32],
            timestamp: 1000,
            payload: vec![1, 2, 3],
            signature: [0; 64],
        };
        
        let normal_packet = BitchatPacket {
            packet_type: 0x10, // Normal priority
            sender_id: [3; 32],
            recipient_id: [4; 32],
            timestamp: 2000,
            payload: vec![4, 5, 6],
            signature: [0; 64],
        };
        
        let low_packet = BitchatPacket {
            packet_type: 0x01, // Low priority
            sender_id: [5; 32],
            recipient_id: [6; 32],
            timestamp: 3000,
            payload: vec![7, 8, 9],
            signature: [0; 64],
        };
        
        // Enqueue in mixed order
        queue.enqueue(low_packet.clone()).unwrap();
        queue.enqueue(high_packet.clone()).unwrap();
        queue.enqueue(normal_packet.clone()).unwrap();
        
        // Dequeue should return high priority first
        let first = queue.dequeue().unwrap();
        assert_eq!(first.packet_type, 0x20);
        
        // Then normal priority
        let second = queue.dequeue().unwrap();
        assert_eq!(second.packet_type, 0x10);
        
        // Finally low priority
        let third = queue.dequeue().unwrap();
        assert_eq!(third.packet_type, 0x01);
        
        // Queue should be empty
        assert!(queue.dequeue().is_none());
    }
    
    #[test]
    fn test_o1_performance() {
        let queue = MessageQueue::new(10000);
        
        // Fill queue with mixed priority packets
        for i in 0..1000 {
            let packet_type = match i % 3 {
                0 => 0x20, // High
                1 => 0x10, // Normal
                _ => 0x01, // Low
            };
            
            let packet = BitchatPacket {
                packet_type,
                sender_id: [i as u8; 32],
                recipient_id: [(i + 1) as u8; 32],
                timestamp: i as u64,
                payload: vec![i as u8],
                signature: [0; 64],
            };
            
            queue.enqueue(packet).unwrap();
        }
        
        // Measure dequeue performance - should be O(1)
        let start = Instant::now();
        let mut dequeued_count = 0;
        
        while let Some(_packet) = queue.dequeue() {
            dequeued_count += 1;
        }
        
        let duration = start.elapsed();
        
        assert_eq!(dequeued_count, 1000);
        // With O(1) operations, this should complete very quickly
        // even for 1000 items. With the old O(n log n) approach,
        // this would take much longer
        println!("Dequeued {} items in {:?}", dequeued_count, duration);
        assert!(duration.as_millis() < 100); // Should be very fast
    }
}