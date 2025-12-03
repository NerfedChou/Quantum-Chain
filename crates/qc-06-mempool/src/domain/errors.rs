//! Mempool error types.
//!
//! Defines all error conditions for the Mempool subsystem.

use super::entities::{Address, Hash, U256};

/// Mempool error type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MempoolError {
    /// Transaction already exists in the pool.
    DuplicateTransaction(Hash),

    /// Gas price is below minimum.
    GasPriceTooLow { price: U256, minimum: U256 },

    /// Transaction gas limit exceeds maximum.
    GasLimitTooHigh { limit: u64, maximum: u64 },

    /// Account has reached maximum pending transactions.
    AccountLimitReached { address: Address, limit: usize },

    /// Pool has reached maximum capacity.
    PoolFull { capacity: usize },

    /// Transaction not found in the pool.
    TransactionNotFound(Hash),

    /// Insufficient balance for transaction (U256 per SPEC-06).
    InsufficientBalance { required: U256, available: U256 },

    /// Invalid nonce (not the expected next nonce).
    InvalidNonce { expected: u64, actual: u64 },

    /// Nonce too far in the future (gap too large).
    NonceTooHigh {
        expected: u64,
        actual: u64,
        max_gap: u64,
    },

    /// Fee bump too small for Replace-by-Fee.
    InsufficientFeeBump {
        old_price: U256,
        new_price: U256,
        min_bump_percent: u64,
    },

    /// Replace-by-Fee is disabled.
    RbfDisabled,

    /// Transaction is already pending inclusion (cannot modify).
    TransactionPendingInclusion(Hash),

    /// Cannot evict transaction (e.g., pending inclusion).
    CannotEvict(Hash),

    /// Unauthorized sender for IPC message.
    UnauthorizedSender { sender_id: u8, allowed: Vec<u8> },

    /// Transaction signature not verified.
    SignatureNotVerified,

    /// Message timestamp is too old.
    TimestampTooOld { timestamp: u64, now: u64 },

    /// Message timestamp is too far in the future.
    TimestampTooFuture { timestamp: u64, now: u64 },

    /// Invalid HMAC signature.
    InvalidSignature,

    /// Replay attack detected (nonce reused).
    ReplayDetected { nonce: u64 },

    /// State provider error.
    StateError(String),

    /// Internal error.
    Internal(String),
}

impl std::fmt::Display for MempoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTransaction(hash) => {
                write!(f, "Duplicate transaction: {:?}", &hash[..4])
            }
            Self::GasPriceTooLow { price, minimum } => {
                write!(f, "Gas price {} below minimum {}", price, minimum)
            }
            Self::GasLimitTooHigh { limit, maximum } => {
                write!(f, "Gas limit {} exceeds maximum {}", limit, maximum)
            }
            Self::AccountLimitReached { address, limit } => {
                write!(
                    f,
                    "Account {:?} reached limit of {} transactions",
                    &address[..4],
                    limit
                )
            }
            Self::PoolFull { capacity } => {
                write!(f, "Pool full at {} transactions", capacity)
            }
            Self::TransactionNotFound(hash) => {
                write!(f, "Transaction not found: {:?}", &hash[..4])
            }
            Self::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: required {}, available {}",
                    required, available
                )
            }
            Self::InvalidNonce { expected, actual } => {
                write!(f, "Invalid nonce: expected {}, got {}", expected, actual)
            }
            Self::NonceTooHigh {
                expected,
                actual,
                max_gap,
            } => {
                write!(
                    f,
                    "Nonce {} too high (expected {}, max gap {})",
                    actual, expected, max_gap
                )
            }
            Self::InsufficientFeeBump {
                old_price,
                new_price,
                min_bump_percent,
            } => {
                write!(
                    f,
                    "Insufficient fee bump: {} -> {} (min {}%)",
                    old_price, new_price, min_bump_percent
                )
            }
            Self::RbfDisabled => write!(f, "Replace-by-Fee is disabled"),
            Self::TransactionPendingInclusion(hash) => {
                write!(f, "Transaction {:?} pending inclusion", &hash[..4])
            }
            Self::CannotEvict(hash) => {
                write!(f, "Cannot evict transaction {:?}", &hash[..4])
            }
            Self::UnauthorizedSender { sender_id, allowed } => {
                write!(
                    f,
                    "Unauthorized sender {}, allowed: {:?}",
                    sender_id, allowed
                )
            }
            Self::SignatureNotVerified => write!(f, "Transaction signature not verified"),
            Self::TimestampTooOld { timestamp, now } => {
                write!(f, "Timestamp {} is too old (now: {})", timestamp, now)
            }
            Self::TimestampTooFuture { timestamp, now } => {
                write!(
                    f,
                    "Timestamp {} is too far in the future (now: {})",
                    timestamp, now
                )
            }
            Self::InvalidSignature => write!(f, "Invalid HMAC signature"),
            Self::ReplayDetected { nonce } => {
                write!(f, "Replay attack detected for nonce {}", nonce)
            }
            Self::StateError(msg) => write!(f, "State error: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for MempoolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MempoolError::GasPriceTooLow {
            price: U256::from(500_000_000u64),
            minimum: U256::from(1_000_000_000u64),
        };
        let msg = err.to_string();
        assert!(msg.contains("500000000"));
        assert!(msg.contains("1000000000"));
    }

    #[test]
    fn test_duplicate_transaction_error() {
        let hash = [0xAB; 32];
        let err = MempoolError::DuplicateTransaction(hash);
        assert!(err.to_string().contains("Duplicate"));
    }

    #[test]
    fn test_insufficient_fee_bump_error() {
        let err = MempoolError::InsufficientFeeBump {
            old_price: U256::from(100u64),
            new_price: U256::from(105u64),
            min_bump_percent: 10,
        };
        assert!(err.to_string().contains("fee bump"));
        assert!(err.to_string().contains("10%"));
    }

    #[test]
    fn test_insufficient_balance_uses_u256() {
        let err = MempoolError::InsufficientBalance {
            required: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            available: U256::from(500_000_000_000_000_000u128),  // 0.5 ETH
        };
        let msg = err.to_string();
        assert!(msg.contains("Insufficient balance"));
    }
}
