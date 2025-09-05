use dashmap::DashMap;
use tokio::sync::broadcast;
use std::sync::Arc;

/// Simple in-memory pub/sub broker based on broadcast channels per topic.
#[derive(Default)]
pub struct InMemoryBroker {
    topics: DashMap<String, broadcast::Sender<String>>,
}

impl InMemoryBroker {
    pub fn new() -> Self { Self { topics: DashMap::new() } }

    /// Publish a message to a topic.
    pub fn publish(&self, topic: &str, payload: String) {
        if let Some(sender) = self.topics.get(topic) {
            let _ = sender.send(payload);
            return;
        }
        let (tx, _rx) = broadcast::channel(1024);
        let _ = tx.send(payload);
        self.topics.insert(topic.to_string(), tx);
    }

    /// Subscribe to a topic, returning a broadcast receiver.
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<String> {
        if let Some(sender) = self.topics.get(topic) {
            return sender.subscribe();
        }
        let (tx, rx) = broadcast::channel(1024);
        self.topics.insert(topic.to_string(), tx);
        rx
    }
}

pub type SharedBroker = Arc<InMemoryBroker>;

