//! # Handler Tests

use super::core::BlockStorageHandler;
use super::types::HandlerError;
use crate::adapters::{
    BincodeBlockSerializer, DefaultChecksumProvider, InMemoryKVStore, MockFileSystemAdapter,
    SystemTimeSource,
};
use crate::domain::value_objects::StorageConfig;
use crate::ipc::envelope::{subsystem_ids, AuthenticatedMessage, EnvelopeError};
use crate::ipc::payloads::*;
use crate::service::BlockStorageService;

fn make_test_handler() -> BlockStorageHandler<
    InMemoryKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    SystemTimeSource,
    BincodeBlockSerializer,
> {
    let deps = crate::service::BlockStorageDependencies {
        kv_store: InMemoryKVStore::new(),
        fs_adapter: MockFileSystemAdapter::new(50),
        checksum: DefaultChecksumProvider,
        time_source: SystemTimeSource,
        serializer: BincodeBlockSerializer,
    };
    let service = BlockStorageService::new(deps, StorageConfig::default());
    BlockStorageHandler::new(service, [0u8; 32])
}

use crate::test_utils::{
    compute_test_block_hash, current_timestamp, make_test_block, MessageBuilder,
};

fn make_msg<T: Clone>(sender_id: u8, payload: T, nonce: u64) -> AuthenticatedMessage<T> {
    MessageBuilder::new()
        .sender(sender_id)
        .with_payload(payload)
        .nonce(nonce)
        .build()
}

#[test]
fn test_handle_block_validated_from_consensus() {
    let mut handler = make_test_handler();
    let block = make_test_block(0, [0; 32]);

    let msg = make_msg(
        subsystem_ids::CONSENSUS,
        BlockValidatedPayload {
            block: block.clone(),
            block_hash: [0; 32],
            block_height: 0,
        },
        1,
    );

    let result = handler.handle_block_validated(msg);
    assert!(result.is_ok());
}

#[test]
fn test_handle_block_validated_rejects_wrong_sender() {
    let mut handler = make_test_handler();
    let block = make_test_block(0, [0; 32]);

    let msg = make_msg(
        subsystem_ids::MEMPOOL, // Wrong sender!
        BlockValidatedPayload {
            block,
            block_hash: [0; 32],
            block_height: 0,
        },
        1,
    );

    let result = handler.handle_block_validated(msg);
    assert!(matches!(
        result,
        Err(HandlerError::Envelope(
            EnvelopeError::UnauthorizedSender { .. }
        ))
    ));
}

#[test]
fn test_choreography_assembly_via_handler() {
    let mut handler = make_test_handler();
    let block = make_test_block(0, [0; 32]);

    // Test 1: BlockValidated from wrong sender is rejected
    let wrong_sender_msg = make_msg(
        subsystem_ids::MEMPOOL, // Wrong!
        BlockValidatedPayload {
            block: block.clone(),
            block_hash: [0; 32],
            block_height: 0,
        },
        100,
    );
    assert!(handler.handle_block_validated(wrong_sender_msg).is_err());

    // Test 2: Valid BlockValidated is accepted
    let valid_msg = make_msg(
        subsystem_ids::CONSENSUS,
        BlockValidatedPayload {
            block,
            block_hash: [0; 32],
            block_height: 0,
        },
        101,
    );
    assert!(handler.handle_block_validated(valid_msg).is_ok());

    // Test 3: MerkleRootComputed from wrong sender is rejected
    let wrong_merkle_msg = make_msg(
        subsystem_ids::CONSENSUS, // INVALID: Must be TRANSACTION_INDEXING
        MerkleRootComputedPayload {
            block_hash: [0; 32],
            merkle_root: [0xAA; 32],
        },
        102,
    );
    assert!(handler
        .handle_merkle_root_computed(wrong_merkle_msg)
        .is_err());

    // Test 4: StateRootComputed from wrong sender is rejected
    let wrong_state_msg = make_msg(
        subsystem_ids::CONSENSUS, // INVALID: Must be STATE_MANAGEMENT
        StateRootComputedPayload {
            block_hash: [0; 32],
            state_root: [0xBB; 32],
        },
        103,
    );
    assert!(handler.handle_state_root_computed(wrong_state_msg).is_err());
}

#[test]
fn test_get_chain_info_empty_chain() {
    let mut handler = make_test_handler();

    let msg = make_msg(
        subsystem_ids::BLOCK_PRODUCTION,
        GetChainInfoRequestPayload {
            recent_blocks_count: 24,
        },
        100,
    );

    let result = handler.handle_get_chain_info(msg);
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.payload.chain_tip_height, 0);
    assert!(response.payload.recent_blocks.is_empty());
}

#[test]
fn test_get_chain_info_with_blocks() {
    let mut handler = make_test_handler();

    let block = make_test_block(0, [0; 32]);
    let block_hash = compute_test_block_hash(&block);

    // Send all three choreography events
    let validated_msg = make_msg(
        subsystem_ids::CONSENSUS,
        BlockValidatedPayload {
            block,
            block_hash,
            block_height: 0,
        },
        100,
    );
    handler.handle_block_validated(validated_msg).unwrap();

    let merkle_msg = make_msg(
        subsystem_ids::TRANSACTION_INDEXING,
        MerkleRootComputedPayload {
            block_hash,
            merkle_root: [0xAA; 32],
        },
        101,
    );
    handler.handle_merkle_root_computed(merkle_msg).unwrap();

    let state_msg = make_msg(
        subsystem_ids::STATE_MANAGEMENT,
        StateRootComputedPayload {
            block_hash,
            state_root: [0xBB; 32],
        },
        102,
    );
    handler.handle_state_root_computed(state_msg).unwrap();

    // Now query chain info
    let chain_info_msg = make_msg(
        subsystem_ids::BLOCK_PRODUCTION,
        GetChainInfoRequestPayload {
            recent_blocks_count: 24,
        },
        103,
    );

    let result = handler.handle_get_chain_info(chain_info_msg);
    assert!(result.is_ok());
}
