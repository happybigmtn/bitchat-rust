//
//  KeychainBridge.h
//  BitCraps iOS Keychain Bridge
//
//  Objective-C bridge for iOS Keychain Services integration with Rust
//

#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>

NS_ASSUME_NONNULL_BEGIN

/**
 * Security level for keychain items
 */
typedef NS_ENUM(NSInteger, BCSecurityLevel) {
    BCSecurityLevelLow = 0,
    BCSecurityLevelMedium = 1,
    BCSecurityLevelHigh = 2,
    BCSecurityLevelCritical = 3
};

/**
 * Biometric authentication type
 */
typedef NS_ENUM(NSInteger, BCBiometricType) {
    BCBiometricTypeNone = 0,
    BCBiometricTypeTouchID = 1,
    BCBiometricTypeFaceID = 2
};

/**
 * Keychain Bridge for secure storage operations
 */
@interface BCKeychainBridge : NSObject

/**
 * Shared instance for singleton access
 */
+ (instancetype)sharedInstance;

/**
 * Check if biometric authentication is available
 * @return YES if biometric authentication is available
 */
- (BOOL)isBiometricAvailable;

/**
 * Get the type of biometric authentication available
 * @return The biometric type
 */
- (BCBiometricType)availableBiometricType;

/**
 * Generate a new key in the Secure Enclave
 * @param keyAlias The alias for the key
 * @param requireBiometric Whether biometric authentication is required
 * @param error Error pointer
 * @return YES if successful
 */
- (BOOL)generateKey:(NSString *)keyAlias 
    requireBiometric:(BOOL)requireBiometric 
               error:(NSError **)error;

/**
 * Store data in the keychain
 * @param data The data to store
 * @param key The key identifier
 * @param securityLevel The security level
 * @param error Error pointer
 * @return YES if successful
 */
- (BOOL)storeData:(NSData *)data 
           forKey:(NSString *)key 
    securityLevel:(BCSecurityLevel)securityLevel 
            error:(NSError **)error;

/**
 * Retrieve data from the keychain
 * @param key The key identifier
 * @param prompt Biometric prompt message
 * @param error Error pointer
 * @return The stored data, or nil if not found
 */
- (nullable NSData *)retrieveDataForKey:(NSString *)key 
                                 prompt:(NSString *)prompt 
                                  error:(NSError **)error;

/**
 * Delete data from the keychain
 * @param key The key identifier
 * @param error Error pointer
 * @return YES if successful
 */
- (BOOL)deleteDataForKey:(NSString *)key error:(NSError **)error;

/**
 * Encrypt data using Secure Enclave key
 * @param data The data to encrypt
 * @param keyAlias The key alias
 * @param error Error pointer
 * @return The encrypted data
 */
- (nullable NSData *)encryptData:(NSData *)data 
                     withKeyAlias:(NSString *)keyAlias 
                            error:(NSError **)error;

/**
 * Decrypt data using Secure Enclave key
 * @param encryptedData The encrypted data
 * @param keyAlias The key alias
 * @param prompt Biometric prompt message
 * @param error Error pointer
 * @return The decrypted data
 */
- (nullable NSData *)decryptData:(NSData *)encryptedData 
                     withKeyAlias:(NSString *)keyAlias 
                           prompt:(NSString *)prompt 
                            error:(NSError **)error;

/**
 * Sign data using Secure Enclave key
 * @param data The data to sign
 * @param keyAlias The key alias
 * @param prompt Biometric prompt message
 * @param error Error pointer
 * @return The signature
 */
- (nullable NSData *)signData:(NSData *)data 
                  withKeyAlias:(NSString *)keyAlias 
                        prompt:(NSString *)prompt 
                         error:(NSError **)error;

/**
 * Verify a signature
 * @param signature The signature to verify
 * @param data The original data
 * @param keyAlias The key alias
 * @param error Error pointer
 * @return YES if the signature is valid
 */
- (BOOL)verifySignature:(NSData *)signature 
                forData:(NSData *)data 
           withKeyAlias:(NSString *)keyAlias 
                  error:(NSError **)error;

/**
 * Check if Secure Enclave is available
 * @return YES if Secure Enclave is available
 */
- (BOOL)isSecureEnclaveAvailable;

/**
 * Invalidate all biometric keys (e.g., after biometric change)
 */
- (void)invalidateAllBiometricKeys;

@end

NS_ASSUME_NONNULL_END