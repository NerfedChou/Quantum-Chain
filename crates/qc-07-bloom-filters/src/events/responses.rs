//! IPC Response Messages
//!
//! Reference: IPC-MATRIX.md Subsystem 7 - OUTGOING messages

use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// Bloom filter response
///
/// Reference: IPC-MATRIX.md - BloomFilter (outgoing)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomFilterResponse {
    /// Filter ID for tracking
    pub filter_id: u64,
    /// Serialized bit array
    pub bit_array: Vec<u8>,
    /// Number of hash functions (k)
    pub hash_count: u8,
    /// False positive rate
    pub false_positive_rate: f32,
    /// Block range this filter covers
    pub block_range: (u64, u64),
}

/// Filtered transactions response
///
/// Reference: IPC-MATRIX.md - FilteredTransactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredTransactionsResponse {
    /// Correlation ID matching the request
    pub correlation_id: u64,
    /// Block number
    pub block_number: u64,
    /// Matching transaction hashes
    pub transactions: Vec<Hash>,
    /// Whether false positives are included
    pub false_positives_included: bool,
    /// Estimated false positive rate
    pub false_positive_estimate: f64,
}

/// Error response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Correlation ID matching the request
    pub correlation_id: u64,
    /// Error code
    pub error_code: u32,
    /// Error message
    pub error_message: String,
}

/// Error codes for Bloom filter operations
pub mod error_codes {
    /// Filter size exceeds maximum
    pub const FILTER_TOO_LARGE: u32 = 7001;
    /// Too many elements requested
    pub const TOO_MANY_ELEMENTS: u32 = 7002;
    /// Invalid FPR (outside 0.01-0.1 range)
    pub const INVALID_FPR: u32 = 7003;
    /// Rate limited
    pub const RATE_LIMITED: u32 = 7004;
    /// Unauthorized sender
    pub const UNAUTHORIZED: u32 = 7005;
    /// Block not found
    pub const BLOCK_NOT_FOUND: u32 = 7006;
    /// Internal error
    pub const INTERNAL_ERROR: u32 = 7099;
}
