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

    #[test]
    fn test_reject_unauthorized_sender() {
        let master_secret = vec![0xABu8; 32];
        let handler = IpcHandler::new(MockService, master_secret.clone());

        let payload = PropagateBlockRequest {
            block_hash: [1u8; 32],
            block_data: vec![0u8; 100],
            tx_hashes: vec![[2u8; 32]],
        };

        // Create a mock message from wrong sender (Mempool = 6)
        let msg = AuthenticatedMessage {
            version: 1,
            sender_id: 6, // Wrong sender
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
        };

        // Use empty bytes for signature check (will fail signature first)
        let result = handler.handle_propagate_block(msg, &[]);
        // Should fail on signature before reaching authorization check
        assert!(matches!(result, Err(PropagationError::IpcSecurityError(_))));
    }
}
