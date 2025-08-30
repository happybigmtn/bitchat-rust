use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::protocol::{BitchatPacket, PeerId};
use crate::transport::Transport;

/// Multi-transport coordinator
///
/// Feynman: Like having multiple roads to the same destination -
/// highway (Internet), local roads (WiFi), and walking paths (Bluetooth).
/// The coordinator picks the best route based on traffic, distance,
/// and whether the road is even open.
pub struct MultiTransportCoordinator {
    transports: Arc<RwLock<HashMap<TransportType, Box<dyn Transport>>>>,
    peer_transports: Arc<RwLock<HashMap<PeerId, Vec<TransportType>>>>,
    transport_metrics: Arc<RwLock<HashMap<TransportType, TransportMetrics>>>,
    failover_policy: FailoverPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportType {
    Bluetooth,
    WiFiDirect,
    Internet,
    Mesh,
}

#[derive(Debug, Clone)]
pub struct TransportMetrics {
    pub latency_ms: f64,
    pub bandwidth_kbps: f64,
    pub packet_loss: f64,
    pub reliability_score: f64,
    pub last_updated: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum FailoverPolicy {
    FastestFirst,    // Use lowest latency
    MostReliable,    // Use most reliable
    LoadBalanced,    // Distribute across transports
    EnergyEfficient, // Prefer low-power transports
}

impl MultiTransportCoordinator {
    pub fn new(failover_policy: FailoverPolicy) -> Self {
        Self {
            transports: Arc::new(RwLock::new(HashMap::new())),
            peer_transports: Arc::new(RwLock::new(HashMap::new())),
            transport_metrics: Arc::new(RwLock::new(HashMap::new())),
            failover_policy,
        }
    }

    /// Register a transport
    pub async fn register_transport(
        &self,
        transport_type: TransportType,
        transport: Box<dyn Transport>,
    ) {
        self.transports
            .write()
            .await
            .insert(transport_type, transport);

        // Initialize metrics
        self.transport_metrics.write().await.insert(
            transport_type,
            TransportMetrics {
                latency_ms: 100.0,
                bandwidth_kbps: 1000.0,
                packet_loss: 0.0,
                reliability_score: 1.0,
                last_updated: std::time::Instant::now(),
            },
        );
    }

    /// Send packet selecting best transport
    ///
    /// Feynman: Like a smart GPS that knows traffic conditions -
    /// it picks the fastest route considering current conditions,
    /// not just distance.
    pub async fn send_packet(
        &self,
        peer_id: PeerId,
        packet: &BitchatPacket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get available transports for peer
        let available = self.get_available_transports(&peer_id).await;

        if available.is_empty() {
            return Err("No transport available for peer".into());
        }

        // Select best transport based on policy
        let packet_size = packet.payload.as_ref().map(|p| p.len()).unwrap_or(0);
        let selected = self.select_transport(&available, packet_size).await?;

        // Try primary transport
        let mut transports = self.transports.write().await;
        if let Some(transport) = transports.get_mut(&selected) {
            let mut packet_copy = packet.clone();
            match packet_copy.serialize() {
                Ok(serialized) => match transport.send(peer_id, serialized).await {
                    Ok(()) => {
                        self.update_success_metrics(selected).await;
                        return Ok(());
                    }
                    Err(e) => {
                        self.update_failure_metrics(selected).await;
                        eprintln!("Transport {} failed: {}", selected as u8, e);
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to serialize packet: {}", e).into());
                }
            }
        }

        // Failover to other transports
        for transport_type in available {
            if transport_type == selected {
                continue; // Already tried
            }

            if let Some(transport) = transports.get_mut(&transport_type) {
                let mut packet_copy = packet.clone();
                if let Ok(serialized) = packet_copy.serialize() {
                    if transport.send(peer_id, serialized).await.is_ok() {
                        self.update_success_metrics(transport_type).await;
                        return Ok(());
                    }
                }
            }
        }

        Err("All transports failed".into())
    }

    /// Get available transports for a peer
    async fn get_available_transports(&self, peer_id: &PeerId) -> Vec<TransportType> {
        let peer_transports = self.peer_transports.read().await;
        peer_transports.get(peer_id).cloned().unwrap_or_default()
    }

    /// Select best transport based on policy and metrics
    async fn select_transport(
        &self,
        available: &[TransportType],
        packet_size: usize,
    ) -> Result<TransportType, Box<dyn std::error::Error>> {
        let metrics = self.transport_metrics.read().await;

        match self.failover_policy {
            FailoverPolicy::FastestFirst => {
                // Select transport with lowest latency
                available
                    .iter()
                    .min_by_key(|t| {
                        metrics
                            .get(t)
                            .map(|m| m.latency_ms as u64)
                            .unwrap_or(u64::MAX)
                    })
                    .copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::MostReliable => {
                // Select transport with highest reliability
                available
                    .iter()
                    .max_by_key(|t| {
                        metrics
                            .get(t)
                            .map(|m| (m.reliability_score * 1000.0) as u64)
                            .unwrap_or(0)
                    })
                    .copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::LoadBalanced => {
                // Round-robin or weighted selection
                // For now, just pick first available
                available
                    .first()
                    .copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::EnergyEfficient => {
                // Prefer Bluetooth for small packets, WiFi for medium, Internet for large
                if packet_size < 1000 {
                    available
                        .iter()
                        .find(|&&t| t == TransportType::Bluetooth)
                        .or_else(|| available.first())
                        .copied()
                        .ok_or("No transport available".into())
                } else {
                    available
                        .iter()
                        .find(|&&t| t == TransportType::WiFiDirect)
                        .or_else(|| available.first())
                        .copied()
                        .ok_or("No transport available".into())
                }
            }
        }
    }

    /// Update metrics after successful send
    async fn update_success_metrics(&self, transport_type: TransportType) {
        let mut metrics = self.transport_metrics.write().await;
        if let Some(m) = metrics.get_mut(&transport_type) {
            m.reliability_score = (m.reliability_score * 0.95) + 0.05; // Smooth increase
            m.last_updated = std::time::Instant::now();
        }
    }

    /// Update metrics after failed send
    async fn update_failure_metrics(&self, transport_type: TransportType) {
        let mut metrics = self.transport_metrics.write().await;
        if let Some(m) = metrics.get_mut(&transport_type) {
            m.reliability_score *= 0.9; // Decrease reliability
            m.packet_loss = (m.packet_loss * 0.9) + 0.1; // Increase loss estimate
            m.last_updated = std::time::Instant::now();
        }
    }
}
