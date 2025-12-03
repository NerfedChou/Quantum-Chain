//! # Genesis Module
//!
//! Genesis block creation and chain initialization.
//!
//! ## Architecture (v2.3)
//!
//! The genesis block is the foundation of the chain with special properties:
//!
//! - Height: 0
//! - Parent hash: 32 zero bytes
//! - Merkle root: Empty tree root
//! - State root: Empty trie root
//! - Timestamp: Chain genesis timestamp
//!
//! ## Initialization Sequence
//!
//! 1. Create genesis block with deterministic content
//! 2. Store genesis in Block Storage (bypasses assembly)
//! 3. Initialize State Management with empty state root
//! 4. Set finalized height to 0
//! 5. Initialize Transaction Indexing with empty Merkle tree

pub mod builder;

pub use builder::{GenesisBlock, GenesisBuilder, GenesisConfig, GenesisError};
