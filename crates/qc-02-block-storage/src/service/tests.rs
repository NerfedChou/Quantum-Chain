//! # Block Storage Service Tests

use super::*;
use crate::adapters::{
    BincodeBlockSerializer, DefaultChecksumProvider, InMemoryKVStore, MockFileSystemAdapter,
    SystemTimeSource,
};
use crate::ports::inbound::{BlockAssemblerApi, BlockStorageApi};
use crate::ports::outbound::KeyValueStore;
use shared_types::{Hash, ValidatedBlock};

fn make_test_service() -> BlockStorageService<
    InMemoryKVStore,
    MockFileSystemAdapter,
    DefaultChecksumProvider,
    SystemTimeSource,
    BincodeBlockSerializer,
> {
    let deps = BlockStorageDependencies {
        kv_store: InMemoryKVStore::new(),
        fs_adapter: MockFileSystemAdapter::new(50),
        checksum: DefaultChecksumProvider,
        time_source: SystemTimeSource,
        serializer: BincodeBlockSerializer,
    };
    BlockStorageService::new(deps, StorageConfig::default())
}

use crate::test_utils::{make_test_block, ZERO_HASH};

const MERKLE_ROOT: [u8; 32] = [0xAA; 32];
const STATE_ROOT: [u8; 32] = [0xBB; 32];

#[test]
fn test_write_and_read_block() {
    let mut service = make_test_service();

    let block = make_test_block(0, ZERO_HASH);
    let merkle_root = MERKLE_ROOT;
    let state_root = STATE_ROOT;

    let hash = service
        .write_block(block.clone(), merkle_root, state_root)
        .unwrap();

    let stored = service.read_block(&hash).unwrap();
    assert_eq!(stored.merkle_root, merkle_root);
    assert_eq!(stored.state_root, state_root);
    assert_eq!(stored.block.header.height, 0);
}

#[test]
fn test_disk_full_invariant() {
    let deps = BlockStorageDependencies {
        kv_store: InMemoryKVStore::new(),
        fs_adapter: MockFileSystemAdapter::new(4), // Below 5% threshold
        checksum: DefaultChecksumProvider,
        time_source: SystemTimeSource,
        serializer: BincodeBlockSerializer,
    };
    let mut service = BlockStorageService::new(deps, StorageConfig::default());

    let block = make_test_block(0, ZERO_HASH);
    let result = service.write_block(block, ZERO_HASH, ZERO_HASH);

    assert!(matches!(result, Err(StorageError::DiskFull { .. })));
}

#[test]
fn test_parent_not_found_invariant() {
    let mut service = make_test_service();

    // Try to write block at height 5 without parents
    let block = make_test_block(5, [0xFF; 32]);
    let result = service.write_block(block, ZERO_HASH, ZERO_HASH);

    assert!(matches!(result, Err(StorageError::ParentNotFound { .. })));
}

#[test]
fn test_finalization_monotonicity() {
    let mut service = make_test_service();

    // Write 10 blocks
    let mut parent_hash = ZERO_HASH;
    for height in 0..10 {
        let block = make_test_block(height, parent_hash);
        parent_hash = service.write_block(block, ZERO_HASH, ZERO_HASH).unwrap();
    }

    // Finalize height 5
    service.mark_finalized(5).unwrap();

    // Cannot regress to height 3
    let result = service.mark_finalized(3);
    assert!(matches!(
        result,
        Err(StorageError::InvalidFinalization { .. })
    ));

    // Can finalize higher
    service.mark_finalized(7).unwrap();
    assert_eq!(service.get_finalized_height().unwrap(), 7);
}

#[test]
fn test_choreography_assembly() {
    let mut service = make_test_service();

    let block = make_test_block(0, ZERO_HASH);
    let block_hash = service.compute_block_hash(&block);
    let now = 1000;

    // Send events in choreography order
    service.on_block_validated(block.clone(), now).unwrap();

    // Block not written yet (need merkle + state)
    assert!(!service.block_exists(&block_hash));

    service
        .on_merkle_root_computed(block_hash, MERKLE_ROOT, now)
        .unwrap();

    // Still not written
    assert!(!service.block_exists(&block_hash));

    service
        .on_state_root_computed(block_hash, STATE_ROOT, now)
        .unwrap();

    // INVARIANT-4: All 3 components arrived → atomic write completed
    // Verify via height (service recomputes block_hash internally)
    assert!(service.block_exists_at_height(0));
}

// DELETED test_unauthorized_sender_rejected (logic moved to IPC adapter)

// =========================================================================
// Atomic Write Guarantees (SPEC-02 Section 5.1)
// =========================================================================

#[test]
fn test_write_includes_all_required_entries() {
    let mut service = make_test_service();
    let block = make_test_block(0, ZERO_HASH);
    let merkle_root = MERKLE_ROOT;
    let state_root = STATE_ROOT;

    let hash = service.write_block(block, merkle_root, state_root).unwrap();

    assert!(service.block_exists(&hash));
    assert!(service.block_exists_at_height(0));
    let stored = service.read_block(&hash).unwrap();
    assert_eq!(stored.merkle_root, merkle_root);
    assert_eq!(stored.state_root, state_root);
}

// =========================================================================
// Disk Space Safety
// =========================================================================

#[test]
fn test_write_succeeds_when_disk_at_5_percent() {
    let deps = BlockStorageDependencies {
        kv_store: InMemoryKVStore::new(),
        fs_adapter: MockFileSystemAdapter::new(5),
        checksum: DefaultChecksumProvider,
        time_source: SystemTimeSource,
        serializer: BincodeBlockSerializer,
    };
    let mut service = BlockStorageService::new(deps, StorageConfig::default());

    let block = make_test_block(0, ZERO_HASH);
    let result = service.write_block(block, ZERO_HASH, ZERO_HASH);

    assert!(result.is_ok());
}

// =========================================================================
// Data Integrity / Checksum
// =========================================================================

#[test]
fn test_valid_checksum_passes_verification() {
    let mut service = make_test_service();
    let block = make_test_block(0, ZERO_HASH);

    let hash = service.write_block(block, MERKLE_ROOT, STATE_ROOT).unwrap();

    let result = service.read_block(&hash);
    assert!(result.is_ok());
}

// =========================================================================
// Sequential Block Requirement
// =========================================================================

#[test]
fn test_genesis_block_has_no_parent_requirement() {
    let mut service = make_test_service();
    let genesis = make_test_block(0, ZERO_HASH);

    let result = service.write_block(genesis, ZERO_HASH, ZERO_HASH);
    assert!(result.is_ok());
}

#[test]
fn test_write_succeeds_with_parent_present() {
    let mut service = make_test_service();

    let genesis = make_test_block(0, ZERO_HASH);
    let genesis_hash = service.write_block(genesis, ZERO_HASH, ZERO_HASH).unwrap();

    let child = make_test_block(1, genesis_hash);
    let result = service.write_block(child, ZERO_HASH, ZERO_HASH);

    assert!(result.is_ok());
}

// =========================================================================
// Finalization Logic
// =========================================================================

#[test]
fn test_finalization_rejects_same_height() {
    let mut service = make_test_service();

    let mut parent_hash = ZERO_HASH;
    for height in 0..6 {
        let block = make_test_block(height, parent_hash);
        parent_hash = service.write_block(block, ZERO_HASH, ZERO_HASH).unwrap();
    }

    service.mark_finalized(3).unwrap();

    let result = service.mark_finalized(3);
    assert!(matches!(
        result,
        Err(StorageError::InvalidFinalization { .. })
    ));
}

#[test]
fn test_finalization_requires_block_exists() {
    let mut service = make_test_service();

    let genesis = make_test_block(0, ZERO_HASH);
    service.write_block(genesis, ZERO_HASH, ZERO_HASH).unwrap();

    let result = service.mark_finalized(100);
    assert!(matches!(result, Err(StorageError::HeightNotFound { .. })));
}

// =========================================================================

// =========================================================================
// Batch Read / Node Syncing
// =========================================================================

#[test]
fn test_read_block_range_returns_sequential_blocks() {
    let mut service = make_test_service();

    let mut parent_hash = ZERO_HASH;
    for height in 0..21 {
        let block = make_test_block(height, parent_hash);
        parent_hash = service.write_block(block, ZERO_HASH, ZERO_HASH).unwrap();
    }

    let blocks = service.read_block_range(5, 10).unwrap();

    assert_eq!(blocks.len(), 10);
    for (i, block) in blocks.iter().enumerate() {
        assert_eq!(block.block.header.height, 5 + i as u64);
    }
}

#[test]
fn test_read_block_range_respects_limit_cap() {
    let mut service = make_test_service();

    let mut parent_hash = ZERO_HASH;
    for height in 0..150 {
        let block = make_test_block(height, parent_hash);
        parent_hash = service.write_block(block, ZERO_HASH, ZERO_HASH).unwrap();
    }

    let blocks = service.read_block_range(0, 500).unwrap();

    assert_eq!(blocks.len(), 100);
}

#[test]
fn test_read_block_range_returns_partial_if_chain_end() {
    let mut service = make_test_service();

    let mut parent_hash = ZERO_HASH;
    for height in 0..10 {
        let block = make_test_block(height, parent_hash);
        parent_hash = service.write_block(block, ZERO_HASH, ZERO_HASH).unwrap();
    }

    let blocks = service.read_block_range(5, 20).unwrap();

    assert_eq!(blocks.len(), 5);
}

#[test]
fn test_read_block_range_fails_on_invalid_start() {
    let mut service = make_test_service();

    let genesis = make_test_block(0, ZERO_HASH);
    service.write_block(genesis, ZERO_HASH, ZERO_HASH).unwrap();

    let result = service.read_block_range(100, 10);
    assert!(matches!(result, Err(StorageError::HeightNotFound { .. })));
}

// =========================================================================
// Stateful Assembler
// =========================================================================

#[test]
fn test_assembly_buffers_partial_components() {
    let mut service = make_test_service();
    let block = make_test_block(0, ZERO_HASH);
    let block_hash = service.compute_block_hash(&block);
    let now = 1000;

    service.on_block_validated(block, now).unwrap();
    service
        .on_merkle_root_computed(block_hash, MERKLE_ROOT, now)
        .unwrap();

    assert!(!service.block_exists_at_height(0));
}

// =========================================================================
// Transaction Data Retrieval
// =========================================================================

#[test]
fn test_get_transaction_location_returns_not_found() {
    let service = make_test_service();
    let unknown_tx_hash = [0xFF; 32];

    let result = service.get_transaction_location(&unknown_tx_hash);
    assert!(matches!(
        result,
        Err(StorageError::TransactionNotFound { .. })
    ));
}

#[test]
fn test_get_transaction_hashes_for_block_not_found() {
    let service = make_test_service();
    let unknown_block_hash = [0xFF; 32];

    let result = service.get_transaction_hashes_for_block(&unknown_block_hash);
    assert!(matches!(result, Err(StorageError::BlockNotFound { .. })));
}

#[test]
fn test_block_exists_at_height_returns_false_for_unknown() {
    let service = make_test_service();

    assert!(!service.block_exists_at_height(999));
}

#[test]
fn test_get_metadata_returns_default_on_empty() {
    let service = make_test_service();

    let metadata = service.get_metadata().unwrap();
    assert_eq!(metadata.latest_height, 0);
    assert_eq!(metadata.finalized_height, 0);
}

#[test]
fn test_get_latest_height_updates_after_write() {
    let mut service = make_test_service();

    assert_eq!(service.get_latest_height().unwrap(), 0);

    let genesis = make_test_block(0, ZERO_HASH);
    let genesis_hash = service.write_block(genesis, ZERO_HASH, ZERO_HASH).unwrap();

    let block1 = make_test_block(1, genesis_hash);
    service.write_block(block1, ZERO_HASH, ZERO_HASH).unwrap();

    assert_eq!(service.get_latest_height().unwrap(), 1);
}

#[test]
fn test_duplicate_block_write_rejected() {
    let mut service = make_test_service();

    let block = make_test_block(0, ZERO_HASH);

    let _hash = service
        .write_block(block.clone(), MERKLE_ROOT, STATE_ROOT)
        .unwrap();

    let result = service.write_block(block, MERKLE_ROOT, STATE_ROOT);
    assert!(matches!(result, Err(StorageError::BlockExists { .. })));
}

#[test]
fn test_persistent_transaction_index() {
    use shared_types::{Transaction, ValidatedTransaction};

    let config = StorageConfig::new().with_persist_transaction_index(true);
    let kv_store = InMemoryKVStore::new();

    let deps = BlockStorageDependencies {
        kv_store,
        fs_adapter: MockFileSystemAdapter::new(50),
        checksum: DefaultChecksumProvider,
        time_source: SystemTimeSource,
        serializer: BincodeBlockSerializer,
    };
    let mut service = BlockStorageService::new(deps, config);

    let mut block = make_test_block(0, ZERO_HASH);
    let tx_hash = [0xDE; 32];
    let inner_tx = Transaction {
        from: MERKLE_ROOT,
        to: Some(STATE_ROOT),
        value: 100,
        nonce: 0,
        data: vec![],
        signature: [0u8; 64],
    };
    let validated_tx = ValidatedTransaction {
        inner: inner_tx,
        tx_hash,
    };
    block.transactions.push(validated_tx);

    let _hash = service.write_block(block, MERKLE_ROOT, STATE_ROOT).unwrap();

    let location = service.get_transaction_location(&tx_hash).unwrap();
    assert_eq!(location.block_height, 0);
    assert_eq!(location.transaction_index, 0);

    let tx_key = KeyPrefix::transaction_key(&tx_hash);
    assert!(service.kv_store.exists(&tx_key).unwrap());
}
