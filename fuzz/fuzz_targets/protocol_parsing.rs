#![no_main]
use libfuzzer_sys::fuzz_target;
use bitchat::protocol::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz message deserialization
    if let Ok(message) = Message::deserialize(data) {
        // If parsing succeeds, re-serialization should work
        let reserialized = message.serialize().unwrap();
        
        // Round-trip should be consistent
        let reparsed = Message::deserialize(&reserialized).unwrap();
        assert_eq!(message.id, reparsed.id);
        assert_eq!(message.message_type, reparsed.message_type);
    }
});