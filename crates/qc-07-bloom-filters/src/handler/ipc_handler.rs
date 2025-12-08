//! IPC Handler for Bloom Filter subsystem
//!
//! Reference: IPC-MATRIX.md Subsystem 7 - Security Boundaries
//!
//! Validates incoming messages per security rules:
//! - Accept BuildFilterRequest from Subsystem 13 ONLY
//! - Accept UpdateFilterRequest from Subsystem 13 ONLY
//! - Accept TransactionHashUpdate from Subsystem 3 ONLY
//! - Reject >1000 watched addresses
//! - Reject FPR <0.01 or >0.1
//! - Reject >1 filter update per 10 blocks per client

use std::collections::HashMap;
use std::sync::RwLock;

use shared_types::{AuthenticatedMessage, SubsystemId};

use crate::error::FilterError;
use crate::events::{BuildFilterRequest, TransactionHashUpdate, UpdateFilterRequest};

/// Rate limit configuration
const MAX_UPDATES_PER_WINDOW: usize = 1;
const RATE_LIMIT_BLOCKS: u64 = 10;

/// Subsystem IDs for authorization (from SubsystemId enum)
const SUBSYSTEM_LIGHT_CLIENT: u8 = 13;
const SUBSYSTEM_TRANSACTION_INDEXING: u8 = 3;

/// IPC Handler for Bloom Filter requests
///
/// Enforces security boundaries per IPC-MATRIX.md
pub struct BloomFilterHandler {
    /// Rate limiting: client_id -> (last_update_block, update_count)
    rate_limits: RwLock<HashMap<String, (u64, usize)>>,
    /// Current block height (for rate limiting)
    current_block: RwLock<u64>,
}

impl BloomFilterHandler {
    /// Create a new handler
    pub fn new() -> Self {
        Self {
            rate_limits: RwLock::new(HashMap::new()),
            current_block: RwLock::new(0),
        }
    }

    /// Update current block height
    pub fn set_current_block(&self, height: u64) {
        let mut current = self.current_block.write().unwrap();
        *current = height;
    }

    /// Validate a BuildFilterRequest
    ///
    /// Security rules per IPC-MATRIX.md:
    /// - ONLY Light Clients (13) can request filters
    /// - Reject >1000 watched addresses
    /// - Reject FPR <0.01 or >0.1
    pub fn validate_build_filter(
        &self,
        msg: &AuthenticatedMessage<BuildFilterRequest>,
    ) -> Result<(), FilterError> {
        // Rule 1: Only Subsystem 13 (Light Clients) can build filters
        if msg.sender_id != SUBSYSTEM_LIGHT_CLIENT {
            return Err(FilterError::UnauthorizedSender(
                SubsystemId::from_u8(msg.sender_id).unwrap_or(SubsystemId::PeerDiscovery),
            ));
        }

        let payload = &msg.payload;

        // Rule 2: Reject >1000 watched addresses (privacy risk)
        if payload.watched_addresses.len() > 1000 {
            return Err(FilterError::TooManyAddresses {
                count: payload.watched_addresses.len(),
                max: 1000,
            });
        }

        // Rule 3: Reject FPR <0.01 (too precise = privacy risk)
        if payload.target_fpr < 0.01 {
            return Err(FilterError::InvalidFPR {
                fpr: payload.target_fpr as f64,
            });
        }

        // Rule 4: Reject FPR >0.1 (too noisy = useless)
        if payload.target_fpr > 0.1 {
            return Err(FilterError::InvalidFPR {
                fpr: payload.target_fpr as f64,
            });
        }

        Ok(())
    }

    /// Validate an UpdateFilterRequest
    ///
    /// Security rules per IPC-MATRIX.md:
    /// - ONLY Light Clients (13) can update filters
    /// - Reject >1 filter update per 10 blocks per client
    pub fn validate_update_filter(
        &self,
        msg: &AuthenticatedMessage<UpdateFilterRequest>,
        client_id: &str,
    ) -> Result<(), FilterError> {
        // Rule 1: Only Subsystem 13 (Light Clients) can update filters
        if msg.sender_id != SUBSYSTEM_LIGHT_CLIENT {
            return Err(FilterError::UnauthorizedSender(
                SubsystemId::from_u8(msg.sender_id).unwrap_or(SubsystemId::PeerDiscovery),
            ));
        }

        // Rule 2: Rate limiting - max 1 update per 10 blocks
        let current_block = *self.current_block.read().unwrap();
        let mut rate_limits = self.rate_limits.write().unwrap();

        if let Some((last_block, count)) = rate_limits.get_mut(client_id) {
            // Check if we're still in the rate limit window
            if current_block < *last_block + RATE_LIMIT_BLOCKS {
                if *count >= MAX_UPDATES_PER_WINDOW {
                    return Err(FilterError::RateLimited);
                }
                *count += 1;
            } else {
                // New window
                *last_block = current_block;
                *count = 1;
            }
        } else {
            // First update from this client
            rate_limits.insert(client_id.to_string(), (current_block, 1));
        }

        Ok(())
    }

    /// Validate a TransactionHashUpdate
    ///
    /// Security rules per IPC-MATRIX.md:
    /// - ONLY Transaction Indexing (3) can provide hashes
    pub fn validate_tx_hash_update(
        &self,
        msg: &AuthenticatedMessage<TransactionHashUpdate>,
    ) -> Result<(), FilterError> {
        // Rule: Only Subsystem 3 (Transaction Indexing) can provide hashes
        if msg.sender_id != SUBSYSTEM_TRANSACTION_INDEXING {
            return Err(FilterError::UnauthorizedSender(
                SubsystemId::from_u8(msg.sender_id).unwrap_or(SubsystemId::PeerDiscovery),
            ));
        }

        Ok(())
    }

    /// Check if a sender is authorized for a specific message type
    pub fn is_authorized(&self, sender: u8, message_type: &str) -> bool {
        match message_type {
            "BuildFilterRequest" | "UpdateFilterRequest" => sender == SUBSYSTEM_LIGHT_CLIENT,
            "TransactionHashUpdate" => sender == SUBSYSTEM_TRANSACTION_INDEXING,
            _ => false,
        }
    }

    /// Check if a sender is authorized to build filters
    /// Used for quick authorization checks without full message validation
    pub fn is_authorized_for_build_filter(&self, sender: u8) -> bool {
        sender == SUBSYSTEM_LIGHT_CLIENT
    }

    /// Check if a sender is authorized to send transaction hash updates
    /// Used for quick authorization checks without full message validation
    pub fn is_authorized_for_tx_update(&self, sender: u8) -> bool {
        sender == SUBSYSTEM_TRANSACTION_INDEXING
    }

    /// Check rate limit for filter updates
    /// Returns true if update is allowed, false if rate limited
    pub fn check_update_rate_limit(&self, client_id: &str, current_block: u64) -> bool {
        let mut rate_limits = self.rate_limits.write().unwrap();

        if let Some((last_block, count)) = rate_limits.get_mut(client_id) {
            if current_block < *last_block + RATE_LIMIT_BLOCKS {
                if *count >= MAX_UPDATES_PER_WINDOW {
                    return false;
                }
                *count += 1;
            } else {
                *last_block = current_block;
                *count = 1;
            }
        } else {
            rate_limits.insert(client_id.to_string(), (current_block, 1));
        }

        true
    }
}

impl Default for BloomFilterHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_authenticated<T>(sender_id: u8, payload: T) -> AuthenticatedMessage<T> {
        AuthenticatedMessage {
            version: 1,
            sender_id,
            recipient_id: SubsystemId::BloomFilters.as_u8(),
            correlation_id: Uuid::nil(),
            reply_to: None,
            timestamp: 0,
            nonce: Uuid::nil(),
            payload,
            signature: [0u8; 64],
        }
    }

    #[test]
    fn test_reject_request_from_unauthorized_sender() {
        let handler = BloomFilterHandler::new();

        // BuildFilterRequest from Consensus (8) → REJECTED
        let bad_request = create_authenticated(
            SubsystemId::Consensus.as_u8(),
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );

        let result = handler.validate_build_filter(&bad_request);
        assert!(matches!(result, Err(FilterError::UnauthorizedSender(_))));

        // BuildFilterRequest from Light Clients (13) → ACCEPTED
        let good_request = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );

        let result = handler.validate_build_filter(&good_request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_filter_with_too_many_addresses() {
        let handler = BloomFilterHandler::new();

        // Request with 1001 watched addresses → REJECTED
        let addresses: Vec<[u8; 20]> = (0..1001).map(|i| [i as u8; 20]).collect();
        let request = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: addresses,
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );

        let result = handler.validate_build_filter(&request);
        assert!(matches!(result, Err(FilterError::TooManyAddresses { .. })));
    }

    #[test]
    fn test_reject_invalid_fpr() {
        let handler = BloomFilterHandler::new();

        // FPR = 0.001 → REJECTED (< 0.01)
        let request_too_low = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.001,
            },
        );

        let result = handler.validate_build_filter(&request_too_low);
        assert!(matches!(result, Err(FilterError::InvalidFPR { .. })));

        // FPR = 0.2 → REJECTED (> 0.1)
        let request_too_high = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.2,
            },
        );

        let result = handler.validate_build_filter(&request_too_high);
        assert!(matches!(result, Err(FilterError::InvalidFPR { .. })));
    }

    #[test]
    fn test_rate_limit_filter_updates() {
        let handler = BloomFilterHandler::new();
        handler.set_current_block(100);

        let request = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            UpdateFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                filter_id: 1,
                add_addresses: vec![[0xAA; 20]],
                remove_addresses: vec![],
            },
        );

        let client_id = "client_1";

        // First update → ACCEPTED
        let result1 = handler.validate_update_filter(&request, client_id);
        assert!(result1.is_ok());

        // Second update within 10 blocks → REJECTED
        let result2 = handler.validate_update_filter(&request, client_id);
        assert!(matches!(result2, Err(FilterError::RateLimited)));

        // Move forward 10 blocks
        handler.set_current_block(110);

        // Now update should be accepted again
        let result3 = handler.validate_update_filter(&request, client_id);
        assert!(result3.is_ok());
    }

    #[test]
    fn test_tx_hash_update_authorization() {
        let handler = BloomFilterHandler::new();

        // From Transaction Indexing (3) → ACCEPTED
        let good_update = create_authenticated(
            SUBSYSTEM_TRANSACTION_INDEXING,
            TransactionHashUpdate {
                block_number: 100,
                hashes: vec![[0xAA; 32]],
            },
        );

        let result = handler.validate_tx_hash_update(&good_update);
        assert!(result.is_ok());

        // From Consensus (8) → REJECTED
        let bad_update = create_authenticated(
            SubsystemId::Consensus.as_u8(),
            TransactionHashUpdate {
                block_number: 100,
                hashes: vec![[0xAA; 32]],
            },
        );

        let result = handler.validate_tx_hash_update(&bad_update);
        assert!(matches!(result, Err(FilterError::UnauthorizedSender(_))));
    }

    #[test]
    fn test_envelope_only_identity() {
        let handler = BloomFilterHandler::new();

        // Even if payload could contain identity info, we only use envelope.sender_id
        // This test ensures we're checking the envelope, not any payload field
        let request = create_authenticated(
            SubsystemId::Consensus.as_u8(), // Unauthorized sender in envelope
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );

        // Should reject based on envelope.sender_id
        let result = handler.validate_build_filter(&request);
        assert!(matches!(result, Err(FilterError::UnauthorizedSender(_))));
    }

    // =========================================================================
    // COMPREHENSIVE UNAUTHORIZED SENDER TESTS (IPC-MATRIX.md Compliance)
    // =========================================================================

    /// Test: BuildFilterRequest rejected from Block Storage (2)
    #[test]
    fn test_reject_build_filter_from_block_storage() {
        let handler = BloomFilterHandler::new();
        let request = create_authenticated(
            SubsystemId::BlockStorage.as_u8(),
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );
        let result = handler.validate_build_filter(&request);
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Block Storage (2) should NOT be authorized to BuildFilterRequest"
        );
    }

    /// Test: BuildFilterRequest rejected from Mempool (6)
    #[test]
    fn test_reject_build_filter_from_mempool() {
        let handler = BloomFilterHandler::new();
        let request = create_authenticated(
            SubsystemId::Mempool.as_u8(),
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );
        let result = handler.validate_build_filter(&request);
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Mempool (6) should NOT be authorized to BuildFilterRequest"
        );
    }

    /// Test: BuildFilterRequest rejected from Transaction Indexing (3)
    #[test]
    fn test_reject_build_filter_from_tx_indexing() {
        let handler = BloomFilterHandler::new();
        let request = create_authenticated(
            SUBSYSTEM_TRANSACTION_INDEXING,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );
        let result = handler.validate_build_filter(&request);
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Transaction Indexing (3) should NOT be authorized to BuildFilterRequest"
        );
    }

    /// Test: TransactionHashUpdate rejected from Light Clients (13)
    #[test]
    fn test_reject_tx_update_from_light_clients() {
        let handler = BloomFilterHandler::new();
        let update = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            TransactionHashUpdate {
                block_number: 100,
                hashes: vec![[0xAA; 32]],
            },
        );
        let result = handler.validate_tx_hash_update(&update);
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Light Clients (13) should NOT be authorized to TransactionHashUpdate"
        );
    }

    /// Test: TransactionHashUpdate rejected from Block Storage (2)
    #[test]
    fn test_reject_tx_update_from_block_storage() {
        let handler = BloomFilterHandler::new();
        let update = create_authenticated(
            SubsystemId::BlockStorage.as_u8(),
            TransactionHashUpdate {
                block_number: 100,
                hashes: vec![[0xAA; 32]],
            },
        );
        let result = handler.validate_tx_hash_update(&update);
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Block Storage (2) should NOT be authorized to TransactionHashUpdate"
        );
    }

    /// Test: UpdateFilterRequest rejected from Transaction Indexing (3)
    #[test]
    fn test_reject_update_filter_from_tx_indexing() {
        let handler = BloomFilterHandler::new();
        let request = create_authenticated(
            SUBSYSTEM_TRANSACTION_INDEXING,
            UpdateFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                filter_id: 1,
                add_addresses: vec![[0xAA; 20]],
                remove_addresses: vec![],
            },
        );
        let result = handler.validate_update_filter(&request, "client_1");
        assert!(
            matches!(result, Err(FilterError::UnauthorizedSender(_))),
            "Transaction Indexing (3) should NOT be authorized to UpdateFilterRequest"
        );
    }

    /// Test: Verify only Light Clients (13) can build filters (documentation test)
    #[test]
    fn test_only_light_clients_authorized_for_build_filter() {
        let handler = BloomFilterHandler::new();

        // Per IPC-MATRIX.md Subsystem 7:
        // BuildFilterRequest: Light Clients (13) ONLY
        let valid_request = create_authenticated(
            SUBSYSTEM_LIGHT_CLIENT,
            BuildFilterRequest {
                correlation_id: 1,
                reply_to: "test".to_string(),
                watched_addresses: vec![[0xAA; 20]],
                start_block: 0,
                end_block: 100,
                target_fpr: 0.05,
            },
        );
        assert!(
            handler.validate_build_filter(&valid_request).is_ok(),
            "Light Clients (13) MUST be authorized for BuildFilterRequest"
        );

        // All other subsystems should be rejected
        let unauthorized_ids = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        for sender_id in unauthorized_ids {
            assert!(
                !handler.is_authorized_for_build_filter(sender_id),
                "Subsystem {} should NOT be authorized for BuildFilterRequest",
                sender_id
            );
        }
    }

    /// Test: Verify only Transaction Indexing (3) can send hash updates
    #[test]
    fn test_only_tx_indexing_authorized_for_hash_update() {
        let handler = BloomFilterHandler::new();

        // Per IPC-MATRIX.md Subsystem 7:
        // TransactionHashUpdate: Transaction Indexing (3) ONLY
        assert!(
            handler.is_authorized_for_tx_update(SUBSYSTEM_TRANSACTION_INDEXING),
            "Transaction Indexing (3) MUST be authorized for TransactionHashUpdate"
        );

        // All other subsystems should be rejected
        let unauthorized_ids = [1u8, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        for sender_id in unauthorized_ids {
            assert!(
                !handler.is_authorized_for_tx_update(sender_id),
                "Subsystem {} should NOT be authorized for TransactionHashUpdate",
                sender_id
            );
        }
    }
}

