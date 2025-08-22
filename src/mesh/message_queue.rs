use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::protocol::BitchatPacket;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    High = 2,
    Normal = 1,
    Low = 0,
}

struct PriorityMessage {
    priority: MessagePriority,
    packet: BitchatPacket,
}

/// Lock-free priority message queue for the mesh service using crossbeam-channel
pub struct MessageQueue {
    sender: Sender<PriorityMessage>,
    receiver: Receiver<PriorityMessage>,
    current_size: AtomicUsize,
    max_size: usize,
}

impl MessageQueue {
    pub fn new(max_size: usize) -> Self {
        let (sender, receiver) = unbounded();
        Self {
            sender,
            receiver,
            current_size: AtomicUsize::new(0),
            max_size,
        }
    }
    
    pub fn enqueue(&self, packet: BitchatPacket) -> Result<(), &'static str> {
        // Check size limit atomically
        let current = self.current_size.load(Ordering::Acquire);
        if current >= self.max_size {
            return Err("Queue is full");
        }
        
        // Determine priority based on packet type
        let priority = match packet.packet_type {
            p if p >= 0x20 && p <= 0x27 => MessagePriority::High, // Game packets
            p if p >= 0x10 && p <= 0x17 => MessagePriority::Normal, // Regular messages
            _ => MessagePriority::Low,
        };
        
        let priority_msg = PriorityMessage { priority, packet };
        
        // Try to send the message
        match self.sender.try_send(priority_msg) {
            Ok(_) => {
                self.current_size.fetch_add(1, Ordering::AcqRel);
                Ok(())
            }
            Err(_) => Err("Failed to send message"),
        }
    }
    
    pub fn dequeue(&self) -> Option<BitchatPacket> {
        // Collect all available messages and sort by priority
        let mut messages = Vec::new();
        
        // Drain all available messages without blocking
        loop {
            match self.receiver.try_recv() {
                Ok(msg) => {
                    messages.push(msg);
                    self.current_size.fetch_sub(1, Ordering::AcqRel);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        
        if messages.is_empty() {
            return None;
        }
        
        // Sort by priority (highest first)
        messages.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        // Return the highest priority message
        let highest_priority = messages.remove(0);
        
        // Re-enqueue the remaining messages
        for msg in messages {
            if let Err(_) = self.sender.try_send(msg) {
                // If we can't re-enqueue, we lost the message
                // In a production system, you might want to log this
            } else {
                self.current_size.fetch_add(1, Ordering::AcqRel);
            }
        }
        
        Some(highest_priority.packet)
    }
    
    /// Non-blocking dequeue that returns immediately if no messages available
    pub fn try_dequeue(&self) -> Option<BitchatPacket> {
        self.dequeue()
    }
    
    /// Get current queue size
    pub fn len(&self) -> usize {
        self.current_size.load(Ordering::Acquire)
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