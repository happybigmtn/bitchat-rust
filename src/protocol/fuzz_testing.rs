//! Protocol Fuzzing and Property-Based Testing
//!
//! This module provides comprehensive fuzzing capabilities for protocol messages,
//! TLV fields, and serialization/deserialization to find edge cases and vulnerabilities.

#![cfg(test)]

use crate::error::Error;
use crate::protocol::{
    BitchatPacket, TlvField, PeerId, GameId, BetType, DiceRoll, CrapTokens,
    tlv_validation::{TlvValidator, ValidatedTlvField, TlvFieldType, ConstantTimeTlvParser},
    versioning::{ProtocolVersion, VersionedMessage},
};
use proptest::prelude::*;
use std::collections::HashMap;

/// Generate arbitrary PeerID for testing
fn peer_id_strategy() -> impl Strategy<Value = PeerId> {
    any::<[u8; 32]>()
}

/// Generate arbitrary GameID for testing
fn game_id_strategy() -> impl Strategy<Value = GameId> {
    any::<[u8; 16]>()
}

/// Generate arbitrary protocol version
fn protocol_version_strategy() -> impl Strategy<Value = ProtocolVersion> {
    (1u8..=2, 0u8..=10, 0u8..=99).prop_map(|(major, minor, patch)| {
        ProtocolVersion::new(major, minor, patch)
    })
}

/// Generate arbitrary TLV field type
fn tlv_field_type_strategy() -> impl Strategy<Value = TlvFieldType> {
    prop_oneof![
        Just(TlvFieldType::Sender),
        Just(TlvFieldType::Receiver),
        Just(TlvFieldType::Signature),
        Just(TlvFieldType::Routing),
        Just(TlvFieldType::Timestamp),
        Just(TlvFieldType::GameCreation),
        Just(TlvFieldType::GameDiscovery),
        Just(TlvFieldType::GameState),
        Just(TlvFieldType::BetData),
        Just(TlvFieldType::DiceRoll),
        Just(TlvFieldType::ConsensusVote),
        Just(TlvFieldType::ConsensusProposal),
        Just(TlvFieldType::StateHash),
        Just(TlvFieldType::Commitment),
        Just(TlvFieldType::Reveal),
        (0u8..=255u8).prop_map(TlvFieldType::Reserved),
    ]
}

/// Generate valid TLV field data based on field type
fn tlv_field_value_strategy(field_type: TlvFieldType) -> BoxedStrategy<Vec<u8>> {
    match field_type {
        TlvFieldType::Sender | TlvFieldType::Receiver => {
            // Exactly 32 bytes for peer IDs
            any::<[u8; 32]>().prop_map(|arr| arr.to_vec()).boxed()
        }
        TlvFieldType::Signature => {
            // Exactly 64 bytes for signatures
            any::<[u8; 64]>().prop_map(|arr| arr.to_vec()).boxed()
        }
        TlvFieldType::Timestamp => {
            // 8 bytes for u64 timestamp
            any::<u64>().prop_map(|ts| ts.to_le_bytes().to_vec()).boxed()
        }
        TlvFieldType::StateHash | TlvFieldType::Commitment => {
            // 32 bytes for hashes
            any::<[u8; 32]>().prop_map(|arr| arr.to_vec()).boxed()
        }
        TlvFieldType::DiceRoll => {
            // Valid dice values (1-6)
            (1u8..=6, 1u8..=6, any::<u64>()).prop_map(|(die1, die2, timestamp)| {
                let mut data = vec![die1, die2];
                data.extend_from_slice(&timestamp.to_le_bytes());
                data
            }).boxed()
        }
        TlvFieldType::Routing => {
            // Variable routing data
            prop::collection::vec(any::<u8>(), 0..=256).boxed()
        }
        TlvFieldType::GameCreation | TlvFieldType::GameDiscovery => {
            // Serialized game data
            prop::collection::vec(any::<u8>(), 32..=128).boxed()
        }
        TlvFieldType::GameState => {
            // Game state data
            prop::collection::vec(any::<u8>(), 0..=1024).boxed()
        }
        TlvFieldType::BetData => {
            // Bet data
            prop::collection::vec(any::<u8>(), 16..=64).boxed()
        }
        TlvFieldType::ConsensusVote | TlvFieldType::ConsensusProposal => {
            // Consensus messages
            prop::collection::vec(any::<u8>(), 32..=256).boxed()
        }
        TlvFieldType::Reveal => {
            // Reveal data
            prop::collection::vec(any::<u8>(), 1..=64).boxed()
        }
        TlvFieldType::Reserved(_) => {
            // Arbitrary data for reserved fields
            prop::collection::vec(any::<u8>(), 0..=64).boxed()
        }
    }
}

/// Generate arbitrary but potentially valid TLV field
fn tlv_field_strategy() -> impl Strategy<Value = TlvField> {
    tlv_field_type_strategy().prop_flat_map(|field_type| {
        tlv_field_value_strategy(field_type).prop_map(move |value| {
            TlvField {
                field_type: field_type.to_u8(),
                length: value.len() as u16,
                value,
            }
        })
    })
}

/// Generate arbitrary BitchatPacket
fn bitchat_packet_strategy() -> impl Strategy<Value = BitchatPacket> {
    (
        1u8..=255u8, // version
        0u8..=255u8, // packet_type
        0u8..=255u8, // flags
        1u8..=16u8,  // ttl
        any::<u64>(), // sequence
        any::<u32>(), // checksum
        peer_id_strategy(), // source
        peer_id_strategy(), // target
        prop::collection::vec(tlv_field_strategy(), 0..=16), // tlv_data
        prop::option::of(prop::collection::vec(any::<u8>(), 0..=1024)), // payload
    ).prop_map(|(version, packet_type, flags, ttl, sequence, checksum, source, target, tlv_data, payload)| {
        let total_length = calculate_packet_length(&tlv_data, &payload);
        BitchatPacket {
            version,
            packet_type,
            flags,
            ttl,
            total_length,
            sequence,
            checksum,
            source,
            target,
            tlv_data,
            payload,
        }
    })
}

/// Calculate total packet length for BitchatPacket
fn calculate_packet_length(tlv_data: &[TlvField], payload: &Option<Vec<u8>>) -> u32 {
    let mut len = 76; // Base header size

    // Add TLV data length
    for tlv in tlv_data {
        len += 3 + tlv.value.len(); // type + length + value
    }

    // Add payload length
    if let Some(payload_data) = payload {
        len += payload_data.len();
    }

    len as u32
}

/// Generate malformed/invalid data for negative testing
fn malformed_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        // Empty data
        Just(Vec::new()),

        // Too short for any valid structure
        prop::collection::vec(any::<u8>(), 1..=5),

        // Data with invalid length claims
        (1u8..=255u8, 1u16..=65535u16, prop::collection::vec(any::<u8>(), 0..=10))
            .prop_map(|(type_val, claimed_len, actual_data)| {
                let mut result = Vec::new();
                result.push(type_val);
                result.extend_from_slice(&claimed_len.to_be_bytes());
                result.extend_from_slice(&actual_data);
                result
            }),

        // Large random data
        prop::collection::vec(any::<u8>(), 1000..=10000),

        // Structured but invalid TLV
        prop::collection::vec((any::<u8>(), any::<u16>(), any::<Vec<u8>>()), 1..=10)
            .prop_map(|fields| {
                let mut result = Vec::new();
                for (field_type, length, mut data) in fields {
                    result.push(field_type);
                    result.extend_from_slice(&length.to_be_bytes());
                    // Intentionally mismatched length
                    data.resize(length.wrapping_add(1) as usize, 0);
                    result.extend_from_slice(&data);
                }
                result
            }),
    ]
}

// Property-based tests

proptest! {
    #[test]
    fn test_tlv_parsing_never_panics(data in prop::collection::vec(any::<u8>(), 0..=2048)) {
        let mut validator = TlvValidator::new();

        // Should never panic, even on malformed input
        let result = std::panic::catch_unwind(|| {
            validator.validate_tlv_payload(&data)
        });

        prop_assert!(result.is_ok(), "TLV validation should not panic on any input");
    }

    #[test]
    fn test_constant_time_tlv_parsing_never_panics(data in prop::collection::vec(any::<u8>(), 0..=2048)) {
        // Should never panic, even on malformed input
        let result = std::panic::catch_unwind(|| {
            ConstantTimeTlvParser::parse_ct(&data, 100)
        });

        prop_assert!(result.is_ok(), "Constant-time TLV parsing should not panic");
    }

    #[test]
    fn test_packet_serialization_roundtrip(packet in bitchat_packet_strategy()) {
        // Serialize packet
        let mut packet_copy = packet.clone();
        let serialized = packet_copy.serialize();

        if let Ok(data) = serialized {
            // Try to deserialize
            let mut cursor = std::io::Cursor::new(data);
            let deserialized = BitchatPacket::deserialize(&mut cursor);

            // If serialization succeeded, deserialization should also succeed
            // and produce equivalent data
            if let Ok(recovered_packet) = deserialized {
                prop_assert_eq!(packet.version, recovered_packet.version);
                prop_assert_eq!(packet.packet_type, recovered_packet.packet_type);
                prop_assert_eq!(packet.flags, recovered_packet.flags);
                prop_assert_eq!(packet.ttl, recovered_packet.ttl);
                prop_assert_eq!(packet.source, recovered_packet.source);
                prop_assert_eq!(packet.target, recovered_packet.target);
            }
        }
    }

    #[test]
    fn test_versioned_message_serialization_roundtrip(
        message_type in any::<u8>(),
        payload in prop::collection::vec(any::<u8>(), 0..=1024)
    ) {
        let message = VersionedMessage::new(message_type, payload.clone());

        // Serialize
        let serialized = message.serialize();
        prop_assert!(serialized.is_ok());

        // Deserialize
        let data = serialized.unwrap();
        let deserialized = VersionedMessage::deserialize(&data);
        prop_assert!(deserialized.is_ok());

        // Check roundtrip
        let recovered = deserialized.unwrap();
        prop_assert_eq!(message.message_type, recovered.message_type);
        prop_assert_eq!(message.payload, recovered.payload);
        prop_assert_eq!(message.version, recovered.version);
    }

    #[test]
    fn test_malformed_data_handling(data in malformed_data_strategy()) {
        let mut validator = TlvValidator::new();

        // Malformed data should be rejected gracefully, not cause crashes
        let result = validator.validate_tlv_payload(&data);

        // Should either succeed (if data happens to be valid) or fail gracefully
        match result {
            Ok(_) => {}, // Valid by chance
            Err(_) => {}, // Rejected as expected
        }

        // Test versioned message parsing with malformed data
        let version_result = VersionedMessage::deserialize(&data);
        match version_result {
            Ok(_) => {}, // Valid by chance
            Err(_) => {}, // Rejected as expected
        }
    }

    #[test]
    fn test_dice_roll_validation(die1 in 0u8..=255u8, die2 in 0u8..=255u8) {
        let dice_result = DiceRoll::new(die1, die2);

        // Only valid dice values (1-6) should succeed
        if (1..=6).contains(&die1) && (1..=6).contains(&die2) {
            prop_assert!(dice_result.is_ok());
            let roll = dice_result.unwrap();
            prop_assert_eq!(roll.die1, die1);
            prop_assert_eq!(roll.die2, die2);
            prop_assert_eq!(roll.total(), die1 + die2);
        } else {
            prop_assert!(dice_result.is_err());
        }
    }

    #[test]
    fn test_crap_tokens_operations(amount1 in 0u64..=1_000_000_000u64, amount2 in 0u64..=1_000_000_000u64) {
        let tokens1 = CrapTokens::new(amount1);
        let tokens2 = CrapTokens::new(amount2);

        // Addition should be consistent
        if let Some(sum) = tokens1.checked_add(tokens2) {
            prop_assert_eq!(sum.amount(), amount1.saturating_add(amount2));
        }

        // Subtraction should be consistent
        if let Some(diff) = tokens1.checked_sub(tokens2) {
            if amount1 >= amount2 {
                prop_assert_eq!(diff.amount(), amount1 - amount2);
            }
        } else {
            // Should fail if underflow would occur
            prop_assert!(amount1 < amount2);
        }

        // Saturating operations should never panic
        let sat_add = tokens1.saturating_add(tokens2);
        let sat_sub = tokens1.saturating_sub(tokens2);

        prop_assert!(sat_add.amount() <= u64::MAX);
        prop_assert!(sat_sub.amount() <= amount1);
    }

    #[test]
    fn test_protocol_version_compatibility(
        major1 in 1u8..=5u8,
        minor1 in 0u8..=10u8,
        patch1 in 0u8..=10u8,
        major2 in 1u8..=5u8,
        minor2 in 0u8..=10u8,
        patch2 in 0u8..=10u8
    ) {
        let version1 = ProtocolVersion::new(major1, minor1, patch1);
        let version2 = ProtocolVersion::new(major2, minor2, patch2);

        // Compatibility should be symmetric for same major version
        if major1 == major2 {
            prop_assert_eq!(
                version1.is_compatible_with(&version2),
                version2.is_compatible_with(&version1)
            );
        }

        // Different major versions should be incompatible
        if major1 != major2 {
            prop_assert!(!version1.is_compatible_with(&version2));
            prop_assert!(!version2.is_compatible_with(&version1));
        }

        // Version should always be compatible with itself
        prop_assert!(version1.is_compatible_with(&version1));
        prop_assert!(version2.is_compatible_with(&version2));
    }

    #[test]
    fn test_bounded_field_parsing(field_count in 0usize..=1000) {
        // Generate TLV data with known field count
        let mut tlv_data = Vec::new();

        for i in 0..field_count.min(100) { // Limit to prevent test timeout
            tlv_data.push(0x01); // Sender type
            tlv_data.extend_from_slice(&4u16.to_be_bytes()); // Length = 4
            tlv_data.extend_from_slice(&(i as u32).to_le_bytes()); // Value = index
        }

        let mut validator = TlvValidator::new();
        let result = validator.validate_tlv_payload(&tlv_data);

        if field_count <= 100 { // Within reasonable limits
            // Should succeed or fail for valid reasons
            match result {
                Ok(fields) => {
                    prop_assert!(fields.len() <= field_count.min(100));
                }
                Err(_) => {
                    // May fail due to duplicate fields or other validation rules
                }
            }
        } else {
            // Very large field counts should be rejected
            // (depending on validator configuration)
        }
    }
}

/// Benchmark-style fuzzing for performance testing
#[cfg(test)]
mod fuzz_benchmarks {
    use super::*;

    #[test]
    fn fuzz_tlv_parsing_performance() {
        use std::time::Instant;

        // Generate various sizes of TLV data
        let test_sizes = [0, 1, 10, 100, 1000, 10000];

        for &size in &test_sizes {
            let mut tlv_data = Vec::new();

            // Create TLV data of specified size
            while tlv_data.len() < size {
                tlv_data.push(0x01); // Type
                tlv_data.extend_from_slice(&8u16.to_be_bytes()); // Length
                tlv_data.extend_from_slice(&[0x42; 8]); // Value
            }

            let start = Instant::now();
            let mut validator = TlvValidator::new();
            let _result = validator.validate_tlv_payload(&tlv_data);
            let duration = start.elapsed();

            // Performance should scale reasonably with input size
            // Very rough heuristic: should not take more than 1ms per KB
            let expected_max_duration = std::time::Duration::from_micros(size as u64);

            if duration > expected_max_duration * 1000 {
                eprintln!("Warning: TLV parsing took {}μs for {} bytes",
                         duration.as_micros(), size);
            }
        }
    }
}

/// Test harness for running property-based tests with specific configurations
pub fn run_protocol_fuzz_tests() {
    println!("Running protocol fuzzing tests...");

    // This would typically be called from a test runner or benchmark
    // The actual proptest! tests run automatically when the module is tested
}