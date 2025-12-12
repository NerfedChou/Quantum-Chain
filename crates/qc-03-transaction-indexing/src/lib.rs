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
//! ## Domain Invariants (SPEC-03 Section 2.5)
//!
//! | ID | Invariant | Enforcement | Location |
//! |----|-----------|-------------|----------|
//! | INVARIANT-1 | Power of Two Padding | Leaves padded to 2^ceil(log2(n)) | entities.rs:73-80 |
//! | INVARIANT-2 | Proof Validity | All generated proofs MUST verify | entities.rs:224-235 |
//! | INVARIANT-3 | Deterministic Hashing | SHA3-256 canonical serialization | entities.rs:241-246 |
//! | INVARIANT-4 | Index Consistency | Cached merkle_root == tree.root() | handler.rs |
//! | INVARIANT-5 | Bounded Tree Cache | LRU eviction when full | entities.rs:363-366 |
//!
//! ## Proof Size Characteristics
//!
//! Proofs scale logarithmically with transaction count:
//! - 100 transactions → ~231 bytes per proof
//! - 1,000 transactions → ~330 bytes per proof
//! - 10,000 transactions → ~462 bytes per proof
//!
//! See [`MerkleProof`] documentation for detailed size breakdown.
//!
//! ## Hexagonal Architecture
//!
//! - **Domain Layer** (`domain/`): Pure Merkle tree logic, no I/O dependencies
//! - **Ports Layer** (`ports/`): Inbound API traits, Outbound SPI traits
//! - **IPC Layer** (`ipc/`): Event-driven message handling with security
//! - **Adapters Layer** (`adapters/`): API Gateway handler for admin panel
//!
//! ## Security
//!
//! - **Envelope-Only Identity**: All IPC payloads omit identity fields
//! - **Sender Authorization**: `BlockValidated` only from Consensus (8)
//! - **Replay Prevention**: Nonce tracking with 60-second timestamp window
//! - **Test Mode Bypass**: Signature bypass gated behind `#[cfg(test)]`

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;

// Re-export main types for convenience
pub use domain::{
    sort_canonically,
    HashAlgorithm,
    IndexConfig,
    IndexingError,
    IndexingErrorPayload,
    IndexingErrorType,
    IndexingStats,
    MerkleConfig,
    MerkleProof,
    MerkleTree,
    // Advanced features (Phase 1)
    MultiProof,
    ProofNode,
    SiblingPosition,
    TransactionIndex,
    TransactionLocation,
    // Security hardening (Phase 2)
    LEAF_DOMAIN,
    MAX_PROOF_DEPTH,
    NODE_DOMAIN,
    PARALLEL_THRESHOLD,
    SENTINEL_HASH,
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

pub use adapters::{handle_api_query, ApiGatewayHandler, ApiQueryError, Qc03Metrics};
