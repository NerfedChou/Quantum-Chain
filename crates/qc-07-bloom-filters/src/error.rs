//! Error types for Bloom Filter subsystem

use shared_types::SubsystemId;
use thiserror::Error;

/// Errors that can occur in the Bloom Filter subsystem
#[derive(Debug, Error)]
pub enum FilterError {
    #[error("Filter size exceeds maximum: {size} > {max}")]
    FilterTooLarge { size: usize, max: usize },

    #[error("Too many elements: {count} > {max}")]
    TooManyElements { count: usize, max: usize },

    #[error("Invalid false positive rate: {fpr} (must be between 0.01 and 0.1)")]
    InvalidFPR { fpr: f64 },

    #[error("Block not found: {height}")]
    BlockNotFound { height: u64 },

    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),

    #[error("Rate limited: too many filter updates")]
    RateLimited,

    #[error("Too many watched addresses: {count} > {max}")]
    TooManyAddresses { count: usize, max: usize },

    #[error("Invalid filter parameters: {0}")]
    InvalidParameters(String),

    #[error("Data provider error: {0}")]
    DataError(#[from] DataError),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid method: {0}")]
    InvalidMethod(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Filter not found: {0}")]
    FilterNotFound(String),
}

/// Errors from data providers
#[derive(Debug, Error)]
pub enum DataError {
    #[error("Block not found: {height}")]
    BlockNotFound { height: u64 },

    #[error("Transaction not found: {hash:?}")]
    TransactionNotFound { hash: [u8; 32] },

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}
