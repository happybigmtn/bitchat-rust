//
//  KeychainBridge.m
//  BitCraps iOS Keychain Bridge
//
//  Implementation of the Objective-C bridge for iOS Keychain Services
//

#import "KeychainBridge.h"
#import <CommonCrypto/CommonCrypto.h>

// Error domain for keychain operations
NSString *const BCKeychainErrorDomain = @"com.bitcraps.keychain.error";

// Error codes
typedef NS_ENUM(NSInteger, BCKeychainErrorCode) {
    BCKeychainErrorCodeUnknown = -1,
    BCKeychainErrorCodeItemNotFound = -2,
    BCKeychainErrorCodeDuplicateItem = -3,
    BCKeychainErrorCodeInvalidParameters = -4,
    BCKeychainErrorCodeAuthenticationFailed = -5,
    BCKeychainErrorCodeSecureEnclaveUnavailable = -6
};

@interface BCKeychainBridge ()
@property (nonatomic, strong) LAContext *authContext;
@property (nonatomic, strong) NSMutableDictionary<NSString *, SecKeyRef> *keyCache;
@end

@implementation BCKeychainBridge

#pragma mark - Singleton

+ (instancetype)sharedInstance {
    static BCKeychainBridge *sharedInstance = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        sharedInstance = [[self alloc] init];
    });
    return sharedInstance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _authContext = [[LAContext alloc] init];
        _keyCache = [NSMutableDictionary dictionary];
    }
    return self;
}

#pragma mark - Biometric Authentication

- (BOOL)isBiometricAvailable {
    NSError *error = nil;
    BOOL canEvaluate = [self.authContext canEvaluatePolicy:LAPolicyDeviceOwnerAuthenticationWithBiometrics 
                                                       error:&error];
    return canEvaluate;
}

- (BCBiometricType)availableBiometricType {
    if (![self isBiometricAvailable]) {
        return BCBiometricTypeNone;
    }
    
    if (@available(iOS 11.0, *)) {
        switch (self.authContext.biometryType) {
            case LABiometryTypeFaceID:
                return BCBiometricTypeFaceID;
            case LABiometryTypeTouchID:
                return BCBiometricTypeTouchID;
            default:
                return BCBiometricTypeNone;
        }
    } else {
        // Pre-iOS 11, only Touch ID was available
        return BCBiometricTypeTouchID;
    }
}

#pragma mark - Key Generation

- (BOOL)generateKey:(NSString *)keyAlias 
    requireBiometric:(BOOL)requireBiometric 
               error:(NSError **)error {
    
    // Check if Secure Enclave is available
    if (![self isSecureEnclaveAvailable]) {
        if (error) {
            *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                         code:BCKeychainErrorCodeSecureEnclaveUnavailable
                                     userInfo:@{NSLocalizedDescriptionKey: @"Secure Enclave is not available"}];
        }
        return NO;
    }
    
    // Create access control
    SecAccessControlRef accessControl = NULL;
    if (requireBiometric) {
        CFErrorRef cfError = NULL;
        accessControl = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAccessControlBiometryCurrentSet | kSecAccessControlPrivateKeyUsage,
            &cfError
        );
        
        if (cfError) {
            if (error) {
                *error = (__bridge NSError *)cfError;
            }
            CFRelease(cfError);
            return NO;
        }
    } else {
        accessControl = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAccessControlPrivateKeyUsage,
            NULL
        );
    }
    
    // Generate key attributes
    NSDictionary *attributes = @{
        (id)kSecAttrKeyType: (id)kSecAttrKeyTypeECSECPrimeRandom,
        (id)kSecAttrKeySizeInBits: @256,
        (id)kSecAttrTokenID: (id)kSecAttrTokenIDSecureEnclave,
        (id)kSecPrivateKeyAttrs: @{
            (id)kSecAttrIsPermanent: @YES,
            (id)kSecAttrApplicationTag: [keyAlias dataUsingEncoding:NSUTF8StringEncoding],
            (id)kSecAttrAccessControl: (__bridge id)accessControl
        }
    };
    
    // Generate key pair
    CFErrorRef cfError = NULL;
    SecKeyRef privateKey = SecKeyCreateRandomKey((__bridge CFDictionaryRef)attributes, &cfError);
    
    if (accessControl) {
        CFRelease(accessControl);
    }
    
    if (cfError) {
        if (error) {
            *error = (__bridge NSError *)cfError;
        }
        CFRelease(cfError);
        return NO;
    }
    
    // Cache the key
    self.keyCache[keyAlias] = privateKey;
    
    return YES;
}

#pragma mark - Data Storage

- (BOOL)storeData:(NSData *)data 
           forKey:(NSString *)key 
    securityLevel:(BCSecurityLevel)securityLevel 
            error:(NSError **)error {
    
    // Prepare the query
    NSMutableDictionary *query = [NSMutableDictionary dictionary];
    query[(id)kSecClass] = (id)kSecClassGenericPassword;
    query[(id)kSecAttrAccount] = key;
    query[(id)kSecValueData] = data;
    query[(id)kSecAttrService] = @"com.bitcraps.keychain";
    
    // Set accessibility based on security level
    switch (securityLevel) {
        case BCSecurityLevelCritical:
            query[(id)kSecAttrAccessible] = (id)kSecAttrAccessibleWhenUnlockedThisDeviceOnly;
            break;
        case BCSecurityLevelHigh:
            query[(id)kSecAttrAccessible] = (id)kSecAttrAccessibleWhenUnlocked;
            break;
        case BCSecurityLevelMedium:
            query[(id)kSecAttrAccessible] = (id)kSecAttrAccessibleAfterFirstUnlock;
            break;
        default:
            query[(id)kSecAttrAccessible] = (id)kSecAttrAccessibleAlways;
            break;
    }
    
    // Delete any existing item
    SecItemDelete((__bridge CFDictionaryRef)query);
    
    // Add the new item
    OSStatus status = SecItemAdd((__bridge CFDictionaryRef)query, NULL);
    
    if (status != errSecSuccess) {
        if (error) {
            *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                         code:status
                                     userInfo:@{NSLocalizedDescriptionKey: [self errorMessageForStatus:status]}];
        }
        return NO;
    }
    
    return YES;
}

- (nullable NSData *)retrieveDataForKey:(NSString *)key 
                                 prompt:(NSString *)prompt 
                                  error:(NSError **)error {
    
    // Prepare the query
    NSMutableDictionary *query = [NSMutableDictionary dictionary];
    query[(id)kSecClass] = (id)kSecClassGenericPassword;
    query[(id)kSecAttrAccount] = key;
    query[(id)kSecAttrService] = @"com.bitcraps.keychain";
    query[(id)kSecReturnData] = @YES;
    query[(id)kSecMatchLimit] = (id)kSecMatchLimitOne;
    
    // Set authentication prompt if provided
    if (prompt) {
        LAContext *context = [[LAContext alloc] init];
        context.localizedReason = prompt;
        query[(id)kSecUseAuthenticationContext] = context;
    }
    
    // Execute query
    CFTypeRef result = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    
    if (status == errSecSuccess) {
        NSData *data = (__bridge_transfer NSData *)result;
        return data;
    } else {
        if (error) {
            *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                         code:status
                                     userInfo:@{NSLocalizedDescriptionKey: [self errorMessageForStatus:status]}];
        }
        return nil;
    }
}

- (BOOL)deleteDataForKey:(NSString *)key error:(NSError **)error {
    // Prepare the query
    NSDictionary *query = @{
        (id)kSecClass: (id)kSecClassGenericPassword,
        (id)kSecAttrAccount: key,
        (id)kSecAttrService: @"com.bitcraps.keychain"
    };
    
    // Delete the item
    OSStatus status = SecItemDelete((__bridge CFDictionaryRef)query);
    
    if (status != errSecSuccess && status != errSecItemNotFound) {
        if (error) {
            *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                         code:status
                                     userInfo:@{NSLocalizedDescriptionKey: [self errorMessageForStatus:status]}];
        }
        return NO;
    }
    
    return YES;
}

#pragma mark - Encryption/Decryption

- (nullable NSData *)encryptData:(NSData *)data 
                     withKeyAlias:(NSString *)keyAlias 
                            error:(NSError **)error {
    
    SecKeyRef publicKey = [self getPublicKeyForAlias:keyAlias error:error];
    if (!publicKey) {
        return nil;
    }
    
    CFErrorRef cfError = NULL;
    NSData *encryptedData = (__bridge_transfer NSData *)SecKeyCreateEncryptedData(
        publicKey,
        kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA256AESGCM,
        (__bridge CFDataRef)data,
        &cfError
    );
    
    if (cfError) {
        if (error) {
            *error = (__bridge NSError *)cfError;
        }
        CFRelease(cfError);
        return nil;
    }
    
    return encryptedData;
}

- (nullable NSData *)decryptData:(NSData *)encryptedData 
                     withKeyAlias:(NSString *)keyAlias 
                           prompt:(NSString *)prompt 
                            error:(NSError **)error {
    
    SecKeyRef privateKey = [self getPrivateKeyForAlias:keyAlias prompt:prompt error:error];
    if (!privateKey) {
        return nil;
    }
    
    CFErrorRef cfError = NULL;
    NSData *decryptedData = (__bridge_transfer NSData *)SecKeyCreateDecryptedData(
        privateKey,
        kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA256AESGCM,
        (__bridge CFDataRef)encryptedData,
        &cfError
    );
    
    if (cfError) {
        if (error) {
            *error = (__bridge NSError *)cfError;
        }
        CFRelease(cfError);
        return nil;
    }
    
    return decryptedData;
}

#pragma mark - Signing/Verification

- (nullable NSData *)signData:(NSData *)data 
                  withKeyAlias:(NSString *)keyAlias 
                        prompt:(NSString *)prompt 
                         error:(NSError **)error {
    
    SecKeyRef privateKey = [self getPrivateKeyForAlias:keyAlias prompt:prompt error:error];
    if (!privateKey) {
        return nil;
    }
    
    CFErrorRef cfError = NULL;
    NSData *signature = (__bridge_transfer NSData *)SecKeyCreateSignature(
        privateKey,
        kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
        (__bridge CFDataRef)data,
        &cfError
    );
    
    if (cfError) {
        if (error) {
            *error = (__bridge NSError *)cfError;
        }
        CFRelease(cfError);
        return nil;
    }
    
    return signature;
}

- (BOOL)verifySignature:(NSData *)signature 
                forData:(NSData *)data 
           withKeyAlias:(NSString *)keyAlias 
                  error:(NSError **)error {
    
    SecKeyRef publicKey = [self getPublicKeyForAlias:keyAlias error:error];
    if (!publicKey) {
        return NO;
    }
    
    CFErrorRef cfError = NULL;
    BOOL isValid = SecKeyVerifySignature(
        publicKey,
        kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
        (__bridge CFDataRef)data,
        (__bridge CFDataRef)signature,
        &cfError
    );
    
    if (cfError) {
        if (error) {
            *error = (__bridge NSError *)cfError;
        }
        CFRelease(cfError);
        return NO;
    }
    
    return isValid;
}

#pragma mark - Secure Enclave

- (BOOL)isSecureEnclaveAvailable {
    return TARGET_OS_SIMULATOR == 0; // Not available in simulator
}

- (void)invalidateAllBiometricKeys {
    // Clear key cache
    [self.keyCache removeAllObjects];
    
    // Query for all keys with biometric protection
    NSDictionary *query = @{
        (id)kSecClass: (id)kSecClassKey,
        (id)kSecAttrKeyClass: (id)kSecAttrKeyClassPrivate,
        (id)kSecAttrTokenID: (id)kSecAttrTokenIDSecureEnclave,
        (id)kSecReturnRef: @YES,
        (id)kSecMatchLimit: (id)kSecMatchLimitAll
    };
    
    CFTypeRef result = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    
    if (status == errSecSuccess && result) {
        NSArray *keys = (__bridge_transfer NSArray *)result;
        for (id keyRef in keys) {
            SecItemDelete((__bridge CFDictionaryRef)@{
                (id)kSecValueRef: keyRef
            });
        }
    }
}

#pragma mark - Helper Methods

- (SecKeyRef)getPrivateKeyForAlias:(NSString *)alias 
                             prompt:(NSString *)prompt 
                              error:(NSError **)error {
    
    // Check cache first
    if (self.keyCache[alias]) {
        return self.keyCache[alias];
    }
    
    // Query for the key
    NSMutableDictionary *query = [NSMutableDictionary dictionary];
    query[(id)kSecClass] = (id)kSecClassKey;
    query[(id)kSecAttrApplicationTag] = [alias dataUsingEncoding:NSUTF8StringEncoding];
    query[(id)kSecAttrKeyClass] = (id)kSecAttrKeyClassPrivate;
    query[(id)kSecReturnRef] = @YES;
    
    if (prompt) {
        LAContext *context = [[LAContext alloc] init];
        context.localizedReason = prompt;
        query[(id)kSecUseAuthenticationContext] = context;
    }
    
    CFTypeRef keyRef = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &keyRef);
    
    if (status == errSecSuccess) {
        SecKeyRef key = (SecKeyRef)keyRef;
        self.keyCache[alias] = key;
        return key;
    } else {
        if (error) {
            *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                         code:status
                                     userInfo:@{NSLocalizedDescriptionKey: [self errorMessageForStatus:status]}];
        }
        return NULL;
    }
}

- (SecKeyRef)getPublicKeyForAlias:(NSString *)alias error:(NSError **)error {
    SecKeyRef privateKey = [self getPrivateKeyForAlias:alias prompt:nil error:error];
    if (!privateKey) {
        return NULL;
    }
    
    SecKeyRef publicKey = SecKeyCopyPublicKey(privateKey);
    if (!publicKey && error) {
        *error = [NSError errorWithDomain:BCKeychainErrorDomain
                                     code:BCKeychainErrorCodeUnknown
                                 userInfo:@{NSLocalizedDescriptionKey: @"Failed to get public key"}];
    }
    
    return publicKey;
}

- (NSString *)errorMessageForStatus:(OSStatus)status {
    switch (status) {
        case errSecItemNotFound:
            return @"Item not found in keychain";
        case errSecDuplicateItem:
            return @"Duplicate item already exists";
        case errSecAuthFailed:
            return @"Authentication failed";
        case errSecInteractionNotAllowed:
            return @"User interaction not allowed";
        default:
            return [NSString stringWithFormat:@"Keychain error: %d", (int)status];
    }
}

@end