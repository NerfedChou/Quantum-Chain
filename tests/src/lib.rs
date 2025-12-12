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
// Clippy allows for test code (matching CI configuration)
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::const_is_empty)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::map_entry)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::iter_kv_map)]
#![allow(clippy::explicit_auto_deref)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::identity_op)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::manual_strip)]
#![allow(clippy::len_zero)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unused_self)]
#![allow(unused_mut)]
#![allow(clippy::useless_asref)]
#![allow(clippy::repeat_vec_with_capacity)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::fn_to_numeric_cast)]
#![allow(clippy::unused_io_amount)]
#![allow(clippy::slow_vector_initialization)]
#![allow(clippy::iter_over_hash_type)]
#![allow(clippy::unnecessary_to_owned)]
#![allow(clippy::expect_fun_call)]
#![allow(clippy::repeat_once)]
#![allow(unused_assignments)]
#![allow(clippy::manual_repeat_n)]

pub mod benchmarks;
pub mod exploits;
pub mod integration;
