//! Loop Budget Demo - Preventing Unbounded Resource Consumption
//!
//! This example demonstrates how to use the LoopBudget utility to prevent
//! infinite loops from consuming unbounded CPU and memory resources.

use bitcraps::utils::{LoopBudget, BoundedLoop, CircuitBreaker, LoadShedder, OverflowHandler};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, Instant};

/// Example 1: Basic loop budget usage
async fn basic_loop_budget_example() {
    println!("=== Basic Loop Budget Example ===");
    
    // Create budget allowing 100 iterations per second
    let budget = LoopBudget::new(100);
    let mut iterations = 0;
    
    let start_time = Instant::now();
    
    for _ in 0..150 {
        if !budget.can_proceed() {
            println!("Budget exhausted at iteration {}, backing off...", iterations);
            budget.backoff().await;
            continue;
        }
        
        budget.consume(1);
        iterations += 1;
        
        // Simulate some work
        sleep(Duration::from_millis(1)).await;
    }
    
    println!("Completed {} iterations in {:?}", iterations, start_time.elapsed());
    println!("Budget utilization: {:.1}%\n", budget.utilization());
}

/// Example 2: Message processing with bounded channels
async fn bounded_message_processing_example() {
    println!("=== Bounded Message Processing Example ===");
    
    // Create bounded channel
    let (sender, receiver) = mpsc::channel::<String>(100);
    
    // Create bounded loop with budget
    let budget = LoopBudget::for_network();
    let overflow_handler = OverflowHandler::DropOldest;
    let mut bounded_loop = BoundedLoop::new(receiver, budget, overflow_handler);
    
    // Spawn producer task
    let producer = tokio::spawn(async move {
        for i in 0..200 {
            let message = format!("Message {}", i);
            if let Err(_) = sender.send(message).await {
                println!("Channel closed, stopping producer");
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    });
    
    // Process messages with budget control
    let processor = tokio::spawn(async move {
        bounded_loop.process_with_budget(|message| async move {
            println!("Processing: {}", message);
            // Simulate processing time
            sleep(Duration::from_millis(20)).await;
        }).await;
        
        // Print statistics
        let stats = bounded_loop.stats();
        println!("Total iterations: {}", stats.iterations.load(std::sync::atomic::Ordering::Relaxed));
        println!("Budget exceeded: {}", stats.budget_exceeded.load(std::sync::atomic::Ordering::Relaxed));
        println!("Messages dropped: {}", stats.messages_dropped.load(std::sync::atomic::Ordering::Relaxed));
    });
    
    // Wait for completion
    let _ = tokio::join!(producer, processor);
    println!();
}

/// Example 3: Circuit breaker pattern
async fn circuit_breaker_example() {
    println!("=== Circuit Breaker Example ===");
    
    let breaker = Arc::new(CircuitBreaker::new(3, Duration::from_secs(2)));
    let mut successful_requests = 0;
    let mut failed_requests = 0;
    let mut rejected_requests = 0;
    
    for i in 0..20 {
        if !breaker.allow_request() {
            println!("Request {} rejected - circuit breaker OPEN", i);
            rejected_requests += 1;
            continue;
        }
        
        // Simulate API call that might fail
        let success = i < 5 || i > 10; // Fail requests 5-10
        
        if success {
            println!("Request {} succeeded", i);
            breaker.record_success();
            successful_requests += 1;
        } else {
            println!("Request {} failed", i);
            breaker.record_failure();
            failed_requests += 1;
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    println!("Successful: {}, Failed: {}, Rejected: {}", 
             successful_requests, failed_requests, rejected_requests);
    println!("Final circuit state: {:?}\n", breaker.state());
}

/// Example 4: Load shedding under pressure
async fn load_shedding_example() {
    println!("=== Load Shedding Example ===");
    
    let load_shedder = Arc::new(LoadShedder::new(50)); // Max 50 items in queue
    let mut processed = 0;
    let mut shed = 0;
    
    for i in 0..200 {
        // Simulate varying queue load
        let queue_size = if i < 30 { 20 } else if i < 100 { 80 } else { 30 };
        load_shedder.update_queue_size(queue_size);
        
        if load_shedder.should_shed() {
            println!("Request {} shed due to overload (queue: {})", i, queue_size);
            shed += 1;
        } else {
            println!("Processing request {} (queue: {})", i, queue_size);
            processed += 1;
            // Simulate processing time
            sleep(Duration::from_millis(1)).await;
        }
    }
    
    println!("Processed: {}, Shed: {}", processed, shed);
    println!("Total shed count: {}\n", load_shedder.shed_count());
}

/// Example 5: Network-style loop with budget
async fn network_loop_example() {
    println!("=== Network Loop Example ===");
    
    let budget = LoopBudget::for_network();
    let (tx, mut rx) = mpsc::unbounded_channel::<i32>();
    
    // Simulate message producer
    let producer = tokio::spawn(async move {
        for i in 0..1000 {
            if tx.send(i).is_err() {
                break;
            }
            
            // Variable message rate
            if i % 50 == 0 {
                sleep(Duration::from_millis(100)).await;
            }
        }
    });
    
    // Message consumer with budget control
    let consumer = tokio::spawn(async move {
        let mut processed_count = 0;
        
        loop {
            // Check budget before processing
            if !budget.can_proceed() {
                println!("Budget exhausted (processed: {}), backing off...", processed_count);
                budget.backoff().await;
                continue;
            }
            
            // Try to receive with timeout
            match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(message)) => {
                    budget.consume(1);
                    processed_count += 1;
                    
                    if processed_count % 100 == 0 {
                        println!("Processed {} messages, budget utilization: {:.1}%", 
                               processed_count, budget.utilization());
                    }
                }
                Ok(None) => {
                    println!("Channel closed, processed {} total messages", processed_count);
                    break;
                }
                Err(_) => {
                    // Timeout - brief sleep to yield CPU
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
    });
    
    let _ = tokio::join!(producer, consumer);
    println!();
}

/// Example 6: Consensus-style loop with budget
async fn consensus_loop_example() {
    println!("=== Consensus Loop Example ===");
    
    let budget = LoopBudget::for_consensus();
    let mut round = 0;
    let start = Instant::now();
    
    // Simulate consensus rounds
    while round < 1000 && start.elapsed() < Duration::from_secs(10) {
        if !budget.can_proceed() {
            println!("Consensus budget exhausted at round {}, backing off...", round);
            budget.backoff().await;
            continue;
        }
        
        budget.consume(1);
        round += 1;
        
        // Simulate consensus work (validation, voting, etc.)
        sleep(Duration::from_millis(2)).await;
        
        if round % 100 == 0 {
            println!("Consensus round {}, utilization: {:.1}%", 
                   round, budget.utilization());
        }
    }
    
    println!("Completed {} consensus rounds in {:?}\n", round, start.elapsed());
}

#[tokio::main]
async fn main() {
    println!("BitCraps Loop Budget Resource Control Demo\n");
    println!("This demo shows how to prevent unbounded resource consumption in loops.\n");
    
    // Run all examples
    basic_loop_budget_example().await;
    bounded_message_processing_example().await;
    circuit_breaker_example().await;
    load_shedding_example().await;
    network_loop_example().await;
    consensus_loop_example().await;
    
    println!("=== Key Benefits ===");
    println!("✓ Prevents CPU starvation from runaway loops");
    println!("✓ Implements backpressure and load shedding");
    println!("✓ Provides circuit breaker pattern for fault tolerance");
    println!("✓ Enables graceful degradation under load");
    println!("✓ Maintains system responsiveness at scale");
}