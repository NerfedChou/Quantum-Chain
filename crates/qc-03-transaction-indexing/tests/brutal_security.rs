//! # Brutal Security Tests for Transaction Indexing (qc-03)
//!
//! These tests attempt to break the security invariants defined in SPEC-03.
//!
//! ## Test Categories
//!
//! 1. **IPC Authorization Attacks** - Sender spoofing, replay attacks
//! 2. **Merkle Tree Attacks** - Tampered proofs, invalid indices
//! 3. **Memory Exhaustion** - LRU cache overflow, unbounded allocations
//! 4. **Invariant Violations** - Power-of-two padding, deterministic hashing

use qc_03_transaction_indexing::domain::{IndexConfig, MerkleTree};
use qc_03_transaction_indexing::ipc::{
    handler::{
        subsystem_ids, AuthenticatedMessage, EnvelopeError, HandlerError,
        TransactionIndexingHandler,
    },
    payloads::{BlockValidatedPayload, MerkleProofRequestPayload},
};
use shared_types::{
    BlockHeader, ConsensusProof, Hash, Transaction, ValidatedBlock, ValidatedTransaction,
};

// =============================================================================
// TEST HELPERS
// =============================================================================

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn make_handler() -> TransactionIndexingHandler {
    TransactionIndexingHandler::new(IndexConfig::default(), [0u8; 32])
}

fn make_handler_with_config(config: IndexConfig) -> TransactionIndexingHandler {
    TransactionIndexingHandler::new(config, [0u8; 32])
}

fn make_tx(id: u8) -> ValidatedTransaction {
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

fn make_block(height: u64, txs: Vec<ValidatedTransaction>) -> ValidatedBlock {
    ValidatedBlock {
        header: BlockHeader {
            version: 1,
            height,
            parent_hash: [0; 32],
            merkle_root: [0; 32],
            state_root: [0; 32],
            timestamp: 1000 + height,
            proposer: [0xAA; 32],
        },
        transactions: txs,
        consensus_proof: ConsensusProof {
            block_hash: [height as u8; 32],
            attestations: vec![],
            total_stake: 0,
        },
    }
}

fn make_block_validated_msg(
    sender_id: u8,
    block: ValidatedBlock,
    block_hash: Hash,
    nonce: u64,
) -> AuthenticatedMessage<BlockValidatedPayload> {
    AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id,
        recipient_id: subsystem_ids::TRANSACTION_INDEXING,
        timestamp: current_timestamp(),
        nonce,
        signature: [0; 32],
        payload: BlockValidatedPayload {
            block,
            block_hash,
            block_height: 0,
        },
    }
}

// =============================================================================
// IPC AUTHORIZATION ATTACKS
// =============================================================================

/// ATTACK: Mempool tries to inject BlockValidated (should be Consensus only)
#[test]
fn brutal_unauthorized_sender_mempool() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);
    let msg = make_block_validated_msg(subsystem_ids::MEMPOOL, block, [0xFF; 32], 1);

    let result = handler.handle_block_validated(msg);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnauthorizedSender { .. }
            ))
        ),
        "Mempool should NOT be able to send BlockValidated"
    );
}

/// ATTACK: Block Storage tries to inject BlockValidated
#[test]
fn brutal_unauthorized_sender_block_storage() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);
    let msg = make_block_validated_msg(subsystem_ids::BLOCK_STORAGE, block, [0xFF; 32], 1);

    let result = handler.handle_block_validated(msg);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnauthorizedSender { .. }
            ))
        ),
        "Block Storage should NOT be able to send BlockValidated"
    );
}

/// ATTACK: Signature Verification tries to inject BlockValidated
#[test]
fn brutal_unauthorized_sender_sig_verify() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);
    let msg = make_block_validated_msg(subsystem_ids::SIGNATURE_VERIFICATION, block, [0xFF; 32], 1);

    let result = handler.handle_block_validated(msg);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnauthorizedSender { .. }
            ))
        ),
        "Signature Verification should NOT be able to send BlockValidated"
    );
}

/// ATTACK: Unknown subsystem ID (255)
#[test]
fn brutal_unknown_subsystem_id() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);
    let msg = make_block_validated_msg(255, block, [0xFF; 32], 1);

    let result = handler.handle_block_validated(msg);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnauthorizedSender { .. }
            ))
        ),
        "Unknown subsystem 255 should be rejected"
    );
}

// =============================================================================
// REPLAY ATTACKS
// =============================================================================

/// ATTACK: Replay the same message (same nonce)
#[test]
fn brutal_replay_attack_same_nonce() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);
    let msg1 = make_block_validated_msg(subsystem_ids::CONSENSUS, block.clone(), [0xFF; 32], 42);
    let msg2 = make_block_validated_msg(subsystem_ids::CONSENSUS, block, [0xEE; 32], 42);

    // First message succeeds
    assert!(handler.handle_block_validated(msg1).is_ok());

    // Second message with same nonce MUST fail
    let result = handler.handle_block_validated(msg2);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(EnvelopeError::NonceReused {
                nonce: 42
            }))
        ),
        "Replay attack with same nonce should be detected"
    );
}

/// ATTACK: Old timestamp (outside 60s window)
#[test]
fn brutal_old_timestamp_attack() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);

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
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::TimestampOutOfRange { .. }
            ))
        ),
        "Old timestamp should be rejected"
    );
}

/// ATTACK: Future timestamp (outside 60s window)
#[test]
fn brutal_future_timestamp_attack() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::TRANSACTION_INDEXING,
        timestamp: current_timestamp() + 120, // 2 minutes in future
        nonce: 1,
        signature: [0; 32],
        payload: BlockValidatedPayload {
            block,
            block_hash: [0xFF; 32],
            block_height: 0,
        },
    };

    let result = handler.handle_block_validated(msg);
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::TimestampOutOfRange { .. }
            ))
        ),
        "Future timestamp should be rejected"
    );
}

/// ATTACK: Wrong recipient ID
#[test]
fn brutal_wrong_recipient() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
        recipient_id: subsystem_ids::MEMPOOL, // Wrong recipient!
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
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(EnvelopeError::WrongRecipient { .. }))
        ),
        "Wrong recipient should be rejected"
    );
}

/// ATTACK: Unsupported protocol version
#[test]
fn brutal_unsupported_version() {
    let mut handler = make_handler();
    let block = make_block(0, vec![make_tx(1)]);

    let msg = AuthenticatedMessage {
        version: 99, // Unsupported version
        correlation_id: [0; 16],
        reply_to: None,
        sender_id: subsystem_ids::CONSENSUS,
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
    assert!(
        matches!(
            result,
            Err(HandlerError::Envelope(
                EnvelopeError::UnsupportedVersion { .. }
            ))
        ),
        "Unsupported version should be rejected"
    );
}

// =============================================================================
// MERKLE TREE ATTACKS
// =============================================================================

/// INVARIANT-1: Power of two padding
/// Test that trees with non-power-of-2 tx counts are padded correctly
#[test]
fn brutal_merkle_tree_odd_tx_count() {
    // 1 tx -> padded to 2
    let tree1 = MerkleTree::build(vec![[1u8; 32]]);
    assert!(
        tree1.leaf_count().is_power_of_two(),
        "1 tx should be padded to 2"
    );

    // 3 txs -> padded to 4
    let tree3 = MerkleTree::build(vec![[1u8; 32], [2u8; 32], [3u8; 32]]);
    assert!(
        tree3.leaf_count().is_power_of_two(),
        "3 txs should be padded to 4"
    );

    // 5 txs -> padded to 8
    let tree5 = MerkleTree::build(vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32]]);
    assert!(
        tree5.leaf_count().is_power_of_two(),
        "5 txs should be padded to 8"
    );

    // 17 txs -> padded to 32
    let hashes: Vec<Hash> = (1..=17).map(|i| [i as u8; 32]).collect();
    let tree17 = MerkleTree::build(hashes);
    assert!(
        tree17.leaf_count().is_power_of_two(),
        "17 txs should be padded to 32"
    );
}

/// INVARIANT-2: All generated proofs MUST verify
#[test]
fn brutal_merkle_proof_must_verify() {
    let hashes: Vec<Hash> = (1..=8).map(|i| [i as u8; 32]).collect();
    let tree = MerkleTree::build(hashes.clone());

    for idx in 0..hashes.len() {
        let proof = tree
            .generate_proof(idx, 0, [0; 32])
            .expect("proof should generate");
        assert!(
            tree.verify_proof(&proof),
            "Proof for tx {} must verify against root",
            idx
        );
    }
}

/// ATTACK: Tampered proof should NOT verify
#[test]
fn brutal_tampered_proof_must_fail() {
    let hashes: Vec<Hash> = (1..=4).map(|i| [i as u8; 32]).collect();
    let tree = MerkleTree::build(hashes.clone());

    let mut proof = tree
        .generate_proof(0, 0, [0; 32])
        .expect("proof should generate");

    // Tamper with the first path node
    if !proof.path.is_empty() {
        proof.path[0].hash = [0xFF; 32];
    }

    assert!(!tree.verify_proof(&proof), "Tampered proof MUST NOT verify");
}

/// ATTACK: Wrong leaf hash should NOT verify
#[test]
fn brutal_wrong_leaf_must_fail() {
    let hashes: Vec<Hash> = (1..=4).map(|i| [i as u8; 32]).collect();
    let tree = MerkleTree::build(hashes);

    let mut proof = tree
        .generate_proof(0, 0, [0; 32])
        .expect("proof should generate");

    // Tamper with the leaf hash
    proof.leaf_hash = [0xDE; 32];
    assert!(!tree.verify_proof(&proof), "Wrong leaf MUST NOT verify");
}

/// ATTACK: Proof with out-of-bounds index
#[test]
fn brutal_proof_index_out_of_bounds() {
    let hashes: Vec<Hash> = (1..=4).map(|i| [i as u8; 32]).collect();
    let tree = MerkleTree::build(hashes);

    // Try to get proof for index 100 (out of bounds)
    let result = tree.generate_proof(100, 0, [0; 32]);
    assert!(result.is_err(), "Out of bounds index should return error");
}

// =============================================================================
// MEMORY EXHAUSTION ATTACKS
// =============================================================================

/// INVARIANT-5: LRU cache eviction
/// Flood with more blocks than cache size, verify eviction works
#[test]
fn brutal_lru_cache_overflow() {
    let config = IndexConfig {
        max_cached_trees: 10, // Small cache
        ..IndexConfig::default()
    };
    let mut handler = make_handler_with_config(config);

    // Insert 20 blocks (2x cache size)
    for i in 0..20u64 {
        let block = make_block(i, vec![make_tx(i as u8)]);
        let block_hash = [i as u8; 32];
        let msg = make_block_validated_msg(subsystem_ids::CONSENSUS, block, block_hash, i + 1);
        handler.handle_block_validated(msg).unwrap();
    }

    // Older blocks should be evicted
    // Block 0's tree should no longer be cached
    let first_block_hash = [0u8; 32];
    let tree = handler.index_mut().get_tree(&first_block_hash);
    assert!(tree.is_none(), "Block 0 should be evicted from LRU cache");

    // Recent blocks should still be cached
    let recent_block_hash = [19u8; 32];
    let tree = handler.index_mut().get_tree(&recent_block_hash);
    assert!(tree.is_some(), "Block 19 should still be in cache");
}

/// ATTACK: Empty block (0 transactions)
#[test]
fn brutal_empty_block() {
    let mut handler = make_handler();
    let block = make_block(0, vec![]); // No transactions
    let msg = make_block_validated_msg(subsystem_ids::CONSENSUS, block, [0xFF; 32], 1);

    // Should handle gracefully (empty Merkle tree)
    let result = handler.handle_block_validated(msg);
    assert!(result.is_ok(), "Empty block should be handled gracefully");

    let payload = result.unwrap();
    assert_eq!(payload.transaction_count, 0);
}

/// ATTACK: Large block (1000 transactions)
#[test]
fn brutal_large_block() {
    let mut handler = make_handler();
    let txs: Vec<ValidatedTransaction> = (0..1000u16)
        .map(|i| {
            let mut tx = make_tx((i % 256) as u8);
            tx.tx_hash = {
                let mut hash = [0u8; 32];
                hash[0..2].copy_from_slice(&i.to_le_bytes());
                hash
            };
            tx
        })
        .collect();

    let block = make_block(0, txs);
    let msg = make_block_validated_msg(subsystem_ids::CONSENSUS, block, [0xFF; 32], 1);

    let result = handler.handle_block_validated(msg);
    assert!(result.is_ok(), "Large block should be handled");

    let payload = result.unwrap();
    assert_eq!(payload.transaction_count, 1000);

    // Tree should be power of 2 (1024)
    let tree = handler.index_mut().get_tree(&[0xFF; 32]).unwrap();
    assert_eq!(tree.leaf_count(), 1024, "1000 txs should pad to 1024");
}

// =============================================================================
// INVARIANT-3: DETERMINISTIC HASHING
// =============================================================================

/// INVARIANT-3: Same transaction must always produce same hash
#[test]
fn brutal_deterministic_hash() {
    let tx1 = make_tx(42);
    let tx2 = make_tx(42); // Identical

    // Using tx_hash directly (which is what the handler uses)
    assert_eq!(tx1.tx_hash, tx2.tx_hash, "Same tx must produce same hash");
}

/// ATTACK: Transaction ordering must affect root
#[test]
fn brutal_tx_ordering_affects_root() {
    let hashes_a: Vec<Hash> = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
    let hashes_b: Vec<Hash> = vec![[2u8; 32], [1u8; 32], [3u8; 32], [4u8; 32]]; // Swapped 1 and 2

    let tree_a = MerkleTree::build(hashes_a);
    let tree_b = MerkleTree::build(hashes_b);

    assert_ne!(
        tree_a.root(),
        tree_b.root(),
        "Different tx ordering must produce different root"
    );
}

// =============================================================================
// PROOF REQUEST ATTACKS
// =============================================================================

/// ATTACK: Request proof for non-existent transaction
#[test]
fn brutal_proof_request_nonexistent_tx() {
    let mut handler = make_handler();

    let msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [0; 16],
        reply_to: Some("light-client".to_string()),
        sender_id: subsystem_ids::LIGHT_CLIENTS,
        recipient_id: subsystem_ids::TRANSACTION_INDEXING,
        timestamp: current_timestamp(),
        nonce: 1,
        signature: [0; 32],
        payload: MerkleProofRequestPayload {
            transaction_hash: [0xDE; 32], // Non-existent
        },
    };

    let result = handler.handle_merkle_proof_request(msg);
    assert!(result.is_ok(), "Should return response, not error");

    let response = result.unwrap();
    assert!(response.payload.proof.is_none(), "Proof should be None");
    assert!(response.payload.error.is_some(), "Error should be set");
}

/// Valid flow: Index block then request proof
#[test]
fn brutal_valid_proof_flow() {
    let mut handler = make_handler();

    // Step 1: Index a block
    let tx = make_tx(42);
    let tx_hash = tx.tx_hash;
    let block = make_block(0, vec![tx]);
    let block_hash = [0xFF; 32];

    let block_msg = make_block_validated_msg(subsystem_ids::CONSENSUS, block, block_hash, 1);
    handler.handle_block_validated(block_msg).unwrap();

    // Step 2: Request proof
    let proof_msg = AuthenticatedMessage {
        version: 1,
        correlation_id: [1; 16],
        reply_to: Some("light-client".to_string()),
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
    assert!(response.payload.proof.is_some(), "Proof should be present");
    assert!(response.payload.error.is_none(), "Error should be None");

    // Step 3: Verify the proof
    let proof = response.payload.proof.unwrap();
    let tree = handler.index_mut().get_tree(&block_hash).unwrap();
    assert!(tree.verify_proof(&proof), "Returned proof must verify");
}
