//! # Domain Layer
//!
//! Pure domain logic for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 2.2: Core Domain Entities (MerkleTree, MerkleProof, ProofNode)
//! - Section 2.3: Index Structures (TransactionIndex, TransactionLocation)
//! - Section 2.4: Value Objects (IndexConfig, MerkleConfig)
//! - Section 2.5: Domain Invariants (5 invariants)
//!
//! ## Hexagonal Architecture
//!
//! This module contains NO I/O dependencies. All external interactions
//! are abstracted through ports in the `ports` module.

pub mod entities;
pub mod errors;
pub mod value_objects;

pub use entities::*;
pub use errors::*;
pub use value_objects::*;
