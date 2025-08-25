package com.bitcraps.android.keystore;

/**
 * JNI interface for Android Keystore operations
 * 
 * This class provides native methods that bridge to the Rust implementation
 * for secure key storage and cryptographic operations.
 */
public class KeystoreJNI {
    static {
        System.loadLibrary("bitcraps_jni");
    }
    
    // Native handle to the Rust keystore instance
    private long nativeHandle;
    
    /**
     * Initialize the keystore JNI bridge
     * @return Native handle to the keystore instance
     */
    public native long initKeystore();
    
    /**
     * Generate a new key in the Android Keystore
     * @param handle Native handle
     * @param alias Key alias
     * @param requireAuth Whether biometric authentication is required
     * @return true if successful
     */
    public native boolean generateKey(long handle, String alias, boolean requireAuth);
    
    /**
     * Encrypt data using a key from the Android Keystore
     * @param handle Native handle
     * @param alias Key alias
     * @param data Data to encrypt
     * @return Encrypted data
     */
    public native byte[] encrypt(long handle, String alias, byte[] data);
    
    /**
     * Decrypt data using a key from the Android Keystore
     * @param handle Native handle
     * @param alias Key alias
     * @param encryptedData Data to decrypt
     * @return Decrypted data
     */
    public native byte[] decrypt(long handle, String alias, byte[] encryptedData);
    
    /**
     * Sign data using a key from the Android Keystore
     * @param handle Native handle
     * @param alias Key alias
     * @param data Data to sign
     * @return Signature
     */
    public native byte[] sign(long handle, String alias, byte[] data);
    
    /**
     * Check if hardware-backed keystore is available
     * @return true if hardware backing is available
     */
    public native boolean isHardwareBackedAvailable();
    
    /**
     * Clean up native resources
     * @param handle Native handle
     */
    public native void cleanup(long handle);
    
    /**
     * Constructor initializes the native handle
     */
    public KeystoreJNI() {
        this.nativeHandle = initKeystore();
    }
    
    /**
     * Get the native handle
     * @return Native handle
     */
    public long getNativeHandle() {
        return nativeHandle;
    }
    
    /**
     * Cleanup on finalization
     */
    @Override
    protected void finalize() throws Throwable {
        if (nativeHandle != 0) {
            cleanup(nativeHandle);
            nativeHandle = 0;
        }
        super.finalize();
    }
}