/**
 * Android JNI bindings for BitCraps mobile security features
 * 
 * This C library provides the bridge between Rust and Android Java/Kotlin code
 * for secure operations including:
 * - Android Keystore System integration
 * - BiometricPrompt authentication
 * - SharedPreferences encryption/decryption
 * - Permission management
 */

#include <jni.h>
#include <android/log.h>
#include <string.h>
#include <stdlib.h>

#define LOG_TAG "BitCrapsSecurityJNI"
#define LOGD(...) __android_log_print(ANDROID_LOG_DEBUG, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

// Cache for frequently used JNI references
static JavaVM *g_jvm = NULL;
static jobject g_context = NULL;
static jclass g_keystore_class = NULL;
static jclass g_biometric_class = NULL;
static jclass g_preferences_class = NULL;

/**
 * JNI initialization - called when library is loaded
 */
JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *reserved) {
    JNIEnv *env;
    g_jvm = vm;
    
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return JNI_ERR;
    }
    
    // Cache frequently used classes
    jclass local_keystore_class = (*env)->FindClass(env, "com/bitcraps/app/security/KeystoreHelper");
    if (local_keystore_class) {
        g_keystore_class = (*env)->NewGlobalRef(env, local_keystore_class);
        (*env)->DeleteLocalRef(env, local_keystore_class);
    }
    
    jclass local_biometric_class = (*env)->FindClass(env, "com/bitcraps/app/security/BiometricHelper");
    if (local_biometric_class) {
        g_biometric_class = (*env)->NewGlobalRef(env, local_biometric_class);
        (*env)->DeleteLocalRef(env, local_biometric_class);
    }
    
    jclass local_preferences_class = (*env)->FindClass(env, "com/bitcraps/app/security/SecurePreferences");
    if (local_preferences_class) {
        g_preferences_class = (*env)->NewGlobalRef(env, local_preferences_class);
        (*env)->DeleteLocalRef(env, local_preferences_class);
    }
    
    LOGD("BitCraps Security JNI initialized successfully");
    return JNI_VERSION_1_6;
}

/**
 * JNI cleanup - called when library is unloaded
 */
JNIEXPORT void JNICALL JNI_OnUnload(JavaVM *vm, void *reserved) {
    JNIEnv *env;
    
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) == JNI_OK) {
        if (g_keystore_class) {
            (*env)->DeleteGlobalRef(env, g_keystore_class);
            g_keystore_class = NULL;
        }
        if (g_biometric_class) {
            (*env)->DeleteGlobalRef(env, g_biometric_class);
            g_biometric_class = NULL;
        }
        if (g_preferences_class) {
            (*env)->DeleteGlobalRef(env, g_preferences_class);
            g_preferences_class = NULL;
        }
        if (g_context) {
            (*env)->DeleteGlobalRef(env, g_context);
            g_context = NULL;
        }
    }
    
    LOGD("BitCraps Security JNI cleaned up");
}

/**
 * Set Android application context (called from Java)
 */
JNIEXPORT void JNICALL
Java_com_bitcraps_BitCrapsNative_setApplicationContext(JNIEnv *env, jclass clazz, jobject context) {
    if (g_context) {
        (*env)->DeleteGlobalRef(env, g_context);
    }
    g_context = (*env)->NewGlobalRef(env, context);
    LOGD("Application context set");
}

// ============= Android Keystore Functions =============

/**
 * Initialize Android Keystore connection
 */
int android_keystore_init(const char *keystore_alias) {
    if (!g_jvm || !g_context || !g_keystore_class) {
        LOGE("JNI not properly initialized for keystore");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the initialization method
    jmethodID init_method = (*env)->GetStaticMethodID(env, g_keystore_class, 
                                                     "initializeKeystore", 
                                                     "(Landroid/content/Context;Ljava/lang/String;)I");
    if (!init_method) {
        LOGE("Failed to find keystore init method");
        return -3;
    }
    
    // Convert C string to Java string
    jstring j_alias = (*env)->NewStringUTF(env, keystore_alias);
    if (!j_alias) {
        LOGE("Failed to create Java string for keystore alias");
        return -4;
    }
    
    // Call the Java method
    jint result = (*env)->CallStaticIntMethod(env, g_keystore_class, init_method, 
                                             g_context, j_alias);
    
    // Cleanup
    (*env)->DeleteLocalRef(env, j_alias);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -5;
    }
    
    LOGD("Android Keystore initialized with result: %d", result);
    return result;
}

/**
 * Generate or retrieve a key from Android Keystore
 */
int android_keystore_get_key(const char *keystore_alias, const char *key_alias, 
                            unsigned char *key_buffer, size_t buffer_size, size_t *actual_size) {
    if (!g_jvm || !g_context || !g_keystore_class) {
        LOGE("JNI not properly initialized for keystore");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the get key method
    jmethodID get_key_method = (*env)->GetStaticMethodID(env, g_keystore_class, 
                                                        "getOrCreateKey", 
                                                        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)[B");
    if (!get_key_method) {
        LOGE("Failed to find get key method");
        return -3;
    }
    
    // Convert C strings to Java strings
    jstring j_keystore_alias = (*env)->NewStringUTF(env, keystore_alias);
    jstring j_key_alias = (*env)->NewStringUTF(env, key_alias);
    
    if (!j_keystore_alias || !j_key_alias) {
        LOGE("Failed to create Java strings");
        if (j_keystore_alias) (*env)->DeleteLocalRef(env, j_keystore_alias);
        if (j_key_alias) (*env)->DeleteLocalRef(env, j_key_alias);
        return -4;
    }
    
    // Call the Java method
    jbyteArray key_array = (jbyteArray)(*env)->CallStaticObjectMethod(env, g_keystore_class, 
                                                                      get_key_method, 
                                                                      g_context, j_keystore_alias, j_key_alias);
    
    // Cleanup strings
    (*env)->DeleteLocalRef(env, j_keystore_alias);
    (*env)->DeleteLocalRef(env, j_key_alias);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -5;
    }
    
    if (!key_array) {
        LOGE("Failed to get key from keystore");
        return -6;
    }
    
    // Copy key data to buffer
    jsize key_length = (*env)->GetArrayLength(env, key_array);
    if (key_length > buffer_size) {
        LOGE("Key buffer too small: need %d, have %zu", key_length, buffer_size);
        (*env)->DeleteLocalRef(env, key_array);
        return -7;
    }
    
    jbyte *key_data = (*env)->GetByteArrayElements(env, key_array, NULL);
    if (!key_data) {
        LOGE("Failed to get key array elements");
        (*env)->DeleteLocalRef(env, key_array);
        return -8;
    }
    
    memcpy(key_buffer, key_data, key_length);
    *actual_size = key_length;
    
    // Cleanup
    (*env)->ReleaseByteArrayElements(env, key_array, key_data, JNI_ABORT);
    (*env)->DeleteLocalRef(env, key_array);
    
    LOGD("Retrieved key of size %zu from Android Keystore", *actual_size);
    return 0;
}

/**
 * Encrypt data using Android Keystore
 */
int android_keystore_encrypt_store(const char *keystore_alias, const char *key_alias,
                                  const unsigned char *data, size_t data_size,
                                  unsigned char *encrypted_data, size_t encrypted_buffer_size,
                                  size_t *actual_encrypted_size) {
    if (!g_jvm || !g_context || !g_keystore_class) {
        LOGE("JNI not properly initialized for keystore");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the encrypt method
    jmethodID encrypt_method = (*env)->GetStaticMethodID(env, g_keystore_class, 
                                                        "encryptData", 
                                                        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;[B)[B");
    if (!encrypt_method) {
        LOGE("Failed to find encrypt method");
        return -3;
    }
    
    // Convert parameters to Java types
    jstring j_keystore_alias = (*env)->NewStringUTF(env, keystore_alias);
    jstring j_key_alias = (*env)->NewStringUTF(env, key_alias);
    jbyteArray j_data = (*env)->NewByteArray(env, data_size);
    
    if (!j_keystore_alias || !j_key_alias || !j_data) {
        LOGE("Failed to create Java objects for encryption");
        if (j_keystore_alias) (*env)->DeleteLocalRef(env, j_keystore_alias);
        if (j_key_alias) (*env)->DeleteLocalRef(env, j_key_alias);
        if (j_data) (*env)->DeleteLocalRef(env, j_data);
        return -4;
    }
    
    // Copy data to Java byte array
    (*env)->SetByteArrayRegion(env, j_data, 0, data_size, (const jbyte *)data);
    
    // Call the Java encryption method
    jbyteArray encrypted_array = (jbyteArray)(*env)->CallStaticObjectMethod(env, g_keystore_class,
                                                                            encrypt_method,
                                                                            g_context, j_keystore_alias,
                                                                            j_key_alias, j_data);
    
    // Cleanup input parameters
    (*env)->DeleteLocalRef(env, j_keystore_alias);
    (*env)->DeleteLocalRef(env, j_key_alias);
    (*env)->DeleteLocalRef(env, j_data);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -5;
    }
    
    if (!encrypted_array) {
        LOGE("Encryption failed - null result");
        return -6;
    }
    
    // Copy encrypted data to output buffer
    jsize encrypted_length = (*env)->GetArrayLength(env, encrypted_array);
    if (encrypted_length > encrypted_buffer_size) {
        LOGE("Encrypted buffer too small: need %d, have %zu", encrypted_length, encrypted_buffer_size);
        (*env)->DeleteLocalRef(env, encrypted_array);
        return -7;
    }
    
    jbyte *encrypted_data_ptr = (*env)->GetByteArrayElements(env, encrypted_array, NULL);
    if (!encrypted_data_ptr) {
        LOGE("Failed to get encrypted array elements");
        (*env)->DeleteLocalRef(env, encrypted_array);
        return -8;
    }
    
    memcpy(encrypted_data, encrypted_data_ptr, encrypted_length);
    *actual_encrypted_size = encrypted_length;
    
    // Cleanup
    (*env)->ReleaseByteArrayElements(env, encrypted_array, encrypted_data_ptr, JNI_ABORT);
    (*env)->DeleteLocalRef(env, encrypted_array);
    
    LOGD("Encrypted %zu bytes to %zu bytes using Android Keystore", data_size, *actual_encrypted_size);
    return 0;
}

/**
 * Decrypt data using Android Keystore
 */
int android_keystore_decrypt_retrieve(const char *keystore_alias, const char *key_alias,
                                     const unsigned char *encrypted_data, size_t encrypted_size,
                                     unsigned char *decrypted_data, size_t decrypted_buffer_size,
                                     size_t *actual_decrypted_size) {
    if (!g_jvm || !g_context || !g_keystore_class) {
        LOGE("JNI not properly initialized for keystore");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the decrypt method
    jmethodID decrypt_method = (*env)->GetStaticMethodID(env, g_keystore_class, 
                                                        "decryptData", 
                                                        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;[B)[B");
    if (!decrypt_method) {
        LOGE("Failed to find decrypt method");
        return -3;
    }
    
    // Convert parameters to Java types
    jstring j_keystore_alias = (*env)->NewStringUTF(env, keystore_alias);
    jstring j_key_alias = (*env)->NewStringUTF(env, key_alias);
    jbyteArray j_encrypted_data = (*env)->NewByteArray(env, encrypted_size);
    
    if (!j_keystore_alias || !j_key_alias || !j_encrypted_data) {
        LOGE("Failed to create Java objects for decryption");
        if (j_keystore_alias) (*env)->DeleteLocalRef(env, j_keystore_alias);
        if (j_key_alias) (*env)->DeleteLocalRef(env, j_key_alias);
        if (j_encrypted_data) (*env)->DeleteLocalRef(env, j_encrypted_data);
        return -4;
    }
    
    // Copy encrypted data to Java byte array
    (*env)->SetByteArrayRegion(env, j_encrypted_data, 0, encrypted_size, (const jbyte *)encrypted_data);
    
    // Call the Java decryption method
    jbyteArray decrypted_array = (jbyteArray)(*env)->CallStaticObjectMethod(env, g_keystore_class,
                                                                            decrypt_method,
                                                                            g_context, j_keystore_alias,
                                                                            j_key_alias, j_encrypted_data);
    
    // Cleanup input parameters
    (*env)->DeleteLocalRef(env, j_keystore_alias);
    (*env)->DeleteLocalRef(env, j_key_alias);
    (*env)->DeleteLocalRef(env, j_encrypted_data);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -5;
    }
    
    if (!decrypted_array) {
        LOGE("Decryption failed - null result");
        return -6;
    }
    
    // Copy decrypted data to output buffer
    jsize decrypted_length = (*env)->GetArrayLength(env, decrypted_array);
    if (decrypted_length > decrypted_buffer_size) {
        LOGE("Decrypted buffer too small: need %d, have %zu", decrypted_length, decrypted_buffer_size);
        (*env)->DeleteLocalRef(env, decrypted_array);
        return -7;
    }
    
    jbyte *decrypted_data_ptr = (*env)->GetByteArrayElements(env, decrypted_array, NULL);
    if (!decrypted_data_ptr) {
        LOGE("Failed to get decrypted array elements");
        (*env)->DeleteLocalRef(env, decrypted_array);
        return -8;
    }
    
    memcpy(decrypted_data, decrypted_data_ptr, decrypted_length);
    *actual_decrypted_size = decrypted_length;
    
    // Cleanup
    (*env)->ReleaseByteArrayElements(env, decrypted_array, decrypted_data_ptr, JNI_ABORT);
    (*env)->DeleteLocalRef(env, decrypted_array);
    
    LOGD("Decrypted %zu bytes to %zu bytes using Android Keystore", encrypted_size, *actual_decrypted_size);
    return 0;
}

// ============= BiometricPrompt Functions =============

/**
 * Check if biometric authentication is available
 */
int android_biometric_is_available() {
    if (!g_jvm || !g_context || !g_biometric_class) {
        LOGE("JNI not properly initialized for biometric");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the availability check method
    jmethodID check_method = (*env)->GetStaticMethodID(env, g_biometric_class, 
                                                      "isBiometricAvailable", 
                                                      "(Landroid/content/Context;)I");
    if (!check_method) {
        LOGE("Failed to find biometric availability method");
        return -3;
    }
    
    // Call the Java method
    jint result = (*env)->CallStaticIntMethod(env, g_biometric_class, check_method, g_context);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -4;
    }
    
    LOGD("Biometric availability check result: %d", result);
    return result;
}

/**
 * Authenticate using BiometricPrompt
 */
int android_biometric_authenticate(const char *title, const char *subtitle, const char *description,
                                  const char *negative_button, int allow_device_credential,
                                  int require_confirmation, unsigned char *result_buffer,
                                  size_t buffer_size, size_t *actual_size) {
    if (!g_jvm || !g_context || !g_biometric_class) {
        LOGE("JNI not properly initialized for biometric");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Find the authenticate method
    jmethodID auth_method = (*env)->GetStaticMethodID(env, g_biometric_class, 
                                                     "authenticateUser", 
                                                     "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZ)[B");
    if (!auth_method) {
        LOGE("Failed to find biometric authenticate method");
        return -3;
    }
    
    // Convert parameters to Java types
    jstring j_title = (*env)->NewStringUTF(env, title);
    jstring j_subtitle = (*env)->NewStringUTF(env, subtitle);
    jstring j_description = (*env)->NewStringUTF(env, description);
    jstring j_negative = (*env)->NewStringUTF(env, negative_button);
    
    if (!j_title || !j_subtitle || !j_description || !j_negative) {
        LOGE("Failed to create Java strings for biometric authentication");
        if (j_title) (*env)->DeleteLocalRef(env, j_title);
        if (j_subtitle) (*env)->DeleteLocalRef(env, j_subtitle);
        if (j_description) (*env)->DeleteLocalRef(env, j_description);
        if (j_negative) (*env)->DeleteLocalRef(env, j_negative);
        return -4;
    }
    
    // Call the Java authentication method (this will block until user responds)
    jbyteArray result_array = (jbyteArray)(*env)->CallStaticObjectMethod(env, g_biometric_class,
                                                                         auth_method, g_context,
                                                                         j_title, j_subtitle, j_description, j_negative,
                                                                         (jboolean)allow_device_credential,
                                                                         (jboolean)require_confirmation);
    
    // Cleanup strings
    (*env)->DeleteLocalRef(env, j_title);
    (*env)->DeleteLocalRef(env, j_subtitle);
    (*env)->DeleteLocalRef(env, j_description);
    (*env)->DeleteLocalRef(env, j_negative);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -5;
    }
    
    if (!result_array) {
        LOGE("Biometric authentication failed - null result");
        return 1; // Authentication failed
    }
    
    // Copy result data to output buffer
    jsize result_length = (*env)->GetArrayLength(env, result_array);
    if (result_length > buffer_size) {
        LOGE("Result buffer too small: need %d, have %zu", result_length, buffer_size);
        (*env)->DeleteLocalRef(env, result_array);
        return -6;
    }
    
    if (result_length > 0) {
        jbyte *result_data = (*env)->GetByteArrayElements(env, result_array, NULL);
        if (result_data) {
            memcpy(result_buffer, result_data, result_length);
            (*env)->ReleaseByteArrayElements(env, result_array, result_data, JNI_ABORT);
        }
    }
    
    *actual_size = result_length;
    (*env)->DeleteLocalRef(env, result_array);
    
    LOGD("Biometric authentication completed with %zu bytes result", *actual_size);
    return 0; // Success
}

// ============= Permission Functions =============

/**
 * Check permission status
 */
int android_check_permission(const char *permission) {
    if (!g_jvm || !g_context) {
        LOGE("JNI not properly initialized for permission check");
        return -1;
    }
    
    JNIEnv *env;
    if ((*g_jvm)->GetEnv(g_jvm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) {
        LOGE("Failed to get JNI environment");
        return -2;
    }
    
    // Use ContextCompat.checkSelfPermission
    jclass context_compat_class = (*env)->FindClass(env, "androidx/core/content/ContextCompat");
    if (!context_compat_class) {
        LOGE("Failed to find ContextCompat class");
        return -3;
    }
    
    jmethodID check_method = (*env)->GetStaticMethodID(env, context_compat_class, 
                                                      "checkSelfPermission", 
                                                      "(Landroid/content/Context;Ljava/lang/String;)I");
    if (!check_method) {
        LOGE("Failed to find checkSelfPermission method");
        (*env)->DeleteLocalRef(env, context_compat_class);
        return -4;
    }
    
    jstring j_permission = (*env)->NewStringUTF(env, permission);
    if (!j_permission) {
        LOGE("Failed to create Java string for permission");
        (*env)->DeleteLocalRef(env, context_compat_class);
        return -5;
    }
    
    // Call the permission check method
    jint result = (*env)->CallStaticIntMethod(env, context_compat_class, check_method, 
                                             g_context, j_permission);
    
    // Cleanup
    (*env)->DeleteLocalRef(env, j_permission);
    (*env)->DeleteLocalRef(env, context_compat_class);
    
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
        return -6;
    }
    
    // Convert Android permission result to our format
    // PackageManager.PERMISSION_GRANTED = 0, PERMISSION_DENIED = -1
    if (result == 0) {
        return 0; // Granted
    } else {
        return 1; // Denied
    }
}

/**
 * Request single permission
 */
int android_request_permission(const char *permission) {
    // This would typically require an Activity context and callback handling
    // For now, return success as a placeholder
    LOGD("Permission request for: %s (placeholder implementation)", permission);
    return 0; // Granted
}

/**
 * Request multiple permissions
 */
int android_request_permissions(const char **permissions, int count, int *results) {
    // This would typically require an Activity context and callback handling
    // For now, return success for all permissions as placeholders
    LOGD("Batch permission request for %d permissions (placeholder implementation)", count);
    for (int i = 0; i < count; i++) {
        results[i] = 0; // All granted
    }
    return 0;
}

/**
 * Check if should show rationale for permission
 */
int android_should_show_rationale(const char *permission) {
    // This would require Activity context in real implementation
    LOGD("Should show rationale for: %s (placeholder implementation)", permission);
    return 0; // No rationale needed
}