//! # Domain Layer for State Management
//!
//! Pure domain logic per SPEC-04 and Hexagonal Architecture.
//!
//! ## Modules
//!
//! - `entities`: Core data structures (AccountState, StateConfig)
//! - `trie`: Patricia Merkle Trie implementation
//! - `proofs`: State and storage proof structures
//! - `errors`: Domain error types
//! - `conflicts`: Transaction conflict detection
//! - `cache`: Versioned state cache (reorg-aware)
//! - `parallel`: Parallel storage root computation
//! - `flat_storage`: O(1) execution reads (Dual-Path)
//! - `verify`: Iterative proof verification (Stack-safe)

pub mod cache;
pub mod conflicts;
pub mod entities;
pub mod errors;
pub mod flat_storage;
pub mod parallel;
pub mod proofs;
pub mod trie;
pub mod verify;

pub use cache::*;
pub use conflicts::*;
pub use entities::*;
pub use errors::*;
pub use flat_storage::*;
pub use parallel::*;
pub use proofs::*;
pub use trie::*;
pub use verify::*;
