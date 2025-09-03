//! Adaptive compression for network messages

use crate::error::Result;
use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Compression algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    /// No compression
    None,
    /// LZ4 - Fast compression, good for real-time
    Lz4,
    /// Zlib - Better compression ratio, moderate speed
    Zlib,
    /// Zstd - Best compression ratio (future addition)
    #[allow(dead_code)]
    Zstd,
}

/// Payload type detection for optimal compression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadType {
    /// Binary data (already compressed or random)
    BinaryData,
    /// JSON or text data
    JsonText,
    /// Repeated structures (game states, etc.)
    RepeatedStructure,
    /// Small message (don't compress)
    SmallMessage,
}

/// Compressed payload with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedPayload {
    /// Compression algorithm used
    pub algorithm: CompressionAlgorithm,
    /// Original size before compression
    pub original_size: usize,
    /// Compressed data
    pub data: Vec<u8>,
}

/// Compression statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub total_compressed: u64,
    pub total_decompressed: u64,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub lz4_count: u64,
    pub zlib_count: u64,
    pub none_count: u64,
    pub average_ratio: f64,
}

/// Payload analyzer for compression selection
pub struct PayloadAnalyzer;

impl PayloadAnalyzer {
    /// Analyze payload to determine optimal compression
    pub fn analyze(data: &[u8]) -> PayloadType {
        // Small messages don't benefit from compression
        if data.len() < 100 {
            return PayloadType::SmallMessage;
        }

        // Calculate entropy to detect data type
        let entropy = Self::calculate_entropy(data);

        // High entropy suggests already compressed or random data
        if entropy > 7.5 {
            return PayloadType::BinaryData;
        }

        // Check for text patterns (ASCII printable characters)
        let text_chars = data.iter().filter(|&&b| (32..=126).contains(&b)).count();
        let text_ratio = text_chars as f64 / data.len() as f64;

        if text_ratio > 0.8 {
            return PayloadType::JsonText;
        }

        // Check for repeated patterns
        if Self::has_repeated_patterns(data) {
            return PayloadType::RepeatedStructure;
        }

        PayloadType::BinaryData
    }

    /// Calculate Shannon entropy
    fn calculate_entropy(data: &[u8]) -> f64 {
        let mut freq = [0u64; 256];

        for &byte in data {
            freq[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &freq {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// Check for repeated patterns
    fn has_repeated_patterns(data: &[u8]) -> bool {
        if data.len() < 32 {
            return false;
        }

        // Simple check: look for repeated 4-byte sequences
        let mut pattern_count = 0;
        let chunk_size = 4;
        let sample_size = data.len().min(256);

        for i in 0..sample_size.saturating_sub(chunk_size * 2) {
            let pattern = &data[i..i + chunk_size];

            for j in (i + chunk_size)..sample_size.saturating_sub(chunk_size) {
                if &data[j..j + chunk_size] == pattern {
                    pattern_count += 1;
                    if pattern_count > 10 {
                        return true;
                    }
                }
            }
        }

        false
    }
}

/// Adaptive compression system
pub struct AdaptiveCompression {
    stats: CompressionStats,
}

impl Default for AdaptiveCompression {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveCompression {
    /// Create new compression system
    pub fn new() -> Self {
        Self {
            stats: CompressionStats::default(),
        }
    }

    /// Compress data with adaptive algorithm selection
    pub fn compress_adaptive(&mut self, data: &[u8]) -> Result<CompressedPayload> {
        let analysis = PayloadAnalyzer::analyze(data);

        let algorithm = match analysis {
            PayloadType::SmallMessage => CompressionAlgorithm::None,
            PayloadType::BinaryData => CompressionAlgorithm::None,
            PayloadType::JsonText => CompressionAlgorithm::Zlib,
            PayloadType::RepeatedStructure => CompressionAlgorithm::Lz4,
        };

        self.compress_with_algorithm(data, algorithm)
    }

    /// Compress with specific algorithm
    pub fn compress_with_algorithm(
        &mut self,
        data: &[u8],
        algorithm: CompressionAlgorithm,
    ) -> Result<CompressedPayload> {
        let original_size = data.len();

        let compressed_data = match algorithm {
            CompressionAlgorithm::None => {
                self.stats.none_count += 1;
                data.to_vec()
            }
            CompressionAlgorithm::Lz4 => {
                self.stats.lz4_count += 1;
                compress_prepend_size(data)
            }
            CompressionAlgorithm::Zlib => {
                self.stats.zlib_count += 1;
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder
                    .write_all(data)
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                encoder
                    .finish()
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?
            }
            CompressionAlgorithm::Zstd => {
                // Use zstd with default level
                let mut encoder = zstd::stream::Encoder::new(Vec::new(), 0)
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                encoder
                    .write_all(data)
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                encoder
                    .finish()
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?
            }
        };

        // Update statistics
        self.stats.total_compressed += 1;
        self.stats.bytes_before += original_size as u64;
        self.stats.bytes_after += compressed_data.len() as u64;

        if self.stats.bytes_before > 0 {
            self.stats.average_ratio =
                self.stats.bytes_after as f64 / self.stats.bytes_before as f64;
        }

        Ok(CompressedPayload {
            algorithm,
            original_size,
            data: compressed_data,
        })
    }

    /// Decompress payload
    pub fn decompress(&mut self, payload: &CompressedPayload) -> Result<Vec<u8>> {
        let decompressed = match payload.algorithm {
            CompressionAlgorithm::None => payload.data.clone(),
            CompressionAlgorithm::Lz4 => decompress_size_prepended(&payload.data)
                .map_err(|e| crate::error::Error::Serialization(e.to_string()))?,
            CompressionAlgorithm::Zlib => {
                let mut decoder = ZlibDecoder::new(Vec::new());
                decoder
                    .write_all(&payload.data)
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                decoder
                    .finish()
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?
            }
            CompressionAlgorithm::Zstd => {
                let mut decoder = zstd::stream::Decoder::new(&payload.data[..])
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                let mut out = Vec::new();
                std::io::copy(&mut decoder, &mut out)
                    .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;
                out
            }
        };

        // Verify size
        if decompressed.len() != payload.original_size {
            return Err(crate::error::Error::Protocol(format!(
                "Decompression size mismatch: expected {}, got {}",
                payload.original_size,
                decompressed.len()
            )));
        }

        self.stats.total_decompressed += 1;

        Ok(decompressed)
    }

    /// Get compression ratio for algorithm
    pub fn test_compression_ratio(&self, data: &[u8], algorithm: CompressionAlgorithm) -> f64 {
        let compressed = match algorithm {
            CompressionAlgorithm::None => return 1.0,
            CompressionAlgorithm::Lz4 => compress_prepend_size(data),
            CompressionAlgorithm::Zlib => {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                if encoder.write_all(data).is_err() {
                    return 1.0;
                }
                match encoder.finish() {
                    Ok(data) => data,
                    Err(_) => return 1.0,
                }
            }
            CompressionAlgorithm::Zstd => {
                match zstd::stream::encode_all(&data[..], 0) {
                    Ok(c) => c,
                    Err(_) => return 1.0,
                }
            }
        };

        compressed.len() as f64 / data.len() as f64
    }

    /// Choose best algorithm for data
    pub fn choose_best_algorithm(&self, data: &[u8]) -> CompressionAlgorithm {
        if data.len() < 100 {
            return CompressionAlgorithm::None;
        }

        let lz4_ratio = self.test_compression_ratio(data, CompressionAlgorithm::Lz4);
        let zlib_ratio = self.test_compression_ratio(data, CompressionAlgorithm::Zlib);

        // Choose algorithm with best ratio, but prefer LZ4 for speed if ratios are close
        if lz4_ratio < 0.9 && (lz4_ratio - zlib_ratio).abs() < 0.1 {
            CompressionAlgorithm::Lz4
        } else if zlib_ratio < lz4_ratio && zlib_ratio < 0.9 {
            CompressionAlgorithm::Zlib
        } else if lz4_ratio < 0.9 {
            CompressionAlgorithm::Lz4
        } else {
            CompressionAlgorithm::None
        }
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStats {
        self.stats.clone()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = CompressionStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_analysis() {
        // Small message
        let small = b"hello";
        assert_eq!(PayloadAnalyzer::analyze(small), PayloadType::SmallMessage);

        // Text data
        let text = b"This is a longer text message that should be detected as text content for compression";
        let text_extended = text.repeat(3);
        assert_eq!(
            PayloadAnalyzer::analyze(&text_extended),
            PayloadType::JsonText
        );

        // Binary data (random)
        let binary = vec![255, 0, 128, 64, 32, 16, 8, 4, 2, 1];
        let binary_extended = binary.repeat(20);
        assert_eq!(
            PayloadAnalyzer::analyze(&binary_extended),
            PayloadType::BinaryData
        );

        // Repeated structure
        let repeated = b"AAAABBBBCCCCDDDD";
        let repeated_extended = repeated.repeat(10);
        assert_eq!(
            PayloadAnalyzer::analyze(&repeated_extended),
            PayloadType::RepeatedStructure
        );
    }

    #[test]
    fn test_compression_roundtrip() {
        let mut compressor = AdaptiveCompression::new();

        let original = b"This is test data that will be compressed and decompressed";
        let original_extended = original.repeat(10);

        // Test LZ4
        let compressed = compressor
            .compress_with_algorithm(&original_extended, CompressionAlgorithm::Lz4)
            .unwrap();
        assert!(compressed.data.len() < original_extended.len());

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, original_extended);

        // Test Zlib
        let compressed = compressor
            .compress_with_algorithm(&original_extended, CompressionAlgorithm::Zlib)
            .unwrap();
        assert!(compressed.data.len() < original_extended.len());

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, original_extended);

        // Test None
        let compressed = compressor
            .compress_with_algorithm(&original_extended, CompressionAlgorithm::None)
            .unwrap();
        assert_eq!(compressed.data.len(), original_extended.len());

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, original_extended);
    }

    #[test]
    fn test_adaptive_compression() {
        let mut compressor = AdaptiveCompression::new();

        // Small message - should not compress
        let small = b"small";
        let compressed = compressor.compress_adaptive(small).unwrap();
        assert_eq!(compressed.algorithm, CompressionAlgorithm::None);

        // Text data - should use Zlib
        let text = b"This is a text message that repeats. This is a text message that repeats.";
        let text_extended = text.repeat(5);
        let compressed = compressor.compress_adaptive(&text_extended).unwrap();
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Zlib);

        // Check compression actually worked
        assert!(compressed.data.len() < text_extended.len());
    }
}
