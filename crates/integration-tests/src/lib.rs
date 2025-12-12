//! # Integration Tests Crate
//!
//! This crate contains integration tests that verify multiple subsystems
//! work together correctly via the shared-bus as per IPC-MATRIX.md.
//!
//! ## Structure
//!
//! ```text
//! integration-tests/
//! ├── src/
//! │   ├── lib.rs              # This file
//! │   ├── flows.rs            # Integration flows between subsystems
//! │   ├── e2e_choreography.rs # End-to-end V2.3 choreography tests
//! │   ├── runtime_simulation.rs # Full runtime simulation tests
//! │   └── exploits/           # Attack simulations (the "mean" tests)
//! │       ├── mod.rs          # Exploit harness & helpers
//! │       ├── phase1_exploits.rs  # Phase 1: Historical attacks (3 subsystems)
//! │       ├── historical/     # Bitcoin/Ethereum lessons
//! │       ├── modern/         # 2024 attack vectors
//! │       └── architectural/  # Design logic exploits
//! ```
//!
//! ## Integration Flows
//!
//! 1. **Sig Verification (10) → Event Bus → Mempool (6)**: Verified transactions
//! 2. **Sig Verification (10) → Event Bus → Peer Discovery (1)**: DDoS defense
//! 3. **Cross-subsystem event publishing**: V2.3 Choreography compliance
//!
//! ## End-to-End Choreography (e2e_choreography.rs)
//!
//! Tests the complete block processing flow:
//! - Consensus (8) validates block → publishes BlockValidated
//! - Transaction Indexing (3) computes Merkle root → publishes MerkleRootComputed
//! - State Management (4) computes state root → publishes StateRootComputed
//! - Block Storage (2) assembles all components → publishes BlockStored
//!
//! ## Runtime Simulation (runtime_simulation.rs)
//!
//! Full node runtime simulation without Docker:
//! - Complete data flow through all subsystems
//! - Concurrent block processing
//! - Event subscription and handling
//! - Merkle proof generation and verification
//!
//! ## Exploit Categories (exploits/)
//!
//! ### Phase 1 Exploits (Current - 3 Subsystems)
//! - **Mt. Gox Malleability** (2014): S-value flip attack on qc_10
//! - **Wormhole Bypass** (2022): Mock injection on qc_06
//! - **Eclipse Table Poisoning**: Routing table flood on qc_01
//! - **Dust Exhaustion**: Fee-based eviction on qc_06
//!
//! ### Historical Attacks
//! - **Timejacking** (Bitcoin 2011): Clock manipulation
//! - **Penny-Flooding**: Mempool spam
//!
//! ### Modern Attacks (2024)
//! - **Eclipse by Staging**: DDoS on staging area
//! - **Mempool DEA**: Data availability exhaustion
//!
//! ### Architectural Exploits
//! - **Zombie Assembler**: Two-phase commit timeout
//! - **Ghost Transaction**: Capacity gap attack
//!
//! ## Key Difference: Logic Tests vs Exploit Tests
//!
//! - **Logic Tests**: "Does 1 + 1 = 2?"
//! - **Exploit Tests**: "Does 1 + 1 = 2 when 500 threads are screaming?"
//!
//! Exploit tests are UNFORGIVING. Failure is failure. If a test fails,
//! it exposes a vulnerability that MUST be fixed.

pub mod e2e_choreography;
pub mod exploits;
pub mod flows;
pub mod runtime_simulation;
