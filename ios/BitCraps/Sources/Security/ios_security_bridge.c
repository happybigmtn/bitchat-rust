/**
 * iOS C bridge for BitCraps mobile security features
 * 
 * This C library provides the bridge between Rust and iOS Objective-C/Swift code
 * for secure operations including:
 * - iOS Keychain Services integration
 * - LocalAuthentication framework (TouchID/FaceID)
 * - Secure Enclave cryptographic operations
 */

#include <Security/Security.h>
#include <LocalAuthentication/LocalAuthentication.h>
#include <CommonCrypto/CommonCrypto.h>
#include <Foundation/Foundation.h>
#include <string.h>
#include <stdlib.h>

// Forward declarations for iOS security bridge functions
extern int ios_keychain_store_item(const char *service, const char *account, 
                                  const unsigned char *data, size_t data_length,
                                  const char *access_group, int access_control, 
                                  int synchronizable);
extern int ios_keychain_retrieve_item(const char *service, const char *account,
                                     const char *access_group, unsigned char *data_buffer,
                                     size_t buffer_size, size_t *actual_size);
extern int ios_biometric_is_available(void);
extern int ios_biometric_authenticate(const char *reason, const char *fallback_title,
                                     unsigned char *result_buffer, size_t buffer_size,
                                     size_t *actual_size);

/**
 * Convert C string to NSString
 */
NSString* cstring_to_nsstring(const char *cstr) {
    if (!cstr) return nil;
    return [NSString stringWithUTF8String:cstr];
}

/**
 * Convert NSData to C buffer
 */
int nsdata_to_buffer(NSData *data, unsigned char *buffer, size_t buffer_size, size_t *actual_size) {
    if (!data || !buffer || !actual_size) {
        return -1;
    }
    
    NSUInteger data_length = [data length];
    if (data_length > buffer_size) {
        return -2; // Buffer too small
    }
    
    [data getBytes:buffer length:data_length];
    *actual_size = data_length;
    return 0;
}

/**
 * Convert access control integer to SecAccessControl
 */
SecAccessControlRef create_access_control(int access_control_flag, CFErrorRef *error) {
    SecAccessControlCreateFlags flags = 0;
    
    switch (access_control_flag) {
        case 1: // BiometricAny
            flags = kSecAccessControlBiometryAny;
            break;
        case 2: // BiometricCurrentSet
            flags = kSecAccessControlBiometryCurrentSet;
            break;
        case 3: // DevicePasscode
            flags = kSecAccessControlDevicePasscode;
            break;
        case 4: // BiometricOrPasscode
            flags = kSecAccessControlBiometryAny | kSecAccessControlOr | kSecAccessControlDevicePasscode;
            break;
        case 5: // ApplicationPassword
            flags = kSecAccessControlApplicationPassword;
            break;
        default:
            return NULL;
    }
    
    return SecAccessControlCreateWithFlags(kCFAllocatorDefault, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, flags, error);
}

// ============= iOS Keychain Functions =============

/**
 * Store item in iOS Keychain
 */
int ios_keychain_store_item(const char *service, const char *account, 
                           const unsigned char *data, size_t data_length,
                           const char *access_group, int access_control, 
                           int synchronizable) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        NSString *accountString = cstring_to_nsstring(account);
        NSData *itemData = [NSData dataWithBytes:data length:data_length];
        
        if (!serviceString || !accountString || !itemData) {
            return -1; // Invalid parameters
        }
        
        // Create keychain query dictionary
        NSMutableDictionary *keychainQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
            (__bridge id)kSecAttrAccount: accountString,
            (__bridge id)kSecValueData: itemData,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                keychainQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Add synchronization setting
        keychainQuery[(__bridge id)kSecAttrSynchronizable] = synchronizable ? @YES : @NO;
        
        // Create access control if needed
        if (access_control > 0) {
            CFErrorRef accessControlError = NULL;
            SecAccessControlRef accessControlRef = create_access_control(access_control, &accessControlError);
            
            if (accessControlRef) {
                keychainQuery[(__bridge id)kSecAttrAccessControl] = (__bridge id)accessControlRef;
                CFRelease(accessControlRef);
            } else {
                if (accessControlError) CFRelease(accessControlError);
                return -2; // Failed to create access control
            }
        }
        
        // Delete existing item first (if any)
        SecItemDelete((__bridge CFDictionaryRef)keychainQuery);
        
        // Add the new item
        OSStatus status = SecItemAdd((__bridge CFDictionaryRef)keychainQuery, NULL);
        
        switch (status) {
            case errSecSuccess:
                return 0; // Success
            case errSecDuplicateItem:
                return -3; // Item already exists
            case errSecUserCancel:
                return -4; // User cancelled
            case errSecAuthFailed:
                return -5; // Authentication failed
            default:
                return -6; // Other error
        }
    }
}

/**
 * Retrieve item from iOS Keychain
 */
int ios_keychain_retrieve_item(const char *service, const char *account,
                              const char *access_group, unsigned char *data_buffer,
                              size_t buffer_size, size_t *actual_size) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        NSString *accountString = cstring_to_nsstring(account);
        
        if (!serviceString || !accountString) {
            return -1; // Invalid parameters
        }
        
        // Create keychain query dictionary
        NSMutableDictionary *keychainQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
            (__bridge id)kSecAttrAccount: accountString,
            (__bridge id)kSecReturnData: @YES,
            (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitOne,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                keychainQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Query keychain
        CFTypeRef result = NULL;
        OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)keychainQuery, &result);
        
        if (status == errSecSuccess && result) {
            NSData *itemData = (__bridge NSData *)result;
            int copy_result = nsdata_to_buffer(itemData, data_buffer, buffer_size, actual_size);
            CFRelease(result);
            
            if (copy_result != 0) {
                return -7; // Buffer too small or copy failed
            }
            
            return 0; // Success
        } else {
            switch (status) {
                case errSecItemNotFound:
                    return -25300; // Item not found (use actual Keychain error code)
                case errSecUserCancel:
                    return -4; // User cancelled
                case errSecAuthFailed:
                    return -5; // Authentication failed
                default:
                    return -6; // Other error
            }
        }
    }
}

/**
 * Update existing keychain item
 */
int ios_keychain_update_item(const char *service, const char *account,
                            const char *access_group, const unsigned char *new_data,
                            size_t new_data_length, int new_access_control) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        NSString *accountString = cstring_to_nsstring(account);
        NSData *newItemData = [NSData dataWithBytes:new_data length:new_data_length];
        
        if (!serviceString || !accountString || !newItemData) {
            return -1; // Invalid parameters
        }
        
        // Create search query
        NSMutableDictionary *searchQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
            (__bridge id)kSecAttrAccount: accountString,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                searchQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Create update attributes
        NSMutableDictionary *updateAttributes = [@{
            (__bridge id)kSecValueData: newItemData,
        } mutableCopy];
        
        // Update access control if specified
        if (new_access_control >= 0) {
            CFErrorRef accessControlError = NULL;
            SecAccessControlRef accessControlRef = create_access_control(new_access_control, &accessControlError);
            
            if (accessControlRef) {
                updateAttributes[(__bridge id)kSecAttrAccessControl] = (__bridge id)accessControlRef;
                CFRelease(accessControlRef);
            } else {
                if (accessControlError) CFRelease(accessControlError);
                return -2; // Failed to create access control
            }
        }
        
        // Update the item
        OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)searchQuery,
                                       (__bridge CFDictionaryRef)updateAttributes);
        
        switch (status) {
            case errSecSuccess:
                return 0; // Success
            case errSecItemNotFound:
                return -25300; // Item not found
            case errSecUserCancel:
                return -4; // User cancelled
            case errSecAuthFailed:
                return -5; // Authentication failed
            default:
                return -6; // Other error
        }
    }
}

/**
 * Delete item from iOS Keychain
 */
int ios_keychain_delete_item(const char *service, const char *account,
                            const char *access_group) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        NSString *accountString = cstring_to_nsstring(account);
        
        if (!serviceString || !accountString) {
            return -1; // Invalid parameters
        }
        
        // Create delete query
        NSMutableDictionary *deleteQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
            (__bridge id)kSecAttrAccount: accountString,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                deleteQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Delete the item
        OSStatus status = SecItemDelete((__bridge CFDictionaryRef)deleteQuery);
        
        switch (status) {
            case errSecSuccess:
                return 0; // Success
            case errSecItemNotFound:
                return -25300; // Item not found (this is ok for delete)
            default:
                return -6; // Other error
        }
    }
}

/**
 * List all accounts for a service
 */
int ios_keychain_list_accounts(const char *service, const char *access_group,
                              char **accounts_buffer, size_t buffer_size,
                              size_t *actual_count) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        
        if (!serviceString) {
            return -1; // Invalid parameters
        }
        
        // Create search query
        NSMutableDictionary *searchQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
            (__bridge id)kSecReturnAttributes: @YES,
            (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitAll,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                searchQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Query keychain
        CFTypeRef result = NULL;
        OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)searchQuery, &result);
        
        if (status == errSecSuccess && result) {
            NSArray *items = (__bridge NSArray *)result;
            NSUInteger count = [items count];
            
            if (count > buffer_size) {
                CFRelease(result);
                return -7; // Buffer too small
            }
            
            // Extract account names
            for (NSUInteger i = 0; i < count; i++) {
                NSDictionary *item = items[i];
                NSString *account = item[(__bridge id)kSecAttrAccount];
                
                if (account) {
                    const char *accountCString = [account UTF8String];
                    if (accountCString) {
                        // Allocate and copy the account string
                        size_t accountLength = strlen(accountCString) + 1;
                        accounts_buffer[i] = malloc(accountLength);
                        if (accounts_buffer[i]) {
                            strcpy(accounts_buffer[i], accountCString);
                        }
                    }
                }
            }
            
            *actual_count = count;
            CFRelease(result);
            return 0; // Success
        } else {
            *actual_count = 0;
            switch (status) {
                case errSecItemNotFound:
                    return 0; // No items found, but that's ok
                default:
                    return -6; // Other error
            }
        }
    }
}

/**
 * Clear all keychain items for a service
 */
int ios_keychain_clear_all_items(const char *service, const char *access_group) {
    @autoreleasepool {
        NSString *serviceString = cstring_to_nsstring(service);
        
        if (!serviceString) {
            return -1; // Invalid parameters
        }
        
        // Create delete query for all items
        NSMutableDictionary *deleteQuery = [@{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: serviceString,
        } mutableCopy];
        
        // Add access group if specified
        if (access_group) {
            NSString *accessGroupString = cstring_to_nsstring(access_group);
            if (accessGroupString) {
                deleteQuery[(__bridge id)kSecAttrAccessGroup] = accessGroupString;
            }
        }
        
        // Delete all items
        OSStatus status = SecItemDelete((__bridge CFDictionaryRef)deleteQuery);
        
        switch (status) {
            case errSecSuccess:
            case errSecItemNotFound: // No items to delete is ok
                return 0; // Success
            default:
                return -6; // Other error
        }
    }
}

/**
 * Free string allocated by list_accounts
 */
void ios_keychain_free_string(char *string) {
    if (string) {
        free(string);
    }
}

// ============= iOS Biometric Authentication Functions =============

/**
 * Check if biometric authentication is available
 */
int ios_biometric_is_available(void) {
    @autoreleasepool {
        LAContext *context = [[LAContext alloc] init];
        NSError *error = nil;
        
        BOOL canEvaluate = [context canEvaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics 
                                                error:&error];
        
        if (canEvaluate) {
            return 0; // Available
        } else {
            switch (error.code) {
                case LAErrorBiometryNotEnrolled:
                    return 1; // Not enrolled
                case LAErrorBiometryNotAvailable:
                    return 2; // Hardware unavailable
                case LAErrorBiometryLockout:
                    return 3; // Security update required
                default:
                    return -1; // Other error
            }
        }
    }
}

/**
 * Get supported biometric types
 */
int ios_biometric_get_types(unsigned char *types_buffer, size_t buffer_size, size_t *actual_size) {
    @autoreleasepool {
        LAContext *context = [[LAContext alloc] init];
        NSError *error = nil;
        
        BOOL canEvaluate = [context canEvaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics 
                                                error:&error];
        
        if (!canEvaluate) {
            *actual_size = 0;
            return -1; // Not available
        }
        
        // Get biometry type
        NSMutableData *typesData = [NSMutableData data];
        
        if (@available(iOS 11.0, *)) {
            LABiometryType biometryType = context.biometryType;
            switch (biometryType) {
                case LABiometryTypeTouchID:
                    [typesData appendBytes:"TouchID" length:7];
                    break;
                case LABiometryTypeFaceID:
                    [typesData appendBytes:"FaceID" length:6];
                    break;
                case LABiometryNone:
                default:
                    [typesData appendBytes:"None" length:4];
                    break;
            }
        } else {
            // Pre-iOS 11, assume TouchID if biometrics are available
            [typesData appendBytes:"TouchID" length:7];
        }
        
        return nsdata_to_buffer(typesData, types_buffer, buffer_size, actual_size);
    }
}

/**
 * Authenticate using biometrics
 */
int ios_biometric_authenticate(const char *reason, const char *fallback_title,
                              unsigned char *result_buffer, size_t buffer_size,
                              size_t *actual_size) {
    @autoreleasepool {
        LAContext *context = [[LAContext alloc] init];
        NSString *reasonString = cstring_to_nsstring(reason);
        
        if (!reasonString) {
            return -1; // Invalid parameters
        }
        
        // Set fallback title if provided
        if (fallback_title) {
            NSString *fallbackString = cstring_to_nsstring(fallback_title);
            if (fallbackString) {
                context.localizedFallbackTitle = fallbackString;
            }
        }
        
        // Create dispatch semaphore for synchronous operation
        dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
        __block int authResult = -1;
        __block NSData *authData = nil;
        
        // Evaluate authentication policy
        [context evaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics
                localizedReason:reasonString
                          reply:^(BOOL success, NSError * _Nullable error) {
            if (success) {
                authResult = 0; // Success
                // Create mock authentication data
                NSString *authInfo = @"biometric_auth_success";
                authData = [authInfo dataUsingEncoding:NSUTF8StringEncoding];
            } else {
                switch (error.code) {
                    case LAErrorUserFallback:
                    case LAErrorUserCancel:
                        authResult = 2; // Cancelled
                        break;
                    case LAErrorAuthenticationFailed:
                        authResult = 1; // Failed
                        break;
                    default:
                        authResult = -2; // Other error
                        break;
                }
            }
            
            dispatch_semaphore_signal(semaphore);
        }];
        
        // Wait for authentication to complete (with timeout)
        dispatch_time_t timeout = dispatch_time(DISPATCH_TIME_NOW, 60 * NSEC_PER_SEC); // 60 seconds
        long waitResult = dispatch_semaphore_wait(semaphore, timeout);
        
        if (waitResult != 0) {
            return -3; // Timeout
        }
        
        // Copy authentication data to result buffer
        if (authResult == 0 && authData) {
            int copy_result = nsdata_to_buffer(authData, result_buffer, buffer_size, actual_size);
            if (copy_result != 0) {
                return -4; // Buffer too small
            }
        } else {
            *actual_size = 0;
        }
        
        return authResult;
    }
}

// ============= iOS Secure Enclave Functions =============

/**
 * Generate key in Secure Enclave
 */
int ios_keychain_generate_se_key(const char *key_tag, int key_type, int access_control,
                                 unsigned char *public_key_buffer, size_t public_key_buffer_size,
                                 size_t *public_key_size, void **key_ref) {
    @autoreleasepool {
        NSString *keyTagString = cstring_to_nsstring(key_tag);
        if (!keyTagString) {
            return -1; // Invalid parameters
        }
        
        NSData *keyTagData = [keyTagString dataUsingEncoding:NSUTF8StringEncoding];
        
        // Create access control
        CFErrorRef accessControlError = NULL;
        SecAccessControlRef accessControlRef = create_access_control(access_control, &accessControlError);
        if (!accessControlRef) {
            if (accessControlError) CFRelease(accessControlError);
            return -2; // Failed to create access control
        }
        
        // Set key generation parameters
        CFMutableDictionaryRef keyGenParams = CFDictionaryCreateMutable(kCFAllocatorDefault, 0,
                                                                        &kCFTypeDictionaryKeyCallBacks,
                                                                        &kCFTypeDictionaryValueCallBacks);
        
        // Set key type (EC for Secure Enclave)
        CFDictionarySetValue(keyGenParams, kSecAttrKeyType, kSecAttrKeyTypeECSECPrimeRandom);
        
        // Set key size based on type
        int keySize = (key_type == 1) ? 384 : 256; // ECC384 or ECC256
        CFNumberRef keySizeNumber = CFNumberCreate(kCFAllocatorDefault, kCFNumberIntType, &keySize);
        CFDictionarySetValue(keyGenParams, kSecAttrKeySizeInBits, keySizeNumber);
        CFRelease(keySizeNumber);
        
        // Set Secure Enclave token
        CFDictionarySetValue(keyGenParams, kSecAttrTokenID, kSecAttrTokenIDSecureEnclave);
        
        // Set key tag for identification
        CFDictionarySetValue(keyGenParams, kSecAttrApplicationTag, (__bridge CFDataRef)keyTagData);
        
        // Set access control
        CFDictionarySetValue(keyGenParams, kSecAttrAccessControl, accessControlRef);
        
        // Generate key pair
        SecKeyRef privateKey = NULL;
        OSStatus status = SecKeyGeneratePair(keyGenParams, NULL, &privateKey);
        
        // Cleanup parameters
        CFRelease(keyGenParams);
        CFRelease(accessControlRef);
        
        if (status != errSecSuccess || !privateKey) {
            return -3; // Key generation failed
        }
        
        // Get public key
        SecKeyRef publicKey = SecKeyCopyPublicKey(privateKey);
        if (!publicKey) {
            CFRelease(privateKey);
            return -4; // Failed to get public key
        }
        
        // Export public key data
        CFErrorRef exportError = NULL;
        CFDataRef publicKeyData = SecKeyCopyExternalRepresentation(publicKey, &exportError);
        
        CFRelease(publicKey);
        
        if (!publicKeyData) {
            CFRelease(privateKey);
            if (exportError) CFRelease(exportError);
            return -5; // Failed to export public key
        }
        
        // Copy public key to buffer
        NSData *pubKeyNSData = (__bridge NSData *)publicKeyData;
        int copy_result = nsdata_to_buffer(pubKeyNSData, public_key_buffer, public_key_buffer_size, public_key_size);
        
        CFRelease(publicKeyData);
        
        if (copy_result != 0) {
            CFRelease(privateKey);
            return -6; // Buffer too small
        }
        
        // Store private key reference
        *key_ref = (void *)privateKey;
        
        return 0; // Success
    }
}

/**
 * Sign data with Secure Enclave key
 */
int ios_keychain_sign_with_se(const char *key_tag, const unsigned char *data, size_t data_length,
                             unsigned char *signature_buffer, size_t signature_buffer_size,
                             size_t *signature_size) {
    @autoreleasepool {
        NSString *keyTagString = cstring_to_nsstring(key_tag);
        if (!keyTagString) {
            return -1; // Invalid parameters
        }
        
        NSData *keyTagData = [keyTagString dataUsingEncoding:NSUTF8StringEncoding];
        NSData *dataToSign = [NSData dataWithBytes:data length:data_length];
        
        // Create query to find the private key
        NSDictionary *keyQuery = @{
            (__bridge id)kSecClass: (__bridge id)kSecClassKey,
            (__bridge id)kSecAttrApplicationTag: keyTagData,
            (__bridge id)kSecAttrKeyType: (__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
            (__bridge id)kSecReturnRef: @YES,
        };
        
        // Find the private key
        CFTypeRef keyRef = NULL;
        OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)keyQuery, &keyRef);
        
        if (status != errSecSuccess || !keyRef) {
            return -2; // Key not found
        }
        
        SecKeyRef privateKey = (SecKeyRef)keyRef;
        
        // Sign the data
        CFErrorRef signError = NULL;
        CFDataRef signature = SecKeyCreateSignature(privateKey, kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
                                                   (__bridge CFDataRef)dataToSign, &signError);
        
        CFRelease(keyRef);
        
        if (!signature) {
            if (signError) CFRelease(signError);
            return -3; // Signing failed
        }
        
        // Copy signature to buffer
        NSData *sigNSData = (__bridge NSData *)signature;
        int copy_result = nsdata_to_buffer(sigNSData, signature_buffer, signature_buffer_size, signature_size);
        
        CFRelease(signature);
        
        if (copy_result != 0) {
            return -4; // Buffer too small
        }
        
        return 0; // Success
    }
}

/**
 * Delete Secure Enclave key
 */
int ios_keychain_delete_se_key(const char *key_tag) {
    @autoreleasepool {
        NSString *keyTagString = cstring_to_nsstring(key_tag);
        if (!keyTagString) {
            return -1; // Invalid parameters
        }
        
        NSData *keyTagData = [keyTagString dataUsingEncoding:NSUTF8StringEncoding];
        
        // Create delete query
        NSDictionary *deleteQuery = @{
            (__bridge id)kSecClass: (__bridge id)kSecClassKey,
            (__bridge id)kSecAttrApplicationTag: keyTagData,
            (__bridge id)kSecAttrKeyType: (__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        };
        
        // Delete the key
        OSStatus status = SecItemDelete((__bridge CFDictionaryRef)deleteQuery);
        
        switch (status) {
            case errSecSuccess:
            case errSecItemNotFound: // Already deleted is ok
                return 0; // Success
            default:
                return -2; // Other error
        }
    }
}

/**
 * Invalidate all biometric-protected keys
 */
int ios_keychain_invalidate_biometric_keys(void) {
    // This would typically involve deleting all keys with biometric access control
    // For now, return success as a placeholder
    return 0; // Success
}