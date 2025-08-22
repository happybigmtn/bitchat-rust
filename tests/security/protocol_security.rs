#[tokio::test]
async fn test_replay_attack_prevention() {
    let mut message_processor = MessageProcessor::new();
    let message = create_test_message();
    
    // First processing should succeed
    let result1 = message_processor.process_message(message.clone()).await;
    assert!(result1.is_ok());
    
    // Replayed message should be rejected
    let result2 = message_processor.process_message(message).await;
    assert!(result2.is_err());
    assert_matches!(result2.unwrap_err(), ProcessingError::ReplayAttack);
}

#[tokio::test]
async fn test_dos_protection() {
    let mut rate_limiter = RateLimiter::new(10, Duration::from_secs(1));
    let peer_id = PeerId::random();
    
    // Allow normal rate
    for _ in 0..10 {
        assert!(rate_limiter.check_rate(peer_id).is_ok());
    }
    
    // Block excessive rate
    assert!(rate_limiter.check_rate(peer_id).is_err());
    
    // Should recover after time window
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(rate_limiter.check_rate(peer_id).is_ok());
}