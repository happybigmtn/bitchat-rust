//! Example demonstrating the new error handling system
//!
//! This example shows how to use the production-grade error module with:
//! - Error codes for telemetry
//! - Structured context for debugging
//! - Error categories for monitoring
//! - Retry strategies

use bitcraps::error::{Error, ErrorBuilder, ErrorCategory, Result};

fn main() {
    // Example 1: Using helper functions for common errors
    demo_helper_functions();
    
    // Example 2: Using the error builder pattern
    demo_error_builder();
    
    // Example 3: Error categorization and monitoring
    demo_error_monitoring();
    
    // Example 4: Retry strategy demonstration
    demo_retry_strategy();
}

fn demo_helper_functions() {
    println!("\n=== Helper Functions Demo ===");
    
    // Network timeout error
    let err = Error::network_timeout("api.example.com", 5000);
    println!("Network Timeout Error:");
    println!("  Code: {}", err.code());
    println!("  Category: {:?}", err.category());
    println!("  Severity: {:?}", err.severity());
    println!("  Retryable: {}", err.is_retryable());
    
    // Insufficient balance error
    let err = Error::insufficient_balance_for("place_bet", 1000, 500);
    println!("\nInsufficient Balance Error:");
    println!("  Code: {}", err.code());
    println!("  Category: {:?}", err.category());
    println!("  Message: {}", err);
    
    // Validation error
    let err = Error::validation_failed("email", "format", "not-an-email");
    println!("\nValidation Error:");
    println!("  Code: {}", err.code());
    println!("  Category: {:?}", err.category());
    println!("  Retryable: {}", err.is_retryable());
}

fn demo_error_builder() {
    println!("\n=== Error Builder Demo ===");
    
    // Build a complex error with metadata
    let err = ErrorBuilder::new("E007", ErrorCategory::Network)
        .metadata("attempt", "3")
        .metadata("max_retries", "5")
        .metadata("endpoint", "validator-1.example.com")
        .related("E058") // Related to timeout error
        .with_stack_trace()
        .network("Connection to validator failed", Some("validator-1.example.com".to_string()));
    
    println!("Complex Network Error:");
    println!("  Code: {}", err.code());
    println!("  Context metadata: {:?}", err.context().metadata);
    println!("  Related codes: {:?}", err.context().related_codes);
    
    // Build a crypto error
    let err = ErrorBuilder::new("E005", ErrorCategory::Security)
        .metadata("algorithm", "X25519")
        .metadata("key_length", "32")
        .crypto("Key generation failed", "generate_keypair");
    
    println!("\nCrypto Error:");
    println!("  Code: {}", err.code());
    println!("  Category: {:?}", err.category());
    println!("  Severity: {:?}", err.severity());
}

fn demo_error_monitoring() {
    println!("\n=== Error Monitoring Demo ===");
    
    // Simulate different error categories
    let errors = vec![
        Error::Security {
            message: "Invalid signature".to_string(),
            violation_type: "signature_verification".to_string(),
            context: std::sync::Arc::new(
                bitcraps::error::ErrorContext::new("E041", ErrorCategory::Security)
            ),
        },
        Error::Consensus {
            message: "Byzantine fault detected".to_string(),
            round: 42,
            context: std::sync::Arc::new(
                bitcraps::error::ErrorContext::new("E044", ErrorCategory::Consensus)
            ),
        },
        Error::ResourceExhausted {
            message: "Connection pool exhausted".to_string(),
            resource_type: "connections".to_string(),
            limit: 1000,
            context: std::sync::Arc::new(
                bitcraps::error::ErrorContext::new("E048", ErrorCategory::Resources)
            ),
        },
    ];
    
    for err in errors {
        println!("\nError: {}", err);
        println!("  Category: {:?}", err.category());
        println!("  Severity: {:?}", err.severity());
        println!("  Alert Priority: {}", match err.severity() {
            bitcraps::error::ErrorSeverity::Critical => "PAGE IMMEDIATELY",
            bitcraps::error::ErrorSeverity::High => "Alert on-call",
            bitcraps::error::ErrorSeverity::Medium => "Create ticket",
            bitcraps::error::ErrorSeverity::Low => "Log only",
        });
    }
}

fn demo_retry_strategy() {
    println!("\n=== Retry Strategy Demo ===");
    
    let test_errors = vec![
        ("Network timeout", Error::network_timeout("api.example.com", 5000)),
        ("Validation failure", Error::validation_failed("age", "minimum", "-5")),
        ("Resource limit", Error::resource_exhausted("memory", 8192)),
        ("Security violation", Error::Security {
            message: "Unauthorized access".to_string(),
            violation_type: "auth".to_string(),
            context: std::sync::Arc::new(
                bitcraps::error::ErrorContext::new("E041", ErrorCategory::Security)
            ),
        }),
    ];
    
    for (name, err) in test_errors {
        println!("\n{}:", name);
        println!("  Retryable: {}", err.is_retryable());
        println!("  Strategy: {:?}", err.retry_strategy());
    }
}

/// Example function showing error propagation
fn process_transaction(amount: u64) -> Result<String> {
    // Validate input
    if amount == 0 {
        return Err(Error::validation_failed("amount", "non_zero", "0"));
    }
    
    // Check balance (simulated)
    let available_balance = 500;
    if amount > available_balance {
        return Err(Error::insufficient_balance_for(
            "transaction",
            amount,
            available_balance,
        ));
    }
    
    // Simulate network operation
    if amount > 1000 {
        return Err(Error::network_timeout("payment-gateway.example.com", 30000));
    }
    
    Ok(format!("Transaction {} processed successfully", amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process_transaction() {
        // Test validation error
        let result = process_transaction(0);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "E021");
        assert!(!err.is_retryable());
        
        // Test insufficient balance
        let result = process_transaction(600);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "E015");
        
        // Test success
        let result = process_transaction(100);
        assert!(result.is_ok());
    }
}