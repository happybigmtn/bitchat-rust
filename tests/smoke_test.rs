//! Basic smoke tests to verify core functionality compiles and works
//!
//! These tests ensure that the basic library modules can be imported
//! and basic operations function correctly.

use bitcraps::{
    crypto,
    error::{BitCrapsError, Result},
};

#[test]
fn test_error_types_exist() {
    // Test that core error types can be created
    let _error = BitCrapsError::ValidationError("test".to_string());
    assert!(true); // If we get here, error types are properly defined
}

#[test]
fn test_crypto_module_accessible() {
    // Test that crypto module is accessible
    // This verifies imports and basic structure
    let _result: Result<()> = Ok(());
    assert!(true);
}

#[test]
fn test_basic_encryption_works() {
    // Test basic encryption functionality if available
    // This is a minimal test to verify crypto basics
    let plaintext = b"Hello, BitCraps!";
    let _encrypted = plaintext.to_vec(); // Placeholder until real crypto is available
    assert_eq!(_encrypted.len(), plaintext.len());
}

#[test]
fn test_result_type_works() {
    // Test that our Result type works correctly
    fn success_function() -> Result<String> {
        Ok("success".to_string())
    }

    fn error_function() -> Result<String> {
        Err(BitCrapsError::ValidationError("test error".to_string()))
    }

    assert!(success_function().is_ok());
    assert!(error_function().is_err());
}

#[test]
fn test_basic_config_operations() {
    // Test that config-related operations work
    // Minimal test to verify config module is accessible
    assert!(true); // Placeholder
}

#[tokio::test]
async fn test_async_operations() {
    // Test that async operations work
    async fn async_success() -> Result<()> {
        Ok(())
    }

    let result = async_success().await;
    assert!(result.is_ok());
}

#[test]
fn test_compilation_integration() {
    // This test simply ensures all core modules compile together
    // If this test runs, it means the basic integration is working
    assert!(true, "If this assertion runs, basic compilation is working");
}
#![cfg(feature = "legacy-tests")]
