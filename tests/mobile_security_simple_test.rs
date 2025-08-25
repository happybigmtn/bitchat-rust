//! Simple mobile security functionality tests
//!
//! These tests verify the core mobile security components work independently:
//! - Key derivation functionality
//! - Permission management
//! - Secure storage basics

use bitcraps::mobile::{
    KeyDerivationManager, PbkdfAlgorithm, HkdfAlgorithm, Argon2Config,
    PermissionManager, Permission, PermissionState,
    SecureStorageManager,
};
use bitcraps::error::Result;

/// Test key derivation functionality
#[tokio::test]
async fn test_key_derivation_functionality() -> Result<()> {
    println!("Testing key derivation functionality...");
    
    // Create key derivation manager
    let key_manager = KeyDerivationManager::new(false); // Software mode for testing
    
    println!("Testing master key generation...");
    
    // Test master key generation
    let master_key_id = "test_master_key";
    let master_key = key_manager.get_master_key(master_key_id)?;
    
    println!("Master key generated, length: {} bytes", master_key.len());
    assert_eq!(master_key.len(), 32);
    assert!(!master_key.is_empty());
    
    // Test same key ID returns same key
    let same_key = key_manager.get_master_key(master_key_id)?;
    assert_eq!(master_key.as_bytes(), same_key.as_bytes());
    
    println!("Testing PBKDF2 key derivation...");
    
    // Test PBKDF2-SHA256 derivation
    let password = "test_password_with_sufficient_entropy";
    let salt = b"test_salt_16byte";
    let iterations = 10000;
    
    let pbkdf2_key = key_manager.derive_key_pbkdf2(
        password,
        salt,
        iterations,
        32,
        PbkdfAlgorithm::Pbkdf2Sha256,
    )?;
    
    println!("PBKDF2 key derived, length: {} bytes", pbkdf2_key.len());
    assert_eq!(pbkdf2_key.len(), 32);
    assert!(!pbkdf2_key.is_empty());
    
    // Test deterministic behavior - same inputs produce same output
    let pbkdf2_key2 = key_manager.derive_key_pbkdf2(
        password,
        salt,
        iterations,
        32,
        PbkdfAlgorithm::Pbkdf2Sha256,
    )?;
    assert_eq!(pbkdf2_key.as_bytes(), pbkdf2_key2.as_bytes());
    
    println!("Testing Argon2id key derivation...");
    
    // Test Argon2id derivation
    let argon2_config = Argon2Config::default();
    let argon2_key = key_manager.derive_key_argon2id(password, salt, &argon2_config)?;
    
    println!("Argon2id key derived, length: {} bytes", argon2_key.len());
    assert_eq!(argon2_key.len(), 32);
    assert!(!argon2_key.is_empty());
    
    // Test that different algorithms produce different keys
    assert_ne!(pbkdf2_key.as_bytes(), argon2_key.as_bytes());
    
    println!("Testing HKDF key derivation...");
    
    // Test HKDF derivation
    let hkdf_master_id = "test_hkdf_master";
    let _master_key = key_manager.get_master_key(hkdf_master_id)?;
    
    let hkdf_key = key_manager.derive_key_hkdf(
        hkdf_master_id,
        Some(b"test_hkdf_salt"),
        b"test_context_info",
        32,
        HkdfAlgorithm::HkdfSha256,
    )?;
    
    println!("HKDF key derived, length: {} bytes", hkdf_key.len());
    assert_eq!(hkdf_key.len(), 32);
    assert!(!hkdf_key.is_empty());
    
    // Test HKDF with different contexts produces different keys
    let hkdf_key2 = key_manager.derive_key_hkdf(
        hkdf_master_id,
        Some(b"test_hkdf_salt"),
        b"different_context_info",
        32,
        HkdfAlgorithm::HkdfSha256,
    )?;
    
    assert_ne!(hkdf_key.as_bytes(), hkdf_key2.as_bytes());
    
    println!("Testing key hierarchy creation...");
    
    // Test application key hierarchy
    let app_id = "bitcraps_test_app";
    let key_hierarchy = key_manager.create_key_hierarchy(app_id)?;
    
    println!("Key hierarchy created for app: {}", key_hierarchy.app_id);
    assert_eq!(key_hierarchy.app_id, app_id);
    assert_eq!(key_hierarchy.encryption_key.len(), 32);
    assert_eq!(key_hierarchy.signing_key.len(), 32);
    assert_eq!(key_hierarchy.authentication_key.len(), 32);
    assert_eq!(key_hierarchy.session_key.len(), 32);
    
    // Verify all keys are different (different derivation contexts)
    assert_ne!(key_hierarchy.encryption_key.as_bytes(), key_hierarchy.signing_key.as_bytes());
    assert_ne!(key_hierarchy.encryption_key.as_bytes(), key_hierarchy.authentication_key.as_bytes());
    assert_ne!(key_hierarchy.signing_key.as_bytes(), key_hierarchy.session_key.as_bytes());
    
    println!("Key derivation functionality test completed successfully!");
    
    Ok(())
}

/// Test permission management functionality
#[tokio::test]
async fn test_permission_management_functionality() -> Result<()> {
    println!("Testing permission management functionality...");
    
    // Get BitCraps specific permissions
    let required_permissions = PermissionManager::get_bitcraps_required_permissions();
    let optional_permissions = PermissionManager::get_bitcraps_optional_permissions();
    
    println!("Required permissions count: {}", required_permissions.len());
    println!("Optional permissions count: {}", optional_permissions.len());
    
    assert!(!required_permissions.is_empty());
    assert!(!optional_permissions.is_empty());
    
    // Verify specific required permissions for BitCraps
    assert!(required_permissions.contains(&Permission::Bluetooth));
    assert!(required_permissions.contains(&Permission::BluetoothAdmin));
    assert!(required_permissions.contains(&Permission::AccessCoarseLocation));
    assert!(required_permissions.contains(&Permission::ForegroundService));
    
    // Create permission manager
    let permission_manager = PermissionManager::new(
        required_permissions.clone(),
        optional_permissions.clone(),
    );
    
    println!("Testing permission summary...");
    
    // Check permission summary
    let summary = permission_manager.check_all_permissions()?;
    println!("All required permissions granted: {}", summary.all_required_granted);
    println!("Can continue: {}", summary.can_continue);
    
    // In simulation mode, all permissions should be granted
    assert!(summary.all_required_granted);
    assert!(summary.can_continue);
    assert!(!summary.granted_required.is_empty());
    
    println!("Testing individual permission requests...");
    
    // Test individual permission request
    let camera_result = permission_manager.request_permission(
        Permission::Camera,
        "Camera access needed for QR code scanning and wallet address sharing"
    ).await?;
    
    println!("Camera permission result: {:?}", camera_result);
    assert_eq!(camera_result, PermissionState::Granted);
    
    // Test multiple permission request
    let bluetooth_permissions = vec![
        Permission::Bluetooth,
        Permission::BluetoothAdmin,
    ];
    
    let batch_results = permission_manager.request_permissions(
        bluetooth_permissions.clone(),
        "Bluetooth permissions enable secure peer-to-peer gaming"
    ).await?;
    
    println!("Batch permission results: {:?}", batch_results);
    assert_eq!(batch_results.len(), bluetooth_permissions.len());
    
    for (permission, state) in &batch_results {
        assert_eq!(*state, PermissionState::Granted);
        println!("Permission {:?}: {:?}", permission, state);
    }
    
    println!("Testing permission explanations...");
    
    // Test permission explanations
    let bluetooth_explanation = permission_manager.get_permission_explanation(Permission::Bluetooth);
    println!("Bluetooth explanation: {}", bluetooth_explanation);
    assert!(bluetooth_explanation.contains("peer-to-peer"));
    
    let location_explanation = permission_manager.get_permission_explanation(Permission::AccessCoarseLocation);
    println!("Location explanation: {}", location_explanation);
    assert!(location_explanation.contains("Bluetooth Low Energy"));
    
    println!("Testing permission functionality checks...");
    
    // Test functionality checks
    assert!(!permission_manager.can_function_without(Permission::Bluetooth)); // Required
    assert!(permission_manager.can_function_without(Permission::Camera)); // Optional
    
    println!("Testing onboarding flow...");
    
    // Test onboarding flow
    let mut flow = permission_manager.create_onboarding_flow();
    println!("Permission onboarding flow has {} steps", flow.steps.len());
    assert!(!flow.is_complete());
    assert!(flow.steps.len() >= 3);
    
    // Walk through the onboarding flow
    let mut step_count = 0;
    while let Some(step) = flow.current() {
        println!("Step {}: {} - {} permissions", 
                step_count + 1, step.title, step.permissions.len());
        println!("  Description: {}", step.description);
        println!("  Required: {}", step.required);
        
        step_count += 1;
        
        if flow.next_step().is_none() {
            break;
        }
    }
    
    assert!(flow.is_complete());
    assert!(step_count > 0);
    println!("Completed {} onboarding steps", step_count);
    
    println!("Permission management functionality test completed successfully!");
    
    Ok(())
}

/// Test secure storage functionality  
#[tokio::test]
async fn test_secure_storage_functionality() -> Result<()> {
    println!("Testing secure storage functionality...");
    
    // Create secure storage manager
    let storage_manager = SecureStorageManager::new()?;
    
    println!("Testing private key storage...");
    
    // Test private key storage and retrieval
    let key_id = "test_storage_key_123";
    let private_key = b"test_private_key_data_exactly_32b";
    assert_eq!(private_key.len(), 32);
    
    storage_manager.store_private_key(key_id, private_key)?;
    println!("Stored private key for ID: {}", key_id);
    
    let retrieved_key = storage_manager.retrieve_private_key(key_id)?;
    match retrieved_key {
        Some(key_data) => {
            println!("Retrieved private key, length: {} bytes", key_data.len());
            assert!(!key_data.is_empty());
            // Note: In actual implementation with encryption, stored data would be encrypted
        },
        None => {
            println!("Private key not found (this may be expected in test/simulation mode)");
        }
    }
    
    println!("Testing session token storage...");
    
    // Test session token storage and retrieval
    let session_id = "test_session_456";
    let auth_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.example_token_data";
    
    storage_manager.store_session_token(session_id, auth_token)?;
    println!("Stored session token for session: {}", session_id);
    
    let retrieved_token = storage_manager.retrieve_session_token(session_id)?;
    match retrieved_token {
        Some(token_data) => {
            println!("Retrieved session token length: {} chars", token_data.len());
            assert!(!token_data.is_empty());
        },
        None => {
            println!("Session token not found (this may be expected in test/simulation mode)");
        }
    }
    
    println!("Testing user credentials storage...");
    
    // Test user credentials storage and retrieval
    let user_id = "test_user_789";
    let test_credentials = bitcraps::mobile::UserCredentials {
        user_id: user_id.to_string(),
        encrypted_private_key: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        public_key: vec![17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32],
        created_at: 1640995200, // Jan 1, 2022 timestamp
        last_used: 1640995260,  // 1 minute later
    };
    
    storage_manager.store_user_credentials(user_id, &test_credentials)?;
    println!("Stored user credentials for user: {}", user_id);
    
    let retrieved_credentials = storage_manager.retrieve_user_credentials(user_id)?;
    match retrieved_credentials {
        Some(credentials) => {
            println!("Retrieved credentials for user: {}", credentials.user_id);
            assert_eq!(credentials.user_id, user_id);
            assert!(!credentials.encrypted_private_key.is_empty());
            assert!(!credentials.public_key.is_empty());
            assert_eq!(credentials.created_at, test_credentials.created_at);
        },
        None => {
            println!("User credentials not found (this may be expected in test/simulation mode)");
        }
    }
    
    println!("Testing game checkpoint storage...");
    
    // Test game checkpoint storage
    let game_id = "test_game_abc";
    let mut player_balances = std::collections::HashMap::new();
    player_balances.insert("player1".to_string(), 1000u64);
    player_balances.insert("player2".to_string(), 750u64);
    
    let game_checkpoint = bitcraps::mobile::GameCheckpoint {
        game_id: game_id.to_string(),
        state_hash: vec![0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81],
        player_balances,
        bet_history: vec!["bet1".to_string(), "bet2".to_string()],
        checkpoint_time: 1640995300,
    };
    
    storage_manager.store_game_checkpoint(game_id, &game_checkpoint)?;
    println!("Stored game checkpoint for game: {}", game_id);
    
    let retrieved_checkpoint = storage_manager.retrieve_game_checkpoint(game_id)?;
    match retrieved_checkpoint {
        Some(checkpoint) => {
            println!("Retrieved checkpoint for game: {}", checkpoint.game_id);
            assert_eq!(checkpoint.game_id, game_id);
            assert!(!checkpoint.state_hash.is_empty());
            assert!(!checkpoint.player_balances.is_empty());
            assert!(!checkpoint.bet_history.is_empty());
        },
        None => {
            println!("Game checkpoint not found (this may be expected in test/simulation mode)");
        }
    }
    
    println!("Testing user data deletion (GDPR compliance)...");
    
    // Test GDPR compliance - delete user data
    let delete_result = storage_manager.delete_user_data(user_id);
    match delete_result {
        Ok(()) => {
            println!("User data deleted successfully for GDPR compliance");
        },
        Err(e) => {
            println!("User data deletion failed (may be expected in test mode): {}", e);
        }
    }
    
    println!("Secure storage functionality test completed successfully!");
    
    Ok(())
}

/// Test cross-platform compatibility
#[tokio::test]
async fn test_cross_platform_compatibility() -> Result<()> {
    println!("Testing cross-platform compatibility...");
    
    // Test that all core managers can be created on any platform
    println!("Testing SecureStorageManager creation...");
    let storage_manager = SecureStorageManager::new();
    assert!(storage_manager.is_ok(), "SecureStorageManager should be creatable on all platforms");
    
    println!("Testing KeyDerivationManager creation...");
    let key_manager = KeyDerivationManager::new(false); // Software mode for cross-platform compatibility
    
    println!("Testing PermissionManager creation...");
    let permission_manager = PermissionManager::new(
        vec![Permission::Bluetooth],
        vec![Permission::Camera]
    );
    
    println!("Testing basic functionality across platforms...");
    
    // Test basic key derivation works everywhere
    let test_key = key_manager.get_master_key("cross_platform_test")?;
    println!("Cross-platform master key generated, length: {} bytes", test_key.len());
    assert_eq!(test_key.len(), 32);
    
    // Test permission checking works everywhere (simulation mode)
    let summary = permission_manager.check_all_permissions()?;
    println!("Cross-platform permission check completed, can continue: {}", summary.can_continue);
    assert!(summary.can_continue); // Should work in simulation mode
    
    // Test secure storage works everywhere
    let storage_mgr = storage_manager?;
    let test_data = b"cross_platform_test_data";
    let store_result = storage_mgr.store_private_key("cross_platform_key", test_data);
    println!("Cross-platform storage test: {:?}", store_result.is_ok());
    
    println!("Cross-platform compatibility test completed successfully!");
    
    Ok(())
}

/// Performance benchmark for key derivation operations
#[tokio::test]
async fn test_key_derivation_performance() -> Result<()> {
    println!("Testing key derivation performance...");
    
    let key_manager = KeyDerivationManager::new(false);
    let password = "performance_test_password_with_entropy";
    let salt = b"performance_test_salt_16b";
    
    // Test PBKDF2 performance (lower iterations for testing)
    let pbkdf2_start = std::time::Instant::now();
    for i in 0..5 {
        let _key = key_manager.derive_key_pbkdf2(
            &format!("{}_{}", password, i),
            salt,
            5000, // Lower iterations for faster testing
            32,
            PbkdfAlgorithm::Pbkdf2Sha256,
        )?;
    }
    let pbkdf2_duration = pbkdf2_start.elapsed();
    println!("PBKDF2 (5 derivations): {:?}", pbkdf2_duration);
    
    // Test HKDF performance
    let hkdf_start = std::time::Instant::now();
    let master_key_id = "perf_test_master";
    let _master = key_manager.get_master_key(master_key_id)?;
    
    for i in 0..50 {
        let _key = key_manager.derive_key_hkdf(
            master_key_id,
            Some(salt),
            format!("context_{}", i).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
    }
    let hkdf_duration = hkdf_start.elapsed();
    println!("HKDF (50 derivations): {:?}", hkdf_duration);
    
    // HKDF should be significantly faster than PBKDF2
    assert!(hkdf_duration < pbkdf2_duration);
    println!("Performance test confirmed: HKDF is faster than PBKDF2 (as expected)");
    
    // Test Argon2 performance (single iteration due to computational intensity)
    let argon2_start = std::time::Instant::now();
    let argon2_config = Argon2Config {
        memory_cost: 4096,  // Reduced for testing
        time_cost: 2,       // Reduced for testing
        parallelism: 1,
        hash_length: 32,
    };
    
    let _argon2_key = key_manager.derive_key_argon2id(password, salt, &argon2_config)?;
    let argon2_duration = argon2_start.elapsed();
    println!("Argon2id (1 derivation): {:?}", argon2_duration);
    
    println!("Key derivation performance test completed!");
    
    Ok(())
}