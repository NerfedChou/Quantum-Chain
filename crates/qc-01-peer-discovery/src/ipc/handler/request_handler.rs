//! IPC Message Handler logic.

use std::collections::HashMap;
use std::sync::Arc;

use crate::ipc::payloads::{
    FullNodeListRequestPayload, PeerFilter, PeerListRequestPayload, PeerListResponsePayload,
};
use crate::ipc::security::{AuthorizationRules, SecurityError, SubsystemId};
use crate::ports::PeerDiscoveryApi;

use shared_types::security::{KeyProvider, MessageVerifier, NonceCache};
use shared_types::AuthenticatedMessage;

use super::key_provider::StaticKeyProvider;
use super::types::{CorrelationId, PendingRequest};

/// IPC Handler for Peer Discovery subsystem.
///
/// Uses the centralized `MessageVerifier` from `shared-types` for all
/// security validation, ensuring consistent application of the security
/// policy defined in Architecture.md and IPC-MATRIX.md.
pub struct IpcHandler<K: KeyProvider> {
    /// Our subsystem ID.
    subsystem_id: u8,
    /// Pending outbound requests awaiting responses.
    pending_requests: HashMap<CorrelationId, PendingRequest>,
    /// Default timeout for requests (seconds).
    default_timeout: u64,
    /// Centralized message verifier from shared-types.
    verifier: MessageVerifier<K>,
}

impl IpcHandler<StaticKeyProvider> {
    /// Create a new IPC handler with a default secret.
    ///
    /// # Arguments
    ///
    /// * `secret` - The shared secret for HMAC validation
    ///
    /// # Note
    ///
    /// In production, use `with_key_provider` with a proper key management system.
    #[must_use]
    pub fn new(secret: &[u8]) -> Self {
        let key_provider = StaticKeyProvider::new(secret);
        let nonce_cache = Arc::new(NonceCache::new());
        Self::with_key_provider(key_provider, nonce_cache)
    }
}

impl<K: KeyProvider> IpcHandler<K> {
    /// Default request timeout in seconds.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

    /// Create a new IPC handler with a custom key provider.
    #[must_use]
    pub fn with_key_provider(key_provider: K, nonce_cache: Arc<NonceCache>) -> Self {
        let verifier = MessageVerifier::new(
            SubsystemId::PeerDiscovery.as_u8(),
            nonce_cache,
            key_provider,
        );

        Self {
            subsystem_id: SubsystemId::PeerDiscovery.as_u8(),
            pending_requests: HashMap::new(),
            default_timeout: Self::DEFAULT_TIMEOUT_SECS,
            verifier,
        }
    }

    /// Get our subsystem ID.
    #[must_use]
    pub const fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Helper: Verify message authentication using centralized security module
    fn verify_message<P>(
        &self,
        message: &AuthenticatedMessage<P>,
        message_bytes: &[u8],
    ) -> Result<(), SecurityError> {
        let verification_result = self.verifier.verify(message, message_bytes);
        if !verification_result.is_valid() {
            return Err(SecurityError::from_verification_result(verification_result));
        }
        Ok(())
    }

    /// Handle an incoming PeerListRequest using the centralized security module.
    pub fn handle_peer_list_request<S: PeerDiscoveryApi>(
        &self,
        message: &AuthenticatedMessage<PeerListRequestPayload>,
        message_bytes: &[u8],
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Verify message
        self.verify_message(message, message_bytes)?;

        // Step 2: Validate sender is authorized for this specific message type
        AuthorizationRules::validate_peer_list_sender(message.sender_id)?;

        // Step 3: Process the request
        let payload = &message.payload;
        let peers = if let Some(ref filter) = payload.filter {
            // Filter peers by reputation
            service
                .get_random_peers(payload.max_peers * 2) // Get extra to account for filtering
                .into_iter()
                .filter(|p| p.reputation_score >= filter.min_reputation)
                .take(payload.max_peers)
                .collect()
        } else {
            service.get_random_peers(payload.max_peers)
        };

        let total_available = service.get_stats().total_peers;

        Ok(PeerListResponsePayload {
            peers,
            total_available,
        })
    }

    /// Handle an incoming FullNodeListRequest using the centralized security module.
    pub fn handle_full_node_list_request<S: PeerDiscoveryApi>(
        &self,
        message: &AuthenticatedMessage<FullNodeListRequestPayload>,
        message_bytes: &[u8],
        service: &S,
    ) -> Result<PeerListResponsePayload, SecurityError> {
        // Step 1: Verify message
        self.verify_message(message, message_bytes)?;

        // Step 2: Validate sender is authorized (Subsystem 13 only)
        AuthorizationRules::validate_full_node_list_sender(message.sender_id)?;

        // Step 3: Process the request
        let payload = &message.payload;
        let filter = PeerFilter {
            min_reputation: 50, // Full nodes require baseline reputation per SPEC-01 Section 6.1
            exclude_subnets: vec![],
        };

        let peers = service
            .get_random_peers(payload.max_nodes * 2)
            .into_iter()
            .filter(|p| p.reputation_score >= filter.min_reputation)
            .take(payload.max_nodes)
            .collect();

        let total_available = service.get_stats().total_peers;

        Ok(PeerListResponsePayload {
            peers,
            total_available,
        })
    }

    /// Register a pending outbound request.
    pub fn register_pending_request(&mut self, correlation_id: CorrelationId, now: u64) {
        self.pending_requests.insert(
            correlation_id,
            PendingRequest {
                sent_at: now,
                timeout_secs: self.default_timeout,
                correlation_id,
            },
        );
    }

    /// Handle a response by matching correlation ID.
    pub fn match_response(&mut self, correlation_id: &CorrelationId) -> Option<PendingRequest> {
        self.pending_requests.remove(correlation_id)
    }

    /// Remove expired pending requests.
    pub fn gc_expired_requests(&mut self, now: u64) -> usize {
        let before_count = self.pending_requests.len();
        self.pending_requests.retain(|_, req| {
            let deadline = req.sent_at.saturating_add(req.timeout_secs);
            now <= deadline
        });
        before_count - self.pending_requests.len()
    }

    /// Get count of pending requests.
    #[must_use]
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.len()
    }
}
