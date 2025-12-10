//! # Domain Errors
//!
//! Error types for Cross-Chain Communication.
//!
//! Reference: SPEC-15 Section 6 (Lines 498-530)

use thiserror::Error;

/// Hash type (32-byte SHA-256).
pub type Hash = [u8; 32];

/// Address type (20-byte).
pub type Address = [u8; 20];

/// Secret type (32-byte).
pub type Secret = [u8; 32];

/// Cross-chain error types.
#[derive(Debug, Error)]
pub enum CrossChainError {
    /// Unsupported blockchain.
    #[error("Unsupported chain: {0}")]
    UnsupportedChain(String),

    /// HTLC not found.
    #[error("HTLC not found: {0:?}")]
    HTLCNotFound(Hash),

    /// Invalid secret (doesn't match hashlock).
    #[error("Invalid secret")]
    InvalidSecret,

    /// HTLC has expired.
    #[error("HTLC expired")]
    HTLCExpired,

    /// HTLC not expired (cannot refund yet).
    #[error("HTLC not expired (cannot refund)")]
    HTLCNotExpired,

    /// Invalid timelock margin between source and target.
    /// Reference: System.md Line 752
    #[error("Invalid timelock margin: source={source_timelock}, target={target_timelock}, required={required_margin}")]
    InvalidTimelockMargin {
        /// Source HTLC timelock
        source_timelock: u64,
        /// Target HTLC timelock
        target_timelock: u64,
        /// Required margin in seconds
        required_margin: u64,
    },

    /// Not finalized (insufficient confirmations).
    #[error("Not finalized: {got}/{required} confirmations")]
    NotFinalized {
        /// Confirmations received
        got: u64,
        /// Required confirmations
        required: u64,
    },

    /// Unauthorized claimer.
    #[error("Unauthorized claimer")]
    UnauthorizedClaimer,

    /// Invalid swap state transition.
    #[error("Invalid swap transition: {from} -> {to}")]
    InvalidSwapTransition {
        /// Current state
        from: String,
        /// Attempted state
        to: String,
    },

    /// Invalid HTLC state transition.
    #[error("Invalid HTLC transition: {from} -> {to}")]
    InvalidHTLCTransition {
        /// Current state
        from: String,
        /// Attempted state
        to: String,
    },

    /// Invalid proof.
    #[error("Invalid proof")]
    InvalidProof,

    /// Swap not found.
    #[error("Swap not found: {0:?}")]
    SwapNotFound(Hash),

    /// Network error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Already claimed.
    #[error("HTLC already claimed")]
    AlreadyClaimed,

    /// Already refunded.
    #[error("HTLC already refunded")]
    AlreadyRefunded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_chain_error() {
        let err = CrossChainError::UnsupportedChain("Solana".to_string());
        assert!(err.to_string().contains("Solana"));
    }

    #[test]
    fn test_htlc_not_found_error() {
        let err = CrossChainError::HTLCNotFound([1u8; 32]);
        assert!(err.to_string().contains("HTLC not found"));
    }

    #[test]
    fn test_invalid_timelock_margin_error() {
        let err = CrossChainError::InvalidTimelockMargin {
            source_timelock: 10000,
            target_timelock: 9000,
            required_margin: 21600,
        };
        assert!(err.to_string().contains("21600"));
    }

    #[test]
    fn test_not_finalized_error() {
        let err = CrossChainError::NotFinalized { got: 3, required: 6 };
        assert!(err.to_string().contains("3/6"));
    }

    #[test]
    fn test_invalid_secret_error() {
        let err = CrossChainError::InvalidSecret;
        assert!(err.to_string().contains("Invalid secret"));
    }
}
