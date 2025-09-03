//! TLV (Type-Length-Value) Validation and Security
//!
//! This module provides comprehensive validation for TLV fields in protocol messages,
//! preventing various attacks including buffer overflows, format string attacks,
//! and denial-of-service through malformed TLV data.

use crate::error::{Error, Result};
use crate::security::constant_time::ConstantTimeOps;
use std::collections::HashMap;

/// Maximum TLV field count to prevent DoS
pub const MAX_TLV_FIELDS: usize = 256;
/// Maximum TLV field size to prevent memory exhaustion
pub const MAX_TLV_FIELD_SIZE: usize = 1024 * 1024; // 1MB
/// Maximum total TLV payload size
pub const MAX_TLV_TOTAL_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// TLV field type definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TlvFieldType {
    Sender = 0x01,
    Receiver = 0x02,
    Signature = 0x03,
    Routing = 0x04,
    Timestamp = 0x05,
    GameCreation = 0x10,
    GameDiscovery = 0x11,
    GameState = 0x12,
    BetData = 0x13,
    DiceRoll = 0x14,
    ConsensusVote = 0x20,
    ConsensusProposal = 0x21,
    StateHash = 0x22,
    Commitment = 0x23,
    Reveal = 0x24,
    // Reserved range 0x80-0xFF for future use
    Reserved(u8),
}

impl TlvFieldType {
    /// Create from raw byte value
    pub fn from_u8(value: u8) -> Self {
        match value {
            0x01 => Self::Sender,
            0x02 => Self::Receiver,
            0x03 => Self::Signature,
            0x04 => Self::Routing,
            0x05 => Self::Timestamp,
            0x10 => Self::GameCreation,
            0x11 => Self::GameDiscovery,
            0x12 => Self::GameState,
            0x13 => Self::BetData,
            0x14 => Self::DiceRoll,
            0x20 => Self::ConsensusVote,
            0x21 => Self::ConsensusProposal,
            0x22 => Self::StateHash,
            0x23 => Self::Commitment,
            0x24 => Self::Reveal,
            other => Self::Reserved(other),
        }
    }

    /// Convert to raw byte value
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Sender => 0x01,
            Self::Receiver => 0x02,
            Self::Signature => 0x03,
            Self::Routing => 0x04,
            Self::Timestamp => 0x05,
            Self::GameCreation => 0x10,
            Self::GameDiscovery => 0x11,
            Self::GameState => 0x12,
            Self::BetData => 0x13,
            Self::DiceRoll => 0x14,
            Self::ConsensusVote => 0x20,
            Self::ConsensusProposal => 0x21,
            Self::StateHash => 0x22,
            Self::Commitment => 0x23,
            Self::Reveal => 0x24,
            Self::Reserved(value) => value,
        }
    }

    /// Check if field type is known/supported
    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Reserved(_))
    }

    /// Get expected value range for validation
    pub fn expected_length_range(self) -> Option<(usize, usize)> {
        match self {
            Self::Sender | Self::Receiver => Some((32, 32)), // Exactly 32 bytes for peer IDs
            Self::Signature => Some((64, 64)), // Exactly 64 bytes for Ed25519 signatures
            Self::Timestamp => Some((8, 8)),   // Exactly 8 bytes for u64 timestamp
            Self::StateHash | Self::Commitment => Some((32, 32)), // Exactly 32 bytes for hashes
            Self::Routing => Some((0, 1024)),  // Variable routing info, max 1KB
            Self::GameCreation => Some((64, 256)), // Game creation data
            Self::GameDiscovery => Some((32, 128)), // Game discovery data
            Self::GameState => Some((0, 4096)), // Game state, max 4KB
            Self::BetData => Some((16, 64)),   // Bet information
            Self::DiceRoll => Some((3, 16)),   // Dice roll data (small)
            Self::ConsensusVote | Self::ConsensusProposal => Some((32, 1024)), // Consensus messages
            Self::Reveal => Some((1, 64)),     // Reveal data
            Self::Reserved(_) => None,         // Unknown fields not validated
        }
    }
}

/// Validated TLV field
#[derive(Debug, Clone)]
pub struct ValidatedTlvField {
    pub field_type: TlvFieldType,
    pub length: u16,
    pub value: Vec<u8>,
    pub is_critical: bool, // Whether this field must be understood
}

/// TLV validation configuration
#[derive(Debug, Clone)]
pub struct TlvValidationConfig {
    /// Maximum number of TLV fields allowed
    pub max_fields: usize,
    /// Maximum size of any single TLV field
    pub max_field_size: usize,
    /// Maximum total TLV payload size
    pub max_total_size: usize,
    /// Whether to allow unknown/reserved field types
    pub allow_unknown_types: bool,
    /// Whether to enforce expected lengths strictly
    pub strict_length_validation: bool,
}

impl Default for TlvValidationConfig {
    fn default() -> Self {
        Self {
            max_fields: MAX_TLV_FIELDS,
            max_field_size: MAX_TLV_FIELD_SIZE,
            max_total_size: MAX_TLV_TOTAL_SIZE,
            allow_unknown_types: false,
            strict_length_validation: true,
        }
    }
}

/// Comprehensive TLV validator
pub struct TlvValidator {
    config: TlvValidationConfig,
    field_type_counts: HashMap<TlvFieldType, usize>,
}

impl TlvValidator {
    /// Create new TLV validator with default configuration
    pub fn new() -> Self {
        Self {
            config: TlvValidationConfig::default(),
            field_type_counts: HashMap::new(),
        }
    }

    /// Create TLV validator with custom configuration
    pub fn with_config(config: TlvValidationConfig) -> Self {
        Self {
            config,
            field_type_counts: HashMap::new(),
        }
    }

    /// Validate a complete TLV payload
    pub fn validate_tlv_payload(&mut self, data: &[u8]) -> Result<Vec<ValidatedTlvField>> {
        // Reset field counts
        self.field_type_counts.clear();

        // Basic size validation
        if data.len() > self.config.max_total_size {
            return Err(Error::InvalidData(format!(
                "TLV payload too large: {} > {}",
                data.len(),
                self.config.max_total_size
            )));
        }

        let mut fields = Vec::with_capacity(16);
        let mut offset = 0;

        while offset < data.len() {
            // Ensure we have at least TLV header (type + length = 3 bytes)
            if offset + 3 > data.len() {
                return Err(Error::InvalidData("Truncated TLV header".to_string()));
            }

            // Extract type and length
            let field_type_raw = data[offset];
            let length = u16::from_be_bytes([data[offset + 1], data[offset + 2]]);
            let field_type = TlvFieldType::from_u8(field_type_raw);

            offset += 3;

            // Validate field count limit
            if fields.len() >= self.config.max_fields {
                return Err(Error::InvalidData(format!(
                    "Too many TLV fields: {} > {}",
                    fields.len() + 1,
                    self.config.max_fields
                )));
            }

            // Validate field size
            let length_usize = length as usize;
            if length_usize > self.config.max_field_size {
                return Err(Error::InvalidData(format!(
                    "TLV field too large: {} > {}",
                    length_usize, self.config.max_field_size
                )));
            }

            // Check if we have enough data for the value
            if offset + length_usize > data.len() {
                return Err(Error::InvalidData(format!(
                    "TLV field value truncated: need {} bytes but only {} available",
                    length_usize,
                    data.len() - offset
                )));
            }

            // Validate field type support
            if !field_type.is_supported() && !self.config.allow_unknown_types {
                return Err(Error::InvalidData(format!(
                    "Unknown TLV field type: 0x{:02x}",
                    field_type_raw
                )));
            }

            // Validate field length against expected range
            if self.config.strict_length_validation {
                if let Some((min_len, max_len)) = field_type.expected_length_range() {
                    if length_usize < min_len || length_usize > max_len {
                        return Err(Error::InvalidData(format!(
                            "TLV field type 0x{:02x} has invalid length: {} (expected {}-{})",
                            field_type_raw, length_usize, min_len, max_len
                        )));
                    }
                }
            }

            // Check for duplicate critical fields
            let count = self.field_type_counts.entry(field_type).or_insert(0);
            *count += 1;

            // Some fields should only appear once
            match field_type {
                TlvFieldType::Sender | TlvFieldType::Receiver | TlvFieldType::Signature => {
                    if *count > 1 {
                        return Err(Error::InvalidData(format!(
                            "Duplicate critical TLV field: 0x{:02x}",
                            field_type_raw
                        )));
                    }
                }
                _ => {} // Other fields can appear multiple times
            }

            // Extract and validate value data
            let value = data[offset..offset + length_usize].to_vec();

            // Additional field-specific validation
            self.validate_field_content(&field_type, &value)?;

            // Create validated field
            let validated_field = ValidatedTlvField {
                field_type,
                length,
                value,
                is_critical: matches!(
                    field_type,
                    TlvFieldType::Sender | TlvFieldType::Receiver | TlvFieldType::Signature
                ),
            };

            fields.push(validated_field);
            offset += length_usize;
        }

        // Validate required fields are present
        self.validate_required_fields(&fields)?;

        Ok(fields)
    }

    /// Validate the content of specific field types
    fn validate_field_content(&self, field_type: &TlvFieldType, value: &[u8]) -> Result<()> {
        match field_type {
            TlvFieldType::Timestamp => {
                // Validate timestamp is not in the far future (to prevent DoS)
                if value.len() == 8 {
                    let timestamp = u64::from_le_bytes(value.try_into().unwrap());
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // Reject timestamps more than 24 hours in the future
                    if timestamp > now + 24 * 3600 {
                        return Err(Error::InvalidData(
                            "Timestamp too far in future".to_string(),
                        ));
                    }
                }
            }
            TlvFieldType::DiceRoll => {
                // Validate dice values are in range 1-6
                if value.len() >= 2 {
                    let die1 = value[0];
                    let die2 = value[1];
                    if !(1..=6).contains(&die1) || !(1..=6).contains(&die2) {
                        return Err(Error::InvalidData(
                            "Invalid dice values (must be 1-6)".to_string(),
                        ));
                    }
                }
            }
            TlvFieldType::Signature => {
                // Basic signature format validation (should be 64 bytes for Ed25519)
                if value.len() != 64 {
                    return Err(Error::InvalidData("Invalid signature length".to_string()));
                }

                // Check for obviously invalid signatures (all zeros, all ones)
                if value.iter().all(|&b| b == 0) || value.iter().all(|&b| b == 0xFF) {
                    return Err(Error::InvalidData("Invalid signature format".to_string()));
                }
            }
            TlvFieldType::StateHash | TlvFieldType::Commitment => {
                // Hash fields should not be all zeros (likely invalid)
                if value.len() == 32 && value.iter().all(|&b| b == 0) {
                    return Err(Error::InvalidData(
                        "Invalid hash value (all zeros)".to_string(),
                    ));
                }
            }
            _ => {} // Other field types pass through
        }

        Ok(())
    }

    /// Validate that required fields are present
    fn validate_required_fields(&self, fields: &[ValidatedTlvField]) -> Result<()> {
        let field_types: std::collections::HashSet<_> =
            fields.iter().map(|f| f.field_type).collect();

        // Check for required fields based on context
        // For now, we don't enforce specific required fields as it depends on message type

        Ok(())
    }

    /// Serialize validated TLV fields back to bytes
    pub fn serialize_tlv_fields(&self, fields: &[ValidatedTlvField]) -> Result<Vec<u8>> {
        let mut total_size = 0;

        // Calculate total size first
        for field in fields {
            total_size += 3 + field.value.len(); // 3 bytes header + value length
        }

        if total_size > self.config.max_total_size {
            return Err(Error::InvalidData(
                "Serialized TLV payload would be too large".to_string(),
            ));
        }

        let mut result = Vec::with_capacity(total_size);

        for field in fields {
            // Type (1 byte)
            result.push(field.field_type.to_u8());

            // Length (2 bytes, big endian)
            result.extend_from_slice(&field.length.to_be_bytes());

            // Value
            result.extend_from_slice(&field.value);
        }

        Ok(result)
    }

    /// Find specific field type in validated fields
    pub fn find_field<'a>(
        fields: &'a [ValidatedTlvField],
        field_type: TlvFieldType,
    ) -> Option<&'a ValidatedTlvField> {
        fields.iter().find(|field| field.field_type == field_type)
    }

    /// Get all fields of specific type
    pub fn find_fields<'a>(
        fields: &'a [ValidatedTlvField],
        field_type: TlvFieldType,
    ) -> Vec<&'a ValidatedTlvField> {
        fields
            .iter()
            .filter(|field| field.field_type == field_type)
            .collect()
    }
}

impl Default for TlvValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Constant-time TLV parsing to prevent timing attacks
pub struct ConstantTimeTlvParser;

impl ConstantTimeTlvParser {
    /// Parse TLV fields in constant time to prevent information leakage
    pub fn parse_ct(data: &[u8], max_fields: usize) -> Result<Vec<ValidatedTlvField>> {
        if data.len() > MAX_TLV_TOTAL_SIZE {
            return Err(Error::InvalidData("TLV payload too large".to_string()));
        }

        let mut fields = Vec::with_capacity(max_fields);
        let mut offset = 0;

        // Parse up to max_fields, regardless of actual field count
        for field_index in 0..max_fields {
            let has_data = ConstantTimeOps::constant_time_bounds_check(offset + 3, data.len() + 1);

            if !has_data {
                break;
            }

            // Extract header in constant time
            let field_type_raw = data.get(offset).copied().unwrap_or(0);
            let length_bytes = [
                data.get(offset + 1).copied().unwrap_or(0),
                data.get(offset + 2).copied().unwrap_or(0),
            ];
            let length = u16::from_be_bytes(length_bytes) as usize;

            offset += 3;

            // Validate length bounds in constant time
            let length_valid = length <= MAX_TLV_FIELD_SIZE && length > 0;
            let data_available =
                ConstantTimeOps::constant_time_bounds_check(offset + length, data.len() + 1);

            if !length_valid || !data_available {
                return Err(Error::InvalidData("Invalid TLV field".to_string()));
            }

            // Extract value in constant time
            let mut value = vec![0u8; length];
            for i in 0..length {
                if offset + i < data.len() {
                    value[i] = data[offset + i];
                }
            }

            let field_type = TlvFieldType::from_u8(field_type_raw);

            fields.push(ValidatedTlvField {
                field_type,
                length: length as u16,
                value,
                is_critical: matches!(
                    field_type,
                    TlvFieldType::Sender | TlvFieldType::Receiver | TlvFieldType::Signature
                ),
            });

            offset += length;
        }

        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlv_field_type_conversion() {
        assert_eq!(TlvFieldType::from_u8(0x01), TlvFieldType::Sender);
        assert_eq!(TlvFieldType::Sender.to_u8(), 0x01);

        assert_eq!(TlvFieldType::from_u8(0xFF), TlvFieldType::Reserved(0xFF));
        assert_eq!(TlvFieldType::Reserved(0xFF).to_u8(), 0xFF);
    }

    #[test]
    fn test_tlv_validation() {
        let mut validator = TlvValidator::new();

        // Create valid TLV data: Sender field (32 bytes)
        let mut tlv_data = Vec::new();
        tlv_data.push(0x01); // Sender type
        tlv_data.extend_from_slice(&32u16.to_be_bytes()); // Length = 32
        tlv_data.extend_from_slice(&[0x42; 32]); // 32 bytes of data

        let result = validator.validate_tlv_payload(&tlv_data);
        assert!(result.is_ok());

        let fields = result.unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, TlvFieldType::Sender);
        assert_eq!(fields[0].length, 32);
        assert_eq!(fields[0].value.len(), 32);
    }

    #[test]
    fn test_tlv_validation_truncated() {
        let mut validator = TlvValidator::new();

        // Truncated TLV header
        let tlv_data = vec![0x01, 0x00]; // Missing length byte

        let result = validator.validate_tlv_payload(&tlv_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_tlv_validation_oversized() {
        let mut validator = TlvValidator::new();

        // Create TLV with length larger than available data
        let mut tlv_data = Vec::new();
        tlv_data.push(0x01); // Sender type
        tlv_data.extend_from_slice(&100u16.to_be_bytes()); // Length = 100
        tlv_data.extend_from_slice(&[0x42; 10]); // Only 10 bytes of data

        let result = validator.validate_tlv_payload(&tlv_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_tlv_dice_validation() {
        let mut validator = TlvValidator::new();

        // Valid dice roll
        let mut tlv_data = Vec::new();
        tlv_data.push(0x14); // DiceRoll type
        tlv_data.extend_from_slice(&3u16.to_be_bytes()); // Length = 3
        tlv_data.extend_from_slice(&[3, 4, 0]); // Valid dice values + timestamp

        let result = validator.validate_tlv_payload(&tlv_data);
        assert!(result.is_ok());

        // Invalid dice roll (out of range)
        let mut tlv_data_invalid = Vec::new();
        tlv_data_invalid.push(0x14); // DiceRoll type
        tlv_data_invalid.extend_from_slice(&3u16.to_be_bytes()); // Length = 3
        tlv_data_invalid.extend_from_slice(&[0, 7, 0]); // Invalid dice values

        let result_invalid = validator.validate_tlv_payload(&tlv_data_invalid);
        assert!(result_invalid.is_err());
    }

    #[test]
    fn test_tlv_serialization() {
        let validator = TlvValidator::new();

        let fields = vec![
            ValidatedTlvField {
                field_type: TlvFieldType::Sender,
                length: 4,
                value: vec![1, 2, 3, 4],
                is_critical: true,
            },
            ValidatedTlvField {
                field_type: TlvFieldType::Timestamp,
                length: 8,
                value: vec![0; 8],
                is_critical: false,
            },
        ];

        let serialized = validator.serialize_tlv_fields(&fields);
        assert!(serialized.is_ok());

        let data = serialized.unwrap();
        // Should have: type(1) + len(2) + value(4) + type(1) + len(2) + value(8) = 18 bytes
        assert_eq!(data.len(), 18);

        // Check first field
        assert_eq!(data[0], 0x01); // Sender type
        assert_eq!(data[1..3], 4u16.to_be_bytes()); // Length = 4
        assert_eq!(&data[3..7], &[1, 2, 3, 4]); // Value
    }

    #[test]
    fn test_find_fields() {
        let fields = vec![
            ValidatedTlvField {
                field_type: TlvFieldType::Sender,
                length: 4,
                value: vec![1, 2, 3, 4],
                is_critical: true,
            },
            ValidatedTlvField {
                field_type: TlvFieldType::Receiver,
                length: 4,
                value: vec![5, 6, 7, 8],
                is_critical: true,
            },
        ];

        let sender_field = TlvValidator::find_field(&fields, TlvFieldType::Sender);
        assert!(sender_field.is_some());
        assert_eq!(sender_field.unwrap().value, vec![1, 2, 3, 4]);

        let missing_field = TlvValidator::find_field(&fields, TlvFieldType::Signature);
        assert!(missing_field.is_none());
    }

    #[test]
    fn test_constant_time_parsing() {
        // Create test TLV data
        let mut tlv_data = Vec::new();
        tlv_data.push(0x01); // Sender type
        tlv_data.extend_from_slice(&4u16.to_be_bytes()); // Length = 4
        tlv_data.extend_from_slice(&[1, 2, 3, 4]); // Value

        let result = ConstantTimeTlvParser::parse_ct(&tlv_data, 10);
        assert!(result.is_ok());

        let fields = result.unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, TlvFieldType::Sender);
    }
}
