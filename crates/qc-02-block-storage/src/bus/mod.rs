//! # Event Bus Adapter for Block Storage
//!
//! This module provides the adapter that connects Block Storage to the shared event bus,
//! implementing the V2.3 Choreography pattern.
//!
//! ## Architecture
//!
//! ```text
//! [Event Bus] ──subscribe──→ [BlockStorageBusAdapter] ──→ [BlockStorageHandler]
//!                                    │
//!                                    ↓
//!                           [BlockStorageService]
//! ```
//!
//! ## Event Subscriptions
//!
//! | Event | Source | Action |
//! |-------|--------|--------|
//! | `BlockValidated` | Consensus (8) | Buffer block, try assembly |
//! | `MerkleRootComputed` | Tx Indexing (3) | Add merkle root, try assembly |
//! | `StateRootComputed` | State Mgmt (4) | Add state root, try assembly |
//! | `MarkFinalized` | Finality (9) | Mark block as finalized |
//!
//! ## Event Publications
//!
//! | Event | Trigger | Subscribers |
//! |-------|---------|-------------|
//! | `BlockStored` | Assembly complete | Tx Indexing (3), State Mgmt (4) |
//! | `BlockFinalized` | Finalization confirmed | Consensus (8), Validators |

mod adapter;

pub use adapter::BlockStorageBusAdapter;

// Event type constants for the bus
pub mod event_types {
    /// Block validated by consensus - triggers storage assembly
    pub const BLOCK_VALIDATED: &str = "BlockValidated";

    /// Merkle root computed by transaction indexing
    pub const MERKLE_ROOT_COMPUTED: &str = "MerkleRootComputed";

    /// State root computed by state management
    pub const STATE_ROOT_COMPUTED: &str = "StateRootComputed";

    /// Request to mark a block as finalized
    pub const MARK_FINALIZED: &str = "MarkFinalized";

    /// Published when a block is successfully stored
    pub const BLOCK_STORED: &str = "BlockStored";

    /// Published when a block is finalized
    pub const BLOCK_FINALIZED: &str = "BlockFinalized";

    /// Published when assembly times out (for monitoring)
    pub const ASSEMBLY_TIMEOUT: &str = "AssemblyTimeout";
}
