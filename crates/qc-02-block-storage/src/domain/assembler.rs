//! # Stateful Assembler
//!
//! Implements the V2.3 Choreography Pattern for block assembly.
//!
//! ## Architecture (Architecture.md v2.3)
//!
//! Block Storage is a **Stateful Assembler** that:
//! 1. Subscribes to THREE independent events (no orchestrator)
//! 2. Buffers incoming components by `block_hash` until all three arrive
//! 3. Performs atomic write when all components are present
//! 4. Implements assembly timeout for resource exhaustion defense
//!
//! ## SPEC-02 Reference
//!
//! - Section 2.4: Stateful Assembler Structures
//! - Section 2.6: INVARIANT-7 (Assembly Timeout), INVARIANT-8 (Bounded Buffer)

use super::entities::Timestamp;
use shared_types::{Hash, ValidatedBlock};
use std::collections::HashMap;

/// Configuration for the assembly buffer.
///
/// ## SPEC-02 Section 2.4
///
/// - `assembly_timeout_secs`: Maximum time to wait for all components (default: 30s)
/// - `max_pending_assemblies`: Maximum buffer size to prevent memory exhaustion (default: 1000)
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// Maximum time to wait for all components before purging (default: 30 seconds).
    ///
    /// SECURITY (INVARIANT-7): This prevents memory exhaustion from orphaned partial blocks.
    pub assembly_timeout_secs: u64,

    /// Maximum number of pending assemblies (default: 1000).
    ///
    /// SECURITY (INVARIANT-8): Bounds memory usage. If exceeded, oldest entries are purged.
    pub max_pending_assemblies: usize,
}

impl Default for AssemblyConfig {
    fn default() -> Self {
        Self {
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
        }
    }
}

/// Buffer for assembling block components from multiple subsystems.
///
/// ## Architecture (V2.3 Choreography Pattern)
///
/// Unlike the rejected "Orchestrator" pattern where Consensus would assemble
/// a complete package, this subsystem receives THREE independent events:
/// - BlockValidated (from Consensus)
/// - MerkleRootComputed (from Transaction Indexing)
/// - StateRootComputed (from State Management)
///
/// Each event may arrive in any order. This buffer holds partial assemblies
/// until all components are present.
///
/// ## Security
///
/// - INVARIANT-7: Entries are purged after `assembly_timeout_secs`
/// - INVARIANT-8: Buffer is bounded to `max_pending_assemblies`
pub struct BlockAssemblyBuffer {
    /// Pending assemblies keyed by block_hash.
    pending: HashMap<Hash, PendingBlockAssembly>,
    /// Configuration for assembly behavior.
    config: AssemblyConfig,
}

impl BlockAssemblyBuffer {
    /// Create a new assembly buffer with the given configuration.
    pub fn new(config: AssemblyConfig) -> Self {
        Self {
            pending: HashMap::new(),
            config,
        }
    }

    /// Create a new assembly buffer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(AssemblyConfig::default())
    }

    /// Handle incoming BlockValidated event from Consensus (Subsystem 8).
    ///
    /// Creates or updates the pending assembly for this block_hash.
    pub fn add_block_validated(&mut self, block_hash: Hash, block: ValidatedBlock, now: Timestamp) {
        let assembly = self
            .pending
            .entry(block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(block_hash, now));

        assembly.block_height = block.header.height;
        assembly.validated_block = Some(block);
    }

    /// Handle incoming MerkleRootComputed event from Transaction Indexing (Subsystem 3).
    pub fn add_merkle_root(&mut self, block_hash: Hash, merkle_root: Hash, now: Timestamp) {
        let assembly = self
            .pending
            .entry(block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(block_hash, now));

        assembly.merkle_root = Some(merkle_root);
    }

    /// Handle incoming StateRootComputed event from State Management (Subsystem 4).
    pub fn add_state_root(&mut self, block_hash: Hash, state_root: Hash, now: Timestamp) {
        let assembly = self
            .pending
            .entry(block_hash)
            .or_insert_with(|| PendingBlockAssembly::new(block_hash, now));

        assembly.state_root = Some(state_root);
    }

    /// Check if an assembly is complete (all three components present).
    pub fn is_complete(&self, block_hash: &Hash) -> bool {
        self.pending
            .get(block_hash)
            .map(|a| a.is_complete())
            .unwrap_or(false)
    }

    /// Get a reference to a pending assembly.
    pub fn get(&self, block_hash: &Hash) -> Option<&PendingBlockAssembly> {
        self.pending.get(block_hash)
    }

    /// Remove and return a complete assembly for processing.
    ///
    /// Returns `None` if the assembly doesn't exist or is not complete.
    pub fn take_complete(&mut self, block_hash: &Hash) -> Option<PendingBlockAssembly> {
        if self.is_complete(block_hash) {
            self.pending.remove(block_hash)
        } else {
            None
        }
    }

    /// Garbage collect expired assemblies (INVARIANT-7).
    ///
    /// Returns the list of purged block hashes for logging/monitoring.
    pub fn gc_expired(&mut self, now: Timestamp) -> Vec<Hash> {
        let timeout = self.config.assembly_timeout_secs;
        let expired: Vec<Hash> = self
            .pending
            .iter()
            .filter(|(_, a)| a.is_expired(now, timeout))
            .map(|(h, _)| *h)
            .collect();

        for hash in &expired {
            self.pending.remove(hash);
        }

        expired
    }

    /// Garbage collect expired assemblies and return full data (INVARIANT-7).
    ///
    /// Returns tuples of (block_hash, assembly_data) for event emission.
    pub fn gc_expired_with_data(&mut self, now: Timestamp) -> Vec<(Hash, PendingBlockAssembly)> {
        let timeout = self.config.assembly_timeout_secs;
        let expired: Vec<Hash> = self
            .pending
            .iter()
            .filter(|(_, a)| a.is_expired(now, timeout))
            .map(|(h, _)| *h)
            .collect();

        let mut result = Vec::new();
        for hash in expired {
            if let Some(assembly) = self.pending.remove(&hash) {
                result.push((hash, assembly));
            }
        }

        result
    }

    /// Enforce the maximum pending assemblies limit (INVARIANT-8).
    ///
    /// Purges the oldest assemblies if the limit is exceeded.
    /// Returns the list of purged block hashes.
    pub fn enforce_max_pending(&mut self) -> Vec<Hash> {
        if self.pending.len() <= self.config.max_pending_assemblies {
            return vec![];
        }

        // Sort by started_at to find oldest
        let mut entries: Vec<_> = self
            .pending
            .iter()
            .map(|(h, a)| (*h, a.started_at))
            .collect();
        entries.sort_by_key(|(_, started_at)| *started_at);

        let to_remove = self.pending.len() - self.config.max_pending_assemblies;
        let purged: Vec<Hash> = entries.iter().take(to_remove).map(|(h, _)| *h).collect();

        for hash in &purged {
            self.pending.remove(hash);
        }

        purged
    }

    /// Get the number of pending assemblies.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Get the configuration.
    pub fn config(&self) -> &AssemblyConfig {
        &self.config
    }
}

/// A partial block assembly awaiting completion.
///
/// Tracks which of the three required components have arrived.
#[derive(Debug, Clone)]
pub struct PendingBlockAssembly {
    /// Block hash (key for this assembly).
    pub block_hash: Hash,
    /// Block height for ordering.
    pub block_height: u64,
    /// When this assembly was first started (for timeout).
    pub started_at: Timestamp,
    /// The validated block (from Consensus, Subsystem 8).
    pub validated_block: Option<ValidatedBlock>,
    /// Merkle root of transactions (from Tx Indexing, Subsystem 3).
    pub merkle_root: Option<Hash>,
    /// State root after execution (from State Management, Subsystem 4).
    pub state_root: Option<Hash>,
}

impl PendingBlockAssembly {
    /// Create a new empty pending assembly.
    pub fn new(block_hash: Hash, started_at: Timestamp) -> Self {
        Self {
            block_hash,
            block_height: 0,
            started_at,
            validated_block: None,
            merkle_root: None,
            state_root: None,
        }
    }

    /// Check if all three components are present.
    pub fn is_complete(&self) -> bool {
        self.validated_block.is_some() && self.merkle_root.is_some() && self.state_root.is_some()
    }

    /// Check if this assembly has timed out.
    pub fn is_expired(&self, now: Timestamp, timeout_secs: u64) -> bool {
        now.saturating_sub(self.started_at) > timeout_secs
    }

    /// Get the components as a tuple if complete.
    ///
    /// Returns `None` if not all components are present.
    pub fn take_components(self) -> Option<(ValidatedBlock, Hash, Hash)> {
        match (self.validated_block, self.merkle_root, self.state_root) {
            (Some(block), Some(merkle), Some(state)) => Some((block, merkle, state)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::{BlockHeader, ConsensusProof};

    fn make_test_block(height: u64) -> ValidatedBlock {
        ValidatedBlock {
            header: BlockHeader {
                version: 1,
                height,
                parent_hash: [0; 32],
                merkle_root: [0; 32],
                state_root: [0; 32],
                timestamp: 1000,
                proposer: [0; 32],
            },
            transactions: vec![],
            consensus_proof: ConsensusProof::default(),
        }
    }

    #[test]
    fn test_assembly_completes_with_all_three() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xAB; 32];
        let now = 1000;

        // Add BlockValidated
        buffer.add_block_validated(block_hash, make_test_block(1), now);
        assert!(!buffer.is_complete(&block_hash));

        // Add MerkleRootComputed
        buffer.add_merkle_root(block_hash, [0xCC; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        // Add StateRootComputed
        buffer.add_state_root(block_hash, [0xDD; 32], now);
        assert!(buffer.is_complete(&block_hash));
    }

    #[test]
    fn test_assembly_works_any_order() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xBB; 32];
        let now = 1000;

        // Reverse order: State → Merkle → Block
        buffer.add_state_root(block_hash, [0xDD; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        buffer.add_merkle_root(block_hash, [0xCC; 32], now);
        assert!(!buffer.is_complete(&block_hash));

        buffer.add_block_validated(block_hash, make_test_block(1), now);
        assert!(buffer.is_complete(&block_hash));
    }

    #[test]
    fn test_gc_expired_assemblies() {
        let config = AssemblyConfig {
            assembly_timeout_secs: 30,
            max_pending_assemblies: 1000,
        };
        let mut buffer = BlockAssemblyBuffer::new(config);

        // Add assemblies at time 1000
        for i in 0..10 {
            let block_hash = [i as u8; 32];
            buffer.add_block_validated(block_hash, make_test_block(i as u64), 1000);
        }

        assert_eq!(buffer.len(), 10);

        // GC at time 1031 (31 seconds later, past 30s timeout)
        let expired = buffer.gc_expired(1031);
        assert_eq!(expired.len(), 10);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_enforce_max_pending() {
        let config = AssemblyConfig {
            assembly_timeout_secs: 30,
            max_pending_assemblies: 5,
        };
        let mut buffer = BlockAssemblyBuffer::new(config);

        // Add 10 assemblies with staggered timestamps
        for i in 0..10 {
            let block_hash = [i as u8; 32];
            buffer.add_block_validated(block_hash, make_test_block(i as u64), 1000 + i as u64);
        }

        assert_eq!(buffer.len(), 10);

        // Enforce limit
        let purged = buffer.enforce_max_pending();
        assert_eq!(purged.len(), 5);
        assert_eq!(buffer.len(), 5);

        // Oldest 5 should be gone
        for i in 0..5 {
            let block_hash = [i as u8; 32];
            assert!(buffer.get(&block_hash).is_none());
        }

        // Newest 5 should remain
        for i in 5..10 {
            let block_hash = [i as u8; 32];
            assert!(buffer.get(&block_hash).is_some());
        }
    }

    #[test]
    fn test_take_complete() {
        let mut buffer = BlockAssemblyBuffer::with_defaults();
        let block_hash = [0xCC; 32];
        let now = 1000;

        // Add all components
        buffer.add_block_validated(block_hash, make_test_block(1), now);
        buffer.add_merkle_root(block_hash, [0x11; 32], now);
        buffer.add_state_root(block_hash, [0x22; 32], now);

        // Take complete
        let assembly = buffer.take_complete(&block_hash);
        assert!(assembly.is_some());
        assert!(buffer.get(&block_hash).is_none());

        // Verify components
        let (block, merkle, state) = assembly.unwrap().take_components().unwrap();
        assert_eq!(block.header.height, 1);
        assert_eq!(merkle, [0x11; 32]);
        assert_eq!(state, [0x22; 32]);
    }
}
