#![no_main]
use libfuzzer_sys::fuzz_target;
use bitchat::crypto::Encryption;

fuzz_target!(|data: &[u8]| {
    if data.len() >= 64 {
        let (key_data, message) = data.split_at(32);
        
        // Try to use arbitrary data as key
        if let Ok(public_key) = key_data.try_into() {
            // Encryption should not panic with arbitrary keys
            let _ = Encryption::encrypt(message, &public_key);
        }
    }
});