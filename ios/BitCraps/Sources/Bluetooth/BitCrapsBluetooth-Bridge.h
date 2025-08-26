//
//  BitCrapsBluetooth-Bridge.h
//  BitCraps
//
//  Objective-C bridge header for Rust FFI integration
//  This file provides the C interface declarations that match the Rust FFI exports
//

#ifndef BitCrapsBluetooth_Bridge_h
#define BitCrapsBluetooth_Bridge_h

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// MARK: - Function Pointer Types

/// Event callback function type
typedef void (*ios_event_callback_t)(const char* event_type, const void* event_data, uint32_t data_len);

/// Error callback function type
typedef void (*ios_error_callback_t)(const char* error_message);

// MARK: - Core BLE Management Functions

/// Initialize the iOS BLE manager
/// @return 1 on success, 0 on failure
int ios_ble_initialize(void);

/// Set event callback for iOS BLE events
/// @param callback The callback function to handle events
/// @return 1 on success, 0 on failure
int ios_ble_set_event_callback(ios_event_callback_t callback);

/// Set error callback for iOS BLE errors
/// @param callback The callback function to handle errors
/// @return 1 on success, 0 on failure
int ios_ble_set_error_callback(ios_error_callback_t callback);

/// Start BLE advertising
/// @return 1 on success, 0 on failure
int ios_ble_start_advertising(void);

/// Stop BLE advertising
/// @return 1 on success, 0 on failure
int ios_ble_stop_advertising(void);

/// Start BLE scanning
/// @return 1 on success, 0 on failure
int ios_ble_start_scanning(void);

/// Stop BLE scanning
/// @return 1 on success, 0 on failure
int ios_ble_stop_scanning(void);

// MARK: - Peer Connection Functions

/// Connect to a specific peer
/// @param peer_id Null-terminated string identifying the peer
/// @return 1 on success, 0 on failure
int ios_ble_connect_peer(const char* peer_id);

/// Disconnect from a specific peer
/// @param peer_id Null-terminated string identifying the peer
/// @return 1 on success, 0 on failure
int ios_ble_disconnect_peer(const char* peer_id);

/// Send data to a specific peer
/// @param peer_id Null-terminated string identifying the peer
/// @param data Pointer to the data to send
/// @param data_len Length of the data in bytes
/// @return 1 on success, 0 on failure
int ios_ble_send_data(const char* peer_id, const uint8_t* data, uint32_t data_len);

// MARK: - Event Handling Functions

/// Handle events from iOS (called by Swift/Objective-C)
/// @param event_type Null-terminated string identifying the event type
/// @param event_data Pointer to event-specific data
/// @param data_len Length of the event data in bytes
/// @return 1 on success, 0 on failure
int ios_ble_handle_event(const char* event_type, const void* event_data, uint32_t data_len);

/// Get the current status of the iOS BLE manager
/// @return Bitfield representing current status, -1 on error
/// Bit 0: advertising active
/// Bit 1: scanning active
/// Bit 2: has connections
int ios_ble_get_status(void);

/// Cleanup and shutdown the iOS BLE manager
/// @return 1 on success, 0 on failure
int ios_ble_shutdown(void);

// MARK: - Memory Management Functions

/// Managed buffer structure for data transfer
typedef struct {
    uint8_t* data;
    size_t length;
    size_t capacity;
    bool owned_by_rust;
} managed_buffer_t;

/// Managed string structure for string transfer
typedef struct {
    char* ptr;
    size_t length;
    bool owned_by_rust;
} managed_string_t;

/// iOS event data structure
typedef struct {
    const char* event_type;
    const char* peer_id;
    const uint8_t* data_ptr;
    uint32_t data_len;
    uint64_t timestamp;
} ios_event_data_t;

/// Allocate a buffer on the Rust side for iOS to use
/// @param size Size of the buffer to allocate
/// @return Pointer to managed buffer, NULL on failure
managed_buffer_t* ios_alloc_buffer(size_t size);

/// Free a buffer allocated by Rust
/// @param buffer_ptr Pointer to the managed buffer
void ios_free_buffer(managed_buffer_t* buffer_ptr);

/// Allocate a string on the Rust side for iOS to use
/// @param rust_str Null-terminated C string to copy
/// @return Pointer to managed string, NULL on failure
managed_string_t* ios_alloc_string(const char* rust_str);

/// Free a string allocated by Rust
/// @param string_ptr Pointer to the managed string
void ios_free_string(managed_string_t* string_ptr);

/// Copy data from a managed buffer to a new iOS-managed buffer
/// @param buffer_ptr Pointer to the source managed buffer
/// @param out_data Pointer to receive the new data pointer
/// @param out_length Pointer to receive the data length
/// @return 1 on success, 0 on failure
int ios_copy_buffer_data(const managed_buffer_t* buffer_ptr, uint8_t** out_data, size_t* out_length);

/// Copy string data from a managed string to a new iOS-managed C string
/// @param string_ptr Pointer to the source managed string
/// @param out_c_str Pointer to receive the new C string pointer
/// @return 1 on success, 0 on failure
int ios_copy_string_data(const managed_string_t* string_ptr, char** out_c_str);

/// Create iOS event data structure (to be freed by ios_free_event_data)
/// @param event_type Null-terminated event type string
/// @param peer_id Null-terminated peer ID string
/// @param data_ptr Pointer to event data
/// @param data_len Length of event data
/// @return Pointer to event data structure, NULL on failure
ios_event_data_t* ios_create_event_data(const char* event_type, const char* peer_id, const uint8_t* data_ptr, uint32_t data_len);

/// Free iOS event data structure
/// @param event_ptr Pointer to the event data structure
void ios_free_event_data(ios_event_data_t* event_ptr);

/// Validate a memory pointer and size (for debugging)
/// @param ptr Pointer to validate
/// @param size Size of the memory region
/// @return 1 if valid, 0 if invalid
int ios_validate_memory(const void* ptr, size_t size);

// MARK: - Utility Macros

/// Check if a pointer is valid for the given size
#define IOS_VALIDATE_PTR(ptr, size) (ios_validate_memory((ptr), (size)) == 1)

/// Safe string creation macro
#define IOS_SAFE_STRING(str) ((str) != NULL ? (str) : "")

/// Status bit definitions for ios_ble_get_status()
#define IOS_BLE_STATUS_ADVERTISING  (1 << 0)
#define IOS_BLE_STATUS_SCANNING     (1 << 1)
#define IOS_BLE_STATUS_CONNECTED    (1 << 2)

#ifdef __cplusplus
}
#endif

#endif /* BitCrapsBluetooth_Bridge_h */