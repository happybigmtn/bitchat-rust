use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Default)]
pub struct PerformanceMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicUsize,
    pub avg_latency_ms: AtomicU64,
    pub peak_memory_mb: AtomicU64,
}

impl PerformanceMetrics {
    pub fn record_message_sent(&self, size: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
    }
    
    pub fn record_message_received(&self, size: usize, latency: Duration) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(size as u64, Ordering::Relaxed);
        
        let latency_ms = latency.as_millis() as u64;
        let current_avg = self.avg_latency_ms.load(Ordering::Relaxed);
        let new_avg = (current_avg + latency_ms) / 2;
        self.avg_latency_ms.store(new_avg, Ordering::Relaxed);
    }
    
    pub fn get_throughput(&self) -> (f64, f64) {
        let sent = self.messages_sent.load(Ordering::Relaxed) as f64;
        let received = self.messages_received.load(Ordering::Relaxed) as f64;
        (sent, received)
    }
    
    pub fn get_bandwidth(&self) -> (f64, f64) {
        let sent_bytes = self.bytes_sent.load(Ordering::Relaxed) as f64;
        let received_bytes = self.bytes_received.load(Ordering::Relaxed) as f64;
        (sent_bytes / 1024.0 / 1024.0, received_bytes / 1024.0 / 1024.0)
    }
}