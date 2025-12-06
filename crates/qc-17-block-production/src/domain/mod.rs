//! Domain layer - Pure business logic for block production
//!
//! This module contains the core domain entities, value objects, and services
//! that implement the block production logic. All code here is pure (no I/O,
//! no async) following DDD principles.
//!
//! ## Entities
//!
//! - [`BlockTemplate`]: Block template created by this subsystem
//! - [`MiningJob`]: PoW mining job configuration
//! - [`ProposerDuty`]: PoS proposer duty assignment
//! - [`TransactionCandidate`]: Transaction with metadata for selection
//!
//! ## Services
//!
//! - [`TransactionSelector`]: Optimal transaction selection (greedy knapsack)
//! - [`StatePrefetchCache`]: State simulation and caching
//! - [`NonceValidator`]: Nonce ordering validation
//!
//! ## Invariants
//!
//! All domain logic enforces the 6 critical invariants:
//! 1. Gas limit enforcement
//! 2. Nonce ordering (sequential per sender)
//! 3. State validity (all txs simulate successfully)
//! 4. No duplicate transactions
//! 5. Timestamp monotonicity
//! 6. Fee profitability (greedy selection)

// TODO: Implement domain entities
// TODO: Implement TransactionSelector service
// TODO: Implement StatePrefetchCache
// TODO: Implement invariant checkers

mod entities;
pub mod invariants;
mod services;
pub mod genesis;

pub use entities::*;
pub use genesis::*;
pub use invariants::*;
pub use services::{
    AccountState, NonceValidator, PoSProposer, PoWMiner, StatePrefetchCache, TransactionSelector,
};
