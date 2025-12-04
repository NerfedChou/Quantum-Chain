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

pub mod conflicts;
pub mod entities;
pub mod errors;
pub mod proofs;
pub mod trie;

pub use conflicts::*;
pub use entities::*;
pub use errors::*;
pub use proofs::*;
pub use trie::*;
