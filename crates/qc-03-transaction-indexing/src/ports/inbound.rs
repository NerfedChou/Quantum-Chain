//! # Inbound Ports (Driving Ports)
//!
//! Public APIs exposed by the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 3.1: TransactionIndexingApi trait

use crate::domain::{IndexingError, IndexingStats, MerkleProof, TransactionLocation};
use shared_types::Hash;

/// Primary API for the Transaction Indexing subsystem.
///
/// ## SPEC-03 Section 3.1
///
/// This trait defines the core operations that the Transaction Indexing
/// subsystem exposes to other subsystems and adapters.
pub trait TransactionIndexingApi {
    /// Generate a Merkle proof for a transaction by its hash.
    ///
    /// ## Parameters
    ///
    /// - `transaction_hash`: Hash of the transaction to prove
    ///
    /// ## Returns
    ///
    /// - `Ok(MerkleProof)`: Proof that can verify transaction inclusion
    /// - `Err(TransactionNotFound)`: Transaction not indexed
    /// - `Err(TreeNotCached)`: Merkle tree evicted, must rebuild
    fn generate_proof(&mut self, transaction_hash: Hash) -> Result<MerkleProof, IndexingError>;

    /// Verify a Merkle proof against a known root.
    ///
    /// ## INVARIANT-2 Guarantee
    ///
    /// If this returns true, the proof is cryptographically valid.
    fn verify_proof(&self, proof: &MerkleProof) -> bool;

    /// Get the location of a transaction by hash.
    fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, IndexingError>;

    /// Check if a transaction is indexed.
    fn is_indexed(&self, transaction_hash: Hash) -> bool;

    /// Get indexing statistics.
    fn get_stats(&self) -> IndexingStats;
}
