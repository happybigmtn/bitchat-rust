//! iOS Keychain Services integration
//!
//! This module provides native integration with iOS Keychain Services for:
//! - Secure credential storage in hardware-backed keychain
//! - TouchID/FaceID biometric authentication
//! - Access control policies and item sharing
//! - Background access and synchronization
//! - Keychain item migration and backup

#![allow(dead_code)]

use crate::error::{Error, Result};
use crate::utils::GrowableBuffer;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::os::raw::c_int;
use std::ptr;

/// iOS Keychain interface (alias for compatibility)
pub type IOSKeychain = IOSKeychainManager;

/// Keychain error types
#[derive(Debug)]
pub enum KeychainError {
    ItemNotFound,
    DuplicateItem,
    AccessDenied,
    InvalidQuery,
    UnexpectedError,
}

/// iOS Keychain Services manager
pub struct IOSKeychainManager {
    service_identifier: String,
    access_group: Option<String>,
    synchronizable: bool,
    buffer: GrowableBuffer,
}

impl IOSKeychainManager {
    /// Create new iOS Keychain manager
    pub fn new(service_identifier: &str) -> Result<Self> {
        Ok(Self {
            service_identifier: service_identifier.to_string(),
            access_group: None,
            synchronizable: false,
            buffer: GrowableBuffer::with_initial_capacity(1024), // Start smaller for keychain data
        })
    }

    /// Create keychain manager with app group sharing
    pub fn new_with_access_group(service_identifier: &str, access_group: &str) -> Result<Self> {
        Ok(Self {
            service_identifier: service_identifier.to_string(),
            access_group: Some(access_group.to_string()),
            synchronizable: false,
            buffer: GrowableBuffer::with_initial_capacity(1024), // Start smaller for keychain data
        })
    }

    /// Enable iCloud Keychain synchronization
    pub fn enable_synchronization(&mut self) {
        self.synchronizable = true;
    }

    /// Store item in keychain with access control
    pub fn store_item(
        &self,
        account: &str,
        data: &[u8],
        access_control: KeychainAccessControl,
    ) -> Result<()> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let account_cstr = CString::new(account)?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            // SAFETY: FFI call to iOS keychain API is safe because:
            // 1. All C strings are properly null-terminated via CString
            // 2. Data pointer and length are valid for the lifetime of the call
            // 3. The iOS API copies the data, not retaining our pointers
            let result = unsafe {
                ios_keychain_store_item(
                    service_cstr.as_ptr(),
                    account_cstr.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                    access_control as c_int,
                    if self.synchronizable { 1 } else { 0 },
                )
            };

            if result != 0 {
                return Err(self.map_keychain_error(result, "store_item"));
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain store for account: {}", account);
        }

        Ok(())
    }

    /// Retrieve item from keychain
    pub fn retrieve_item(&mut self, account: &str) -> Result<Option<Vec<u8>>> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let account_cstr = CString::new(account)?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            // Start with a small buffer, will grow if needed
            let buffer_slice = self.buffer.get_mut(1024)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::OutOfMemory, e))?;
            let mut actual_size: usize = 0;

            // SAFETY: FFI call to iOS keychain API is safe because:
            // 1. All C strings are properly null-terminated
            // 2. Buffer pointer and size are valid for the write operation
            // 3. actual_size is properly initialized and will be updated by the API
            let result = unsafe {
                ios_keychain_retrieve_item(
                    service_cstr.as_ptr(),
                    account_cstr.as_ptr(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                    buffer_slice.as_mut_ptr(),
                    buffer_slice.len(),
                    &mut actual_size,
                )
            };

            match result {
                0 => {
                    // If the data was larger than our initial buffer, try again with correct size
                    if actual_size > buffer_slice.len() {
                        let larger_buffer = self.buffer.get_mut(actual_size)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::OutOfMemory, e))?;
                        // SAFETY: Second retrieval with correct buffer size
                        // Buffer has been resized to accommodate actual_size bytes
                        let result = unsafe {
                            ios_keychain_retrieve_item(
                                service_cstr.as_ptr(),
                                account_cstr.as_ptr(),
                                access_group_cstr
                                    .as_ref()
                                    .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                                larger_buffer.as_mut_ptr(),
                                larger_buffer.len(),
                                &mut actual_size,
                            )
                        };
                        
                        if result == 0 {
                            self.buffer.mark_used(actual_size);
                            Ok(Some(self.buffer.as_slice(actual_size).to_vec()))
                        } else {
                            Err(self.map_keychain_error(result, "retrieve_item"))
                        }
                    } else {
                        self.buffer.mark_used(actual_size);
                        Ok(Some(self.buffer.as_slice(actual_size).to_vec()))
                    }
                }
                -25300 => Ok(None), // errSecItemNotFound
                _ => Err(self.map_keychain_error(result, "retrieve_item")),
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain retrieve for account: {}", account);
            Ok(Some(b"simulated_keychain_data".to_vec()))
        }
    }

    /// Update existing keychain item
    pub fn update_item(
        &self,
        account: &str,
        new_data: &[u8],
        new_access_control: Option<KeychainAccessControl>,
    ) -> Result<()> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let account_cstr = CString::new(account)?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            let result = unsafe {
                ios_keychain_update_item(
                    service_cstr.as_ptr(),
                    account_cstr.as_ptr(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                    new_data.as_ptr(),
                    new_data.len(),
                    new_access_control.map_or(-1, |ac| ac as c_int),
                )
            };

            if result != 0 {
                return Err(self.map_keychain_error(result, "update_item"));
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain update for account: {}", account);
        }

        Ok(())
    }

    /// Delete item from keychain
    pub fn delete_item(&self, account: &str) -> Result<()> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let account_cstr = CString::new(account)?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            let result = unsafe {
                ios_keychain_delete_item(
                    service_cstr.as_ptr(),
                    account_cstr.as_ptr(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                )
            };

            if result != 0 && result != -25300 {
                // Ignore "not found" errors
                return Err(self.map_keychain_error(result, "delete_item"));
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain delete for account: {}", account);
        }

        Ok(())
    }

    /// List all accounts for this service
    pub fn list_accounts(&self) -> Result<Vec<String>> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            // Allocate buffer for account names (C string array)
            let mut accounts_buffer = vec![ptr::null_mut::<c_char>(); 100];
            let mut actual_count: usize = 0;

            let result = unsafe {
                ios_keychain_list_accounts(
                    service_cstr.as_ptr(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                    accounts_buffer.as_mut_ptr(),
                    accounts_buffer.len(),
                    &mut actual_count,
                )
            };

            if result != 0 {
                return Err(self.map_keychain_error(result, "list_accounts"));
            }

            // Convert C strings to Rust strings
            let mut accounts = Vec::new();
            for i in 0..actual_count {
                if !accounts_buffer[i].is_null() {
                    // SAFETY: The pointer is guaranteed to be valid and null-terminated
                    // by the iOS keychain API contract
                    let account_cstr = unsafe { CStr::from_ptr(accounts_buffer[i]) };
                    if let Ok(account_str) = account_cstr.to_str() {
                        accounts.push(account_str.to_string());
                    }

                    // Free the C string
                    // SAFETY: The pointer was allocated by iOS keychain API and must be freed
                    unsafe { ios_keychain_free_string(accounts_buffer[i]) };
                }
            }

            Ok(accounts)
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain list accounts");
            Ok(vec!["sim_account1".to_string(), "sim_account2".to_string()])
        }
    }

    /// Generate cryptographic key in Secure Enclave
    pub fn generate_secure_enclave_key(
        &self,
        key_tag: &str,
        key_type: SecureEnclaveKeyType,
        access_control: KeychainAccessControl,
    ) -> Result<SecureEnclaveKey> {
        let key_tag_cstr = CString::new(key_tag)?;

        #[cfg(target_os = "ios")]
        {
            let mut public_key_buffer = vec![0u8; 256];
            let mut public_key_size: usize = 0;
            let mut key_ref: *mut c_void = ptr::null_mut();

            // SAFETY: FFI call to generate Secure Enclave key is safe because:
            // 1. key_tag is properly null-terminated
            // 2. key_ref pointer will be set by the API to a valid key reference
            let result = unsafe {
                ios_keychain_generate_se_key(
                    key_tag_cstr.as_ptr(),
                    key_type as c_int,
                    access_control as c_int,
                    public_key_buffer.as_mut_ptr(),
                    public_key_buffer.len(),
                    &mut public_key_size,
                    &mut key_ref,
                )
            };

            if result != 0 {
                return Err(self.map_keychain_error(result, "generate_se_key"));
            }

            public_key_buffer.truncate(public_key_size);

            Ok(SecureEnclaveKey {
                key_tag: key_tag.to_string(),
                key_type,
                public_key: public_key_buffer,
                key_ref: key_ref as usize,
                created_at: current_timestamp(),
            })
        }

        #[cfg(not(target_os = "ios"))]
        {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(key_tag.as_bytes());
            let mock_public_key = hasher.finalize().to_vec();

            Ok(SecureEnclaveKey {
                key_tag: key_tag.to_string(),
                key_type,
                public_key: mock_public_key,
                key_ref: 0,
                created_at: current_timestamp(),
            })
        }
    }

    /// Sign data using Secure Enclave key
    pub fn sign_with_secure_enclave(&self, key_tag: &str, data: &[u8]) -> Result<Vec<u8>> {
        let key_tag_cstr = CString::new(key_tag)?;

        #[cfg(target_os = "ios")]
        {
            let mut signature_buffer = vec![0u8; 256];
            let mut signature_size: usize = 0;

            // SAFETY: FFI call to sign with Secure Enclave is safe because:
            // 1. key_tag is properly null-terminated
            // 2. Data pointer and length are valid for the read operation
            // 3. Signature buffer has sufficient capacity (256 bytes)
            let result = unsafe {
                ios_keychain_sign_with_se(
                    key_tag_cstr.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    signature_buffer.as_mut_ptr(),
                    signature_buffer.len(),
                    &mut signature_size,
                )
            };

            if result != 0 {
                return Err(self.map_keychain_error(result, "sign_with_se"));
            }

            signature_buffer.truncate(signature_size);
            Ok(signature_buffer)
        }

        #[cfg(not(target_os = "ios"))]
        {
            // Mock signature
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(key_tag.as_bytes());
            hasher.update(data);
            Ok(hasher.finalize().to_vec())
        }
    }

    /// Delete Secure Enclave key
    pub fn delete_secure_enclave_key(&self, key_tag: &str) -> Result<()> {
        let key_tag_cstr = CString::new(key_tag)?;

        #[cfg(target_os = "ios")]
        {
            // SAFETY: FFI call with properly null-terminated C string
            let result = unsafe { ios_keychain_delete_se_key(key_tag_cstr.as_ptr()) };

            if result != 0 && result != -25300 {
                // Ignore "not found"
                return Err(self.map_keychain_error(result, "delete_se_key"));
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating Secure Enclave key deletion: {}", key_tag);
        }

        Ok(())
    }

    /// Clear all keychain items for this service
    pub fn clear_all_items(&self) -> Result<()> {
        let service_cstr = CString::new(self.service_identifier.clone())?;
        let access_group_cstr = self
            .access_group
            .as_ref()
            .map(|ag| CString::new(ag.as_str()))
            .transpose()?;

        #[cfg(target_os = "ios")]
        {
            let result = unsafe {
                ios_keychain_clear_all_items(
                    service_cstr.as_ptr(),
                    access_group_cstr
                        .as_ref()
                        .map_or(ptr::null(), |cstr| cstr.as_ptr()),
                )
            };

            if result != 0 && result != -25300 {
                // Ignore "not found"
                return Err(self.map_keychain_error(result, "clear_all_items"));
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            log::debug!("Simulating iOS Keychain clear all items");
        }

        Ok(())
    }

    /// Map iOS Keychain error codes to Error enum
    fn map_keychain_error(&self, error_code: c_int, operation: &str) -> Error {
        let description = match error_code {
            -25293 => "errSecAuthFailed: Authentication failed",
            -25300 => "errSecItemNotFound: Item not found",
            -25299 => "errSecDuplicateItem: Item already exists",
            -25308 => "errSecInteractionNotAllowed: User interaction not allowed",
            -25291 => "errSecNotAvailable: No keychain available",
            -25292 => "errSecReadOnly: Keychain is read-only",
            -25240 => "errSecNoSuchKeychain: Keychain does not exist",
            -25244 => "errSecInvalidKeychain: Invalid keychain reference",
            -26275 => "errSecUserCancel: User canceled operation",
            -4 => "errSecUnimplemented: Function not implemented",
            -50 => "errSecParam: Invalid parameters",
            -108 => "errSecAllocate: Failed to allocate memory",
            -25243 => "errSecInvalidItemRef: Invalid item reference",
            -25257 => "errSecDecode: Unable to decode data",
            _ => "Unknown keychain error",
        };

        Error::Platform(format!(
            "iOS Keychain {} failed: {} (code: {})",
            operation, description, error_code
        ))
    }
}

/// Keychain access control policies
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub enum KeychainAccessControl {
    /// No access control
    None = 0,
    /// Biometric authentication required (TouchID/FaceID)
    BiometricAny = 1,
    /// Current biometric set required
    BiometricCurrentSet = 2,
    /// Device passcode required
    DevicePasscode = 3,
    /// Biometric or passcode required
    BiometricOrPasscode = 4,
    /// Application password required
    ApplicationPassword = 5,
}

/// Secure Enclave key types
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub enum SecureEnclaveKeyType {
    /// 256-bit elliptic curve key (secp256r1)
    ECC256 = 0,
    /// 384-bit elliptic curve key (secp384r1)
    ECC384 = 1,
}

/// Secure Enclave key information
#[derive(Debug)]
pub struct SecureEnclaveKey {
    pub key_tag: String,
    pub key_type: SecureEnclaveKeyType,
    pub public_key: Vec<u8>,
    key_ref: usize, // Internal reference
    pub created_at: u64,
}

/// Keychain item metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct KeychainItemInfo {
    pub account: String,
    pub service: String,
    pub access_group: Option<String>,
    pub synchronizable: bool,
    pub created_at: u64,
    pub modified_at: u64,
}

// External C functions for iOS Keychain Services
extern "C" {
    #[cfg(target_os = "ios")]
    fn ios_keychain_store_item(
        service: *const c_char,
        account: *const c_char,
        data: *const u8,
        data_length: usize,
        access_group: *const c_char,
        access_control: c_int,
        synchronizable: c_int,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_retrieve_item(
        service: *const c_char,
        account: *const c_char,
        access_group: *const c_char,
        data_buffer: *mut u8,
        buffer_size: usize,
        actual_size: *mut usize,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_update_item(
        service: *const c_char,
        account: *const c_char,
        access_group: *const c_char,
        new_data: *const u8,
        new_data_length: usize,
        new_access_control: c_int,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_delete_item(
        service: *const c_char,
        account: *const c_char,
        access_group: *const c_char,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_list_accounts(
        service: *const c_char,
        access_group: *const c_char,
        accounts_buffer: *mut *mut c_char,
        buffer_size: usize,
        actual_count: *mut usize,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_generate_se_key(
        key_tag: *const c_char,
        key_type: c_int,
        access_control: c_int,
        public_key_buffer: *mut u8,
        public_key_buffer_size: usize,
        public_key_size: *mut usize,
        key_ref: *mut *mut c_void,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_sign_with_se(
        key_tag: *const c_char,
        data: *const u8,
        data_length: usize,
        signature_buffer: *mut u8,
        signature_buffer_size: usize,
        signature_size: *mut usize,
    ) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_delete_se_key(key_tag: *const c_char) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_clear_all_items(service: *const c_char, access_group: *const c_char) -> c_int;

    #[cfg(target_os = "ios")]
    fn ios_keychain_free_string(string: *mut c_char);
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_manager_creation() {
        let manager = IOSKeychainManager::new("com.bitcraps.test");
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.service_identifier, "com.bitcraps.test");
        assert!(!manager.synchronizable);
    }

    #[test]
    fn test_keychain_operations() {
        let mut manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();

        let account = "test_account";
        let data = b"secret_test_data";

        // Store item
        let result = manager.store_item(account, data, KeychainAccessControl::BiometricAny);
        assert!(result.is_ok());

        // Retrieve item
        let retrieved = manager.retrieve_item(account);
        assert!(retrieved.is_ok());
        assert!(retrieved.unwrap().is_some());

        // Delete item
        let deleted = manager.delete_item(account);
        assert!(deleted.is_ok());
    }

    #[test]
    fn test_secure_enclave_key_generation() {
        let manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();

        let key_tag = "test_se_key";
        let se_key = manager.generate_secure_enclave_key(
            key_tag,
            SecureEnclaveKeyType::ECC256,
            KeychainAccessControl::BiometricAny,
        );

        assert!(se_key.is_ok());

        let key = se_key.unwrap();
        assert_eq!(key.key_tag, key_tag);
        assert_eq!(key.key_type, SecureEnclaveKeyType::ECC256);
        assert!(!key.public_key.is_empty());
    }

    #[test]
    fn test_secure_enclave_signing() {
        let manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();

        let key_tag = "test_signing_key";
        let data_to_sign = b"important_data_to_sign";

        // Generate key first
        let _se_key = manager
            .generate_secure_enclave_key(
                key_tag,
                SecureEnclaveKeyType::ECC256,
                KeychainAccessControl::BiometricAny,
            )
            .unwrap();

        // Sign data
        let signature = manager.sign_with_secure_enclave(key_tag, data_to_sign);
        assert!(signature.is_ok());

        let sig_bytes = signature.unwrap();
        assert!(!sig_bytes.is_empty());

        // Clean up
        let _ = manager.delete_secure_enclave_key(key_tag);
    }

    #[test]
    fn test_access_group_keychain() {
        let manager =
            IOSKeychainManager::new_with_access_group("com.bitcraps.test", "group.bitcraps.shared");

        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(
            manager.access_group,
            Some("group.bitcraps.shared".to_string())
        );
    }

    #[test]
    fn test_synchronizable_keychain() {
        let mut manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();
        assert!(!manager.synchronizable);

        manager.enable_synchronization();
        assert!(manager.synchronizable);
    }

    #[test]
    fn test_list_accounts() {
        let manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();

        let accounts = manager.list_accounts();
        assert!(accounts.is_ok());

        let account_list = accounts.unwrap();
        // In simulation mode, should return mock accounts
        assert!(!account_list.is_empty());
    }

    #[test]
    fn test_clear_all_items() {
        let manager = IOSKeychainManager::new("com.bitcraps.test").unwrap();

        let result = manager.clear_all_items();
        assert!(result.is_ok());
    }
}
