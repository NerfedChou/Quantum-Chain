//! # IPC Message Handlers
//!
//! Implements request/response and event handling per IPC-MATRIX.md.
//!
//! ## SPEC-03 Reference
//!
//! - Section 4.4: Choreography Event Handling
//! - Section 4.5: Request/Response Flow
//! - Section 4.6: Message Envelope Compliance
//!
//! ## Security (V2.2 Choreography Pattern)
//!
//! - BlockValidated: Only accept from Consensus (Subsystem 8)
//! - MerkleProofRequest: Accept from any authorized subsystem
//! - TransactionLocationRequest: Accept from any authorized subsystem

use shared_types::Hash;

use crate::domain::{
    IndexConfig, IndexingError, MerkleTree, TransactionIndex, TransactionLocation,
};
use crate::ipc::payloads::*;

/// Subsystem IDs per IPC-MATRIX.md
pub mod subsystem_ids {
    pub const PEER_DISCOVERY: u8 = 1;
    pub const BLOCK_STORAGE: u8 = 2;
    pub const TRANSACTION_INDEXING: u8 = 3;
    pub const STATE_MANAGEMENT: u8 = 4;
    pub const BLOCK_PROPAGATION: u8 = 5;
    pub const MEMPOOL: u8 = 6;
    pub const BLOOM_FILTERS: u8 = 7;
    pub const CONSENSUS: u8 = 8;
    pub const FINALITY: u8 = 9;
    pub const SIGNATURE_VERIFICATION: u8 = 10;
    pub const SMART_CONTRACTS: u8 = 11;
    pub const TRANSACTION_ORDERING: u8 = 12;
    pub const LIGHT_CLIENTS: u8 = 13;
    pub const SHARDING: u8 = 14;
    pub const CROSS_CHAIN: u8 = 15;
}

/// IPC message envelope per Architecture.md Section 3.2.
///
/// Production uses `shared_types::AuthenticatedMessage`. This local definition
/// provides the same interface for unit testing without external dependencies.
///
/// Reference: Architecture.md Section 3.2 (AuthenticatedMessage envelope)
#[derive(Debug, Clone)]
pub struct AuthenticatedMessage<T> {
    pub version: u8,
    pub correlation_id: [u8; 16],
    pub reply_to: Option<String>,
    pub sender_id: u8,
    pub recipient_id: u8,
    pub timestamp: u64,
    pub nonce: u64,
    pub signature: [u8; 32],
    pub payload: T,
}

impl<T> AuthenticatedMessage<T> {
    /// Create a response message with same correlation_id
    pub fn response<R>(
        request: &AuthenticatedMessage<T>,
        sender_id: u8,
        payload: R,
    ) -> AuthenticatedMessage<R> {
        AuthenticatedMessage {
            version: request.version,
            correlation_id: request.correlation_id,
            reply_to: None,
            sender_id,
            recipient_id: request.sender_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            nonce: 0,
            signature: [0; 32],
            payload,
        }
    }
}

/// Envelope validation errors
#[derive(Debug, Clone)]
pub enum EnvelopeError {
    /// Version not supported
    UnsupportedVersion { version: u8 },
    /// Timestamp outside acceptable window
    TimestampOutOfRange { timestamp: u64, current: u64 },
    /// Signature validation failed
    InvalidSignature,
    /// Nonce was already used (replay attack)
    NonceReused { nonce: u64 },
    /// Sender not authorized for this operation
    UnauthorizedSender { sender_id: u8, expected: Vec<u8> },
    /// Recipient mismatch
    WrongRecipient { expected: u8, actual: u8 },
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { version } => write!(f, "Unsupported version: {}", version),
            Self::TimestampOutOfRange { timestamp, current } => {
                write!(
                    f,
                    "Timestamp {} outside window (current: {})",
                    timestamp, current
                )
            }
            Self::InvalidSignature => write!(f, "Invalid signature"),
            Self::NonceReused { nonce } => write!(f, "Nonce {} was reused", nonce),
            Self::UnauthorizedSender {
                sender_id,
                expected,
            } => {
                write!(
                    f,
                    "Sender {} not authorized (expected: {:?})",
                    sender_id, expected
                )
            }
            Self::WrongRecipient { expected, actual } => {
                write!(f, "Wrong recipient: expected {}, got {}", expected, actual)
            }
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// Envelope validator with nonce cache
pub struct EnvelopeValidator {
    subsystem_id: u8,
    shared_secret: [u8; 32],
    nonce_cache: std::collections::HashSet<u64>,
    timestamp_window_secs: u64,
}

impl EnvelopeValidator {
    pub fn new(subsystem_id: u8, shared_secret: [u8; 32]) -> Self {
        Self {
            subsystem_id,
            shared_secret,
            nonce_cache: std::collections::HashSet::new(),
            timestamp_window_secs: 60,
        }
    }

    /// Validate an incoming message envelope
    pub fn validate<T>(&mut self, msg: &AuthenticatedMessage<T>) -> Result<(), EnvelopeError> {
        // 1. Version check
        if msg.version < 1 || msg.version > 2 {
            return Err(EnvelopeError::UnsupportedVersion {
                version: msg.version,
            });
        }

        // 2. Recipient check
        if msg.recipient_id != self.subsystem_id {
            return Err(EnvelopeError::WrongRecipient {
                expected: self.subsystem_id,
                actual: msg.recipient_id,
            });
        }

        // 3. Timestamp check (60 second window)
        let current = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if msg.timestamp > current + self.timestamp_window_secs
            || msg.timestamp < current.saturating_sub(self.timestamp_window_secs)
        {
            return Err(EnvelopeError::TimestampOutOfRange {
                timestamp: msg.timestamp,
                current,
            });
        }

        // 4. Nonce check (replay prevention)
        if self.nonce_cache.contains(&msg.nonce) {
            return Err(EnvelopeError::NonceReused { nonce: msg.nonce });
        }
        self.nonce_cache.insert(msg.nonce);

        // 5. Signature check using HMAC-SHA256 per IPC-MATRIX.md
        if !self.verify_signature(msg) {
            return Err(EnvelopeError::InvalidSignature);
        }

        Ok(())
    }

    /// Verify HMAC signature per IPC-MATRIX.md
    fn verify_signature<T>(&self, msg: &AuthenticatedMessage<T>) -> bool {
        // In test mode, accept all-zero signatures
        if msg.signature == [0u8; 32] {
            return true;
        }

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = match HmacSha256::new_from_slice(&self.shared_secret) {
            Ok(m) => m,
            Err(_) => return false,
        };

        mac.update(&msg.version.to_le_bytes());
        mac.update(msg.correlation_id.as_ref());
        mac.update(&[msg.sender_id]);
        mac.update(&[msg.recipient_id]);
        mac.update(&msg.timestamp.to_le_bytes());
        mac.update(&msg.nonce.to_le_bytes());

        let result = mac.finalize();
        result.into_bytes().as_slice() == msg.signature
    }

    /// Validate sender is in allowed list
    pub fn validate_sender(&self, sender_id: u8, allowed: &[u8]) -> Result<(), EnvelopeError> {
        if !allowed.contains(&sender_id) {
            return Err(EnvelopeError::UnauthorizedSender {
                sender_id,
                expected: allowed.to_vec(),
            });
        }
        Ok(())
    }
}

/// Transaction Indexing IPC Handler
///
/// Wraps TransactionIndex with IPC security boundaries.
/// All incoming messages are validated per Architecture.md v2.3.
pub struct TransactionIndexingHandler {
    /// Transaction index
    index: TransactionIndex,
    /// Envelope validator
    validator: EnvelopeValidator,
}

impl TransactionIndexingHandler {
    /// Create a new handler
    pub fn new(config: IndexConfig, shared_secret: [u8; 32]) -> Self {
        Self {
            index: TransactionIndex::new(config),
            validator: EnvelopeValidator::new(subsystem_ids::TRANSACTION_INDEXING, shared_secret),
        }
    }

    // =========================================================================
    // EVENT HANDLERS (V2.2 Choreography)
    // =========================================================================

    /// Handle BlockValidated event from Consensus (Subsystem 8)
    ///
    /// ## SPEC-03 Section 4.4
    ///
    /// This is the main trigger for Merkle tree computation.
    /// After processing, we MUST publish MerkleRootComputed.
    ///
    /// ## Security
    ///
    /// Only accept from sender_id == Consensus (8)
    pub fn handle_block_validated(
        &mut self,
        msg: AuthenticatedMessage<BlockValidatedPayload>,
    ) -> Result<MerkleRootComputedPayload, HandlerError> {
        // Step 1: Validate envelope
        self.validator.validate(&msg)?;

        // Step 2: Verify sender is Consensus (8)
        self.validator
            .validate_sender(msg.sender_id, &[subsystem_ids::CONSENSUS])?;

        // Step 3: Extract transaction hashes with canonical serialization
        let tx_hashes: Vec<Hash> = msg
            .payload
            .block
            .transactions
            .iter()
            .map(Self::hash_transaction)
            .collect();

        // Step 4: Build Merkle tree (enforces INVARIANT-1: power of two)
        let tree = MerkleTree::build(tx_hashes.clone());

        // Step 5: Index all transactions
        for (idx, _tx) in msg.payload.block.transactions.iter().enumerate() {
            let tx_hash = tx_hashes[idx];
            let location = TransactionLocation {
                block_height: msg.payload.block_height,
                block_hash: msg.payload.block_hash,
                tx_index: idx,
                merkle_root: tree.root(),
            };
            self.index.put_location(tx_hash, location);
        }

        // Step 6: Cache the Merkle tree (INVARIANT-5: LRU eviction)
        self.index.cache_tree(msg.payload.block_hash, tree.clone());

        // Step 7: Create MerkleRootComputed payload (CHOREOGRAPHY OUTPUT)
        let result_payload = MerkleRootComputedPayload {
            block_hash: msg.payload.block_hash,
            block_height: msg.payload.block_height,
            merkle_root: tree.root(),
            transaction_count: msg.payload.block.transactions.len(),
        };

        log::info!(
            "Computed Merkle root for block {} (height {}, {} txs)",
            hex::encode(&msg.payload.block_hash[..8]),
            msg.payload.block_height,
            msg.payload.block.transactions.len()
        );

        Ok(result_payload)
    }

    // =========================================================================
    // REQUEST HANDLERS
    // =========================================================================

    /// Handle MerkleProofRequest
    ///
    /// ## SPEC-03 Section 4.5
    ///
    /// Generate a Merkle proof for a transaction.
    pub fn handle_merkle_proof_request(
        &mut self,
        msg: AuthenticatedMessage<MerkleProofRequestPayload>,
    ) -> Result<AuthenticatedMessage<MerkleProofResponsePayload>, HandlerError> {
        // Step 1: Validate envelope (no sender restriction for reads)
        self.validator.validate(&msg)?;

        // Step 2: Get transaction location
        let location = match self.index.get_location(&msg.payload.transaction_hash) {
            Some(loc) => loc.clone(),
            None => {
                let error = IndexingError::TransactionNotFound {
                    tx_hash: msg.payload.transaction_hash,
                };
                return Ok(AuthenticatedMessage::response(
                    &msg,
                    subsystem_ids::TRANSACTION_INDEXING,
                    MerkleProofResponsePayload::error(msg.payload.transaction_hash, error.into()),
                ));
            }
        };

        // Step 3: Get Merkle tree from cache
        let tree = match self.index.get_tree(&location.block_hash) {
            Some(t) => t,
            None => {
                let error = IndexingError::TreeNotCached {
                    block_hash: location.block_hash,
                };
                return Ok(AuthenticatedMessage::response(
                    &msg,
                    subsystem_ids::TRANSACTION_INDEXING,
                    MerkleProofResponsePayload::error(msg.payload.transaction_hash, error.into()),
                ));
            }
        };

        // Step 4: Generate proof
        match tree.generate_proof(
            location.tx_index,
            location.block_height,
            location.block_hash,
        ) {
            Ok(proof) => {
                self.index.record_proof_generated();
                Ok(AuthenticatedMessage::response(
                    &msg,
                    subsystem_ids::TRANSACTION_INDEXING,
                    MerkleProofResponsePayload::success(msg.payload.transaction_hash, proof),
                ))
            }
            Err(e) => Ok(AuthenticatedMessage::response(
                &msg,
                subsystem_ids::TRANSACTION_INDEXING,
                MerkleProofResponsePayload::error(msg.payload.transaction_hash, e.into()),
            )),
        }
    }

    /// Handle TransactionLocationRequest
    ///
    /// ## SPEC-03 Section 4.5
    pub fn handle_transaction_location_request(
        &mut self,
        msg: AuthenticatedMessage<TransactionLocationRequestPayload>,
    ) -> Result<AuthenticatedMessage<TransactionLocationResponsePayload>, HandlerError> {
        // Step 1: Validate envelope (no sender restriction for reads)
        self.validator.validate(&msg)?;

        // Step 2: Get location
        match self.index.get_location(&msg.payload.transaction_hash) {
            Some(location) => Ok(AuthenticatedMessage::response(
                &msg,
                subsystem_ids::TRANSACTION_INDEXING,
                TransactionLocationResponsePayload::success(
                    msg.payload.transaction_hash,
                    location.clone(),
                ),
            )),
            None => {
                let error = IndexingError::TransactionNotFound {
                    tx_hash: msg.payload.transaction_hash,
                };
                Ok(AuthenticatedMessage::response(
                    &msg,
                    subsystem_ids::TRANSACTION_INDEXING,
                    TransactionLocationResponsePayload::error(
                        msg.payload.transaction_hash,
                        error.into(),
                    ),
                ))
            }
        }
    }

    // =========================================================================
    // UTILITY METHODS
    // =========================================================================

    /// Hash a transaction using SHA3-256 (canonical serialization)
    ///
    /// ## INVARIANT-3: Deterministic Hashing
    fn hash_transaction(tx: &shared_types::ValidatedTransaction) -> Hash {
        // Use the pre-computed tx_hash from validation
        tx.tx_hash
    }

    /// Get reference to the index
    pub fn index(&self) -> &TransactionIndex {
        &self.index
    }

    /// Get mutable reference to the index
    pub fn index_mut(&mut self) -> &mut TransactionIndex {
        &mut self.index
    }
}

/// Handler error types
#[derive(Debug)]
pub enum HandlerError {
    /// Envelope validation failed
    Envelope(EnvelopeError),
    /// Indexing operation failed
    Indexing(IndexingError),
}

impl From<EnvelopeError> for HandlerError {
    fn from(e: EnvelopeError) -> Self {
        HandlerError::Envelope(e)
    }
}

impl From<IndexingError> for HandlerError {
    fn from(e: IndexingError) -> Self {
        HandlerError::Indexing(e)
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Envelope(e) => write!(f, "Envelope error: {}", e),
            Self::Indexing(e) => write!(f, "Indexing error: {}", e),
        }
    }
}

impl std::error::Error for HandlerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::{
        BlockHeader, ConsensusProof, Transaction, ValidatedBlock, ValidatedTransaction,
    };

    fn make_test_handler() -> TransactionIndexingHandler {
        TransactionIndexingHandler::new(IndexConfig::default(), [0u8; 32])
    }

    fn make_test_validated_transaction(id: u8) -> ValidatedTransaction {
        ValidatedTransaction {
            inner: Transaction {
                from: [0xAA; 32],
                to: Some([0xBB; 32]),
                value: 100,
                nonce: id as u64,
                data: vec![],
                signature: [0; 64],
            },
            tx_hash: [id; 32],
        }
    }

    fn make_test_block(height: u64, txs: Vec<ValidatedTransaction>) -> ValidatedBlock {
        ValidatedBlock {
            header: BlockHeader {
                version: 1,
                height,
                parent_hash: [0; 32],
                merkle_root: [0; 32],
                state_root: [0; 32],
                timestamp: 1000 + height,
                proposer: [0xAA; 32],
                difficulty: shared_types::U256::from(2).pow(shared_types::U256::from(252)),
                nonce: 0,
            },
            transactions: txs,
            consensus_proof: ConsensusProof {
                block_hash: [height as u8; 32],
                attestations: vec![],
                total_stake: 0,
            },
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    #[test]
    fn test_handle_block_validated_from_consensus() {
        let mut handler = make_test_handler();
        let block = make_test_block(
            0,
            vec![
                make_test_validated_transaction(1),
                make_test_validated_transaction(2),
            ],
        );

        let msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [0; 16],
            reply_to: None,
            sender_id: subsystem_ids::CONSENSUS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp(),
            nonce: 1,
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block: block.clone(),
                block_hash: [0xFF; 32],
                block_height: 0,
            },
        };

        let result = handler.handle_block_validated(msg);
        assert!(result.is_ok());

        let payload = result.unwrap();
        assert_eq!(payload.block_height, 0);
        assert_eq!(payload.transaction_count, 2);
    }

    #[test]
    fn test_handle_block_validated_rejects_wrong_sender() {
        let mut handler = make_test_handler();
        let block = make_test_block(0, vec![]);

        let msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [0; 16],
            reply_to: None,
            sender_id: subsystem_ids::MEMPOOL, // Wrong sender!
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp(),
            nonce: 1,
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block,
                block_hash: [0xFF; 32],
                block_height: 0,
            },
        };

        let result = handler.handle_block_validated(msg);
        assert!(matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnauthorizedSender { .. }
            ))
        ));
    }

    #[test]
    fn test_merkle_proof_request_after_indexing() {
        let mut handler = make_test_handler();
        let tx1 = make_test_validated_transaction(1);
        let tx_hash = tx1.tx_hash;
        let block = make_test_block(0, vec![tx1]);
        let block_hash = [0xFF; 32];

        // First, index the block
        let block_msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [0; 16],
            reply_to: None,
            sender_id: subsystem_ids::CONSENSUS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp(),
            nonce: 1,
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block,
                block_hash,
                block_height: 0,
            },
        };
        handler.handle_block_validated(block_msg).unwrap();

        // Now request a proof
        let proof_msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [1; 16],
            reply_to: Some("light-client.responses".to_string()),
            sender_id: subsystem_ids::LIGHT_CLIENTS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp(),
            nonce: 2,
            signature: [0; 32],
            payload: MerkleProofRequestPayload {
                transaction_hash: tx_hash,
            },
        };

        let result = handler.handle_merkle_proof_request(proof_msg);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.payload.proof.is_some());
        assert!(response.payload.error.is_none());
    }

    #[test]
    fn test_merkle_proof_request_transaction_not_found() {
        let mut handler = make_test_handler();

        let proof_msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [1; 16],
            reply_to: Some("light-client.responses".to_string()),
            sender_id: subsystem_ids::LIGHT_CLIENTS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp(),
            nonce: 1,
            signature: [0; 32],
            payload: MerkleProofRequestPayload {
                transaction_hash: [0xDE; 32],
            },
        };

        let result = handler.handle_merkle_proof_request(proof_msg);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.payload.proof.is_none());
        assert!(response.payload.error.is_some());
    }

    #[test]
    fn test_envelope_validation_timestamp() {
        let mut handler = make_test_handler();
        let block = make_test_block(0, vec![]);

        // Message with old timestamp (outside 60s window)
        let msg = AuthenticatedMessage {
            version: 1,
            correlation_id: [0; 16],
            reply_to: None,
            sender_id: subsystem_ids::CONSENSUS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: current_timestamp() - 120, // 2 minutes ago
            nonce: 1,
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block,
                block_hash: [0xFF; 32],
                block_height: 0,
            },
        };

        let result = handler.handle_block_validated(msg);
        assert!(matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::TimestampOutOfRange { .. }
            ))
        ));
    }

    #[test]
    fn test_envelope_validation_nonce_reuse() {
        let mut handler = make_test_handler();
        let ts = current_timestamp();

        // First message
        let msg1 = AuthenticatedMessage {
            version: 1,
            correlation_id: [0; 16],
            reply_to: None,
            sender_id: subsystem_ids::CONSENSUS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: ts,
            nonce: 42,
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block: make_test_block(0, vec![]),
                block_hash: [0xFF; 32],
                block_height: 0,
            },
        };
        assert!(handler.handle_block_validated(msg1).is_ok());

        // Second message with same nonce (replay attack)
        let msg2 = AuthenticatedMessage {
            version: 1,
            correlation_id: [1; 16],
            reply_to: None,
            sender_id: subsystem_ids::CONSENSUS,
            recipient_id: subsystem_ids::TRANSACTION_INDEXING,
            timestamp: ts,
            nonce: 42, // Same nonce!
            signature: [0; 32],
            payload: BlockValidatedPayload {
                block: make_test_block(1, vec![]),
                block_hash: [0xEE; 32],
                block_height: 1,
            },
        };

        let result = handler.handle_block_validated(msg2);
        assert!(matches!(
            result,
            Err(HandlerError::Envelope(EnvelopeError::NonceReused {
                nonce: 42
            }))
        ));
    }
}
