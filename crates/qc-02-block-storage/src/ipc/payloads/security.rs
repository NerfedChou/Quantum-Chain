//! # Payload Security
//!
//! Payload validation and sanitization.
//!
//! ## Security Invariants
//!
//! - **No Identity in Payloads**: Identity comes ONLY from envelope sender_id
//! - **Size Limits**: Payloads validated against max sizes
//! - **Sanitization**: Hash fields validated for format
//!
//! ## Note
//!
//! Payload validation is handled at the handler level with explicit bounds checking.
//! The constants below define the limits used throughout the crate.

/// Maximum number of transactions in a block for range queries
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 10000;

/// Maximum number of blocks in a range response
pub const MAX_BLOCKS_PER_RANGE: u64 = 100;

/// Maximum size for finality proof signatures
pub const MAX_FINALITY_SIGNATURES: usize = 200;
