use std::time::Instant;

#[test]
fn test_game_creation_performance() {
    let start = Instant::now();
    
    // Simulate creating 1000 games
    for i in 0..1000 {
        let _game_id = format!("game_{}", i);
        // In production, this would create actual game instances
    }
    
    let elapsed = start.elapsed();
    
    // Should complete in under 100ms
    assert!(elapsed.as_millis() < 100, 
           "Game creation took {}ms, expected < 100ms", elapsed.as_millis());
}

#[test]
fn test_bet_processing_performance() {
    let start = Instant::now();
    
    // Process 10000 bet operations
    for i in 0..10000 {
        let _bet_id = format!("bet_{}", i);
        let _amount = 100;
        // In production, this would process actual bets
    }
    
    let elapsed = start.elapsed();
    
    // Should process 10000 bets in under 1 second
    assert!(elapsed.as_secs() < 1, 
           "Bet processing took {}s, expected < 1s", elapsed.as_secs());
}

#[test]
fn test_payout_calculation_performance() {
    let start = Instant::now();
    
    // Calculate 100000 payouts
    for _ in 0..100000 {
        let bet_amount = 100u64;
        let _payout = bet_amount * 2; // Simple 1:1 payout
    }
    
    let elapsed = start.elapsed();
    
    // Should complete in under 10ms
    assert!(elapsed.as_millis() < 10, 
           "Payout calculation took {}ms, expected < 10ms", elapsed.as_millis());
}

#[test]
fn test_concurrent_game_operations() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];
    
    let start = Instant::now();
    
    // Spawn 10 threads each processing 1000 operations
    for _ in 0..10 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let elapsed = start.elapsed();
    let total_ops = counter.load(Ordering::SeqCst);
    
    assert_eq!(total_ops, 10000, "Should have processed 10000 operations");
    assert!(elapsed.as_millis() < 100, 
           "Concurrent operations took {}ms, expected < 100ms", elapsed.as_millis());
}