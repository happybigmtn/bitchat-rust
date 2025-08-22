use std::sync::Arc;
use bytes::{Bytes, BytesMut};

pub struct MessagePool {
    small_pool: Vec<BytesMut>,
    medium_pool: Vec<BytesMut>,
    large_pool: Vec<BytesMut>,
}

impl MessagePool {
    pub fn new() -> Self {
        Self {
            small_pool: Vec::with_capacity(100),
            medium_pool: Vec::with_capacity(50),
            large_pool: Vec::with_capacity(10),
        }
    }
    
    pub fn get_buffer(&mut self, size: usize) -> BytesMut {
        match size {
            0..=1024 => self.small_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(1024)),
            1025..=8192 => self.medium_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(8192)),
            _ => self.large_pool.pop()
                .unwrap_or_else(|| BytesMut::with_capacity(size.next_power_of_two())),
        }
    }
    
    pub fn return_buffer(&mut self, mut buffer: BytesMut) {
        buffer.clear();
        match buffer.capacity() {
            0..=1024 if self.small_pool.len() < 100 => {
                self.small_pool.push(buffer);
            }
            1025..=8192 if self.medium_pool.len() < 50 => {
                self.medium_pool.push(buffer);
            }
            _ if self.large_pool.len() < 10 => {
                self.large_pool.push(buffer);
            }
            _ => {} // Drop oversized or excess buffers
        }
    }
}

// Zero-copy message passing
pub struct ZeroCopyMessage {
    pub header: MessageHeader,
    pub payload: Bytes,
}

impl ZeroCopyMessage {
    pub fn new(header: MessageHeader, payload: Bytes) -> Self {
        Self { header, payload }
    }
    
    pub fn split_payload(&self, chunk_size: usize) -> Vec<Bytes> {
        let mut chunks = Vec::new();
        let mut offset = 0;
        
        while offset < self.payload.len() {
            let end = std::cmp::min(offset + chunk_size, self.payload.len());
            chunks.push(self.payload.slice(offset..end));
            offset = end;
        }
        
        chunks
    }
}