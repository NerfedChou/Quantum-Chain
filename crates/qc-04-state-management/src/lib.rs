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
//! ## Security
//! 
//! - Uses centralized `MessageVerifier` from `shared-types`
//! - Enforces IPC-MATRIX.md authorization rules
//! - Only Subsystem 11 (Smart Contracts) can write state
//! - Only Subsystem 6 (Mempool) can check balances

pub mod domain;
pub mod ports;
pub mod events;
pub mod ipc;
pub mod adapters;

pub use domain::*;
pub use ports::*;
pub use events::*;
pub use ipc::*;
pub use adapters::*;
