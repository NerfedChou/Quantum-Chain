//! IPC Request Messages
//!
//! Reference: IPC-MATRIX.md Subsystem 7 - INCOMING messages

use serde::{Deserialize, Serialize};
use shared_types::{Address, Hash};

/// Request to build a new Bloom filter
///
/// Reference: IPC-MATRIX.md - BuildFilterRequest
///
/// Security: Accept from Subsystem 13 (Light Clients) ONLY
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildFilterRequest {
    /// Correlation ID for request-response matching
    pub correlation_id: u64,
    /// Reply topic
    pub reply_to: String,
    /// Addresses to watch
    pub watched_addresses: Vec<Address>,
    /// Start block for filter
    pub start_block: u64,
    /// End block for filter
    pub end_block: u64,
    /// Target false positive rate
    pub target_fpr: f32,
}

/// Request to update an existing filter
///
/// Reference: IPC-MATRIX.md - UpdateFilterRequest
///
/// Security:
/// - Accept from Subsystem 13 (Light Clients) ONLY
/// - Reject >1 filter update per 10 blocks per client
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateFilterRequest {
    /// Correlation ID for request-response matching
    pub correlation_id: u64,
    /// Reply topic
    pub reply_to: String,
    /// Filter ID to update
    pub filter_id: u64,
    /// Addresses to add
    pub add_addresses: Vec<Address>,
    /// Addresses to remove (note: cannot truly remove from Bloom filter)
    pub remove_addresses: Vec<Address>,
}

/// Transaction hash update from indexing subsystem
///
/// Reference: IPC-MATRIX.md - TransactionHashUpdate
///
/// Security: Accept from Subsystem 3 (Transaction Indexing) ONLY
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionHashUpdate {
    /// Block number
    pub block_number: u64,
    /// Transaction hashes in the block
    pub hashes: Vec<Hash>,
}

/// Request for filtered transactions
///
/// Reference: SPEC-07 Section 4.1
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredTransactionsRequest {
    /// Correlation ID for request-response matching
    pub correlation_id: u64,
    /// Reply topic
    pub reply_to: String,
    /// Block height to filter
    pub block_height: u64,
    /// Serialized Bloom filter
    pub filter_bytes: Vec<u8>,
}

/// Request for transaction hashes (to Subsystem 3)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionHashRequest {
    /// Correlation ID for request-response matching
    pub correlation_id: u64,
    /// Reply topic
    pub reply_to: String,
    /// Block height
    pub block_height: u64,
}
