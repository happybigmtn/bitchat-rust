use crate::crypto::encryption::Encryption;

fn main() {
    let keypair = Encryption::generate_keypair();
    println!("Generated keypair - public: {:?}", hex::encode(keypair.public_key));
    // SECURITY: Never log private keys in production
    // println!("Generated keypair - private: {:?}", hex::encode(keypair.private_key));
    
    let message = b"Hello, World!";
    println!("Original message: {:?}", message);
    
    match Encryption::encrypt(message, &keypair.public_key) {
        Ok(encrypted) => {
            println!("Encrypted length: {}", encrypted.len());
            println!("Encrypted (first 64 bytes): {:?}", hex::encode(&encrypted[..std::cmp::min(64, encrypted.len())]));
            
            match Encryption::decrypt(&encrypted, &keypair.private_key) {
                Ok(decrypted) => {
                    println!("Decrypted: {:?}", decrypted);
                    println!("Success: {}", decrypted == message);
                }
                Err(e) => println!("Decryption failed: {}", e),
            }
        }
        Err(e) => println!("Encryption failed: {}", e),
    }
}
