use std::collections::HashMap;
use tokio::sync::mpsc;

pub struct NetworkSimulator {
    nodes: HashMap<PeerId, SimulatedNode>,
    message_delay: Duration,
    packet_loss_rate: f64,
}

pub struct SimulatedNode {
    id: PeerId,
    network: MockNetwork,
    app: BitChatApp,
    message_rx: mpsc::Receiver<Message>,
}

impl NetworkSimulator {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            message_delay: Duration::from_millis(10),
            packet_loss_rate: 0.01,
        }
    }

    pub async fn add_node(&mut self, node_id: PeerId) -> Result<(), SimError> {
        let (tx, rx) = mpsc::channel(1000);
        let network = MockNetwork::new();
        let app = BitChatApp::new(network.clone()).await?;

        let node = SimulatedNode {
            id: node_id,
            network,
            app,
            message_rx: rx,
        };

        self.nodes.insert(node_id, node);
        Ok(())
    }

    pub async fn connect_nodes(&mut self, node_a: PeerId, node_b: PeerId) -> Result<(), SimError> {
        // Simulate connection establishment
        let node_a_ref = self.nodes.get_mut(&node_a).unwrap();
        node_a_ref
            .network
            .add_peer(node_b, PeerInfo::default())
            .await;

        let node_b_ref = self.nodes.get_mut(&node_b).unwrap();
        node_b_ref
            .network
            .add_peer(node_a, PeerInfo::default())
            .await;

        Ok(())
    }

    pub async fn simulate_message_propagation(
        &mut self,
        sender: PeerId,
        message: String,
    ) -> PropagationStats {
        let start_time = Instant::now();
        let mut stats = PropagationStats::new();

        // Send message from sender
        if let Some(sender_node) = self.nodes.get_mut(&sender) {
            sender_node
                .app
                .send_broadcast_message(message.clone())
                .await
                .unwrap();
        }

        // Simulate network propagation with delays
        tokio::time::sleep(self.message_delay).await;

        // Check message receipt at all nodes
        for (peer_id, node) in &mut self.nodes {
            if *peer_id != sender {
                // Simulate packet loss
                if rand::random::<f64>() < self.packet_loss_rate {
                    stats.lost_packets += 1;
                    continue;
                }

                // Check if message was received
                if let Ok(received_msg) = node.message_rx.try_recv() {
                    stats.successful_deliveries += 1;
                    stats.delivery_times.push(start_time.elapsed());
                }
            }
        }

        stats
    }
}
