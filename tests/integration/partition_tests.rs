#[tokio::test]
async fn test_network_partition_recovery() {
    let mut sim = NetworkSimulator::new();

    // Create 5-node network
    let nodes: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();
    for node in &nodes {
        sim.add_node(*node).await.unwrap();
    }

    // Connect in a line topology
    for i in 0..nodes.len() - 1 {
        sim.connect_nodes(nodes[i], nodes[i + 1]).await.unwrap();
    }

    // Send message before partition
    let stats_before = sim
        .simulate_message_propagation(nodes[0], "before partition".to_string())
        .await;
    assert_eq!(stats_before.successful_deliveries, 4);

    // Create partition (disconnect middle node)
    sim.disconnect_nodes(nodes[2], nodes[1]).await.unwrap();
    sim.disconnect_nodes(nodes[2], nodes[3]).await.unwrap();

    // Test message propagation during partition
    let stats_during = sim
        .simulate_message_propagation(nodes[0], "during partition".to_string())
        .await;
    assert!(stats_during.successful_deliveries < 4);

    // Reconnect and test recovery
    sim.connect_nodes(nodes[2], nodes[1]).await.unwrap();
    sim.connect_nodes(nodes[2], nodes[3]).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await; // Allow recovery

    let stats_after = sim
        .simulate_message_propagation(nodes[0], "after recovery".to_string())
        .await;
    assert_eq!(stats_after.successful_deliveries, 4);
}
