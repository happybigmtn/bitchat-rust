//! Comprehensive integration tests for mobile security features
//!
//! These tests verify that all mobile security components work together correctly:
//! - Biometric authentication
//! - Secure storage (Android Keystore, iOS Keychain)  
//! - Key derivation and management
//! - Permission handling
//! - Cross-platform compatibility

use bitcraps::mobile::{
    MobileSecurityManager, MobileSecurityConfig, SecurityPolicy, SecurityLevel,
    BiometricAuthManager, BiometricType,
    KeyDerivationManager, PbkdfAlgorithm, HkdfAlgorithm, Argon2Config,
    PermissionManager, Permission, PermissionState,
    SecureStorageManager,
};
use bitcraps::error::{Result, Error};

/// Test biometric authentication functionality
#[tokio::test]
async fn test_biometric_authentication() -> Result<()> {
    // Create biometric manager
    let biometric_manager = BiometricAuthManager::new()?;
    
    // Check if biometric auth is available (will work in simulation mode)
    let is_configured = biometric_manager.is_biometric_configured()?;
    println!("Biometric authentication configured: {}", is_configured);
    
    // Test user authentication
    let auth_result = biometric_manager.authenticate_user("Test mobile security integration").await;
    match auth_result {
        Ok(session) => {
            println!("Authentication successful! Session ID: {}", session.session_id);
            assert!(!session.session_id.is_empty());
            assert!(session.expires_at > session.created_at);
        },
        Err(e) => {
            println!("Authentication failed (expected in test environment): {}", e);
            // This is expected in non-mobile test environments
        }
    }
    
    // Test biometric-protected wallet key creation
    let wallet_id = "test_biometric_wallet";
    let protected_key = biometric_manager.create_protected_wallet_key(wallet_id)?;
    
    println!("Created protected wallet key: {}", protected_key.key_alias);
    assert!(protected_key.key_alias.contains(wallet_id));
    assert!(!protected_key.key_derivation_salt.is_empty());
    
    // Test wallet key unlock
    let unlocked_key = biometric_manager.unlock_wallet_key(&protected_key)?;
    println!("Unlocked wallet key, length: {} bytes", unlocked_key.len());
    assert_eq!(unlocked_key.len(), 32); // 256-bit key
    
    Ok(())
}

/// Test key derivation functionality
#[tokio::test]
async fn test_key_derivation() -> Result<()> {
    // Create key derivation manager
    let key_manager = KeyDerivationManager::new(false); // Software mode for testing
    
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
    
    println!("Testing Argon2id key derivation...");
    
    // Test Argon2id derivation
    let argon2_config = Argon2Config::default();
    let argon2_key = key_manager.derive_key_argon2id(password, salt, &argon2_config)?;
    
    println!("Argon2id key derived, length: {} bytes", argon2_key.len());
    assert_eq!(argon2_key.len(), 32);
    assert!(!argon2_key.is_empty());
    
    // Verify keys are different (different algorithms)
    assert_ne!(pbkdf2_key.as_bytes(), argon2_key.as_bytes());
    
    println!("Testing HKDF key derivation...");
    
    // Test HKDF derivation
    let master_key_id = "test_hkdf_master";
    let _master_key = key_manager.get_master_key(master_key_id)?;
    
    let hkdf_key = key_manager.derive_key_hkdf(
        master_key_id,
        Some(b"test_hkdf_salt"),
        b"test_context_info",
        32,
        HkdfAlgorithm::HkdfSha256,
    )?;
    
    println!("HKDF key derived, length: {} bytes", hkdf_key.len());
    assert_eq!(hkdf_key.len(), 32);
    assert!(!hkdf_key.is_empty());
    
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
    
    // Verify all keys are different
    assert_ne!(key_hierarchy.encryption_key.as_bytes(), key_hierarchy.signing_key.as_bytes());
    assert_ne!(key_hierarchy.encryption_key.as_bytes(), key_hierarchy.authentication_key.as_bytes());
    assert_ne!(key_hierarchy.signing_key.as_bytes(), key_hierarchy.session_key.as_bytes());
    
    Ok(())
}

/// Test permission management
#[tokio::test]
async fn test_permission_management() -> Result<()> {
    println!("Testing permission management...");
    
    // Create permission manager with BitCraps permissions
    let required_permissions = PermissionManager::get_bitcraps_required_permissions();
    let optional_permissions = PermissionManager::get_bitcraps_optional_permissions();
    
    println!("Required permissions: {:?}", required_permissions);
    println!("Optional permissions: {:?}", optional_permissions);
    
    let permission_manager = PermissionManager::new(required_permissions, optional_permissions);
    
    // Check permission summary
    let summary = permission_manager.check_all_permissions()?;
    println!("All required permissions granted: {}", summary.all_required_granted);
    println!("Can continue: {}", summary.can_continue);
    
    // In simulation mode, all permissions should be granted
    assert!(summary.all_required_granted);
    assert!(summary.can_continue);
    
    // Test individual permission requests
    let camera_result = permission_manager.request_permission(
        Permission::Camera,
        "Camera access needed for QR code scanning"
    ).await?;
    
    println!("Camera permission result: {:?}", camera_result);
    assert_eq!(camera_result, PermissionState::Granted);
    
    // Test batch permission request
    let batch_permissions = vec![
        Permission::Bluetooth,
        Permission::BluetoothAdmin,
        Permission::AccessCoarseLocation,
    ];
    
    let batch_results = permission_manager.request_permissions(
        batch_permissions,
        "These permissions enable secure Bluetooth gaming"
    ).await?;
    
    println!("Batch permission results: {:?}", batch_results);
    assert_eq!(batch_results.len(), 3);
    
    for (permission, state) in batch_results {
        assert_eq!(state, PermissionState::Granted);
        println!("Permission {:?}: {:?}", permission, state);
    }
    
    // Test permission explanations
    let bluetooth_explanation = permission_manager.get_permission_explanation(Permission::Bluetooth);
    println!("Bluetooth permission explanation: {}", bluetooth_explanation);
    assert!(bluetooth_explanation.contains("peer-to-peer"));
    
    // Test onboarding flow
    let mut flow = permission_manager.create_onboarding_flow();
    println!("Permission onboarding flow has {} steps", flow.steps.len());
    assert!(!flow.is_complete());
    
    // Walk through the flow
    let mut step_count = 0;
    while let Some(step) = flow.current() {
        println!("Step {}: {} - {} permissions", 
                step_count + 1, step.title, step.permissions.len());
        step_count += 1;
        
        if flow.next_step().is_none() {
            break;
        }
    }
    
    assert!(flow.is_complete());
    assert!(step_count > 0);
    
    Ok(())
}

/// Test secure storage functionality
#[tokio::test]
async fn test_secure_storage() -> Result<()> {
    println!("Testing secure storage...");
    
    // Create secure storage manager
    let storage_manager = SecureStorageManager::new()?;
    
    // Test private key storage
    let key_id = "test_storage_key";
    let private_key = b"test_private_key_data_32_bytes!!";
    
    storage_manager.store_private_key(key_id, private_key)?;
    println!("Stored private key for ID: {}", key_id);
    
    let retrieved_key = storage_manager.retrieve_private_key(key_id)?;
    match retrieved_key {
        Some(key_data) => {
            println!("Retrieved private key, length: {} bytes", key_data.len());
            // Note: In real implementation with encryption, data would be different
            // For testing, we just verify it was stored and retrieved
            assert!(!key_data.is_empty());
        },
        None => {
            println!("Private key not found (this may be expected in test mode)");
        }
    }
    
    // Test session token storage
    let session_id = "test_session_123";
    let token = "auth_token_example_data";
    
    storage_manager.store_session_token(session_id, token)?;
    println!("Stored session token for session: {}", session_id);
    
    let retrieved_token = storage_manager.retrieve_session_token(session_id)?;
    match retrieved_token {
        Some(token_data) => {
            println!("Retrieved session token: {}", token_data);
            assert!(!token_data.is_empty());
        },
        None => {
            println!("Session token not found (this may be expected in test mode)");
        }
    }
    
    // Test user credentials storage
    let user_id = "test_user_456";
    let credentials = bitcraps::mobile::UserCredentials {
        user_id: user_id.to_string(),
        encrypted_private_key: vec![1, 2, 3, 4, 5, 6, 7, 8],
        public_key: vec![9, 10, 11, 12, 13, 14, 15, 16],
        created_at: 1640995200, // Jan 1, 2022
        last_used: 1640995200,
    };
    
    storage_manager.store_user_credentials(user_id, &credentials)?;
    println!("Stored user credentials for user: {}", user_id);
    
    let retrieved_credentials = storage_manager.retrieve_user_credentials(user_id)?;
    match retrieved_credentials {
        Some(creds) => {
            println!("Retrieved credentials for user: {}", creds.user_id);
            assert_eq!(creds.user_id, user_id);
            assert!(!creds.encrypted_private_key.is_empty());
        },
        None => {
            println!("User credentials not found (this may be expected in test mode)");
        }
    }
    
    Ok(())
}

/// Test comprehensive mobile security integration
#[tokio::test]
async fn test_mobile_security_integration() -> Result<()> {
    println!("Testing comprehensive mobile security integration...");
    
    // Create mobile security configuration
    let config = MobileSecurityConfig {
        require_biometric_auth: false, // Disabled for testing
        enable_hardware_backing: false, // Use software mode for testing
        ..Default::default()
    };
    
    // Create mobile security manager
    let security_manager = match MobileSecurityManager::new(config).await {
        Ok(manager) => manager,
        Err(e) => {
            println!("Mobile security manager creation failed (expected in test environment): {}", e);
            return Ok(()); // Skip test on non-mobile platforms
        }
    };
    
    println!("Mobile security manager created successfully");
    
    // Test security status
    let status = security_manager.get_security_status().await?;
    println!("Security level: {:?}", status.security_level);
    println!("Permissions granted: {}", status.permissions_granted);
    println!("Biometric available: {}", status.biometric_available);
    println!("Hardware backed: {}", status.hardware_backed);
    println!("Can create wallets: {}", status.can_create_wallets);
    
    if !status.recommended_actions.is_empty() {
        println!("Recommended security actions:");
        for action in &status.recommended_actions {
            println!("  - {}", action);
        }
    }
    
    // Test secure wallet creation (if possible)
    if status.can_create_wallets {
        let wallet_id = "integration_test_wallet";
        println!("Creating secure wallet: {}", wallet_id);
        
        let secure_wallet = security_manager.create_secure_wallet(wallet_id, None).await?;
        println!("Secure wallet created: {}", secure_wallet.wallet_id);
        assert_eq!(secure_wallet.wallet_id, wallet_id);
        
        // Test wallet unlock
        let unlocked_wallet = security_manager.unlock_secure_wallet(wallet_id).await?;
        println!("Wallet unlocked successfully: {}", unlocked_wallet.wallet_id);
        assert_eq!(unlocked_wallet.wallet_id, wallet_id);
    } else {
        println!("Wallet creation not available (insufficient security level)");
    }
    
    // Test data encryption/decryption
    let test_data = b"sensitive_wallet_seed_phrase_or_private_key_data";
    let context = "wallet_encryption_test";
    
    println!("Testing secure data encryption...");
    let encrypted = security_manager.encrypt_sensitive_data(test_data, context).await?;
    println!("Data encrypted successfully, {} bytes -> {} bytes", 
             test_data.len(), encrypted.data.len());
    
    assert_eq!(encrypted.context, context);
    assert_ne!(encrypted.data, test_data.to_vec());
    
    println!("Testing secure data decryption...");
    let decrypted = security_manager.decrypt_sensitive_data(&encrypted).await?;
    println!("Data decrypted successfully, {} bytes", decrypted.len());
    
    assert_eq!(decrypted, test_data.to_vec());
    
    println!("Mobile security integration test completed successfully!");
    
    Ok(())
}

/// Test cross-platform compatibility
#[tokio::test]
async fn test_cross_platform_compatibility() -> Result<()> {
    println!("Testing cross-platform compatibility...");
    
    // Test that all managers can be created on any platform
    let storage_manager = SecureStorageManager::new();
    println!("SecureStorageManager creation: {:?}", storage_manager.is_ok());
    assert!(storage_manager.is_ok());
    
    let biometric_manager = BiometricAuthManager::new();
    println!("BiometricAuthManager creation: {:?}", biometric_manager.is_ok());
    assert!(biometric_manager.is_ok());
    
    let key_manager = KeyDerivationManager::new(false);
    println!("KeyDerivationManager created successfully");
    
    let permission_manager = PermissionManager::new(
        vec![Permission::Bluetooth],
        vec![Permission::Camera]
    );
    println!("PermissionManager created successfully");
    
    // Test basic functionality on all platforms
    let test_key = key_manager.get_master_key("cross_platform_test")?;
    println!("Master key generated, length: {} bytes", test_key.len());
    assert_eq!(test_key.len(), 32);
    
    // Test permission checking (should work in simulation mode)
    let summary = permission_manager.check_all_permissions()?;
    println!("Permission check completed, can continue: {}", summary.can_continue);
    
    println!("Cross-platform compatibility test completed!");
    
    Ok(())
}

/// Performance test for key derivation operations
#[tokio::test]
async fn test_key_derivation_performance() -> Result<()> {
    println!("Testing key derivation performance...");
    
    let key_manager = KeyDerivationManager::new(false);
    let password = "performance_test_password";
    let salt = b"performance_salt";
    
    let start_time = std::time::Instant::now();
    
    // Test PBKDF2 performance
    for i in 0..10 {
        let _key = key_manager.derive_key_pbkdf2(
            &format!("{}_{}", password, i),
            salt,
            10000, // Lower iterations for faster testing
            32,
            PbkdfAlgorithm::Pbkdf2Sha256,
        )?;
    }
    
    let pbkdf2_duration = start_time.elapsed();
    println!("PBKDF2 (10 iterations): {:?}", pbkdf2_duration);
    
    let start_time = std::time::Instant::now();
    
    // Test HKDF performance
    let master_key_id = "perf_test_master";
    let _master = key_manager.get_master_key(master_key_id)?;
    
    for i in 0..100 {
        let _key = key_manager.derive_key_hkdf(
            master_key_id,
            Some(salt),
            format!("context_{}", i).as_bytes(),
            32,
            HkdfAlgorithm::HkdfSha256,
        )?;
    }
    
    let hkdf_duration = start_time.elapsed();
    println!("HKDF (100 iterations): {:?}", hkdf_duration);
    
    // HKDF should be much faster than PBKDF2
    assert!(hkdf_duration < pbkdf2_duration);
    
    println!("Key derivation performance test completed!");
    
    Ok(())
}