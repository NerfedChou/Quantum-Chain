//! IPC message handler for the Mempool subsystem.
//!
//! Processes incoming IPC messages with security validation.

use crate::domain::{Hash, MempoolError, MempoolTransaction, TransactionPool};
use crate::ipc::payloads::*;
use crate::ipc::security::AuthorizationRules;
use crate::ports::TimeSource;

/// IPC message handler for the Mempool.
pub struct IpcHandler<T: TimeSource> {
    pool: TransactionPool,
    time_source: T,
}

impl<T: TimeSource> IpcHandler<T> {
    /// Creates a new IPC handler.
    pub fn new(pool: TransactionPool, time_source: T) -> Self {
        Self { pool, time_source }
    }

    /// Returns a reference to the underlying pool.
    pub fn pool(&self) -> &TransactionPool {
        &self.pool
    }

    /// Returns a mutable reference to the underlying pool.
    pub fn pool_mut(&mut self) -> &mut TransactionPool {
        &mut self.pool
    }

    /// Handles AddTransactionRequest.
    ///
    /// # Security
    /// - Validates sender is Subsystem 10 (Signature Verification)
    /// - Validates signature_verified is true
    pub fn handle_add_transaction(
        &mut self,
        sender_id: u8,
        request: AddTransactionRequest,
    ) -> Result<AddTransactionResponse, MempoolError> {
        // Security: Validate sender
        AuthorizationRules::validate_add_transaction(sender_id)?;

        // Security: Validate signature was verified
        if !request.signature_verified {
            return Err(MempoolError::SignatureNotVerified);
        }

        let now = self.time_source.now();
        let tx = MempoolTransaction::new(request.transaction, now);
        let tx_hash = tx.hash;

        match self.pool.add(tx) {
            Ok(()) => Ok(AddTransactionResponse {
                correlation_id: request.correlation_id,
                accepted: true,
                tx_hash: Some(tx_hash),
                error: None,
            }),
            Err(e) => Ok(AddTransactionResponse {
                correlation_id: request.correlation_id,
                accepted: false,
                tx_hash: None,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Handles GetTransactionsRequest.
    ///
    /// # Security
    /// - Validates sender is Subsystem 8 (Consensus)
    pub fn handle_get_transactions(
        &mut self,
        sender_id: u8,
        request: GetTransactionsRequest,
    ) -> Result<GetTransactionsResponse, MempoolError> {
        // Security: Validate sender
        AuthorizationRules::validate_get_transactions(sender_id)?;

        let txs = self
            .pool
            .get_for_block(request.max_count as usize, request.max_gas);

        let tx_hashes: Vec<Hash> = txs.iter().map(|t| t.hash).collect();
        let total_gas: u64 = txs.iter().map(|t| t.gas_limit).sum();

        // Propose the transactions for this block
        let now = self.time_source.now();
        self.pool
            .propose(&tx_hashes, request.target_block_height, now);

        Ok(GetTransactionsResponse {
            correlation_id: request.correlation_id,
            tx_hashes,
            total_gas,
        })
    }

    /// Handles BlockStorageConfirmation.
    ///
    /// # Security
    /// - Validates sender is Subsystem 2 (Block Storage)
    pub fn handle_storage_confirmation(
        &mut self,
        sender_id: u8,
        confirmation: BlockStorageConfirmation,
    ) -> Result<Vec<Hash>, MempoolError> {
        // Security: Validate sender
        AuthorizationRules::validate_storage_confirmation(sender_id)?;

        // Confirm the transactions (permanently delete them)
        let confirmed = self.pool.confirm(&confirmation.included_transactions);
        Ok(confirmed)
    }

    /// Handles BlockRejectedNotification.
    ///
    /// # Security
    /// - Validates sender is Subsystem 2 or 8
    pub fn handle_block_rejected(
        &mut self,
        sender_id: u8,
        notification: BlockRejectedNotification,
    ) -> Result<Vec<Hash>, MempoolError> {
        // Security: Validate sender
        AuthorizationRules::validate_block_rejected(sender_id)?;

        // Rollback the transactions (return to pending)
        let rolled_back = self.pool.rollback(&notification.affected_transactions);
        Ok(rolled_back)
    }

    /// Handles RemoveTransactionsRequest.
    ///
    /// # Security
    /// - Validates sender is Subsystem 8 (Consensus)
    pub fn handle_remove_transactions(
        &mut self,
        sender_id: u8,
        request: RemoveTransactionsRequest,
    ) -> Result<RemoveTransactionsResponse, MempoolError> {
        // Security: Validate sender
        AuthorizationRules::validate_remove_transactions(sender_id)?;

        let mut removed = Vec::new();
        for hash in &request.tx_hashes {
            if self.pool.remove(hash).is_ok() {
                removed.push(*hash);
            }
        }

        Ok(RemoveTransactionsResponse {
            correlation_id: request.correlation_id,
            removed_count: removed.len(),
            removed,
        })
    }

    /// Handles GetStatusRequest.
    pub fn handle_get_status(&self, request: GetStatusRequest) -> GetStatusResponse {
        let now = self.time_source.now();
        let status = self.pool.status(now);

        GetStatusResponse {
            correlation_id: request.correlation_id,
            status: status.into(),
        }
    }

    /// Runs periodic cleanup of timed out transactions.
    pub fn cleanup_timeouts(&mut self) -> Vec<Hash> {
        let now = self.time_source.now();
        self.pool.cleanup_timeouts(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MempoolConfig, U256};
    use crate::ipc::security::subsystem_id;
    use crate::ports::outbound::MockTimeSource;
    use shared_types::SignedTransaction;
    use uuid::Uuid;

    fn create_handler() -> IpcHandler<MockTimeSource> {
        let pool = TransactionPool::new(MempoolConfig::for_testing());
        let time_source = MockTimeSource::new(1000);
        IpcHandler::new(pool, time_source)
    }

    fn create_signed_transaction() -> SignedTransaction {
        SignedTransaction {
            from: [0xBB; 20],
            to: Some([0xCC; 20]),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(2_000_000_000u64),
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        }
    }

    fn create_add_request() -> AddTransactionRequest {
        AddTransactionRequest {
            correlation_id: Uuid::new_v4(),
            transaction: create_signed_transaction(),
            signature_verified: true,
        }
    }

    // =========================================================================
    // ADD TRANSACTION TESTS
    // =========================================================================

    #[test]
    fn test_add_transaction_authorized() {
        let mut handler = create_handler();
        let request = create_add_request();

        let response = handler
            .handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, request)
            .unwrap();

        assert!(response.accepted);
        assert!(response.tx_hash.is_some());
    }

    #[test]
    fn test_add_transaction_unauthorized() {
        let mut handler = create_handler();
        let request = create_add_request();

        // From Consensus (wrong sender)
        let result = handler.handle_add_transaction(subsystem_id::CONSENSUS, request);
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender { .. })));
    }

    #[test]
    fn test_add_transaction_unverified_signature() {
        let mut handler = create_handler();
        let mut request = create_add_request();
        request.signature_verified = false;

        let result = handler.handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, request);
        assert!(matches!(result, Err(MempoolError::SignatureNotVerified)));
    }

    // =========================================================================
    // GET TRANSACTIONS TESTS
    // =========================================================================

    #[test]
    fn test_get_transactions_authorized() {
        let mut handler = create_handler();

        // Add a transaction first
        let add_req = create_add_request();
        handler
            .handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, add_req)
            .unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        let response = handler
            .handle_get_transactions(subsystem_id::CONSENSUS, get_req)
            .unwrap();

        assert_eq!(response.tx_hashes.len(), 1);
    }

    #[test]
    fn test_get_transactions_unauthorized() {
        let mut handler = create_handler();
        let request = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        // From Signature Verification (wrong sender)
        let result = handler.handle_get_transactions(subsystem_id::SIGNATURE_VERIFICATION, request);
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender { .. })));
    }

    // =========================================================================
    // STORAGE CONFIRMATION TESTS
    // =========================================================================

    #[test]
    fn test_storage_confirmation_authorized() {
        let mut handler = create_handler();

        // Add and propose a transaction
        let add_req = create_add_request();
        let response = handler
            .handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, add_req)
            .unwrap();
        let tx_hash = response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        handler
            .handle_get_transactions(subsystem_id::CONSENSUS, get_req)
            .unwrap();

        // Confirm storage
        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![tx_hash],
            storage_timestamp: 2000,
        };

        let confirmed = handler
            .handle_storage_confirmation(subsystem_id::BLOCK_STORAGE, confirmation)
            .unwrap();

        assert_eq!(confirmed, vec![tx_hash]);
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_storage_confirmation_unauthorized() {
        let mut handler = create_handler();
        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![],
            storage_timestamp: 2000,
        };

        // From Consensus (wrong sender)
        let result = handler.handle_storage_confirmation(subsystem_id::CONSENSUS, confirmation);
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender { .. })));
    }

    // =========================================================================
    // BLOCK REJECTED TESTS
    // =========================================================================

    #[test]
    fn test_block_rejected_from_storage_authorized() {
        let mut handler = create_handler();
        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::StorageFailure,
        };

        let result = handler.handle_block_rejected(subsystem_id::BLOCK_STORAGE, notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_from_consensus_authorized() {
        let mut handler = create_handler();
        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };

        let result = handler.handle_block_rejected(subsystem_id::CONSENSUS, notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_unauthorized() {
        let mut handler = create_handler();
        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::Timeout,
        };

        // From Signature Verification (wrong sender)
        let result = handler.handle_block_rejected(subsystem_id::SIGNATURE_VERIFICATION, notification);
        assert!(matches!(result, Err(MempoolError::UnauthorizedSender { .. })));
    }

    // =========================================================================
    // TWO-PHASE COMMIT FLOW TEST
    // =========================================================================

    #[test]
    fn test_full_two_phase_commit_flow() {
        let mut handler = create_handler();

        // Phase 0: Add transaction
        let add_req = create_add_request();
        let add_response = handler
            .handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, add_req)
            .unwrap();
        let tx_hash = add_response.tx_hash.unwrap();

        assert!(handler.pool().get(&tx_hash).unwrap().is_pending());

        // Phase 1: Get transactions (proposes them)
        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        let response = handler
            .handle_get_transactions(subsystem_id::CONSENSUS, get_req)
            .unwrap();

        assert_eq!(response.tx_hashes, vec![tx_hash]);
        assert!(handler.pool().get(&tx_hash).unwrap().is_pending_inclusion());

        // Phase 2a: Confirm storage
        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![tx_hash],
            storage_timestamp: 2000,
        };
        handler
            .handle_storage_confirmation(subsystem_id::BLOCK_STORAGE, confirmation)
            .unwrap();

        // Transaction should be deleted
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_two_phase_commit_rollback_flow() {
        let mut handler = create_handler();

        // Add and propose
        let add_req = create_add_request();
        let add_response = handler
            .handle_add_transaction(subsystem_id::SIGNATURE_VERIFICATION, add_req)
            .unwrap();
        let tx_hash = add_response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        handler
            .handle_get_transactions(subsystem_id::CONSENSUS, get_req)
            .unwrap();

        assert!(handler.pool().get(&tx_hash).unwrap().is_pending_inclusion());

        // Phase 2b: Block rejected
        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![tx_hash],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };
        handler
            .handle_block_rejected(subsystem_id::CONSENSUS, notification)
            .unwrap();

        // Transaction should be back to pending
        assert!(handler.pool().get(&tx_hash).unwrap().is_pending());
    }
}
