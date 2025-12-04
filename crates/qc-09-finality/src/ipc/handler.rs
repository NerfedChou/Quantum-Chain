//! IPC Handler for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 6, IPC-MATRIX.md

use crate::error::{FinalityError, FinalityResult, SubsystemId};
use crate::events::incoming::{AttestationBatch, FinalityCheckRequest, FinalityProofRequest};
use crate::ports::inbound::FinalityApi;
use shared_types::envelope::AuthenticatedMessage;
use shared_types::security::{validate_hmac_signature, validate_timestamp, NonceCache};
use std::sync::Arc;

/// Authorized senders per IPC-MATRIX.md
const CONSENSUS_SUBSYSTEM: SubsystemId = 8;
const CROSS_CHAIN_SUBSYSTEM: SubsystemId = 15;

/// IPC Handler for Finality subsystem
///
/// Reference: IPC-MATRIX.md Subsystem 9 Security Boundaries
///
/// Authorized senders:
/// - AttestationBatch: Consensus (8) ONLY
/// - FinalityCheckRequest: Consensus (8) ONLY
/// - FinalityProofRequest: Cross-Chain (15) ONLY
pub struct FinalityIpcHandler<F>
where
    F: FinalityApi,
{
    finality_service: Arc<F>,
    shared_secret: [u8; 32],
    nonce_cache: Arc<NonceCache>,
}

impl<F> FinalityIpcHandler<F>
where
    F: FinalityApi,
{
    /// Create new IPC handler with shared secret
    pub fn new(finality_service: Arc<F>, shared_secret: [u8; 32]) -> Self {
        Self {
            finality_service,
            shared_secret,
            nonce_cache: NonceCache::new_shared(),
        }
    }

    /// Verify message security (HMAC, timestamp, nonce)
    fn verify_message<T>(
        &self,
        message: &AuthenticatedMessage<T>,
        message_bytes: &[u8],
    ) -> FinalityResult<()> {
        // 1. Verify timestamp
        validate_timestamp(message.timestamp).map_err(|_| FinalityError::IpcSecurityViolation {
            reason: "Timestamp out of range".to_string(),
        })?;

        // 2. Check nonce for replay
        if !self.nonce_cache.check_and_insert(message.nonce) {
            return Err(FinalityError::IpcSecurityViolation {
                reason: "Replay detected - nonce already used".to_string(),
            });
        }

        // 3. Verify HMAC signature
        if !validate_hmac_signature(message_bytes, &message.signature, &self.shared_secret) {
            return Err(FinalityError::IpcSecurityViolation {
                reason: "HMAC verification failed".to_string(),
            });
        }

        Ok(())
    }

    /// Handle attestation batch from Consensus
    ///
    /// SECURITY: Sender MUST be Consensus (8)
    /// Reference: IPC-MATRIX.md Subsystem 9
    pub async fn handle_attestation_batch(
        &self,
        message: AuthenticatedMessage<AttestationBatch>,
        message_bytes: &[u8],
    ) -> FinalityResult<()> {
        // 1. Verify message security
        self.verify_message(&message, message_bytes)?;

        // 2. Verify sender is Consensus
        if message.sender_id != CONSENSUS_SUBSYSTEM {
            return Err(FinalityError::UnauthorizedSender {
                sender_id: message.sender_id,
            });
        }

        // 3. Process attestations
        let batch = message.payload;
        let _result = self
            .finality_service
            .process_attestations(batch.attestations)
            .await?;

        Ok(())
    }

    /// Handle finality check request from Consensus
    ///
    /// SECURITY: Sender MUST be Consensus (8)
    pub async fn handle_finality_check(
        &self,
        message: AuthenticatedMessage<FinalityCheckRequest>,
        message_bytes: &[u8],
    ) -> FinalityResult<bool> {
        // 1. Verify message security
        self.verify_message(&message, message_bytes)?;

        // 2. Verify sender is Consensus
        if message.sender_id != CONSENSUS_SUBSYSTEM {
            return Err(FinalityError::UnauthorizedSender {
                sender_id: message.sender_id,
            });
        }

        // 3. Check finality
        Ok(self
            .finality_service
            .is_finalized(message.payload.block_hash)
            .await)
    }

    /// Handle finality proof request from Cross-Chain
    ///
    /// SECURITY: Sender MUST be Cross-Chain (15)
    pub async fn handle_finality_proof_request(
        &self,
        message: AuthenticatedMessage<FinalityProofRequest>,
        message_bytes: &[u8],
    ) -> FinalityResult<Option<crate::domain::Checkpoint>> {
        // 1. Verify message security
        self.verify_message(&message, message_bytes)?;

        // 2. Verify sender is Cross-Chain
        if message.sender_id != CROSS_CHAIN_SUBSYSTEM {
            return Err(FinalityError::UnauthorizedSender {
                sender_id: message.sender_id,
            });
        }

        // 3. Get finality proof
        let is_finalized = self
            .finality_service
            .is_finalized(message.payload.block_hash)
            .await;

        if is_finalized {
            Ok(self.finality_service.get_last_finalized().await)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Attestation, Checkpoint, FinalityState};
    use crate::ports::inbound::{AttestationResult, SlashableOffenseInfo};
    use async_trait::async_trait;
    use shared_types::security::sign_message;
    use shared_types::Hash;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    // Mock FinalityApi for testing
    struct MockFinalityApi;

    #[async_trait]
    impl FinalityApi for MockFinalityApi {
        async fn process_attestations(
            &self,
            _attestations: Vec<Attestation>,
        ) -> FinalityResult<AttestationResult> {
            Ok(AttestationResult::empty())
        }

        async fn is_finalized(&self, _block_hash: Hash) -> bool {
            true
        }

        async fn get_last_finalized(&self) -> Option<Checkpoint> {
            None
        }

        async fn get_state(&self) -> FinalityState {
            FinalityState::Running
        }

        async fn reset_from_halted(&self) -> FinalityResult<()> {
            Ok(())
        }

        async fn get_finality_lag(&self) -> u64 {
            0
        }

        async fn get_current_epoch(&self) -> u64 {
            1
        }

        async fn get_checkpoint(&self, _epoch: u64) -> Option<Checkpoint> {
            None
        }

        async fn get_epochs_without_finality(&self) -> u64 {
            0
        }

        async fn is_inactivity_leak_active(&self) -> bool {
            false
        }

        async fn get_slashable_offenses(&self) -> Vec<SlashableOffenseInfo> {
            Vec::new()
        }

        async fn take_pending_slashing_events(
            &self,
        ) -> Vec<crate::events::outgoing::SlashableOffenseDetectedEvent> {
            Vec::new()
        }

        async fn take_pending_inactivity_events(
            &self,
        ) -> Vec<crate::events::outgoing::InactivityLeakTriggeredEvent> {
            Vec::new()
        }
    }

    fn create_test_handler() -> FinalityIpcHandler<MockFinalityApi> {
        FinalityIpcHandler::new(Arc::new(MockFinalityApi), [1u8; 32])
    }

    fn create_authenticated_message<T>(
        payload: T,
        sender_id: u8,
        secret: &[u8; 32],
    ) -> (AuthenticatedMessage<T>, Vec<u8>)
    where
        T: serde::Serialize,
    {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = Uuid::new_v4();

        // Create message without signature first
        let mut message = AuthenticatedMessage {
            version: 1,
            sender_id,
            recipient_id: 9, // Finality subsystem
            correlation_id: Uuid::new_v4(),
            reply_to: None,
            timestamp,
            nonce,
            signature: [0u8; 64],
            payload,
        };

        // Serialize for signing (with zero signature)
        let message_bytes = bincode::serialize(&message).unwrap();

        // Sign
        message.signature = sign_message(&message_bytes, secret);

        // Return the message and the ORIGINAL bytes (before signature) for verification
        (message, message_bytes)
    }

    #[tokio::test]
    async fn test_attestation_batch_wrong_sender() {
        let handler = create_test_handler();

        let batch = AttestationBatch::new(vec![], 1, 32);
        let (message, bytes) = create_authenticated_message(batch, 7, &[1u8; 32]); // Wrong sender

        let result = handler.handle_attestation_batch(message, &bytes).await;
        assert!(matches!(
            result,
            Err(FinalityError::UnauthorizedSender { .. })
        ));
    }

    #[tokio::test]
    async fn test_attestation_batch_correct_sender() {
        let handler = create_test_handler();

        let batch = AttestationBatch::new(vec![], 1, 32);
        let (message, bytes) = create_authenticated_message(batch, CONSENSUS_SUBSYSTEM, &[1u8; 32]);

        let result = handler.handle_attestation_batch(message, &bytes).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_finality_check_wrong_sender() {
        let handler = create_test_handler();

        let request = FinalityCheckRequest {
            block_hash: [0u8; 32],
            block_height: 100,
        };
        let (message, bytes) = create_authenticated_message(request, 7, &[1u8; 32]);

        let result = handler.handle_finality_check(message, &bytes).await;
        assert!(matches!(
            result,
            Err(FinalityError::UnauthorizedSender { .. })
        ));
    }

    #[tokio::test]
    async fn test_finality_proof_wrong_sender() {
        let handler = create_test_handler();

        let request = FinalityProofRequest {
            block_hash: [0u8; 32],
            block_height: 100,
        };
        let (message, bytes) = create_authenticated_message(request, 8, &[1u8; 32]); // Wrong sender

        let result = handler.handle_finality_proof_request(message, &bytes).await;
        assert!(matches!(
            result,
            Err(FinalityError::UnauthorizedSender { .. })
        ));
    }

    #[tokio::test]
    async fn test_finality_proof_correct_sender() {
        let handler = create_test_handler();

        let request = FinalityProofRequest {
            block_hash: [0u8; 32],
            block_height: 100,
        };
        let (message, bytes) =
            create_authenticated_message(request, CROSS_CHAIN_SUBSYSTEM, &[1u8; 32]);

        let result = handler.handle_finality_proof_request(message, &bytes).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_hmac_rejected() {
        let handler = create_test_handler();

        let batch = AttestationBatch::new(vec![], 1, 32);
        let (message, bytes) = create_authenticated_message(batch, CONSENSUS_SUBSYSTEM, &[2u8; 32]); // Wrong secret

        let result = handler.handle_attestation_batch(message, &bytes).await;
        assert!(matches!(
            result,
            Err(FinalityError::IpcSecurityViolation { .. })
        ));
    }
}
