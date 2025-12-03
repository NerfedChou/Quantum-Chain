//! # Outbound Ports (Driven Ports)
//!
//! SPIs required by the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 3.2: Driven Ports (TransactionStore, HashProvider, etc.)
//! - Section 3.3: V2.3 BlockDataProvider for cache miss handling

use crate::domain::{MerkleTree, TransactionLocation};
use shared_types::{Hash, Transaction};

/// Abstract interface for transaction storage.
///
/// ## SPEC-03 Section 3.2
///
/// This port allows the indexing subsystem to persist transaction
/// locations for later proof generation.
pub trait TransactionStore: Send + Sync {
    /// Store a transaction location.
    fn put_location(
        &mut self,
        tx_hash: Hash,
        location: TransactionLocation,
    ) -> Result<(), StoreError>;

    /// Get a transaction location by hash.
    fn get_location(&self, tx_hash: Hash) -> Result<Option<TransactionLocation>, StoreError>;

    /// Check if a transaction exists.
    fn exists(&self, tx_hash: Hash) -> Result<bool, StoreError>;

    /// Store a Merkle tree for a block (optional caching).
    fn put_tree(&mut self, block_hash: Hash, tree: MerkleTree) -> Result<(), StoreError>;

    /// Get a cached Merkle tree.
    fn get_tree(&self, block_hash: Hash) -> Result<Option<MerkleTree>, StoreError>;
}

/// V2.3: Interface for querying Block Storage for transaction data.
///
/// ## SPEC-03 Section 3.2
///
/// This port allows Transaction Indexing to query Block Storage for
/// transaction hashes needed to reconstruct Merkle trees for proof generation.
/// This enables bounded memory usage by fetching data on cache miss.
#[async_trait::async_trait]
pub trait BlockDataProvider: Send + Sync {
    /// Get transaction hashes for a block (for Merkle tree reconstruction).
    ///
    /// ## Parameters
    ///
    /// - `block_hash`: Hash of the block to get transaction hashes for
    ///
    /// ## Returns
    ///
    /// - `Ok(TransactionHashesData)`: Transaction hashes and cached Merkle root
    /// - `Err(BlockStorageError)`: Block not found or communication error
    async fn get_transaction_hashes_for_block(
        &self,
        block_hash: Hash,
    ) -> Result<TransactionHashesData, BlockStorageError>;

    /// Get location of a specific transaction.
    ///
    /// ## Parameters
    ///
    /// - `transaction_hash`: Hash of the transaction to locate
    ///
    /// ## Returns
    ///
    /// - `Ok(TransactionLocation)`: Location data for the transaction
    /// - `Err(BlockStorageError)`: Transaction not found or communication error
    async fn get_transaction_location(
        &self,
        transaction_hash: Hash,
    ) -> Result<TransactionLocation, BlockStorageError>;
}

/// V2.3: Transaction hashes data from Block Storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesData {
    /// Block hash these hashes belong to.
    pub block_hash: Hash,
    /// All transaction hashes in canonical order.
    pub transaction_hashes: Vec<Hash>,
    /// Cached Merkle root for verification.
    pub merkle_root: Hash,
}

/// Abstract interface for cryptographic hashing.
///
/// ## SPEC-03 Section 3.2
pub trait HashProvider: Send + Sync {
    /// Hash arbitrary bytes.
    fn hash(&self, data: &[u8]) -> Hash;

    /// Hash two concatenated hashes (for Merkle tree nodes).
    fn hash_pair(&self, left: &Hash, right: &Hash) -> Hash;
}

/// Abstract interface for transaction serialization.
///
/// ## SPEC-03 Section 3.2
///
/// ## INVARIANT-3: Canonical Serialization
///
/// This MUST produce identical bytes for semantically identical transactions.
pub trait TransactionSerializer: Send + Sync {
    /// Serialize a transaction to canonical bytes.
    fn serialize(&self, tx: &Transaction) -> Result<Vec<u8>, SerializationError>;

    /// Compute hash of a transaction using canonical serialization.
    fn hash_transaction(&self, tx: &Transaction) -> Result<Hash, SerializationError>;
}

/// Abstract interface for time operations (for testability).
pub trait TimeSource: Send + Sync {
    /// Get current timestamp in seconds since epoch.
    fn now(&self) -> u64;
}

/// Storage operation errors.
#[derive(Debug, Clone)]
pub enum StoreError {
    IOError { message: String },
    SerializationError { message: String },
    NotFound,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IOError { message } => write!(f, "I/O error: {}", message),
            Self::SerializationError { message } => write!(f, "Serialization error: {}", message),
            Self::NotFound => write!(f, "Not found"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Block Storage communication errors.
#[derive(Debug, Clone)]
pub enum BlockStorageError {
    /// Transaction not found in any stored block.
    TransactionNotFound { tx_hash: Hash },
    /// Block not found in storage.
    BlockNotFound { block_hash: Hash },
    /// Block Storage communication error.
    CommunicationError { message: String },
    /// Request timed out.
    Timeout,
}

impl std::fmt::Display for BlockStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransactionNotFound { .. } => write!(f, "Transaction not found"),
            Self::BlockNotFound { .. } => write!(f, "Block not found"),
            Self::CommunicationError { message } => write!(f, "Communication error: {}", message),
            Self::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for BlockStorageError {}

/// Serialization errors.
#[derive(Debug, Clone)]
pub struct SerializationError {
    pub message: String,
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Serialization error: {}", self.message)
    }
}

impl std::error::Error for SerializationError {}
