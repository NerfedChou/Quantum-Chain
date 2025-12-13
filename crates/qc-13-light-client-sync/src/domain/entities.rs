//! # Domain Entities
//!
//! Core entities for Light Client Sync.
//!
//! Reference: SPEC-13 Section 2.1 (Lines 58-97)

use super::errors::{Hash, LightClientError};
use super::value_objects::{ChainTip, Checkpoint, MerkleProof};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Block header structure (minimal for SPV).
/// Reference: System.md Line 624 (~80 bytes/block)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    /// Hash of this block.
    pub hash: Hash,
    /// Hash of parent block.
    pub parent_hash: Hash,
    /// Block height.
    pub height: u64,
    /// Unix timestamp.
    pub timestamp: u64,
    /// Merkle root of transactions.
    pub merkle_root: Hash,
    /// Difficulty target (for PoW validation).
    pub difficulty: u64,
    /// Nonce (for PoW validation).
    pub nonce: u64,
}

impl BlockHeader {
    /// Create a new block header.
    pub fn new(
        hash: Hash,
        parent_hash: Hash,
        height: u64,
        timestamp: u64,
        merkle_root: Hash,
    ) -> Self {
        Self {
            hash,
            parent_hash,
            height,
            timestamp,
            merkle_root,
            difficulty: 0,
            nonce: 0,
        }
    }

    /// Create genesis header.
    pub fn genesis(hash: Hash, timestamp: u64, merkle_root: Hash) -> Self {
        Self {
            hash,
            parent_hash: [0u8; 32],
            height: 0,
            timestamp,
            merkle_root,
            difficulty: 0,
            nonce: 0,
        }
    }
}

/// Header chain for SPV (stores only headers, not full blocks).
/// Reference: SPEC-13 Lines 59-72
#[derive(Clone, Debug)]
pub struct HeaderChain {
    /// Headers indexed by hash.
    headers: HashMap<Hash, BlockHeader>,
    /// Hash indexed by height.
    by_height: BTreeMap<u64, Hash>,
    /// Current chain tip hash.
    tip: Hash,
    /// Current chain height.
    height: u64,
    /// Trusted checkpoints.
    checkpoints: Vec<Checkpoint>,
}

impl HeaderChain {
    /// Create a new header chain with genesis block.
    pub fn new(genesis: BlockHeader) -> Self {
        let hash = genesis.hash;
        let mut headers = HashMap::new();
        let mut by_height = BTreeMap::new();

        headers.insert(hash, genesis);
        by_height.insert(0, hash);

        Self {
            headers,
            by_height,
            tip: hash,
            height: 0,
            checkpoints: Vec::new(),
        }
    }

    /// Append a new header to the chain.
    ///
    /// Reference: SPEC-13 Line 365
    ///
    /// # Errors
    /// - `InvalidHeaderChain` if parent hash doesn't match tip
    /// - `InvalidHeaderChain` if height is not consecutive
    /// - `InvalidHeaderChain` if timestamp doesn't progress
    pub fn append(&mut self, header: BlockHeader) -> Result<(), LightClientError> {
        // Validate parent hash
        if header.parent_hash != self.tip {
            return Err(LightClientError::InvalidHeaderChain(format!(
                "Parent hash mismatch: expected {:?}, got {:?}",
                self.tip, header.parent_hash
            )));
        }

        // Validate height is consecutive
        if header.height != self.height + 1 {
            return Err(LightClientError::InvalidHeaderChain(format!(
                "Height gap: expected {}, got {}",
                self.height + 1,
                header.height
            )));
        }

        // Validate timestamp progression
        if let Some(tip_header) = self.headers.get(&self.tip) {
            if header.timestamp <= tip_header.timestamp {
                return Err(LightClientError::InvalidHeaderChain(
                    "Timestamp must increase".to_string(),
                ));
            }
        }

        // Insert header
        let hash = header.hash;
        let height = header.height;
        self.headers.insert(hash, header);
        self.by_height.insert(height, hash);
        self.tip = hash;
        self.height = height;

        Ok(())
    }

    /// Get header by hash.
    pub fn get_header(&self, hash: &Hash) -> Option<&BlockHeader> {
        self.headers.get(hash)
    }

    /// Get header by height.
    pub fn get_header_by_height(&self, height: u64) -> Option<&BlockHeader> {
        self.by_height
            .get(&height)
            .and_then(|h| self.headers.get(h))
    }

    /// Get current tip.
    pub fn get_tip(&self) -> ChainTip {
        ChainTip::new(self.tip, self.height)
    }

    /// Get current height.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Get header count.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Check if chain is empty.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Add a checkpoint.
    pub fn add_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.checkpoints.push(checkpoint);
    }

    /// Verify chain against checkpoints.
    ///
    /// Reference: System.md Line 646
    pub fn verify_checkpoints(&self) -> Result<(), LightClientError> {
        for cp in &self.checkpoints {
            let Some(hash) = self.by_height.get(&cp.height) else {
                continue;
            };
            if *hash != cp.hash {
                return Err(LightClientError::CheckpointMismatch { height: cp.height });
            }
        }
        Ok(())
    }
}

/// Transaction proven by Merkle proof.
/// Reference: SPEC-13 Lines 89-97
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenTransaction {
    /// Transaction hash.
    pub tx_hash: Hash,
    /// Block hash containing the transaction.
    pub block_hash: Hash,
    /// Block height.
    pub block_height: u64,
    /// Number of confirmations.
    pub confirmations: u64,
    /// Merkle proof of inclusion.
    pub proof: MerkleProof,
    /// Was the proof verified?
    pub verified: bool,
}

impl ProvenTransaction {
    /// Create a new unverified proven transaction.
    pub fn new(tx_hash: Hash, block_hash: Hash, block_height: u64, proof: MerkleProof) -> Self {
        Self {
            tx_hash,
            block_hash,
            block_height,
            confirmations: 0,
            proof,
            verified: false,
        }
    }

    /// Mark as verified with confirmations.
    pub fn mark_verified(&mut self, confirmations: u64) {
        self.verified = true;
        self.confirmations = confirmations;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_genesis() -> BlockHeader {
        BlockHeader::genesis([0u8; 32], 1000, [1u8; 32])
    }

    fn create_header(height: u64, parent_hash: Hash, timestamp: u64) -> BlockHeader {
        let mut hash = [0u8; 32];
        hash[0] = height as u8;
        BlockHeader::new(hash, parent_hash, height, timestamp, [2u8; 32])
    }

    #[test]
    fn test_header_chain_new() {
        let genesis = create_genesis();
        let chain = HeaderChain::new(genesis.clone());
        assert_eq!(chain.height(), 0);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_header_chain_append_valid() {
        let genesis = create_genesis();
        let mut chain = HeaderChain::new(genesis.clone());

        let header1 = create_header(1, genesis.hash, 2000);
        assert!(chain.append(header1.clone()).is_ok());
        assert_eq!(chain.height(), 1);

        let header2 = create_header(2, header1.hash, 3000);
        assert!(chain.append(header2).is_ok());
        assert_eq!(chain.height(), 2);
    }

    #[test]
    fn test_header_chain_append_invalid_parent() {
        let genesis = create_genesis();
        let mut chain = HeaderChain::new(genesis.clone());

        // Wrong parent hash
        let bad_header = create_header(1, [99u8; 32], 2000);
        let result = chain.append(bad_header);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(LightClientError::InvalidHeaderChain(_))
        ));
    }

    #[test]
    fn test_header_chain_append_invalid_height() {
        let genesis = create_genesis();
        let mut chain = HeaderChain::new(genesis.clone());

        // Skip height
        let bad_header = create_header(5, genesis.hash, 2000);
        let result = chain.append(bad_header);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_chain_append_invalid_timestamp() {
        let genesis = create_genesis();
        let mut chain = HeaderChain::new(genesis.clone());

        // Timestamp not increasing
        let bad_header = create_header(1, genesis.hash, 500);
        let result = chain.append(bad_header);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_chain_get_tip() {
        let genesis = create_genesis();
        let chain = HeaderChain::new(genesis.clone());
        let tip = chain.get_tip();
        assert_eq!(tip.hash, genesis.hash);
        assert_eq!(tip.height, 0);
    }

    #[test]
    fn test_header_chain_checkpoint_verification() {
        let genesis = create_genesis();
        let mut chain = HeaderChain::new(genesis.clone());

        // Add valid checkpoint
        chain.add_checkpoint(Checkpoint::hardcoded(0, genesis.hash));
        assert!(chain.verify_checkpoints().is_ok());

        // Add invalid checkpoint
        chain.add_checkpoint(Checkpoint::hardcoded(0, [99u8; 32]));
        assert!(chain.verify_checkpoints().is_err());
    }

    #[test]
    fn test_proven_transaction() {
        let proof = MerkleProof {
            tx_hash: [1u8; 32],
            path: vec![],
            merkle_root: [2u8; 32],
            block_hash: [3u8; 32],
            block_height: 100,
        };

        let mut ptx = ProvenTransaction::new([1u8; 32], [3u8; 32], 100, proof);
        assert!(!ptx.verified);

        ptx.mark_verified(6);
        assert!(ptx.verified);
        assert_eq!(ptx.confirmations, 6);
    }
}
