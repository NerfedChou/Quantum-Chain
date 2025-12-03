//! Security boundaries and authorization for IPC messages.
//!
//! Implements the authorization rules defined in IPC-MATRIX.md for Subsystem 6.
//!
//! # Migration Note (2024-12)
//!
//! HMAC validation, nonce caching, and timestamp validation have been migrated
//! to the centralized `shared-types::security` module. This file now only
//! contains authorization rules and subsystem ID constants.
//!
//! # Authorization Rules
//!
//! | Message Type | Authorized Sender(s) |
//! |--------------|---------------------|
//! | `AddTransactionRequest` | Subsystem 10 ONLY |
//! | `GetTransactionsRequest` | Subsystem 8 ONLY |
//! | `RemoveTransactionsRequest` | Subsystem 8 ONLY |
//! | `BlockStorageConfirmation` | Subsystem 2 ONLY |
//! | `BlockRejectedNotification` | Subsystems 2, 8 |

use crate::domain::MempoolError;

/// Subsystem IDs as defined in Architecture.md.
pub mod subsystem_id {
    /// Peer Discovery
    pub const PEER_DISCOVERY: u8 = 1;
    /// Block Storage
    pub const BLOCK_STORAGE: u8 = 2;
    /// Transaction Indexing
    pub const TX_INDEXING: u8 = 3;
    /// State Management
    pub const STATE_MANAGEMENT: u8 = 4;
    /// Block Propagation
    pub const BLOCK_PROPAGATION: u8 = 5;
    /// Mempool (this subsystem)
    pub const MEMPOOL: u8 = 6;
    /// Bloom Filters
    pub const BLOOM_FILTERS: u8 = 7;
    /// Consensus
    pub const CONSENSUS: u8 = 8;
    /// Finality
    pub const FINALITY: u8 = 9;
    /// Signature Verification
    pub const SIGNATURE_VERIFICATION: u8 = 10;
}

/// Authorization rules for IPC messages.
#[derive(Debug, Clone)]
pub struct AuthorizationRules;

impl AuthorizationRules {
    /// Validates that a sender is authorized to send AddTransactionRequest.
    ///
    /// Only Subsystem 10 (Signature Verification) is allowed.
    pub fn validate_add_transaction(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::SIGNATURE_VERIFICATION {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::SIGNATURE_VERIFICATION],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send GetTransactionsRequest.
    ///
    /// Only Subsystem 8 (Consensus) is allowed.
    pub fn validate_get_transactions(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send RemoveTransactionsRequest.
    ///
    /// Only Subsystem 8 (Consensus) is allowed.
    pub fn validate_remove_transactions(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send BlockStorageConfirmation.
    ///
    /// Only Subsystem 2 (Block Storage) is allowed.
    pub fn validate_storage_confirmation(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::BLOCK_STORAGE {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::BLOCK_STORAGE],
            });
        }
        Ok(())
    }

    /// Validates that a sender is authorized to send BlockRejectedNotification.
    ///
    /// Subsystems 2 (Block Storage) and 8 (Consensus) are allowed.
    pub fn validate_block_rejected(sender_id: u8) -> Result<(), MempoolError> {
        if sender_id != subsystem_id::BLOCK_STORAGE && sender_id != subsystem_id::CONSENSUS {
            return Err(MempoolError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_id::BLOCK_STORAGE, subsystem_id::CONSENSUS],
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_add_transaction_authorized() {
        assert!(
            AuthorizationRules::validate_add_transaction(subsystem_id::SIGNATURE_VERIFICATION)
                .is_ok()
        );
    }

    #[test]
    fn test_validate_add_transaction_unauthorized() {
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::CONSENSUS).is_err());
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::BLOCK_STORAGE).is_err());
        assert!(AuthorizationRules::validate_add_transaction(subsystem_id::MEMPOOL).is_err());
    }

    #[test]
    fn test_validate_get_transactions_authorized() {
        assert!(AuthorizationRules::validate_get_transactions(subsystem_id::CONSENSUS).is_ok());
    }

    #[test]
    fn test_validate_get_transactions_unauthorized() {
        assert!(AuthorizationRules::validate_get_transactions(
            subsystem_id::SIGNATURE_VERIFICATION
        )
        .is_err());
        assert!(
            AuthorizationRules::validate_get_transactions(subsystem_id::BLOCK_STORAGE).is_err()
        );
    }

    #[test]
    fn test_validate_storage_confirmation_authorized() {
        assert!(
            AuthorizationRules::validate_storage_confirmation(subsystem_id::BLOCK_STORAGE).is_ok()
        );
    }

    #[test]
    fn test_validate_storage_confirmation_unauthorized() {
        assert!(
            AuthorizationRules::validate_storage_confirmation(subsystem_id::CONSENSUS).is_err()
        );
        assert!(AuthorizationRules::validate_storage_confirmation(
            subsystem_id::SIGNATURE_VERIFICATION
        )
        .is_err());
    }

    #[test]
    fn test_validate_block_rejected_authorized() {
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::BLOCK_STORAGE).is_ok());
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::CONSENSUS).is_ok());
    }

    #[test]
    fn test_validate_block_rejected_unauthorized() {
        assert!(
            AuthorizationRules::validate_block_rejected(subsystem_id::SIGNATURE_VERIFICATION)
                .is_err()
        );
        assert!(AuthorizationRules::validate_block_rejected(subsystem_id::MEMPOOL).is_err());
    }
}
