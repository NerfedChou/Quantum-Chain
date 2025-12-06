//! Outbound Ports (Driven Ports)
//!
//! Reference: SPEC-07 Section 3.2 - TransactionDataProvider
//!
//! These traits define dependencies that the Bloom Filter subsystem
//! needs from external components (e.g., Transaction Indexing subsystem).

use async_trait::async_trait;
use shared_types::{Address, Hash, SignedTransaction};

use crate::error::DataError;

/// Addresses involved in a single transaction
///
/// More efficient than fetching full transactions when only
/// addresses are needed for filter matching.
#[derive(Clone, Debug)]
pub struct TransactionAddresses {
    /// Transaction hash
    pub tx_hash: Hash,
    /// Sender address
    pub sender: Address,
    /// Recipient address (None for contract creation)
    pub recipient: Option<Address>,
    /// Created contract address (if any)
    pub created_contract: Option<Address>,
    /// Addresses from transaction logs
    pub log_addresses: Vec<Address>,
}

/// Transaction data provider (Driven Port)
///
/// Reference: SPEC-07 Section 3.2, Architecture.md §3.2.1 - Principle of Least Data
///
/// SECURITY: This port returns ONLY the data needed for filtering,
/// not full transaction details. This reduces bandwidth and information leakage.
#[async_trait]
pub trait TransactionDataProvider: Send + Sync {
    /// Get transaction hashes for a block
    async fn get_transaction_hashes(&self, block_height: u64) -> Result<Vec<Hash>, DataError>;

    /// Get full transactions for a block
    async fn get_transactions(
        &self,
        block_height: u64,
    ) -> Result<Vec<SignedTransaction>, DataError>;

    /// Get addresses involved in transactions for a block
    ///
    /// This is MORE EFFICIENT than fetching full transactions.
    async fn get_transaction_addresses(
        &self,
        block_height: u64,
    ) -> Result<Vec<TransactionAddresses>, DataError>;
}
