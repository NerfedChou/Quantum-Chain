//! # Adapters Layer (Hexagonal Architecture)
//!
//! Implements outbound port traits for integration with other subsystems.
//!
//! Reference: SPEC-12-TRANSACTION-ORDERING.md Section 7

mod access_analyzer;
mod conflict_detector;

pub use access_analyzer::StateManagementAccessAnalyzer;
pub use conflict_detector::BalanceBasedConflictDetector;
