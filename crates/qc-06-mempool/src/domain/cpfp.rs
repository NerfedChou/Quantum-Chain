//! # Child-Pays-For-Parent (CPFP) Support
//!
//! Ancestor fee tracking for intelligent transaction prioritization.
//!
//! ## Problem
//!
//! A low-fee parent transaction can get stuck, blocking its high-fee child.
//! Standard gas-price sorting doesn't consider family relationships.
//!
//! ## Solution: Ancestor Fee Rate
//!
//! Calculate effective fee rate considering the entire ancestor chain:
//! ```text
//! effective_rate = (tx_fee + sum(ancestor_fees)) / (tx_size + sum(ancestor_sizes))
//! ```
//!
//! ## Security: Chain Limits
//!
//! - MAX_ANCESTORS = 25 (prevent infinite recursion)
//! - MAX_DESCENDANTS = 25 (prevent mempool bombs)

use super::{Address, Hash, MAX_ANCESTORS, MAX_DESCENDANTS, U256};
use std::collections::{HashMap, HashSet};

/// Ancestor chain information for CPFP calculation.
#[derive(Clone, Debug, Default)]
pub struct AncestorInfo {
    /// Total fees of all ancestors
    pub total_ancestor_fees: U256,
    /// Total size of all ancestors in bytes
    pub total_ancestor_size: usize,
    /// Number of ancestors
    pub ancestor_count: usize,
    /// Set of ancestor transaction hashes
    pub ancestor_hashes: HashSet<Hash>,
}

/// Descendant chain information for limit checking.
#[derive(Clone, Debug, Default)]
pub struct DescendantInfo {
    /// Number of descendants
    pub descendant_count: usize,
    /// Total fees of all descendants
    pub total_descendant_fees: U256,
    /// Set of descendant transaction hashes
    pub descendant_hashes: HashSet<Hash>,
}

/// Transaction family tracker for CPFP.
#[derive(Debug, Default)]
pub struct TransactionFamily {
    /// Parent -> Children mapping
    children: HashMap<Hash, HashSet<Hash>>,
    /// Child -> Parent mapping
    parents: HashMap<Hash, Hash>,
    /// Sender -> Nonce -> TxHash mapping (for finding parents)
    sender_nonces: HashMap<Address, HashMap<u64, Hash>>,
}

impl TransactionFamily {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a transaction with its sender and nonce.
    pub fn register(&mut self, hash: Hash, sender: Address, nonce: u64) {
        // Check if there's a parent (tx with nonce - 1 from same sender)
        if nonce > 0 {
            if let Some(parent_hash) = self
                .sender_nonces
                .get(&sender)
                .and_then(|nonces| nonces.get(&(nonce - 1)))
            {
                // Link child to parent
                self.parents.insert(hash, *parent_hash);
                self.children.entry(*parent_hash).or_default().insert(hash);
            }
        }

        // Register this tx's nonce
        self.sender_nonces
            .entry(sender)
            .or_default()
            .insert(nonce, hash);
    }

    /// Unregister a transaction.
    pub fn unregister(&mut self, hash: &Hash, sender: &Address, nonce: u64) {
        // Remove from sender_nonces
        if let Some(nonces) = self.sender_nonces.get_mut(sender) {
            nonces.remove(&nonce);
        }

        // Remove parent link
        if let Some(parent) = self.parents.remove(hash) {
            if let Some(children) = self.children.get_mut(&parent) {
                children.remove(hash);
            }
        }

        // Remove children links (orphan them)
        self.children.remove(hash);
    }

    /// Get ancestor info for a transaction.
    pub fn get_ancestors(
        &self,
        hash: &Hash,
        get_fee: impl Fn(&Hash) -> Option<(U256, usize)>,
    ) -> AncestorInfo {
        let mut info = AncestorInfo::default();
        let mut current = *hash;
        let mut seen = HashSet::new();

        while let Some(parent) = self.parents.get(&current) {
            if seen.contains(parent) || info.ancestor_count >= MAX_ANCESTORS {
                break;
            }
            seen.insert(*parent);

            if let Some((fee, size)) = get_fee(parent) {
                info.total_ancestor_fees += fee;
                info.total_ancestor_size += size;
                info.ancestor_count += 1;
                info.ancestor_hashes.insert(*parent);
            }

            current = *parent;
        }

        info
    }

    /// Get descendant info for a transaction.
    pub fn get_descendants(&self, hash: &Hash) -> DescendantInfo {
        let mut info = DescendantInfo::default();
        let mut stack = vec![*hash];
        let mut seen = HashSet::new();

        while let Some(current) = stack.pop() {
            if seen.contains(&current) {
                continue;
            }
            seen.insert(current);

            // Skip original hash but count descendants
            if current != *hash {
                info.descendant_count += 1;
                info.descendant_hashes.insert(current);
            }

            // Early break at limit
            if info.descendant_count >= MAX_DESCENDANTS {
                break;
            }

            // Queue children for traversal
            if let Some(children) = self.children.get(&current) {
                stack.extend(children.iter().copied());
            }
        }

        info
    }

    /// Check if adding a transaction would exceed limits.
    pub fn would_exceed_limits(&self, _hash: &Hash, sender: &Address, nonce: u64) -> bool {
        // Create temporary registration to check limits
        let parent_hash = if nonce > 0 {
            self.sender_nonces
                .get(sender)
                .and_then(|nonces| nonces.get(&(nonce - 1)))
                .copied()
        } else {
            None
        };

        // Check ancestor limit
        if let Some(parent) = parent_hash {
            let mut count = 1;
            let mut current = parent;
            while let Some(grandparent) = self.parents.get(&current) {
                count += 1;
                if count > MAX_ANCESTORS {
                    return true;
                }
                current = *grandparent;
            }
        }

        // Check descendant limit of potential parent
        if let Some(parent) = parent_hash {
            let descendants = self.get_descendants(&parent);
            if descendants.descendant_count >= MAX_DESCENDANTS {
                return true;
            }
        }

        false
    }

    /// Calculate effective fee rate (for CPFP prioritization).
    pub fn effective_fee_rate(
        &self,
        hash: &Hash,
        tx_fee: U256,
        tx_size: usize,
        get_fee: impl Fn(&Hash) -> Option<(U256, usize)>,
    ) -> U256 {
        let ancestors = self.get_ancestors(hash, get_fee);

        let total_fee = tx_fee + ancestors.total_ancestor_fees;
        let total_size = tx_size + ancestors.total_ancestor_size;

        if total_size == 0 {
            return U256::zero();
        }

        // Fee per byte
        total_fee / U256::from(total_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENDER_A: Address = [0xAA; 20];

    fn hash_from_nonce(nonce: u64) -> Hash {
        let mut h = [0u8; 32];
        h[0..8].copy_from_slice(&nonce.to_le_bytes());
        h
    }

    #[test]
    fn test_register_parent_child() {
        let mut family = TransactionFamily::new();

        let parent = hash_from_nonce(0);
        let child = hash_from_nonce(1);

        family.register(parent, SENDER_A, 0);
        family.register(child, SENDER_A, 1);

        assert!(family.parents.contains_key(&child));
        assert_eq!(family.parents.get(&child), Some(&parent));
    }

    #[test]
    fn test_ancestor_chain() {
        let mut family = TransactionFamily::new();

        // Create chain: tx0 -> tx1 -> tx2
        for i in 0..3 {
            family.register(hash_from_nonce(i), SENDER_A, i);
        }

        let get_fee = |_hash: &Hash| -> Option<(U256, usize)> { Some((U256::from(1000), 100)) };

        let ancestors = family.get_ancestors(&hash_from_nonce(2), get_fee);
        assert_eq!(ancestors.ancestor_count, 2);
        assert_eq!(ancestors.total_ancestor_fees, U256::from(2000));
        assert_eq!(ancestors.total_ancestor_size, 200);
    }

    #[test]
    fn test_ancestor_limit() {
        let mut family = TransactionFamily::new();

        // Create chain longer than MAX_ANCESTORS
        for i in 0..30 {
            family.register(hash_from_nonce(i), SENDER_A, i);
        }

        let get_fee = |_: &Hash| -> Option<(U256, usize)> { Some((U256::from(100), 50)) };

        let ancestors = family.get_ancestors(&hash_from_nonce(29), get_fee);
        // Should be capped at MAX_ANCESTORS
        assert!(ancestors.ancestor_count <= MAX_ANCESTORS);
    }

    #[test]
    fn test_descendant_count() {
        let mut family = TransactionFamily::new();

        // Create chain: tx0 -> tx1 -> tx2 -> tx3
        for i in 0..4 {
            family.register(hash_from_nonce(i), SENDER_A, i);
        }

        let descendants = family.get_descendants(&hash_from_nonce(0));
        assert_eq!(descendants.descendant_count, 3);
    }

    #[test]
    fn test_effective_fee_rate() {
        let mut family = TransactionFamily::new();

        // Parent: low fee (100 wei, 100 bytes)
        // Child: high fee (1000 wei, 100 bytes)
        family.register(hash_from_nonce(0), SENDER_A, 0);
        family.register(hash_from_nonce(1), SENDER_A, 1);

        let get_fee = |hash: &Hash| -> Option<(U256, usize)> {
            if *hash == hash_from_nonce(0) {
                Some((U256::from(100), 100))
            } else {
                Some((U256::from(1000), 100))
            }
        };

        // Child alone: 1000/100 = 10
        // With parent: (1000+100)/(100+100) = 5.5
        let rate = family.effective_fee_rate(&hash_from_nonce(1), U256::from(1000), 100, get_fee);

        // (1000 + 100) / 200 = 5
        assert_eq!(rate, U256::from(5));
    }

    #[test]
    fn test_would_exceed_limits() {
        let mut family = TransactionFamily::new();

        // Create chain at MAX_ANCESTORS + 1 to exceed
        for i in 0..=MAX_ANCESTORS {
            family.register(hash_from_nonce(i as u64), SENDER_A, i as u64);
        }

        // Next tx would exceed (chain already has MAX_ANCESTORS + 1)
        let exceeds = family.would_exceed_limits(
            &hash_from_nonce((MAX_ANCESTORS + 1) as u64),
            &SENDER_A,
            (MAX_ANCESTORS + 1) as u64,
        );
        assert!(exceeds);
    }

    #[test]
    fn test_unregister() {
        let mut family = TransactionFamily::new();

        family.register(hash_from_nonce(0), SENDER_A, 0);
        family.register(hash_from_nonce(1), SENDER_A, 1);

        family.unregister(&hash_from_nonce(0), &SENDER_A, 0);

        // Parent link should be removed
        assert!(!family.children.contains_key(&hash_from_nonce(0)));
    }
}
