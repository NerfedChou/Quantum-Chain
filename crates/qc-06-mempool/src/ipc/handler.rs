//! IPC message handler for the Mempool subsystem.
//!
//! Processes incoming IPC messages with security validation.

use crate::domain::{Hash, MempoolError, MempoolTransaction, TransactionPool};
use crate::ipc::payloads::*;
use crate::ipc::security::{
    validate_hmac_signature, validate_nonce, validate_timestamp, AuthorizationRules, HmacKey,
    NonceCache,
};
use crate::ports::TimeSource;

/// IPC message handler for the Mempool.
pub struct IpcHandler<T: TimeSource> {
    pool: TransactionPool,
    time_source: T,
    nonce_cache: NonceCache,
    hmac_key: HmacKey,
}

impl<T: TimeSource> IpcHandler<T> {
    /// Creates a new IPC handler.
    pub fn new(pool: TransactionPool, time_source: T) -> Self {
        Self {
            pool,
            time_source,
            nonce_cache: NonceCache::default(),
            hmac_key: [0u8; 32], // In production, load from secure config
        }
    }

    /// Creates a new IPC handler with custom HMAC key.
    pub fn with_hmac_key(pool: TransactionPool, time_source: T, hmac_key: HmacKey) -> Self {
        Self {
            pool,
            time_source,
            nonce_cache: NonceCache::default(),
            hmac_key,
        }
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
    /// - Validates timestamp (prevents stale messages)
    /// - Validates sender is Subsystem 10 (Signature Verification)
    /// - Validates HMAC signature
    /// - Validates nonce (prevents replay attacks)
    /// - Validates signature_verified is true
    pub fn handle_add_transaction(
        &mut self,
        sender_id: u8,
        timestamp: u64,
        nonce: u64,
        signature: &[u8; 32],
        message_bytes: &[u8],
        request: AddTransactionRequest,
    ) -> Result<AddTransactionResponse, MempoolError> {
        let now = self.time_source.now();

        // Security Step 1: Validate timestamp
        validate_timestamp(timestamp, now)?;

        // Security Step 2: Validate sender
        AuthorizationRules::validate_add_transaction(sender_id)?;

        // Security Step 3: Validate HMAC signature
        validate_hmac_signature(message_bytes, signature, &self.hmac_key)?;

        // Security Step 4: Validate nonce (replay prevention)
        validate_nonce(nonce, timestamp, now, &self.nonce_cache)?;

        // Security Step 5: Validate signature was verified
        if !request.signature_verified {
            return Err(MempoolError::SignatureNotVerified);
        }

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
    /// - Validates timestamp
    /// - Validates sender is Subsystem 8 (Consensus)
    /// - Validates HMAC signature
    /// - Validates nonce
    pub fn handle_get_transactions(
        &mut self,
        sender_id: u8,
        timestamp: u64,
        nonce: u64,
        signature: &[u8; 32],
        message_bytes: &[u8],
        request: GetTransactionsRequest,
    ) -> Result<GetTransactionsResponse, MempoolError> {
        let now = self.time_source.now();

        // Security validations
        validate_timestamp(timestamp, now)?;
        AuthorizationRules::validate_get_transactions(sender_id)?;
        validate_hmac_signature(message_bytes, signature, &self.hmac_key)?;
        validate_nonce(nonce, timestamp, now, &self.nonce_cache)?;

        let txs = self
            .pool
            .get_for_block(request.max_count as usize, request.max_gas);

        let tx_hashes: Vec<Hash> = txs.iter().map(|t| t.hash).collect();
        let total_gas: u64 = txs.iter().map(|t| t.gas_limit).sum();

        // Propose the transactions for this block
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
    /// - Validates timestamp
    /// - Validates sender is Subsystem 2 (Block Storage)
    /// - Validates HMAC signature
    /// - Validates nonce
    pub fn handle_storage_confirmation(
        &mut self,
        sender_id: u8,
        timestamp: u64,
        nonce: u64,
        signature: &[u8; 32],
        message_bytes: &[u8],
        confirmation: BlockStorageConfirmation,
    ) -> Result<Vec<Hash>, MempoolError> {
        let now = self.time_source.now();

        // Security validations
        validate_timestamp(timestamp, now)?;
        AuthorizationRules::validate_storage_confirmation(sender_id)?;
        validate_hmac_signature(message_bytes, signature, &self.hmac_key)?;
        validate_nonce(nonce, timestamp, now, &self.nonce_cache)?;

        // Confirm the transactions (permanently delete them)
        let confirmed = self.pool.confirm(&confirmation.included_transactions);
        Ok(confirmed)
    }

    /// Handles BlockRejectedNotification.
    ///
    /// # Security
    /// - Validates timestamp
    /// - Validates sender is Subsystem 2 or 8
    /// - Validates HMAC signature
    /// - Validates nonce
    pub fn handle_block_rejected(
        &mut self,
        sender_id: u8,
        timestamp: u64,
        nonce: u64,
        signature: &[u8; 32],
        message_bytes: &[u8],
        notification: BlockRejectedNotification,
    ) -> Result<Vec<Hash>, MempoolError> {
        let now = self.time_source.now();

        // Security validations
        validate_timestamp(timestamp, now)?;
        AuthorizationRules::validate_block_rejected(sender_id)?;
        validate_hmac_signature(message_bytes, signature, &self.hmac_key)?;
        validate_nonce(nonce, timestamp, now, &self.nonce_cache)?;

        // Rollback the transactions (return to pending)
        let rolled_back = self.pool.rollback(&notification.affected_transactions);
        Ok(rolled_back)
    }

    /// Handles RemoveTransactionsRequest.
    ///
    /// # Security
    /// - Validates timestamp
    /// - Validates sender is Subsystem 8 (Consensus)
    /// - Validates HMAC signature
    /// - Validates nonce
    pub fn handle_remove_transactions(
        &mut self,
        sender_id: u8,
        timestamp: u64,
        nonce: u64,
        signature: &[u8; 32],
        message_bytes: &[u8],
        request: RemoveTransactionsRequest,
    ) -> Result<RemoveTransactionsResponse, MempoolError> {
        let now = self.time_source.now();

        // Security validations
        validate_timestamp(timestamp, now)?;
        AuthorizationRules::validate_remove_transactions(sender_id)?;
        validate_hmac_signature(message_bytes, signature, &self.hmac_key)?;
        validate_nonce(nonce, timestamp, now, &self.nonce_cache)?;

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

    /// Handles GetStatusRequest (no security validation needed - status is public).
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

    /// Helper to create a valid HMAC signature for testing
    fn create_test_signature(message: &[u8], key: &HmacKey) -> [u8; 32] {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(key).unwrap();
        mac.update(message);
        let result = mac.finalize();
        let mut sig = [0u8; 32];
        sig.copy_from_slice(&result.into_bytes());
        sig
    }

    fn create_handler() -> IpcHandler<MockTimeSource> {
        let pool = TransactionPool::new(MempoolConfig::for_testing());
        let time_source = MockTimeSource::new(1000);
        IpcHandler::new(pool, time_source)
    }

    fn create_handler_with_key(key: HmacKey) -> IpcHandler<MockTimeSource> {
        let pool = TransactionPool::new(MempoolConfig::for_testing());
        let time_source = MockTimeSource::new(1000);
        IpcHandler::with_hmac_key(pool, time_source, key)
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
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let request = create_add_request();
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let response = handler
            .handle_add_transaction(
                subsystem_id::SIGNATURE_VERIFICATION,
                now,
                nonce,
                &signature,
                message_bytes,
                request,
            )
            .unwrap();

        assert!(response.accepted);
        assert!(response.tx_hash.is_some());
    }

    #[test]
    fn test_add_transaction_unauthorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let request = create_add_request();
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // From Consensus (wrong sender)
        let result = handler.handle_add_transaction(
            subsystem_id::CONSENSUS,
            now,
            nonce,
            &signature,
            message_bytes,
            request,
        );
        assert!(matches!(
            result,
            Err(MempoolError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_add_transaction_unverified_signature() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let mut request = create_add_request();
        request.signature_verified = false;
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let result = handler.handle_add_transaction(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
            request,
        );
        assert!(matches!(result, Err(MempoolError::SignatureNotVerified)));
    }

    #[test]
    fn test_add_transaction_invalid_hmac() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let request = create_add_request();
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let bad_signature = [0xFFu8; 32]; // Invalid signature

        let result = handler.handle_add_transaction(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &bad_signature,
            message_bytes,
            request,
        );
        assert!(matches!(result, Err(MempoolError::InvalidSignature)));
    }

    #[test]
    fn test_add_transaction_replay_attack() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // First request should succeed
        let request1 = create_add_request();
        let result1 = handler.handle_add_transaction(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
            request1,
        );
        assert!(result1.is_ok());

        // Same nonce should fail (replay attack)
        let request2 = create_add_request();
        let result2 = handler.handle_add_transaction(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
            request2,
        );
        assert!(matches!(result2, Err(MempoolError::ReplayDetected { .. })));
    }

    #[test]
    fn test_add_transaction_timestamp_too_old() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let request = create_add_request();
        let now = 1000u64;
        let old_timestamp = now - 100; // 100 seconds ago
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let result = handler.handle_add_transaction(
            subsystem_id::SIGNATURE_VERIFICATION,
            old_timestamp,
            nonce,
            &signature,
            message_bytes,
            request,
        );
        assert!(matches!(result, Err(MempoolError::TimestampTooOld { .. })));
    }

    // =========================================================================
    // GET TRANSACTIONS TESTS
    // =========================================================================

    #[test]
    fn test_get_transactions_authorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // Add a transaction first
        let add_req = create_add_request();
        handler
            .handle_add_transaction(
                subsystem_id::SIGNATURE_VERIFICATION,
                now,
                1,
                &signature,
                message_bytes,
                add_req,
            )
            .unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        let response = handler
            .handle_get_transactions(
                subsystem_id::CONSENSUS,
                now,
                2,
                &signature,
                message_bytes,
                get_req,
            )
            .unwrap();

        assert_eq!(response.tx_hashes.len(), 1);
    }

    #[test]
    fn test_get_transactions_unauthorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let request = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        // From Signature Verification (wrong sender)
        let result = handler.handle_get_transactions(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
            request,
        );
        assert!(matches!(
            result,
            Err(MempoolError::UnauthorizedSender { .. })
        ));
    }

    // =========================================================================
    // STORAGE CONFIRMATION TESTS
    // =========================================================================

    #[test]
    fn test_storage_confirmation_authorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // Add and propose a transaction
        let add_req = create_add_request();
        let response = handler
            .handle_add_transaction(
                subsystem_id::SIGNATURE_VERIFICATION,
                now,
                1,
                &signature,
                message_bytes,
                add_req,
            )
            .unwrap();
        let tx_hash = response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        handler
            .handle_get_transactions(
                subsystem_id::CONSENSUS,
                now,
                2,
                &signature,
                message_bytes,
                get_req,
            )
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
            .handle_storage_confirmation(
                subsystem_id::BLOCK_STORAGE,
                now,
                3,
                &signature,
                message_bytes,
                confirmation,
            )
            .unwrap();

        assert_eq!(confirmed, vec![tx_hash]);
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_storage_confirmation_unauthorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![],
            storage_timestamp: 2000,
        };

        // From Consensus (wrong sender)
        let result = handler.handle_storage_confirmation(
            subsystem_id::CONSENSUS,
            now,
            nonce,
            &signature,
            message_bytes,
            confirmation,
        );
        assert!(matches!(
            result,
            Err(MempoolError::UnauthorizedSender { .. })
        ));
    }

    // =========================================================================
    // BLOCK REJECTED TESTS
    // =========================================================================

    #[test]
    fn test_block_rejected_from_storage_authorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::StorageFailure,
        };

        let result = handler.handle_block_rejected(
            subsystem_id::BLOCK_STORAGE,
            now,
            nonce,
            &signature,
            message_bytes,
            notification,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_from_consensus_authorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };

        let result = handler.handle_block_rejected(
            subsystem_id::CONSENSUS,
            now,
            nonce,
            &signature,
            message_bytes,
            notification,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_unauthorized() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let nonce = 12345u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::Timeout,
        };

        // From Signature Verification (wrong sender)
        let result = handler.handle_block_rejected(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
            notification,
        );
        assert!(matches!(
            result,
            Err(MempoolError::UnauthorizedSender { .. })
        ));
    }

    // =========================================================================
    // TWO-PHASE COMMIT FLOW TEST
    // =========================================================================

    #[test]
    fn test_full_two_phase_commit_flow() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // Phase 0: Add transaction
        let add_req = create_add_request();
        let add_response = handler
            .handle_add_transaction(
                subsystem_id::SIGNATURE_VERIFICATION,
                now,
                1,
                &signature,
                message_bytes,
                add_req,
            )
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
            .handle_get_transactions(
                subsystem_id::CONSENSUS,
                now,
                2,
                &signature,
                message_bytes,
                get_req,
            )
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
            .handle_storage_confirmation(
                subsystem_id::BLOCK_STORAGE,
                now,
                3,
                &signature,
                message_bytes,
                confirmation,
            )
            .unwrap();

        // Transaction should be deleted
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_two_phase_commit_rollback_flow() {
        let hmac_key = [0u8; 32];
        let mut handler = create_handler_with_key(hmac_key);
        let now = 1000u64;
        let message_bytes = b"test message";
        let signature = create_test_signature(message_bytes, &hmac_key);

        // Add and propose
        let add_req = create_add_request();
        let add_response = handler
            .handle_add_transaction(
                subsystem_id::SIGNATURE_VERIFICATION,
                now,
                1,
                &signature,
                message_bytes,
                add_req,
            )
            .unwrap();
        let tx_hash = add_response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        handler
            .handle_get_transactions(
                subsystem_id::CONSENSUS,
                now,
                2,
                &signature,
                message_bytes,
                get_req,
            )
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
            .handle_block_rejected(
                subsystem_id::CONSENSUS,
                now,
                3,
                &signature,
                message_bytes,
                notification,
            )
            .unwrap();

        // Transaction should be back to pending
        assert!(handler.pool().get(&tx_hash).unwrap().is_pending());
    }
}
