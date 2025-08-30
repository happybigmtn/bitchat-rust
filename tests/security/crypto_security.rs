use bitcraps::crypto::Encryption;

#[tokio::test]
async fn test_key_reuse_vulnerability() {
    let keypair = Encryption::generate_keypair();
    let message1 = b"First message";
    let message2 = b"Second message";

    let encrypted1 = Encryption::encrypt(message1, &keypair.public_key).unwrap();
    let encrypted2 = Encryption::encrypt(message2, &keypair.public_key).unwrap();

    // Ensure different ciphertexts for same key (nonce should differ)
    assert_ne!(encrypted1, encrypted2);

    // Both should decrypt correctly
    let decrypted1 = Encryption::decrypt(&encrypted1, &keypair.private_key).unwrap();
    let decrypted2 = Encryption::decrypt(&encrypted2, &keypair.private_key).unwrap();

    assert_eq!(decrypted1, message1);
    assert_eq!(decrypted2, message2);
}

#[tokio::test]
async fn test_message_tampering_detection() {
    let keypair = Encryption::generate_keypair();
    let original_message = b"Important message";

    let mut encrypted = Encryption::encrypt(original_message, &keypair.public_key).unwrap();

    // Tamper with ciphertext
    if let Some(byte) = encrypted.get_mut(10) {
        *byte = byte.wrapping_add(1);
    }

    // Decryption should fail
    let result = Encryption::decrypt(&encrypted, &keypair.private_key);
    assert!(result.is_err());
}
