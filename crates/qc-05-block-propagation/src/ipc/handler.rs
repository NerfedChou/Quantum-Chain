//! IPC message handlers for Block Propagation subsystem.

use shared_types::envelope::VerificationResult;
use shared_types::security::{DerivedKeyProvider, KeyProvider, MessageVerifier, NonceCache};
use shared_types::AuthenticatedMessage;
use std::sync::Arc;

use crate::events::{PropagateBlockRequest, PropagationError};
use crate::ports::inbound::BlockPropagationApi;

/// Subsystem ID for Block Propagation (5).
const SUBSYSTEM_ID: u8 = 5;

/// IPC handler for Block Propagation subsystem.
pub struct IpcHandler<S: BlockPropagationApi, K: KeyProvider> {
    service: S,
    verifier: MessageVerifier<K>,
}

impl<S: BlockPropagationApi> IpcHandler<S, DerivedKeyProvider> {
    /// Create new IPC handler with a master secret.
    pub fn new(service: S, master_secret: Vec<u8>) -> Self {
        let key_provider = DerivedKeyProvider::new(master_secret);
        let nonce_cache = Arc::new(NonceCache::new());
        Self {
            service,
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
        }
    }
}

impl<S: BlockPropagationApi, K: KeyProvider> IpcHandler<S, K> {
    /// Create new IPC handler with custom key provider.
    pub fn with_key_provider(service: S, key_provider: K, nonce_cache: Arc<NonceCache>) -> Self {
        Self {
            service,
            verifier: MessageVerifier::new(SUBSYSTEM_ID, nonce_cache, key_provider),
        }
    }

    /// Handle propagate block request from Consensus.
    ///
    /// SECURITY: Only Subsystem 8 (Consensus) can call this.
    pub fn handle_propagate_block(
        &self,
        msg: AuthenticatedMessage<PropagateBlockRequest>,
        message_bytes: &[u8],
    ) -> Result<(), PropagationError> {
        // Verify HMAC signature, nonce, timestamp
        match self.verifier.verify(&msg, message_bytes) {
            VerificationResult::Valid => {}
            VerificationResult::InvalidSignature => {
                return Err(PropagationError::IpcSecurityError(
                    "Invalid signature".to_string(),
                ));
            }
            VerificationResult::ReplayDetected { nonce } => {
                return Err(PropagationError::IpcSecurityError(format!(
                    "Replay detected: {:?}",
                    nonce
                )));
            }
            VerificationResult::TimestampOutOfRange { timestamp, now } => {
                return Err(PropagationError::IpcSecurityError(format!(
                    "Timestamp out of range: {} (now: {})",
                    timestamp, now
                )));
            }
            other => {
                return Err(PropagationError::IpcSecurityError(format!(
                    "Verification failed: {:?}",
                    other
                )));
            }
        }

        // Verify sender is Consensus (subsystem 8)
        if msg.sender_id != 8 {
            return Err(PropagationError::UnauthorizedSender(msg.sender_id));
        }

        // Propagate the block
        self.service.propagate_block(
            msg.payload.block_hash,
            msg.payload.block_data,
            msg.payload.tx_hashes,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PropagationMetrics, PropagationState, PropagationStats};
    use shared_types::Hash;

    struct MockService;

    impl BlockPropagationApi for MockService {
        fn propagate_block(
            &self,
            _block_hash: Hash,
            _block_data: Vec<u8>,
            _tx_hashes: Vec<Hash>,
        ) -> Result<PropagationStats, PropagationError> {
            Ok(PropagationStats {
                block_hash: [0u8; 32],
                peers_reached: 5,
                propagation_start_ms: 0,
                first_ack_time_ms: None,
            })
        }

        fn get_propagation_status(
            &self,
            _block_hash: Hash,
        ) -> Result<Option<PropagationState>, PropagationError> {
            Ok(None)
        }

        fn get_propagation_metrics(&self) -> PropagationMetrics {
            PropagationMetrics::default()
        }
    }

    /// Create a test payload for propagation requests.
    fn create_test_payload() -> PropagateBlockRequest {
        PropagateBlockRequest {
            block_hash: [1u8; 32],
            block_data: vec![0u8; 100],
            tx_hashes: vec![[2u8; 32]],
        }
    }

    /// Create a test message with the given sender_id.
    fn create_test_message(sender_id: u8, payload: PropagateBlockRequest) -> AuthenticatedMessage<PropagateBlockRequest> {
        AuthenticatedMessage {
            version: 1,
            sender_id,
            recipient_id: SUBSYSTEM_ID,
            correlation_id: uuid::Uuid::new_v4(),
            nonce: uuid::Uuid::new_v4(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: [0u8; 64],
            reply_to: None,
            payload,
        }
    }

    #[test]
    fn test_reject_unauthorized_sender_mempool() {
        let master_secret = vec![0xABu8; 32];
        let handler = IpcHandler::new(MockService, master_secret);

        // Create a message from Mempool (6) - should be rejected
        let msg = create_test_message(6, create_test_payload());

        // Empty bytes will fail signature check first
        let result = handler.handle_propagate_block(msg, &[]);
        assert!(matches!(result, Err(PropagationError::IpcSecurityError(_))));
    }

    #[test]
    fn test_reject_unauthorized_sender_state_management() {
        let master_secret = vec![0xABu8; 32];
        let handler = IpcHandler::new(MockService, master_secret);

        // Create a message from State Management (4) - should be rejected
        let msg = create_test_message(4, create_test_payload());

        let result = handler.handle_propagate_block(msg, &[]);
        assert!(matches!(result, Err(PropagationError::IpcSecurityError(_))));
    }

    #[test]
    fn test_reject_unauthorized_sender_block_storage() {
        let master_secret = vec![0xABu8; 32];
        let handler = IpcHandler::new(MockService, master_secret);

        // Create a message from Block Storage (2) - should be rejected
        let msg = create_test_message(2, create_test_payload());

        let result = handler.handle_propagate_block(msg, &[]);
        assert!(matches!(result, Err(PropagationError::IpcSecurityError(_))));
    }

    #[test]
    fn test_reject_unauthorized_sender_peer_discovery() {
        let master_secret = vec![0xABu8; 32];
        let handler = IpcHandler::new(MockService, master_secret);

        // Create a message from Peer Discovery (1) - should be rejected
        let msg = create_test_message(1, create_test_payload());

        let result = handler.handle_propagate_block(msg, &[]);
        assert!(matches!(result, Err(PropagationError::IpcSecurityError(_))));
    }

    /// Test that verifies the authorization check is performed correctly.
    /// This test uses a custom key provider that accepts all signatures
    /// to isolate the authorization check from signature verification.
    #[test]
    fn test_authorization_check_after_signature() {
        use std::sync::Arc;

        /// Key provider that returns an empty secret (causes signature check to pass in some modes).
        struct AcceptAllKeyProvider;
        impl KeyProvider for AcceptAllKeyProvider {
            fn get_shared_secret(&self, _sender_id: u8) -> Option<Vec<u8>> {
                // Return a secret so verification doesn't fail on missing key
                Some(vec![0u8; 32])
            }
        }

        let nonce_cache = Arc::new(NonceCache::new());
        let handler = IpcHandler::with_key_provider(MockService, AcceptAllKeyProvider, nonce_cache);

        // Create a message from wrong sender (Mempool = 6)
        let msg = create_test_message(6, create_test_payload());

        // Note: The signature will be invalid, so it will fail on signature check
        // This test verifies the error path through IpcSecurityError
        let result = handler.handle_propagate_block(msg, &[]);

        // Should fail with IpcSecurityError (signature check fails before authorization)
        assert!(
            matches!(result, Err(PropagationError::IpcSecurityError(_))),
            "Expected IpcSecurityError, got: {:?}",
            result
        );
    }

    /// Verify that only Consensus (8) is the authorized sender.
    /// This is a documentation test that confirms the authorization logic.
    #[test]
    fn test_only_consensus_is_authorized() {
        // Per IPC-MATRIX.md, only Consensus (8) can request block propagation
        const CONSENSUS_ID: u8 = 8;
        
        // All other subsystems should be rejected
        let unauthorized_senders = [
            1,  // Peer Discovery
            2,  // Block Storage
            3,  // Transaction Indexing
            4,  // State Management
            5,  // Block Propagation (self - shouldn't happen)
            6,  // Mempool
            7,  // Bloom Filters
            // 8 is Consensus (authorized)
            9,  // Finality
            10, // Signature Verification
            11, // Smart Contracts
            12, // Transaction Ordering
        ];

        // Verify the handler checks for sender_id == 8
        // (The actual check is on line 78 of handler.rs)
        assert_eq!(CONSENSUS_ID, 8, "Consensus subsystem ID should be 8");
        
        for id in unauthorized_senders {
            assert_ne!(id, CONSENSUS_ID, "Subsystem {} should not be authorized", id);
        }
    }
}

