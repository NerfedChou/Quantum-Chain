//! # Outbound Ports (Driven Ports / SPI)
//!
//! Traits that define dependencies this subsystem needs.
//!
//! Reference: SPEC-10 Section 3.2 (Driven Ports)

use crate::domain::entities::VerifiedTransaction;
use thiserror::Error;

/// Error from Mempool operations.
#[derive(Debug, Error)]
pub enum MempoolError {
    /// The mempool is full
    #[error("Mempool is full")]
    Full,

    /// Transaction was rejected
    #[error("Transaction rejected: {reason}")]
    Rejected { reason: String },

    /// Communication error
    #[error("Communication error: {0}")]
    CommunicationError(String),
}

/// Gateway to the Mempool subsystem.
///
/// Reference: SPEC-10 Section 3.2
///
/// This port allows forwarding verified transactions to the Mempool (Subsystem 6).
#[async_trait::async_trait]
pub trait MempoolGateway: Send + Sync {
    /// Submit a verified transaction to the Mempool.
    ///
    /// # Arguments
    /// * `transaction` - The verified transaction to submit
    ///
    /// # Errors
    /// * `MempoolError::Full` - Mempool has reached capacity
    /// * `MempoolError::Rejected` - Transaction was rejected by Mempool
    async fn submit_verified_transaction(
        &self,
        transaction: VerifiedTransaction,
    ) -> Result<(), MempoolError>;
}
