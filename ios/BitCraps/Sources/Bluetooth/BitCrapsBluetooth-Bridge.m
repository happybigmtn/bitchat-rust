//
//  BitCrapsBluetooth-Bridge.m
//  BitCraps
//
//  Objective-C bridge implementation for Rust FFI integration
//  This file provides the C interface implementation that bridges Swift and Rust
//

#import <Foundation/Foundation.h>
#import "BitCrapsBluetooth-Bridge.h"

// MARK: - Private Interface

@interface BitCrapsBluetoothBridgeObjC : NSObject

/// Shared instance for bridging
+ (instancetype)sharedInstance;

/// Initialize the bridge
- (BOOL)initialize;

/// Set event callback
- (BOOL)setEventCallback:(ios_event_callback_t)callback;

/// Set error callback  
- (BOOL)setErrorCallback:(ios_error_callback_t)callback;

/// Handle connection request from Rust
- (BOOL)connectToPeer:(NSString *)peerID;

/// Handle disconnection request from Rust
- (BOOL)disconnectFromPeer:(NSString *)peerID;

/// Handle data send request from Rust
- (BOOL)sendData:(NSData *)data toPeer:(NSString *)peerID;

/// Handle events from Swift layer
- (BOOL)handleSwiftEvent:(NSString *)eventType withData:(NSData *)data fromPeer:(NSString *)peerID;

/// Get current BLE status
- (int)getCurrentStatus;

/// Shutdown the bridge
- (BOOL)shutdown;

@end

// MARK: - Static Variables

static BitCrapsBluetoothBridgeObjC *g_bridgeInstance = nil;
static ios_event_callback_t g_eventCallback = NULL;
static ios_error_callback_t g_errorCallback = NULL;
static BOOL g_isInitialized = NO;

// MARK: - Bridge Implementation

@implementation BitCrapsBluetoothBridgeObjC

+ (instancetype)sharedInstance {
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        g_bridgeInstance = [[self alloc] init];
    });
    return g_bridgeInstance;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        // Set up notification observers for Swift layer communication
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(handleRustConnectPeerRequest:)
                                                     name:@"rustConnectPeerRequest"
                                                   object:nil];
        
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(handleRustSendDataRequest:)
                                                     name:@"rustSendDataRequest"
                                                   object:nil];
        
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(handleRustErrorReceived:)
                                                     name:@"rustErrorReceived"
                                                   object:nil];
        
        NSLog(@"BitCrapsBluetoothBridgeObjC initialized");
    }
    return self;
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    g_eventCallback = NULL;
    g_errorCallback = NULL;
    g_isInitialized = NO;
}

- (BOOL)initialize {
    if (g_isInitialized) {
        return YES; // Already initialized
    }
    
    // Initialize Swift Bluetooth bridge if needed
    // This would typically involve creating/accessing the Swift instance
    
    g_isInitialized = YES;
    NSLog(@"BitCraps Bluetooth bridge initialized");
    return YES;
}

- (BOOL)setEventCallback:(ios_event_callback_t)callback {
    g_eventCallback = callback;
    NSLog(@"Event callback set: %p", callback);
    return YES;
}

- (BOOL)setErrorCallback:(ios_error_callback_t)callback {
    g_errorCallback = callback;
    NSLog(@"Error callback set: %p", callback);
    return YES;
}

- (BOOL)connectToPeer:(NSString *)peerID {
    if (!peerID || peerID.length == 0) {
        NSLog(@"Invalid peer ID for connection");
        return NO;
    }
    
    // Forward to Swift layer via notification
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcConnectPeerRequest"
                                                        object:peerID];
    
    NSLog(@"Connect request forwarded to Swift layer: %@", peerID);
    return YES;
}

- (BOOL)disconnectFromPeer:(NSString *)peerID {
    if (!peerID || peerID.length == 0) {
        NSLog(@"Invalid peer ID for disconnection");
        return NO;
    }
    
    // Forward to Swift layer via notification
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcDisconnectPeerRequest"
                                                        object:peerID];
    
    NSLog(@"Disconnect request forwarded to Swift layer: %@", peerID);
    return YES;
}

- (BOOL)sendData:(NSData *)data toPeer:(NSString *)peerID {
    if (!data || data.length == 0 || !peerID || peerID.length == 0) {
        NSLog(@"Invalid parameters for sendData");
        return NO;
    }
    
    // Create data package for Swift layer
    NSDictionary *dataPackage = @{
        @"peerID": peerID,
        @"data": data
    };
    
    // Forward to Swift layer via notification
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcSendDataRequest"
                                                        object:dataPackage];
    
    NSLog(@"Send data request forwarded to Swift layer: %@ (%lu bytes)", peerID, (unsigned long)data.length);
    return YES;
}

- (BOOL)handleSwiftEvent:(NSString *)eventType withData:(NSData *)data fromPeer:(NSString *)peerID {
    if (!g_eventCallback) {
        NSLog(@"No event callback set, dropping event: %@", eventType);
        return NO;
    }
    
    // Convert to C types for callback
    const char *eventTypeCStr = [eventType UTF8String];
    const char *peerIDCStr = peerID ? [peerID UTF8String] : NULL;
    const void *dataPtr = data ? data.bytes : NULL;
    uint32_t dataLen = data ? (uint32_t)data.length : 0;
    
    // Create combined event data if we have both peer ID and data
    if (peerIDCStr && dataPtr && dataLen > 0) {
        // Combine peer ID and data with null separator
        NSMutableData *combinedData = [NSMutableData data];
        [combinedData appendBytes:peerIDCStr length:strlen(peerIDCStr)];
        [combinedData appendBytes:"\0" length:1]; // Null separator
        [combinedData appendData:data];
        
        g_eventCallback(eventTypeCStr, combinedData.bytes, (uint32_t)combinedData.length);
    } else if (peerIDCStr) {
        g_eventCallback(eventTypeCStr, peerIDCStr, (uint32_t)strlen(peerIDCStr));
    } else {
        g_eventCallback(eventTypeCStr, NULL, 0);
    }
    
    return YES;
}

- (int)getCurrentStatus {
    // Query Swift layer for current status
    // This would typically involve checking the Swift BluetoothBridge state
    
    int status = 0;
    // For now, return a basic status
    // In a real implementation, this would query the actual Swift state
    
    return status;
}

- (BOOL)shutdown {
    if (!g_isInitialized) {
        return YES; // Already shut down
    }
    
    // Notify Swift layer of shutdown
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcShutdownRequest"
                                                        object:nil];
    
    g_eventCallback = NULL;
    g_errorCallback = NULL;
    g_isInitialized = NO;
    
    NSLog(@"BitCraps Bluetooth bridge shut down");
    return YES;
}

// MARK: - Notification Handlers

- (void)handleRustConnectPeerRequest:(NSNotification *)notification {
    NSString *peerID = notification.object;
    if ([peerID isKindOfClass:[NSString class]]) {
        [self connectToPeer:peerID];
    }
}

- (void)handleRustSendDataRequest:(NSNotification *)notification {
    // Handle data send requests from Rust layer
    // The notification object would contain the data send structure
    NSLog(@"Received data send request from Rust layer");
}

- (void)handleRustErrorReceived:(NSNotification *)notification {
    NSString *errorMessage = notification.object;
    if ([errorMessage isKindOfClass:[NSString class]] && g_errorCallback) {
        const char *errorCStr = [errorMessage UTF8String];
        g_errorCallback(errorCStr);
    }
}

@end

// MARK: - C Interface Implementation

int ios_ble_initialize(void) {
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge initialize] ? 1 : 0;
}

int ios_ble_set_event_callback(ios_event_callback_t callback) {
    if (callback == NULL) {
        NSLog(@"Null event callback provided");
        return 0;
    }
    
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge setEventCallback:callback] ? 1 : 0;
}

int ios_ble_set_error_callback(ios_error_callback_t callback) {
    if (callback == NULL) {
        NSLog(@"Null error callback provided");
        return 0;
    }
    
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge setErrorCallback:callback] ? 1 : 0;
}

int ios_ble_start_advertising(void) {
    // Forward to Swift layer
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcStartAdvertisingRequest"
                                                        object:nil];
    return 1;
}

int ios_ble_stop_advertising(void) {
    // Forward to Swift layer
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcStopAdvertisingRequest"
                                                        object:nil];
    return 1;
}

int ios_ble_start_scanning(void) {
    // Forward to Swift layer
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcStartScanningRequest"
                                                        object:nil];
    return 1;
}

int ios_ble_stop_scanning(void) {
    // Forward to Swift layer
    [[NSNotificationCenter defaultCenter] postNotificationName:@"objcStopScanningRequest"
                                                        object:nil];
    return 1;
}

int ios_ble_connect_peer(const char* peer_id) {
    if (peer_id == NULL) {
        NSLog(@"Null peer_id provided to ios_ble_connect_peer");
        return 0;
    }
    
    NSString *peerIDStr = [NSString stringWithUTF8String:peer_id];
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    
    return [bridge connectToPeer:peerIDStr] ? 1 : 0;
}

int ios_ble_disconnect_peer(const char* peer_id) {
    if (peer_id == NULL) {
        NSLog(@"Null peer_id provided to ios_ble_disconnect_peer");
        return 0;
    }
    
    NSString *peerIDStr = [NSString stringWithUTF8String:peer_id];
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    
    return [bridge disconnectFromPeer:peerIDStr] ? 1 : 0;
}

int ios_ble_send_data(const char* peer_id, const uint8_t* data, uint32_t data_len) {
    if (peer_id == NULL || data == NULL || data_len == 0) {
        NSLog(@"Invalid parameters provided to ios_ble_send_data");
        return 0;
    }
    
    NSString *peerIDStr = [NSString stringWithUTF8String:peer_id];
    NSData *dataObj = [NSData dataWithBytes:data length:data_len];
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    
    return [bridge sendData:dataObj toPeer:peerIDStr] ? 1 : 0;
}

int ios_ble_handle_event(const char* event_type, const void* event_data, uint32_t data_len) {
    if (event_type == NULL) {
        NSLog(@"Null event_type provided to ios_ble_handle_event");
        return 0;
    }
    
    NSString *eventTypeStr = [NSString stringWithUTF8String:event_type];
    NSData *eventDataObj = nil;
    NSString *peerIDStr = nil;
    
    if (event_data != NULL && data_len > 0) {
        eventDataObj = [NSData dataWithBytes:event_data length:data_len];
        
        // For some events, the data contains peer ID + null separator + actual data
        if ([eventTypeStr isEqualToString:@"data_received"] || 
            [eventTypeStr isEqualToString:@"peer_connected"] ||
            [eventTypeStr isEqualToString:@"peer_disconnected"]) {
            
            // Try to extract peer ID from the beginning of the data
            const char *dataBytes = (const char *)event_data;
            size_t peerIDLen = strnlen(dataBytes, data_len);
            
            if (peerIDLen < data_len) {
                peerIDStr = [NSString stringWithUTF8String:dataBytes];
                
                // Extract the remaining data after the null separator
                if (peerIDLen + 1 < data_len) {
                    const uint8_t *remainingData = (const uint8_t *)event_data + peerIDLen + 1;
                    uint32_t remainingDataLen = data_len - (uint32_t)peerIDLen - 1;
                    eventDataObj = [NSData dataWithBytes:remainingData length:remainingDataLen];
                }
            }
        }
    }
    
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge handleSwiftEvent:eventTypeStr withData:eventDataObj fromPeer:peerIDStr] ? 1 : 0;
}

int ios_ble_get_status(void) {
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge getCurrentStatus];
}

int ios_ble_shutdown(void) {
    BitCrapsBluetoothBridgeObjC *bridge = [BitCrapsBluetoothBridgeObjC sharedInstance];
    return [bridge shutdown] ? 1 : 0;
}

// MARK: - Memory Management Function Implementations

managed_buffer_t* ios_alloc_buffer(size_t size) {
    if (size == 0) {
        NSLog(@"Cannot allocate zero-sized buffer");
        return NULL;
    }
    
    // Allocate buffer structure
    managed_buffer_t *buffer = malloc(sizeof(managed_buffer_t));
    if (!buffer) {
        NSLog(@"Failed to allocate buffer structure");
        return NULL;
    }
    
    // Allocate data
    buffer->data = malloc(size);
    if (!buffer->data) {
        NSLog(@"Failed to allocate buffer data");
        free(buffer);
        return NULL;
    }
    
    buffer->length = size;
    buffer->capacity = size;
    buffer->owned_by_rust = false; // iOS will manage this buffer
    
    NSLog(@"Allocated buffer: %p (size: %zu)", buffer, size);
    return buffer;
}

void ios_free_buffer(managed_buffer_t* buffer_ptr) {
    if (!buffer_ptr) {
        NSLog(@"Attempt to free null buffer pointer");
        return;
    }
    
    NSLog(@"Freeing buffer: %p (size: %zu, owned_by_rust: %d)", 
          buffer_ptr, buffer_ptr->length, buffer_ptr->owned_by_rust);
    
    if (buffer_ptr->data && !buffer_ptr->owned_by_rust) {
        free(buffer_ptr->data);
    }
    
    free(buffer_ptr);
}

managed_string_t* ios_alloc_string(const char* rust_str) {
    if (!rust_str) {
        NSLog(@"Cannot allocate string from null pointer");
        return NULL;
    }
    
    size_t str_len = strlen(rust_str);
    
    // Allocate string structure
    managed_string_t *managed_str = malloc(sizeof(managed_string_t));
    if (!managed_str) {
        NSLog(@"Failed to allocate string structure");
        return NULL;
    }
    
    // Allocate and copy string
    managed_str->ptr = malloc(str_len + 1);
    if (!managed_str->ptr) {
        NSLog(@"Failed to allocate string data");
        free(managed_str);
        return NULL;
    }
    
    strcpy(managed_str->ptr, rust_str);
    managed_str->length = str_len;
    managed_str->owned_by_rust = false; // iOS will manage this string
    
    NSLog(@"Allocated string: %p (length: %zu)", managed_str, str_len);
    return managed_str;
}

void ios_free_string(managed_string_t* string_ptr) {
    if (!string_ptr) {
        NSLog(@"Attempt to free null string pointer");
        return;
    }
    
    NSLog(@"Freeing string: %p (length: %zu, owned_by_rust: %d)", 
          string_ptr, string_ptr->length, string_ptr->owned_by_rust);
    
    if (string_ptr->ptr && !string_ptr->owned_by_rust) {
        free(string_ptr->ptr);
    }
    
    free(string_ptr);
}

int ios_copy_buffer_data(const managed_buffer_t* buffer_ptr, uint8_t** out_data, size_t* out_length) {
    if (!buffer_ptr || !out_data || !out_length) {
        NSLog(@"Null pointers in ios_copy_buffer_data");
        return 0;
    }
    
    if (!buffer_ptr->data || buffer_ptr->length == 0) {
        NSLog(@"Invalid buffer data");
        return 0;
    }
    
    // Allocate new memory for iOS to own
    uint8_t *copied_data = malloc(buffer_ptr->length);
    if (!copied_data) {
        NSLog(@"Failed to allocate memory for buffer copy");
        return 0;
    }
    
    memcpy(copied_data, buffer_ptr->data, buffer_ptr->length);
    
    *out_data = copied_data;
    *out_length = buffer_ptr->length;
    
    NSLog(@"Copied buffer data: %p -> %p (length: %zu)", buffer_ptr->data, copied_data, buffer_ptr->length);
    return 1;
}

int ios_copy_string_data(const managed_string_t* string_ptr, char** out_c_str) {
    if (!string_ptr || !out_c_str) {
        NSLog(@"Null pointers in ios_copy_string_data");
        return 0;
    }
    
    if (!string_ptr->ptr || string_ptr->length == 0) {
        NSLog(@"Invalid string data");
        return 0;
    }
    
    // Allocate new memory for iOS to own
    char *copied_str = malloc(string_ptr->length + 1);
    if (!copied_str) {
        NSLog(@"Failed to allocate memory for string copy");
        return 0;
    }
    
    strcpy(copied_str, string_ptr->ptr);
    *out_c_str = copied_str;
    
    NSLog(@"Copied string data: %p -> %p (length: %zu)", string_ptr->ptr, copied_str, string_ptr->length);
    return 1;
}

ios_event_data_t* ios_create_event_data(const char* event_type, const char* peer_id, const uint8_t* data_ptr, uint32_t data_len) {
    ios_event_data_t *event_data = malloc(sizeof(ios_event_data_t));
    if (!event_data) {
        NSLog(@"Failed to allocate event data structure");
        return NULL;
    }
    
    event_data->event_type = event_type;
    event_data->peer_id = peer_id;
    event_data->data_ptr = data_ptr;
    event_data->data_len = data_len;
    
    // Get current timestamp
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) == 0) {
        event_data->timestamp = (uint64_t)ts.tv_sec;
    } else {
        event_data->timestamp = 0;
    }
    
    NSLog(@"Created event data: %p (type: %s, timestamp: %llu)", event_data, event_type, event_data->timestamp);
    return event_data;
}

void ios_free_event_data(ios_event_data_t* event_ptr) {
    if (!event_ptr) {
        NSLog(@"Attempt to free null event data pointer");
        return;
    }
    
    NSLog(@"Freeing event data: %p (timestamp: %llu)", event_ptr, event_ptr->timestamp);
    free(event_ptr);
}

int ios_validate_memory(const void* ptr, size_t size) {
    if (!ptr || size == 0) {
        NSLog(@"Memory validation failed: ptr=%p, size=%zu", ptr, size);
        return 0;
    }
    
    // Basic validation - try to read the first byte
    @try {
        volatile uint8_t test_byte = *((const uint8_t*)ptr);
        (void)test_byte; // Suppress unused variable warning
        
        // If size > 1, try to read the last byte
        if (size > 1) {
            volatile uint8_t last_byte = *((const uint8_t*)ptr + size - 1);
            (void)last_byte;
        }
        
        return 1; // Valid
    } @catch (NSException *exception) {
        NSLog(@"Memory validation exception: %@", exception.reason);
        return 0;
    }
}