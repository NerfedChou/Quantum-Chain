//! # IPC Message Handler - Two-Phase Commit Coordinator
//!
//! Processes incoming IPC messages with security validation per IPC-MATRIX.md v2.3.
//!
//! ## Security Validation Order
//!
//! 1. Sender authorization (per-message type)
//! 2. Timestamp bounds (±30s from now)
//! 3. HMAC signature verification
//! 4. Nonce uniqueness (replay prevention)
//!
//! ## Two-Phase Commit Flow
//!
//! ```text
//! Consensus → GetTransactionsRequest → [propose] → PENDING_INCLUSION
//! Storage → BlockStorageConfirmation → [confirm] → DELETED
//! Storage/Consensus → BlockRejectedNotification → [rollback] → PENDING
//! ```

use crate::domain::{Hash, MempoolError, MempoolTransaction, TransactionPool};
use crate::ipc::payloads::*;
use crate::ipc::security::AuthorizationRules;
use crate::ports::TimeSource;
use shared_types::security::{DerivedKeyProvider, KeyProvider, NonceCache};
use std::sync::Arc;
use uuid::Uuid;

/// Security context for IPC message validation.
///
/// Groups the common security parameters required for all IPC handlers,
/// reducing function argument count from 7 to 2 (context + request).
///
/// # Architecture
/// Per IPC-MATRIX.md v2.3, all IPC messages require:
/// - Sender ID validation (per message type)
/// - Timestamp bounds checking
/// - HMAC signature verification
/// - Nonce replay prevention
#[derive(Debug)]
pub struct IpcSecurityContext<'a> {
    /// Subsystem ID of the message sender (from envelope)
    pub sender_id: u8,
    /// Message timestamp (for freshness validation)
    pub timestamp: u64,
    /// Unique message nonce (for replay prevention)
    pub nonce: Uuid,
    /// HMAC signature over message bytes
    pub signature: &'a [u8; 64],
    /// Raw message bytes (for signature verification)
    pub message_bytes: &'a [u8],
}

impl<'a> IpcSecurityContext<'a> {
    /// Creates a new security context from IPC message envelope fields.
    #[inline]
    pub fn new(
        sender_id: u8,
        timestamp: u64,
        nonce: Uuid,
        signature: &'a [u8; 64],
        message_bytes: &'a [u8],
    ) -> Self {
        Self {
            sender_id,
            timestamp,
            nonce,
            signature,
            message_bytes,
        }
    }
}

/// IPC message handler for the Mempool.
///
/// Uses the centralized security module from `shared-types` for all
/// security validation (HMAC, nonce, timestamp).
pub struct IpcHandler<T: TimeSource> {
    pool: TransactionPool,
    time_source: T,
    nonce_cache: Arc<NonceCache>,
    key_provider: DerivedKeyProvider,
}

impl<T: TimeSource> IpcHandler<T> {
    /// Creates a new IPC handler with default master secret.
    ///
    /// # Security Warning
    ///
    /// The default master secret is for development/testing only.
    /// Production deployments MUST use `with_master_secret()`.
    pub fn new(pool: TransactionPool, time_source: T) -> Self {
        Self {
            pool,
            time_source,
            nonce_cache: NonceCache::new_shared(),
            key_provider: DerivedKeyProvider::new(vec![0u8; 32]),
        }
    }

    /// Creates a new IPC handler with custom master secret.
    ///
    /// The master secret is used to derive per-subsystem HMAC keys.
    pub fn with_master_secret(
        pool: TransactionPool,
        time_source: T,
        master_secret: Vec<u8>,
    ) -> Self {
        Self {
            pool,
            time_source,
            nonce_cache: NonceCache::new_shared(),
            key_provider: DerivedKeyProvider::new(master_secret),
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

    /// Validates security for an incoming IPC message.
    ///
    /// Uses the centralized security module from `shared-types`.
    fn validate_security(&self, ctx: &IpcSecurityContext<'_>) -> Result<(), MempoolError> {
        let now = self.time_source.now();

        // Step 1: Validate timestamp
        self.validate_timestamp(ctx.timestamp, now)?;

        // Step 2: Validate HMAC signature using centralized module
        let shared_secret = self
            .key_provider
            .get_shared_secret(ctx.sender_id)
            .ok_or(MempoolError::InvalidSignature)?;

        if !shared_types::security::validate_hmac_signature(
            ctx.message_bytes,
            ctx.signature,
            &shared_secret,
        ) {
            return Err(MempoolError::InvalidSignature);
        }

        // Step 3: Validate nonce using centralized NonceCache
        if !self.nonce_cache.check_and_insert(ctx.nonce) {
            return Err(MempoolError::ReplayDetected {
                nonce: ctx.nonce.as_u128() as u64,
            });
        }

        Ok(())
    }

    /// Validates timestamp is within acceptable bounds.
    fn validate_timestamp(&self, msg_timestamp: u64, now: u64) -> Result<(), MempoolError> {
        let max_age = shared_types::security::MAX_AGE;
        let max_future = shared_types::security::MAX_FUTURE_SKEW;

        if msg_timestamp > now + max_future {
            return Err(MempoolError::TimestampTooFuture {
                timestamp: msg_timestamp,
                now,
            });
        }
        if now > msg_timestamp && now - msg_timestamp > max_age {
            return Err(MempoolError::TimestampTooOld {
                timestamp: msg_timestamp,
                now,
            });
        }
        Ok(())
    }

    /// Handles AddTransactionRequest.
    ///
    /// # Security
    /// - Validates sender is Subsystem 10 (Signature Verification)
    /// - Validates timestamp, HMAC signature, nonce (via centralized module)
    /// - Validates signature_verified is true
    pub fn handle_add_transaction(
        &mut self,
        ctx: &IpcSecurityContext<'_>,
        request: AddTransactionRequest,
    ) -> Result<AddTransactionResponse, MempoolError> {
        // Security Step 1: Validate sender authorization
        AuthorizationRules::validate_add_transaction(ctx.sender_id)?;

        // Security Step 2-4: Validate timestamp, signature, nonce
        self.validate_security(ctx)?;

        // Security Step 5: Validate signature was verified
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
    /// - Validates timestamp, HMAC signature, nonce
    pub fn handle_get_transactions(
        &mut self,
        ctx: &IpcSecurityContext<'_>,
        request: GetTransactionsRequest,
    ) -> Result<GetTransactionsResponse, MempoolError> {
        // Security validations
        AuthorizationRules::validate_get_transactions(ctx.sender_id)?;
        self.validate_security(ctx)?;

        let txs = self
            .pool
            .get_for_block(request.max_count as usize, request.max_gas);

        let tx_hashes: Vec<Hash> = txs.iter().map(|t| t.hash).collect();
        let total_gas: u64 = txs.iter().map(|t| t.gas_limit).sum();

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
    /// - Validates timestamp, HMAC signature, nonce
    pub fn handle_storage_confirmation(
        &mut self,
        ctx: &IpcSecurityContext<'_>,
        confirmation: BlockStorageConfirmation,
    ) -> Result<Vec<Hash>, MempoolError> {
        // Security validations
        AuthorizationRules::validate_storage_confirmation(ctx.sender_id)?;
        self.validate_security(ctx)?;

        // Confirm the transactions (permanently delete them)
        let confirmed = self.pool.confirm(&confirmation.included_transactions);
        Ok(confirmed)
    }

    /// Handles BlockRejectedNotification.
    ///
    /// # Security
    /// - Validates sender is Subsystem 2 or 8
    /// - Validates timestamp, HMAC signature, nonce
    pub fn handle_block_rejected(
        &mut self,
        ctx: &IpcSecurityContext<'_>,
        notification: BlockRejectedNotification,
    ) -> Result<Vec<Hash>, MempoolError> {
        // Security validations
        AuthorizationRules::validate_block_rejected(ctx.sender_id)?;
        self.validate_security(ctx)?;

        // Rollback the transactions (return to pending)
        let rolled_back = self.pool.rollback(&notification.affected_transactions);
        Ok(rolled_back)
    }

    /// Handles RemoveTransactionsRequest.
    ///
    /// # Security
    /// - Validates sender is Subsystem 8 (Consensus)
    /// - Validates timestamp, HMAC signature, nonce
    pub fn handle_remove_transactions(
        &mut self,
        ctx: &IpcSecurityContext<'_>,
        request: RemoveTransactionsRequest,
    ) -> Result<RemoveTransactionsResponse, MempoolError> {
        // Security validations
        AuthorizationRules::validate_remove_transactions(ctx.sender_id)?;
        self.validate_security(ctx)?;

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

    /// Helper to create a valid HMAC signature for testing using centralized module
    fn create_test_signature(message: &[u8], sender_id: u8, master_secret: &[u8]) -> [u8; 64] {
        let key_provider = DerivedKeyProvider::new(master_secret.to_vec());
        let shared_secret = key_provider.get_shared_secret(sender_id).unwrap();
        shared_types::security::sign_message(message, &shared_secret)
    }

    fn create_handler_with_secret(secret: Vec<u8>) -> IpcHandler<MockTimeSource> {
        let pool = TransactionPool::new(MempoolConfig::for_testing());
        let time_source = MockTimeSource::new(1000);
        IpcHandler::with_master_secret(pool, time_source, secret)
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

    /// Helper to create security context for tests
    fn create_test_ctx<'a>(
        sender_id: u8,
        timestamp: u64,
        nonce: Uuid,
        signature: &'a [u8; 64],
        message_bytes: &'a [u8],
    ) -> IpcSecurityContext<'a> {
        IpcSecurityContext::new(sender_id, timestamp, nonce, signature, message_bytes)
    }

    // =========================================================================
    // ADD TRANSACTION TESTS
    // =========================================================================

    #[test]
    fn test_add_transaction_authorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let request = create_add_request();
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );

        let response = handler.handle_add_transaction(&ctx, request).unwrap();

        assert!(response.accepted);
        assert!(response.tx_hash.is_some());
    }

    #[test]
    fn test_add_transaction_unauthorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let request = create_add_request();
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature =
            create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let ctx = create_test_ctx(subsystem_id::CONSENSUS, now, nonce, &signature, message_bytes);

        // From Consensus (wrong sender)
        let result = handler.handle_add_transaction(&ctx, request);
        assert!(matches!(
            result,
            Err(MempoolError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_add_transaction_unverified_signature() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let mut request = create_add_request();
        request.signature_verified = false;
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );

        let result = handler.handle_add_transaction(&ctx, request);
        assert!(matches!(result, Err(MempoolError::SignatureNotVerified)));
    }

    #[test]
    fn test_add_transaction_invalid_hmac() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let request = create_add_request();
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let bad_signature = [0xFFu8; 64]; // Invalid signature
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &bad_signature,
            message_bytes,
        );

        let result = handler.handle_add_transaction(&ctx, request);
        assert!(matches!(result, Err(MempoolError::InvalidSignature)));
    }

    #[test]
    fn test_add_transaction_replay_attack() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4(); // Same nonce for both requests
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );

        // First request should succeed
        let request1 = create_add_request();
        let ctx1 = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );
        let result1 = handler.handle_add_transaction(&ctx1, request1);
        assert!(result1.is_ok());

        // Same nonce should fail (replay attack)
        let request2 = create_add_request();
        let ctx2 = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );
        let result2 = handler.handle_add_transaction(&ctx2, request2);
        assert!(matches!(result2, Err(MempoolError::ReplayDetected { .. })));
    }

    #[test]
    fn test_add_transaction_timestamp_too_old() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let request = create_add_request();
        let now = 1000u64;
        let old_timestamp = now - 100; // 100 seconds ago
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            old_timestamp,
            nonce,
            &signature,
            message_bytes,
        );

        let result = handler.handle_add_transaction(&ctx, request);
        assert!(matches!(result, Err(MempoolError::TimestampTooOld { .. })));
    }

    // =========================================================================
    // GET TRANSACTIONS TESTS
    // =========================================================================

    #[test]
    fn test_get_transactions_authorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let message_bytes = b"test message";

        // Add a transaction first
        let add_req = create_add_request();
        let add_sig = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let add_ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            Uuid::new_v4(),
            &add_sig,
            message_bytes,
        );
        handler.handle_add_transaction(&add_ctx, add_req).unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        let get_sig = create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let get_ctx = create_test_ctx(
            subsystem_id::CONSENSUS,
            now,
            Uuid::new_v4(),
            &get_sig,
            message_bytes,
        );
        let response = handler.handle_get_transactions(&get_ctx, get_req).unwrap();

        assert_eq!(response.tx_hashes.len(), 1);
    }

    #[test]
    fn test_get_transactions_unauthorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );

        let request = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };

        // From Signature Verification (wrong sender)
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );
        let result = handler.handle_get_transactions(&ctx, request);
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
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let message_bytes = b"test message";

        // Add and propose a transaction
        let add_req = create_add_request();
        let add_sig = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let add_ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            Uuid::new_v4(),
            &add_sig,
            message_bytes,
        );
        let response = handler.handle_add_transaction(&add_ctx, add_req).unwrap();
        let tx_hash = response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        let get_sig = create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let get_ctx = create_test_ctx(
            subsystem_id::CONSENSUS,
            now,
            Uuid::new_v4(),
            &get_sig,
            message_bytes,
        );
        handler.handle_get_transactions(&get_ctx, get_req).unwrap();

        // Confirm storage
        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![tx_hash],
            storage_timestamp: 2000,
        };

        let confirm_sig =
            create_test_signature(message_bytes, subsystem_id::BLOCK_STORAGE, &master_secret);
        let confirm_ctx = create_test_ctx(
            subsystem_id::BLOCK_STORAGE,
            now,
            Uuid::new_v4(),
            &confirm_sig,
            message_bytes,
        );
        let confirmed = handler
            .handle_storage_confirmation(&confirm_ctx, confirmation)
            .unwrap();

        assert_eq!(confirmed, vec![tx_hash]);
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_storage_confirmation_unauthorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature =
            create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);

        let confirmation = BlockStorageConfirmation {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            included_transactions: vec![],
            storage_timestamp: 2000,
        };

        // From Consensus (wrong sender)
        let ctx = create_test_ctx(subsystem_id::CONSENSUS, now, nonce, &signature, message_bytes);
        let result = handler.handle_storage_confirmation(&ctx, confirmation);
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
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature =
            create_test_signature(message_bytes, subsystem_id::BLOCK_STORAGE, &master_secret);

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::StorageFailure,
        };

        let ctx =
            create_test_ctx(subsystem_id::BLOCK_STORAGE, now, nonce, &signature, message_bytes);
        let result = handler.handle_block_rejected(&ctx, notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_from_consensus_authorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature =
            create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };

        let ctx = create_test_ctx(subsystem_id::CONSENSUS, now, nonce, &signature, message_bytes);
        let result = handler.handle_block_rejected(&ctx, notification);
        assert!(result.is_ok());
    }

    #[test]
    fn test_block_rejected_unauthorized() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let nonce = Uuid::new_v4();
        let message_bytes = b"test message";
        let signature = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );

        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![],
            rejection_reason: BlockRejectionReason::Timeout,
        };

        // From Signature Verification (wrong sender)
        let ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            nonce,
            &signature,
            message_bytes,
        );
        let result = handler.handle_block_rejected(&ctx, notification);
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
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let message_bytes = b"test message";

        // Phase 0: Add transaction
        let add_req = create_add_request();
        let add_sig = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let add_ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            Uuid::new_v4(),
            &add_sig,
            message_bytes,
        );
        let add_response = handler.handle_add_transaction(&add_ctx, add_req).unwrap();
        let tx_hash = add_response.tx_hash.unwrap();

        assert!(handler.pool().get(&tx_hash).unwrap().is_pending());

        // Phase 1: Get transactions (proposes them)
        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        let get_sig = create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let get_ctx = create_test_ctx(
            subsystem_id::CONSENSUS,
            now,
            Uuid::new_v4(),
            &get_sig,
            message_bytes,
        );
        let response = handler.handle_get_transactions(&get_ctx, get_req).unwrap();

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
        let confirm_sig =
            create_test_signature(message_bytes, subsystem_id::BLOCK_STORAGE, &master_secret);
        let confirm_ctx = create_test_ctx(
            subsystem_id::BLOCK_STORAGE,
            now,
            Uuid::new_v4(),
            &confirm_sig,
            message_bytes,
        );
        handler
            .handle_storage_confirmation(&confirm_ctx, confirmation)
            .unwrap();

        // Transaction should be deleted
        assert!(!handler.pool().contains(&tx_hash));
    }

    #[test]
    fn test_two_phase_commit_rollback_flow() {
        let master_secret = vec![0u8; 32];
        let mut handler = create_handler_with_secret(master_secret.clone());
        let now = 1000u64;
        let message_bytes = b"test message";

        // Add and propose
        let add_req = create_add_request();
        let add_sig = create_test_signature(
            message_bytes,
            subsystem_id::SIGNATURE_VERIFICATION,
            &master_secret,
        );
        let add_ctx = create_test_ctx(
            subsystem_id::SIGNATURE_VERIFICATION,
            now,
            Uuid::new_v4(),
            &add_sig,
            message_bytes,
        );
        let add_response = handler.handle_add_transaction(&add_ctx, add_req).unwrap();
        let tx_hash = add_response.tx_hash.unwrap();

        let get_req = GetTransactionsRequest {
            correlation_id: Uuid::new_v4(),
            max_count: 100,
            max_gas: 1_000_000,
            target_block_height: 1,
        };
        let get_sig = create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let get_ctx = create_test_ctx(
            subsystem_id::CONSENSUS,
            now,
            Uuid::new_v4(),
            &get_sig,
            message_bytes,
        );
        handler.handle_get_transactions(&get_ctx, get_req).unwrap();

        assert!(handler.pool().get(&tx_hash).unwrap().is_pending_inclusion());

        // Phase 2b: Block rejected
        let notification = BlockRejectedNotification {
            correlation_id: Uuid::new_v4(),
            block_hash: [0xCC; 32],
            block_height: 1,
            affected_transactions: vec![tx_hash],
            rejection_reason: BlockRejectionReason::ConsensusRejected,
        };
        let reject_sig =
            create_test_signature(message_bytes, subsystem_id::CONSENSUS, &master_secret);
        let reject_ctx = create_test_ctx(
            subsystem_id::CONSENSUS,
            now,
            Uuid::new_v4(),
            &reject_sig,
            message_bytes,
        );
        handler
            .handle_block_rejected(&reject_ctx, notification)
            .unwrap();

        // Transaction should be back to pending
        assert!(handler.pool().get(&tx_hash).unwrap().is_pending());
    }
}
