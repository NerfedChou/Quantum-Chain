//! # qc-04-state-management
//!
//! State Management subsystem for Quantum-Chain.
//!
//! ## Role in System
//!
//! - **Choreography Participant**: Subscribes to `BlockValidated`, publishes `StateRootComputed`
//! - **Single Source of Truth**: Authoritative current state of all accounts
//! - **Patricia Merkle Trie**: Cryptographic state proofs for light clients
//!
//! ## V2.3 Choreography Flow
//!
//! ```text
//! [Consensus (8)] ──BlockValidated──→ [Event Bus]
//!                                         │
//!                     ┌───────────────────┼───────────────────┐
//!                     ↓                   ↓                   ↓
//!            [Tx Indexing (3)]  [State Management (4)]  [Block Storage (2)]
//!                     │                   │              (Assembler)
//!                     ↓                   ↓                   ↑
//!            MerkleRootComputed   StateRootComputed           │
//!                     │                   │                   │
//!                     └──────→ [Event Bus] ←──────────────────┘
//! ```
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | Balance Non-Negativity | `domain/trie.rs` - `apply_balance_change()` |
//! | INVARIANT-2 | Nonce Monotonicity | `domain/trie.rs` - `apply_nonce_increment()` |
//! | INVARIANT-3 | Deterministic State Root | `domain/trie.rs` - `rebuild_trie()` (sorted keys) |
//! | INVARIANT-4 | Proof Validity | `domain/trie.rs` - `generate_proof()` |
//! | INVARIANT-5 | Atomic Transitions | Single trie mutation per block in handler |
//!
//! ## Security
//!
//! - **Centralized Security**: Uses `MessageVerifier` from `shared-types` crate
//! - **Envelope-Only Identity**: Identity derived solely from `AuthenticatedMessage.sender_id`
//! - **Strict Authorization**: Per IPC-MATRIX.md rules (see below)
//! - **Replay Prevention**: Nonce tracking via centralized `NonceCache`
//!
//! ### IPC Authorization Matrix
//!
//! | Operation | Authorized Sender(s) | Enforcement |
//! |-----------|---------------------|-------------|
//! | `BlockValidated` | Consensus (8) only | `ipc/handler.rs:150-153` |
//! | `StateReadRequest` | 6, 11, 12, 14 | `ipc/handler.rs:230-235` |
//! | `StateWriteRequest` | Smart Contracts (11) only | `ipc/handler.rs:285-288` |
//! | `BalanceCheckRequest` | Mempool (6) only | `ipc/handler.rs:318-321` |
//! | `ConflictDetectionRequest` | Tx Ordering (12) only | `ipc/handler.rs:357-360` |
//!
//! ## Patricia Merkle Trie
//!
//! The implementation follows Ethereum Yellow Paper Appendix D:
//!
//! - **Node Types**: Empty, Leaf, Extension, Branch
//! - **Path Encoding**: Hex-prefix (HP) encoding for nibble paths
//! - **Hash Function**: Keccak256 for Ethereum compatibility
//! - **Serialization**: RLP encoding for canonical representation

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items

pub mod adapters;
pub mod domain;
pub mod events;
pub mod ipc;
pub mod ports;

pub use adapters::*;
pub use domain::*;
pub use events::*;
pub use ipc::*;
pub use ports::*;
