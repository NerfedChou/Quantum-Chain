//! Chain state management
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2.2

use super::BlockHeader;
use shared_types::Hash;
use std::collections::HashMap;

/// Current chain head information
///
/// Reference: SPEC-08 Section 3.1
#[derive(Clone, Debug, Default)]
pub struct ChainHead {
    pub block_hash: Hash,
    pub block_height: u64,
    pub timestamp: u64,
}

/// Chain state tracking known blocks
///
/// Maintains the local view of validated blocks
pub struct ChainState {
    /// Known validated block headers by hash
    known_blocks: HashMap<Hash, BlockHeader>,
    /// Current chain head
    head: ChainHead,
    /// Block hash to height mapping for quick lookups
    height_index: HashMap<u64, Hash>,
}

impl ChainState {
    /// Create a new chain state
    pub fn new() -> Self {
        Self {
            known_blocks: HashMap::new(),
            head: ChainHead::default(),
            height_index: HashMap::new(),
        }
    }

    /// Create chain state with genesis block
    pub fn with_genesis(genesis: BlockHeader) -> Self {
        let mut state = Self::new();
        state.add_block(genesis);
        state
    }

    /// Add a validated block to the chain state
    pub fn add_block(&mut self, header: BlockHeader) {
        let hash = header.hash();
        let height = header.block_height;
        let timestamp = header.timestamp;

        // Update head if this is the highest block
        if height > self.head.block_height || self.head.block_height == 0 {
            self.head = ChainHead {
                block_hash: hash,
                block_height: height,
                timestamp,
            };
        }

        self.height_index.insert(height, hash);
        self.known_blocks.insert(hash, header);
    }

    /// Check if a block hash is known
    pub fn has_block(&self, hash: &Hash) -> bool {
        self.known_blocks.contains_key(hash)
    }

    /// Get a block header by hash
    pub fn get_block(&self, hash: &Hash) -> Option<&BlockHeader> {
        self.known_blocks.get(hash)
    }

    /// Get block hash at a specific height
    pub fn get_hash_at_height(&self, height: u64) -> Option<&Hash> {
        self.height_index.get(&height)
    }

    /// Get the current chain head
    pub fn head(&self) -> &ChainHead {
        &self.head
    }

    /// Get the current height
    pub fn height(&self) -> u64 {
        self.head.block_height
    }

    /// Get count of known blocks
    pub fn block_count(&self) -> usize {
        self.known_blocks.len()
    }

    /// Validate parent chain linkage
    ///
    /// INVARIANT-1: Block parent_hash must reference an existing validated block
    pub fn validate_parent(&self, header: &BlockHeader) -> bool {
        // Genesis block has no parent
        if header.is_genesis() {
            return true;
        }

        self.has_block(&header.parent_hash)
    }

    /// Validate sequential height
    ///
    /// INVARIANT-4: Block height must be parent height + 1
    pub fn validate_height(&self, header: &BlockHeader) -> bool {
        if header.is_genesis() {
            return header.block_height == 0;
        }

        if let Some(parent) = self.get_block(&header.parent_hash) {
            header.block_height == parent.block_height + 1
        } else {
            false
        }
    }

    /// Validate timestamp ordering
    ///
    /// INVARIANT-5: Block timestamp must be > parent timestamp
    pub fn validate_timestamp(&self, header: &BlockHeader) -> bool {
        if header.is_genesis() {
            return true;
        }

        if let Some(parent) = self.get_block(&header.parent_hash) {
            header.timestamp > parent.timestamp
        } else {
            false
        }
    }
}

impl Default for ChainState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_genesis() -> BlockHeader {
        BlockHeader {
            version: 1,
            block_height: 0,
            parent_hash: [0u8; 32],
            timestamp: 1000,
            proposer: [0u8; 32],
            transactions_root: None,
            state_root: None,
            receipts_root: [0u8; 32],
            gas_limit: 30_000_000,
            gas_used: 0,
            extra_data: vec![],
        }
    }

    fn create_child(parent: &BlockHeader) -> BlockHeader {
        BlockHeader {
            version: 1,
            block_height: parent.block_height + 1,
            parent_hash: parent.hash(),
            timestamp: parent.timestamp + 12,
            proposer: [0u8; 32],
            transactions_root: None,
            state_root: None,
            receipts_root: [0u8; 32],
            gas_limit: 30_000_000,
            gas_used: 0,
            extra_data: vec![],
        }
    }

    #[test]
    fn test_chain_state_genesis() {
        let genesis = create_genesis();
        let state = ChainState::with_genesis(genesis.clone());

        assert_eq!(state.height(), 0);
        assert!(state.has_block(&genesis.hash()));
    }

    #[test]
    fn test_chain_state_add_block() {
        let genesis = create_genesis();
        let mut state = ChainState::with_genesis(genesis.clone());

        let block1 = create_child(&genesis);
        state.add_block(block1.clone());

        assert_eq!(state.height(), 1);
        assert!(state.has_block(&block1.hash()));
    }

    #[test]
    fn test_validate_parent() {
        let genesis = create_genesis();
        let mut state = ChainState::with_genesis(genesis.clone());

        let valid_child = create_child(&genesis);
        assert!(state.validate_parent(&valid_child));

        let orphan = BlockHeader {
            parent_hash: [0xFF; 32],
            ..create_child(&genesis)
        };
        assert!(!state.validate_parent(&orphan));
    }

    #[test]
    fn test_validate_height() {
        let genesis = create_genesis();
        let mut state = ChainState::with_genesis(genesis.clone());

        let valid_child = create_child(&genesis);
        state.add_block(valid_child.clone());

        // Height skip should fail
        let skip_block = BlockHeader {
            block_height: 5,
            parent_hash: valid_child.hash(),
            ..create_child(&valid_child)
        };
        assert!(!state.validate_height(&skip_block));
    }

    #[test]
    fn test_validate_timestamp() {
        let genesis = create_genesis();
        let mut state = ChainState::with_genesis(genesis.clone());

        // Valid: timestamp > parent
        let valid_child = create_child(&genesis);
        assert!(state.validate_timestamp(&valid_child));

        // Invalid: timestamp <= parent
        let invalid_child = BlockHeader {
            timestamp: genesis.timestamp,
            ..create_child(&genesis)
        };
        assert!(!state.validate_timestamp(&invalid_child));
    }
}
