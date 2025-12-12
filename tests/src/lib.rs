//! # Quantum-Chain Test Suite
//!
//! Unified test crate containing:
//!
//! ## Structure
//!
//! ```text
//! tests/src/
//! ├── benchmarks/       # Performance tests per subsystem
//! │   ├── qc_01_peer_discovery.rs
//! │   ├── qc_02_block_storage.rs
//! │   └── ...
//! │
//! ├── exploits/         # Attack simulations
//! │   ├── historical/   # Famous past attacks
//! │   │   └── qc_XX/    # By target subsystem
//! │   ├── modern/       # Current threats
//! │   │   └── qc_XX/
//! │   └── architectural/# System-level attacks
//! │       └── qc_XX/
//! │
//! └── integration/      # Cross-subsystem choreography
//! ```
//!
//! ## Running Tests
//!
//! ```bash
//! # All tests
//! cargo test -p qc-tests
//!
//! # By category
//! cargo test -p qc-tests integration::
//! cargo test -p qc-tests exploits::historical::
//! cargo test -p qc-tests exploits::modern::
//! cargo test -p qc-tests exploits::architectural::
//!
//! # Benchmarks
//! cargo bench -p qc-tests
//! ```

#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

pub mod benchmarks;
pub mod exploits;
pub mod integration;
