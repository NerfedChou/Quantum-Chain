//! # LMD-GHOST Fork Choice Rule
//!
//! Latest Message Driven GHOST (Greedy Heaviest Observed Subtree).
//!
//! ## Problem
//!
//! "Longest chain" fork choice is unstable in PoS with network latency.
//! An attacker with 30% stake can create a private chain and reorganize.
//!
//! ## Solution: Weight-Based Tree Traversal
//!
//! 1. Maintain store of all valid headers and latest attestation per validator
//! 2. Weight of a block = sum of stake supporting that block (or descendants)
//! 3. At each fork, choose child with highest weight
//!
//! Reference: SPEC-08-CONSENSUS.md, Ethereum Gasper

use crate::domain::{BlockHeader, ValidatorId, ValidatorSet};
use shared_types::Hash;
use std::collections::{HashMap, HashSet};

/// LMD-GHOST fork choice store.
///
/// Maintains the block tree and latest votes for efficient head computation.
#[derive(Debug)]
pub struct LMDGhostStore {
    /// Block headers indexed by hash
    blocks: HashMap<Hash, BlockHeader>,
    /// Parent -> Children mapping
    children: HashMap<Hash, Vec<Hash>>,
    /// Latest vote from each validator
    latest_votes: HashMap<ValidatorId, Hash>,
    /// Cached weights (invalidated on vote updates)
    weight_cache: HashMap<Hash, u128>,
    /// Cache valid flag
    cache_valid: bool,
    /// Justified checkpoint
    justified_checkpoint: Option<Hash>,
}

impl LMDGhostStore {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            children: HashMap::new(),
            latest_votes: HashMap::new(),
            weight_cache: HashMap::new(),
            cache_valid: false,
            justified_checkpoint: None,
        }
    }

    /// Add a block to the store.
    pub fn add_block(&mut self, header: BlockHeader) {
        let hash = header.hash();
        let parent = header.parent_hash;

        self.children.entry(parent).or_default().push(hash);
        self.blocks.insert(hash, header);
        self.invalidate_cache();
    }

    /// Record an attestation (vote) from a validator.
    pub fn on_attestation(&mut self, validator: ValidatorId, target: Hash) {
        self.latest_votes.insert(validator, target);
        self.invalidate_cache();
    }

    /// Set the justified checkpoint.
    pub fn set_justified(&mut self, checkpoint: Hash) {
        self.justified_checkpoint = Some(checkpoint);
        self.invalidate_cache();
    }

    /// Get the canonical head using GHOST algorithm.
    ///
    /// Starting from justified checkpoint, traverse tree always choosing
    /// the child with highest accumulated weight.
    pub fn get_head(&mut self, validator_set: &ValidatorSet) -> Option<Hash> {
        let justified = self.justified_checkpoint?;

        if !self.blocks.contains_key(&justified) {
            return None;
        }

        // Rebuild cache if necessary
        if !self.cache_valid {
            self.rebuild_weight_cache(validator_set);
        }

        let mut current = justified;

        loop {
            let child_hashes = match self.children.get(&current) {
                Some(children) if !children.is_empty() => children.clone(),
                _ => return Some(current), // Leaf node
            };

            // Choose child with highest weight
            let best_child = child_hashes
                .into_iter()
                .max_by_key(|c| self.get_weight(c))
                .unwrap();

            current = best_child;
        }
    }

    /// Get weight of a block (cached).
    fn get_weight(&self, block: &Hash) -> u128 {
        self.weight_cache.get(block).copied().unwrap_or(0)
    }

    /// Rebuild weight cache from latest votes.
    fn rebuild_weight_cache(&mut self, validator_set: &ValidatorSet) {
        self.weight_cache.clear();

        // For each validator's latest vote, add their stake to all ancestors
        for (validator, target) in &self.latest_votes {
            let stake = validator_set.get_stake(validator).unwrap_or(0);

            // Walk up the tree adding weight
            let mut current = *target;
            let mut visited = HashSet::new();

            while visited.insert(current) {
                let Some(header) = self.blocks.get(&current) else {
                    break;
                };

                *self.weight_cache.entry(current).or_insert(0) += stake;

                // Genesis or same as parent means stop
                if current == header.parent_hash {
                    break;
                }
                current = header.parent_hash;
            }
        }

        self.cache_valid = true;
    }

    /// Invalidate the weight cache.
    fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    /// Check if block is in the store.
    pub fn has_block(&self, hash: &Hash) -> bool {
        self.blocks.contains_key(hash)
    }

    /// Get a block header.
    pub fn get_block(&self, hash: &Hash) -> Option<&BlockHeader> {
        self.blocks.get(hash)
    }

    /// Total vote count.
    pub fn total_votes(&self) -> usize {
        self.latest_votes.len()
    }
}

impl Default for LMDGhostStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ValidatorInfo;

    fn make_header(height: u64, parent: Hash) -> BlockHeader {
        BlockHeader {
            version: 1,
            block_height: height,
            parent_hash: parent,
            timestamp: 1000 + height,
            proposer: [0; 32],
            transactions_root: None,
            state_root: None,
            receipts_root: [0; 32],
            gas_limit: 30_000_000,
            gas_used: 0,
            extra_data: vec![height as u8], // Unique per height
        }
    }

    fn make_validator_set() -> ValidatorSet {
        ValidatorSet::new(
            0,
            vec![
                ValidatorInfo {
                    id: [1; 32],
                    stake: 100,
                    pubkey: [0; 48],
                    active: true,
                },
                ValidatorInfo {
                    id: [2; 32],
                    stake: 100,
                    pubkey: [0; 48],
                    active: true,
                },
                ValidatorInfo {
                    id: [3; 32],
                    stake: 100,
                    pubkey: [0; 48],
                    active: true,
                },
            ],
        )
    }

    #[test]
    fn test_single_chain_head() {
        let mut store = LMDGhostStore::new();
        let vs = make_validator_set();

        let genesis = make_header(0, [0; 32]);
        let genesis_hash = genesis.hash();
        store.add_block(genesis);
        store.set_justified(genesis_hash);

        let block1 = make_header(1, genesis_hash);
        let b1_hash = block1.hash();
        store.add_block(block1);

        // Vote for block1
        store.on_attestation([1; 32], b1_hash);

        let head = store.get_head(&vs);
        assert_eq!(head, Some(b1_hash));
    }

    #[test]
    fn test_fork_chooses_heavier_branch() {
        let mut store = LMDGhostStore::new();
        let vs = make_validator_set();

        let genesis = make_header(0, [0; 32]);
        let genesis_hash = genesis.hash();
        store.add_block(genesis);
        store.set_justified(genesis_hash);

        // Fork A - unique extra_data
        let mut a1 = make_header(1, genesis_hash);
        a1.extra_data = vec![0xA1];
        let a1_hash = a1.hash();
        store.add_block(a1);

        // Fork B - different extra_data
        let mut b1 = make_header(1, genesis_hash);
        b1.extra_data = vec![0xB1];
        let b1_hash = b1.hash();
        store.add_block(b1);

        // 1 vote for A, 2 votes for B
        store.on_attestation([1; 32], a1_hash);
        store.on_attestation([2; 32], b1_hash);
        store.on_attestation([3; 32], b1_hash);

        let head = store.get_head(&vs);
        assert_eq!(head, Some(b1_hash), "Should choose heavier branch B");
    }

    #[test]
    fn test_vote_update_changes_head() {
        let mut store = LMDGhostStore::new();
        let vs = make_validator_set();

        let genesis = make_header(0, [0; 32]);
        let genesis_hash = genesis.hash();
        store.add_block(genesis);
        store.set_justified(genesis_hash);

        // Fork A
        let mut a1 = make_header(1, genesis_hash);
        a1.extra_data = vec![0xA1];
        let a1_hash = a1.hash();
        store.add_block(a1);

        // Fork B
        let mut b1 = make_header(1, genesis_hash);
        b1.extra_data = vec![0xB1];
        let b1_hash = b1.hash();
        store.add_block(b1);

        // Initially A wins (2 votes)
        store.on_attestation([1; 32], a1_hash);
        store.on_attestation([2; 32], a1_hash);
        assert_eq!(store.get_head(&vs), Some(a1_hash));

        // Validator 1 switches to B, validator 3 joins B - now B wins
        store.on_attestation([1; 32], b1_hash);
        store.on_attestation([3; 32], b1_hash);
        assert_eq!(store.get_head(&vs), Some(b1_hash));
    }

    #[test]
    fn test_deep_chain_weight_propagates() {
        let mut store = LMDGhostStore::new();
        let vs = make_validator_set();

        let genesis = make_header(0, [0; 32]);
        let genesis_hash = genesis.hash();
        store.add_block(genesis);
        store.set_justified(genesis_hash);

        // Build chain: genesis -> b1 -> b2 -> b3
        let b1 = make_header(1, genesis_hash);
        let b1_hash = b1.hash();
        store.add_block(b1);

        let b2 = make_header(2, b1_hash);
        let b2_hash = b2.hash();
        store.add_block(b2);

        let b3 = make_header(3, b2_hash);
        let b3_hash = b3.hash();
        store.add_block(b3);

        // Vote for b3 - weight should propagate to b1, b2
        store.on_attestation([1; 32], b3_hash);

        let head = store.get_head(&vs);
        assert_eq!(head, Some(b3_hash));
    }
}
