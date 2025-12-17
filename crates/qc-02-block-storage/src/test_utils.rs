use shared_types::{BlockHeader, ConsensusProof, ValidatedBlock, U256};

pub fn make_test_block(height: u64, parent_hash: [u8; 32]) -> ValidatedBlock {
    ValidatedBlock {
        header: BlockHeader {
            version: 1,
            height,
            parent_hash,
            merkle_root: [0; 32],
            state_root: [0; 32],
            timestamp: 1000 + height,
            proposer: [0xAA; 32],
            difficulty: U256::from(2).pow(U256::from(252)),
            nonce: 0,
        },
        transactions: vec![],
        consensus_proof: ConsensusProof {
            block_hash: [height as u8; 32],
            attestations: vec![],
            total_stake: 0,
        },
    }
}

pub fn compute_test_block_hash(block: &ValidatedBlock) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(block.header.version.to_le_bytes());
    hasher.update(block.header.height.to_le_bytes());
    hasher.update(block.header.parent_hash);
    hasher.update(block.header.merkle_root);
    hasher.update(block.header.state_root);
    hasher.update(block.header.timestamp.to_le_bytes());
    hasher.update(block.header.proposer);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub const ZERO_HASH: [u8; 32] = [0; 32];

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
