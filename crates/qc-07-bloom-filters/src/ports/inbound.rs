//! Inbound Ports (Driving Ports)
//!
//! Reference: SPEC-07 Section 3.1 - BloomFilterApi
//!
//! These traits define the API that external components use to interact
//! with the Bloom Filter subsystem.

use async_trait::async_trait;
use shared_types::{Address, Hash, SignedTransaction};

use crate::domain::{BlockFilter, BloomConfig, BloomFilter};
use crate::error::FilterError;

/// Result of matching a transaction against a filter
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// Whether the transaction matched
    pub matches: bool,
    /// Which field caused the match (if any)
    pub matched_field: Option<MatchedField>,
}

/// Which field of a transaction matched the filter
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchedField {
    /// Transaction sender (from)
    Sender,
    /// Transaction recipient (to)
    Recipient,
    /// Contract creation address
    ContractCreation,
    /// Log address at given index
    LogAddress(usize),
}

/// Transaction receipt for matching against logs
#[derive(Clone, Debug)]
pub struct TransactionReceipt {
    /// Transaction hash
    pub tx_hash: Hash,
    /// Log entries
    pub logs: Vec<LogEntry>,
}

/// A log entry in a transaction receipt
#[derive(Clone, Debug)]
pub struct LogEntry {
    /// Address that emitted the log
    pub address: Address,
    /// Log topics
    pub topics: Vec<Hash>,
    /// Log data
    pub data: Vec<u8>,
}

/// Primary Bloom filter API (Driving Port)
///
/// Reference: SPEC-07 Section 3.1
#[async_trait]
pub trait BloomFilterApi: Send + Sync {
    /// Create a filter for a set of addresses
    ///
    /// # Arguments
    /// * `addresses` - Addresses to watch
    /// * `config` - Filter configuration
    ///
    /// # Returns
    /// A Bloom filter containing all addresses
    fn create_filter(
        &self,
        addresses: &[Address],
        config: &BloomConfig,
    ) -> Result<BloomFilter, FilterError>;

    /// Test if a transaction matches a filter
    ///
    /// Reference: SPEC-07 Section 3.1 - MATCHING FIELDS
    ///
    /// Tests in order:
    /// 1. Transaction sender address (tx.from)
    /// 2. Transaction recipient address (tx.to)
    /// 3. Contract creation address (if tx.to is None)
    /// 4. Log addresses (for each log in receipt)
    fn matches(
        &self,
        filter: &BloomFilter,
        transaction: &SignedTransaction,
        receipt: Option<&TransactionReceipt>,
    ) -> MatchResult;

    /// Get filtered transactions for a block
    ///
    /// Returns all transactions that match the filter (plus possible false positives).
    async fn get_filtered_transactions(
        &self,
        block_height: u64,
        filter: &BloomFilter,
    ) -> Result<Vec<SignedTransaction>, FilterError>;

    /// Create a block filter from transaction data
    fn create_block_filter(
        &self,
        block_hash: Hash,
        block_height: u64,
        tx_hashes: &[Hash],
        addresses: &[Address],
    ) -> Result<BlockFilter, FilterError>;
}
