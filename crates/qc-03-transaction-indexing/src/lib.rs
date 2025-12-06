//! # Transaction Indexing Subsystem (qc-03)
//!
//! The Transaction Indexing subsystem is the system's authority for proving
//! transaction inclusion within a block. It computes Merkle roots for validated
//! blocks and generates cryptographic Merkle proofs for any indexed transaction.
//!
//! ## SPEC-03 Reference
//!
//! This crate implements SPEC-03-TRANSACTION-INDEXING.md with the following
//! key responsibilities:
//!
//! - Compute Merkle root for transactions in a validated block
//! - Generate Merkle proofs for transaction inclusion verification
//! - Verify Merkle proofs against known roots
//! - Index transactions by hash for efficient proof generation
//! - Maintain transaction location mappings (tx_hash → block_height, tx_index)
//!
//! ## V2.2 Choreography Pattern
//!
//! This subsystem is a **participant** in the block processing choreography:
//!
//! ```text
//! Consensus (8) ──BlockValidated──→ [Event Bus] ──→ Transaction Indexing (3)
//!                                                          │
//!                                                          ↓
//!                                                   [Compute Merkle Tree]
//!                                                          │
//!                                                          ↓
//!                                      ←──MerkleRootComputed──→ [Event Bus] ──→ Block Storage (2)
//! ```
//!
//! ## Domain Invariants
//!
//! | ID | Invariant |
//! |----|-----------|
//! | INVARIANT-1 | Power of Two Padding - Leaves padded to 2^ceil(log2(n)) |
//! | INVARIANT-2 | Proof Validity - All generated proofs MUST verify |
//! | INVARIANT-3 | Deterministic Hashing - Same tx = same hash (canonical serialization) |
//! | INVARIANT-4 | Index Consistency - Cached merkle_root == tree.root() |
//! | INVARIANT-5 | Bounded Tree Cache - trees.len() <= max_cached_trees |
//!
//! ## Hexagonal Architecture
//!
//! - **Domain Layer** (`domain/`): Pure Merkle tree logic, no I/O
//! - **Ports Layer** (`ports/`): Inbound API traits, Outbound SPI traits
//! - **IPC Layer** (`ipc/`): Event-driven message handling
//! - **Adapters Layer** (`adapters/`): Secondary adapters (API Gateway handler)

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;

// Re-export main types for convenience
pub use domain::{
    HashAlgorithm, IndexConfig, IndexingError, IndexingErrorPayload, IndexingErrorType,
    IndexingStats, MerkleConfig, MerkleProof, MerkleTree, ProofNode, SiblingPosition,
    TransactionIndex, TransactionLocation, SENTINEL_HASH,
};

pub use ports::{
    BlockDataProvider, BlockStorageError, HashProvider, SerializationError, StoreError, TimeSource,
    TransactionHashesData, TransactionIndexingApi, TransactionSerializer, TransactionStore,
};

pub use ipc::{
    subsystem_ids, BlockValidatedPayload, HandlerError, MerkleProofRequestPayload,
    MerkleProofResponsePayload, MerkleRootComputedPayload, TransactionIndexingHandler,
    TransactionLocationRequestPayload, TransactionLocationResponsePayload,
};

pub use adapters::{ApiGatewayHandler, ApiQueryError, Qc03Metrics, handle_api_query};
