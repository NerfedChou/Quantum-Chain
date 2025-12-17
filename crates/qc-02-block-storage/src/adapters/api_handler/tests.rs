//! # API Handler Tests

use super::*;
use crate::domain::errors::StorageError;
use crate::domain::storage::{StorageMetadata, StoredBlock};
use crate::domain::value_objects::TransactionLocation;
use crate::ports::inbound::BlockStorageApi;
use shared_types::{BlockHeader, ConsensusProof, Hash, ValidatedBlock, U256};

/// Mock service for testing
struct MockStorageService {
    latest_height: u64,
    finalized_height: u64,
}

impl MockStorageService {
    fn new() -> Self {
        Self {
            latest_height: 1000,
            finalized_height: 990,
        }
    }
}

impl BlockStorageApi for MockStorageService {
    fn write_block(
        &mut self,
        _block: ValidatedBlock,
        _merkle_root: Hash,
        _state_root: Hash,
    ) -> Result<Hash, StorageError> {
        Ok([0; 32])
    }

    fn read_block(&self, _hash: &Hash) -> Result<StoredBlock, StorageError> {
        Err(StorageError::BlockNotFound { hash: [0; 32] })
    }

    fn read_block_by_height(&self, height: u64) -> Result<StoredBlock, StorageError> {
        if height <= self.latest_height {
            Ok(StoredBlock {
                block: ValidatedBlock {
                    header: BlockHeader {
                        version: 1,
                        height,
                        parent_hash: [0; 32],
                        merkle_root: [0xAA; 32],
                        state_root: [0xBB; 32],
                        timestamp: 1700000000 + height,
                        proposer: [0; 32],
                        difficulty: U256::from(2).pow(U256::from(252)),
                        nonce: 0,
                    },
                    transactions: vec![],
                    consensus_proof: ConsensusProof::default(),
                },
                merkle_root: [0xAA; 32],
                state_root: [0xBB; 32],
                stored_at: 1700000000,
                checksum: 0,
            })
        } else {
            Err(StorageError::HeightNotFound { height })
        }
    }

    fn read_block_range(
        &self,
        _start_height: u64,
        _limit: u64,
    ) -> Result<Vec<StoredBlock>, StorageError> {
        Ok(vec![])
    }

    fn mark_finalized(&mut self, _height: u64) -> Result<(), StorageError> {
        Ok(())
    }

    fn get_metadata(&self) -> Result<StorageMetadata, StorageError> {
        Ok(StorageMetadata {
            genesis_hash: Some([0; 32]),
            latest_height: self.latest_height,
            finalized_height: self.finalized_height,
            total_blocks: self.latest_height + 1,
            storage_version: 1,
        })
    }

    fn get_latest_height(&self) -> Result<u64, StorageError> {
        Ok(self.latest_height)
    }

    fn get_finalized_height(&self) -> Result<u64, StorageError> {
        Ok(self.finalized_height)
    }

    fn block_exists(&self, _hash: &Hash) -> bool {
        false
    }

    fn block_exists_at_height(&self, height: u64) -> bool {
        height <= self.latest_height
    }

    fn get_transaction_location(
        &self,
        _transaction_hash: &Hash,
    ) -> Result<TransactionLocation, StorageError> {
        Err(StorageError::TransactionNotFound { tx_hash: [0; 32] })
    }

    fn get_transaction_hashes_for_block(
        &self,
        _block_hash: &Hash,
    ) -> Result<Vec<Hash>, StorageError> {
        Ok(vec![])
    }
}

use crate::adapters::security::RateLimitConfig;

// ...

#[test]
fn test_handle_block_number() {
    let service = MockStorageService::new();
    let handler = ApiGatewayHandler::new(service, 100_000_000_000, 500_000_000_000, RateLimitConfig::default());

    let result = handler.handle_block_number();
    assert!(result.get("result").is_some());
    assert_eq!(result["result"], "0x3e8"); // 1000 in hex
}

#[test]
fn test_handle_get_metrics() {
    let service = MockStorageService::new();
    let handler = ApiGatewayHandler::new(service, 100_000_000_000, 500_000_000_000, RateLimitConfig::default());

    let result = handler.handle_get_metrics();
    assert_eq!(result["latest_height"], 1000);
    assert_eq!(result["finalized_height"], 990);
    assert_eq!(result["disk_usage_percent"], 20);
}

#[test]
fn test_handle_ping() {
    let service = MockStorageService::new();
    let handler = ApiGatewayHandler::new(service, 0, 0, RateLimitConfig::default());

    let result = handler.handle_ping();
    assert_eq!(result["result"], "pong");
    assert_eq!(result["subsystem"], "qc-02-block-storage");
}

#[test]
fn test_api_query_dispatch() {
    let service = MockStorageService::new();
    let handler = ApiGatewayHandler::new(service, 0, 0, RateLimitConfig::default());

    // Test eth_blockNumber
    let result = handle_api_query(&handler, "eth_blockNumber", &serde_json::json!([]));
    assert!(result.is_ok());

    // Test unknown method
    let result = handle_api_query(&handler, "unknown_method", &serde_json::json!([]));
    assert!(matches!(result, Err(ApiQueryError::UnknownMethod(_))));
}
