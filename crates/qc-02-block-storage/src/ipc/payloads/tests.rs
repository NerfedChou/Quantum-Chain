//! # Payload Tests

use super::*;

#[test]
fn test_block_query_variants() {
    let by_hash = BlockQuery::ByHash([0xAB; 32]);
    let by_height = BlockQuery::ByHeight(100);

    match by_hash {
        BlockQuery::ByHash(h) => assert_eq!(h, [0xAB; 32]),
        _ => panic!("Expected ByHash"),
    }

    match by_height {
        BlockQuery::ByHeight(h) => assert_eq!(h, 100),
        _ => panic!("Expected ByHeight"),
    }
}

#[test]
fn test_storage_error_payload() {
    let error = StorageErrorPayload {
        error_type: StorageErrorType::BlockNotFound,
        message: "Block not found".into(),
        block_hash: Some([0xAB; 32]),
        block_height: None,
    };

    assert_eq!(error.error_type, StorageErrorType::BlockNotFound);
    assert!(error.block_hash.is_some());
}

#[test]
fn test_get_chain_info_request() {
    let request = GetChainInfoRequestPayload {
        recent_blocks_count: 24,
    };

    assert_eq!(request.recent_blocks_count, 24);
}

#[test]
fn test_block_difficulty_info() {
    use shared_types::U256;

    let info = BlockDifficultyInfo {
        height: 1000,
        timestamp: 1700000000,
        difficulty: U256::from(2).pow(U256::from(235)),
        hash: [0xAB; 32],
    };

    assert_eq!(info.height, 1000);
    assert_eq!(info.timestamp, 1700000000);
    assert_eq!(info.hash, [0xAB; 32]);
}

#[test]
fn test_chain_info_response_empty_chain() {
    let response = ChainInfoResponsePayload {
        chain_tip_height: 0,
        chain_tip_hash: [0; 32],
        chain_tip_timestamp: 0,
        recent_blocks: vec![],
    };

    assert_eq!(response.chain_tip_height, 0);
    assert!(response.recent_blocks.is_empty());
}
