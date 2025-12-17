//! # Block Assembly Buffer
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

use super::config::AssemblyConfig;
use super::pending::PendingBlockAssembly;
use crate::domain::storage::Timestamp;
use shared_types::{Hash, ValidatedBlock};
use std::collections::HashMap;

/// Buffer for assembling block components from multiple subsystems.
///
/// ## Architecture (V2.3 Choreography Pattern)
///
/// Unlike the rejected \"Orchestrator\" pattern where Consensus would assemble
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
