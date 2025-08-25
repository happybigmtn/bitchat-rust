use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

use crate::protocol::PeerId;

/// Network health monitor
/// 
/// Feynman: Like having doctors constantly checking the pulse of
/// the network. They monitor vital signs (latency, connectivity),
/// diagnose problems (network splits), and prescribe treatments
/// (rerouting, reconnection).
pub struct NetworkMonitor {
    topology: Arc<RwLock<NetworkTopology>>,
    health_metrics: Arc<RwLock<HealthMetrics>>,
    anomaly_detector: Arc<AnomalyDetector>,
    alert_sender: mpsc::UnboundedSender<NetworkAlert>,
}

#[derive(Debug, Clone)]
pub struct NetworkTopology {
    pub nodes: HashMap<PeerId, NodeInfo>,
    pub edges: HashMap<(PeerId, PeerId), EdgeInfo>,
    pub clusters: Vec<HashSet<PeerId>>,
    pub bridge_nodes: HashSet<PeerId>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub peer_id: PeerId,
    pub last_seen: std::time::Instant,
    pub connections: Vec<PeerId>,
    pub node_type: NodeType,
}

#[derive(Debug, Clone)]
pub struct EdgeInfo {
    pub latency_ms: f64,
    pub bandwidth_kbps: f64,
    pub last_active: std::time::Instant,
    pub reliability: f64,
}

#[derive(Debug, Clone)]
pub enum NodeType {
    Regular,
    Bridge,
    Bootstrap,
}

#[derive(Debug, Clone)]
pub struct HealthMetrics {
    pub total_nodes: usize,
    pub active_connections: usize,
    pub average_latency: f64,
    pub network_diameter: u32,
    pub clustering_coefficient: f64,
    pub partition_risk: f64,
}

#[derive(Debug, Clone)]
pub enum NetworkAlert {
    AnomalyDetected(Anomaly),
    PartitionRisk,
    HighLatency { peer_id: PeerId, latency: f64 },
    NodeOffline { peer_id: PeerId },
}

#[derive(Debug, Clone)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub severity: f64,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum AnomalyType {
    LatencySpike,
    ConnectivityDrop,
    PartitionDetected,
    UnusualTraffic,
}

pub struct AnomalyDetector {
    baseline_metrics: Arc<RwLock<HealthMetrics>>,
    threshold_multiplier: f64,
}

impl AnomalyDetector {
    pub fn new(threshold_multiplier: f64) -> Self {
        Self {
            baseline_metrics: Arc::new(RwLock::new(HealthMetrics {
                total_nodes: 0,
                active_connections: 0,
                average_latency: 100.0,
                network_diameter: 5,
                clustering_coefficient: 0.3,
                partition_risk: 0.1,
            })),
            threshold_multiplier,
        }
    }
    
    pub async fn check(&self, current_metrics: &HealthMetrics) -> Option<Anomaly> {
        let baseline = self.baseline_metrics.read().await;
        
        // Check for latency anomalies
        if current_metrics.average_latency > baseline.average_latency * self.threshold_multiplier {
            return Some(Anomaly {
                anomaly_type: AnomalyType::LatencySpike,
                severity: current_metrics.average_latency / baseline.average_latency,
                description: format!(
                    "Latency spiked to {:.1}ms (baseline: {:.1}ms)",
                    current_metrics.average_latency,
                    baseline.average_latency
                ),
            });
        }
        
        // Check for connectivity drops
        let connection_ratio = current_metrics.active_connections as f64 / baseline.active_connections.max(1) as f64;
        if connection_ratio < (1.0 / self.threshold_multiplier) {
            return Some(Anomaly {
                anomaly_type: AnomalyType::ConnectivityDrop,
                severity: 1.0 / connection_ratio,
                description: format!(
                    "Active connections dropped to {} (baseline: {})",
                    current_metrics.active_connections,
                    baseline.active_connections
                ),
            });
        }
        
        None
    }
}

impl NetworkMonitor {
    pub async fn start_monitoring(&self) {
        // Start periodic health checks
        let topology = self.topology.clone();
        let health_metrics = self.health_metrics.clone();
        let anomaly_detector = self.anomaly_detector.clone();
        let alert_sender = self.alert_sender.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            
            loop {
                ticker.tick().await;
                
                // Calculate health metrics
                let metrics = Self::calculate_health_metrics(&topology).await;
                *health_metrics.write().await = metrics.clone();
                
                // Check for anomalies
                if let Some(anomaly) = anomaly_detector.check(&metrics).await {
                    alert_sender.send(NetworkAlert::AnomalyDetected(anomaly)).ok();
                }
                
                // Check for network partitions
                if metrics.partition_risk > 0.7 {
                    alert_sender.send(NetworkAlert::PartitionRisk).ok();
                }
            }
        });
    }
    
    async fn calculate_health_metrics(
        topology: &Arc<RwLock<NetworkTopology>>,
    ) -> HealthMetrics {
        let topo = topology.read().await;
        
        HealthMetrics {
            total_nodes: topo.nodes.len(),
            active_connections: topo.edges.len(),
            average_latency: Self::calculate_average_latency(&topo.edges),
            network_diameter: Self::calculate_diameter(&topo),
            clustering_coefficient: Self::calculate_clustering(&topo),
            partition_risk: Self::calculate_partition_risk(&topo),
        }
    }
    
    fn calculate_average_latency(edges: &HashMap<(PeerId, PeerId), EdgeInfo>) -> f64 {
        if edges.is_empty() {
            return 0.0;
        }
        
        let total_latency: f64 = edges.values().map(|edge| edge.latency_ms).sum();
        total_latency / edges.len() as f64
    }
    
    fn calculate_diameter(topology: &NetworkTopology) -> u32 {
        // Simplified diameter calculation using BFS
        let mut max_distance = 0u32;
        
        for start_node in topology.nodes.keys() {
            let distances = Self::bfs_distances(topology, *start_node);
            if let Some(max_dist) = distances.values().max() {
                max_distance = max_distance.max(*max_dist);
            }
        }
        
        max_distance
    }
    
    fn bfs_distances(topology: &NetworkTopology, start: PeerId) -> HashMap<PeerId, u32> {
        let mut distances = HashMap::new();
        let mut queue = VecDeque::new();
        
        distances.insert(start, 0);
        queue.push_back(start);
        
        while let Some(current) = queue.pop_front() {
            let current_distance = distances[&current];
            
            if let Some(node) = topology.nodes.get(&current) {
                for &neighbor in &node.connections {
                    if let std::collections::hash_map::Entry::Vacant(e) = distances.entry(neighbor) {
                        e.insert(current_distance + 1);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        
        distances
    }
    
    fn calculate_clustering(topology: &NetworkTopology) -> f64 {
        if topology.nodes.len() < 3 {
            return 0.0;
        }
        
        let mut total_clustering = 0.0;
        let mut node_count = 0;
        
        for node in topology.nodes.values() {
            if node.connections.len() < 2 {
                continue;
            }
            
            let mut triangle_count = 0;
            let possible_triangles = node.connections.len() * (node.connections.len() - 1) / 2;
            
            for i in 0..node.connections.len() {
                for j in (i + 1)..node.connections.len() {
                    let node1 = node.connections[i];
                    let node2 = node.connections[j];
                    
                    if topology.edges.contains_key(&(node1, node2)) || 
                       topology.edges.contains_key(&(node2, node1)) {
                        triangle_count += 1;
                    }
                }
            }
            
            if possible_triangles > 0 {
                total_clustering += triangle_count as f64 / possible_triangles as f64;
                node_count += 1;
            }
        }
        
        if node_count > 0 {
            total_clustering / node_count as f64
        } else {
            0.0
        }
    }
    
    fn calculate_partition_risk(topology: &NetworkTopology) -> f64 {
        // Count bridge nodes - nodes whose removal would partition the network
        let bridge_count = topology.bridge_nodes.len();
        let total_nodes = topology.nodes.len();
        
        if total_nodes == 0 {
            return 1.0;
        }
        
        // Risk increases with higher ratio of bridge nodes
        let bridge_ratio = bridge_count as f64 / total_nodes as f64;
        
        // Also consider connectivity
        let avg_connections = if total_nodes > 0 {
            topology.edges.len() as f64 / total_nodes as f64
        } else {
            0.0
        };
        
        // Low connectivity and high bridge ratio = high partition risk
        let connectivity_factor = if avg_connections < 2.0 {
            1.0 - (avg_connections / 2.0)
        } else {
            0.0
        };
        
        (bridge_ratio + connectivity_factor).min(1.0)
    }
}