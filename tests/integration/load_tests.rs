#[tokio::test]
async fn test_high_message_throughput() {
    const NUM_NODES: usize = 50;
    const MESSAGES_PER_NODE: usize = 100;

    let mut sim = NetworkSimulator::new();
    let nodes: Vec<PeerId> = (0..NUM_NODES).map(|_| PeerId::random()).collect();

    // Setup fully connected network
    for node in &nodes {
        sim.add_node(*node).await.unwrap();
    }

    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            sim.connect_nodes(nodes[i], nodes[j]).await.unwrap();
        }
    }

    let start_time = Instant::now();
    let mut handles = Vec::new();

    // Spawn concurrent message senders
    for (i, &node_id) in nodes.iter().enumerate() {
        let sim_clone = sim.clone();
        let handle = tokio::spawn(async move {
            for msg_num in 0..MESSAGES_PER_NODE {
                let message = format!("Message {} from node {}", msg_num, i);
                sim_clone
                    .simulate_message_propagation(node_id, message)
                    .await;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all messages to be sent
    for handle in handles {
        handle.await.unwrap();
    }

    let total_time = start_time.elapsed();
    let total_messages = NUM_NODES * MESSAGES_PER_NODE;
    let throughput = total_messages as f64 / total_time.as_secs_f64();

    println!("Throughput: {:.2} messages/second", throughput);
    assert!(throughput > 1000.0); // Expect at least 1000 msg/s
}
