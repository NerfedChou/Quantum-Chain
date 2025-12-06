//! Error types for block production subsystem

use thiserror::Error;

/// Result type alias for block production operations
pub type Result<T> = std::result::Result<T, BlockProductionError>;

/// Errors that can occur during block production
#[derive(Debug, Error)]
pub enum BlockProductionError {
    /// Mempool communication error
    #[error("Mempool error: {0}")]
    MempoolError(String),

    /// State management communication error
    #[error("State error: {0}")]
    StateError(String),

    /// Consensus submission error
    #[error("Consensus error: {0}")]
    ConsensusError(String),

    /// No transactions available in mempool
    #[error("No transactions available")]
    NoTransactionsAvailable,

    /// Gas limit exceeded (internal logic error)
    #[error("Gas limit exceeded: used {used}, limit {limit}")]
    GasLimitExceeded {
        /// Actual gas used
        used: u64,
        /// Block gas limit
        limit: u64,
    },

    /// Nonce mismatch detected
    #[error("Nonce mismatch for address {address}: expected {expected}, got {actual}")]
    NonceMismatch {
        /// Address with nonce mismatch
        address: String,
        /// Expected nonce
        expected: u64,
        /// Actual nonce
        actual: u64,
    },

    /// Invalid transaction signature
    #[error("Invalid transaction signature")]
    InvalidSignature,

    /// PoW mining failed to find valid nonce
    #[error("Mining failed: no valid nonce found")]
    MiningFailed,

    /// Not selected as PoS proposer for this slot
    #[error("Not selected as proposer for slot {slot}")]
    NotProposer {
        /// Slot number
        slot: u64,
    },

    /// Invalid validator key provided
    #[error("Invalid validator key")]
    InvalidValidatorKey,

    /// Block production not currently active
    #[error("Production not active")]
    NotActive,

    /// Feature not yet implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Signature error
    #[error("Signature error: {0}")]
    SignatureError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Unauthorized IPC sender
    #[error("Unauthorized sender: subsystem {sender_id}")]
    UnauthorizedSender {
        /// Subsystem ID that attempted access
        sender_id: u8,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded for subsystem {subsystem_id}")]
    RateLimitExceeded {
        /// Subsystem ID that hit rate limit
        subsystem_id: u8,
    },

    /// Gas price too low
    #[error("Gas price too low: {gas_price}, minimum: {min_gas_price}")]
    GasPriceTooLow {
        /// Provided gas price
        gas_price: String,
        /// Minimum required
        min_gas_price: String,
    },

    /// Gas limit too high
    #[error("Gas limit too high: {gas_limit}, maximum: {max_gas_limit}")]
    GasLimitTooHigh {
        /// Provided gas limit
        gas_limit: u64,
        /// Maximum allowed
        max_gas_limit: u64,
    },

    /// Zero gas limit
    #[error("Transaction has zero gas limit")]
    ZeroGasLimit {
        /// Transaction hash
        tx_hash: String,
    },

    /// Block gas limit exceeded
    #[error("Block gas limit exceeded: {provided} > {max}")]
    BlockGasLimitExceeded {
        /// Provided limit
        provided: u64,
        /// Maximum allowed
        max: u64,
    },

    /// Gas used exceeds limit
    #[error("Gas used exceeds limit: {gas_used} > {gas_limit}")]
    GasUsedExceedsLimit {
        /// Actual gas used
        gas_used: u64,
        /// Gas limit
        gas_limit: u64,
    },

    /// Invalid timestamp
    #[error("Invalid timestamp: {provided}, diff from now: {diff_seconds}s")]
    InvalidTimestamp {
        /// Provided timestamp
        provided: u64,
        /// Difference from current time
        diff_seconds: u64,
    },

    /// Inconsistent state
    #[error("Inconsistent state: {reason}")]
    InconsistentState {
        /// Reason for inconsistency
        reason: String,
    },

    /// Invalid state root
    #[error("Invalid state root: cannot be zero")]
    InvalidStateRoot,

    /// Invalid nonce ordering
    #[error("Invalid nonce ordering for address {address}: expected {expected}, got {actual}")]
    InvalidNonceOrdering {
        /// Address with invalid nonce
        address: String,
        /// Expected nonce
        expected: u64,
        /// Actual nonce
        actual: u64,
    },
}

impl BlockProductionError {
    /// Check if error is recoverable (should retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::NoTransactionsAvailable
                | Self::NotProposer { .. }
                | Self::MempoolError(_)
                | Self::StateError(_)
        )
    }

    /// Check if error is critical (should stop production)
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::InvalidValidatorKey | Self::InvalidConfig(_) | Self::InternalError(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recoverability() {
        assert!(BlockProductionError::NoTransactionsAvailable.is_recoverable());
        assert!(BlockProductionError::NotProposer { slot: 100 }.is_recoverable());
        assert!(!BlockProductionError::InvalidValidatorKey.is_recoverable());
    }

    #[test]
    fn test_error_criticality() {
        assert!(BlockProductionError::InvalidValidatorKey.is_critical());
        assert!(!BlockProductionError::NoTransactionsAvailable.is_critical());
    }
}
