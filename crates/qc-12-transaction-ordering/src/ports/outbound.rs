//! Outbound Ports (Driven Ports / SPI)
//!
//! Reference: SPEC-12 Section 3.2 (Lines 233-270)

use crate::domain::entities::AnnotatedTransaction;
use crate::domain::errors::{AnalysisError, ConflictError};
use crate::domain::value_objects::{AccessPattern, Conflict};
use async_trait::async_trait;

/// State access pattern analyzer
///
/// Analyzes transactions to determine their read/write sets.
/// Reference: SPEC-12 Lines 235-242
#[async_trait]
pub trait AccessPatternAnalyzer: Send + Sync {
    /// Analyze a transaction to determine its access pattern.
    ///
    /// This may involve:
    /// - Static analysis of contract bytecode
    /// - Simulation of transaction execution
    /// - Query to state management (Subsystem 4)
    async fn analyze_access_pattern(
        &self,
        tx_hash: primitive_types::H256,
        sender: primitive_types::H160,
        to: Option<primitive_types::H160>,
        data: &[u8],
    ) -> Result<AccessPattern, AnalysisError>;
}

/// Conflict detector
///
/// Detects conflicts between transactions using state information.
/// Reference: SPEC-12 Lines 245-251
#[async_trait]
pub trait ConflictDetector: Send + Sync {
    /// Detect conflicts between transactions.
    ///
    /// May query Subsystem 4 (State Management) for current balances
    /// and storage values to determine conflicts.
    async fn detect_conflicts(
        &self,
        transactions: &[AnnotatedTransaction],
    ) -> Result<Vec<Conflict>, ConflictError>;
}

/// Mock implementations for testing
#[cfg(test)]
pub mod mocks {
    use super::*;

    /// Mock access pattern analyzer that returns empty patterns
    pub struct MockAccessPatternAnalyzer;

    #[async_trait]
    impl AccessPatternAnalyzer for MockAccessPatternAnalyzer {
        async fn analyze_access_pattern(
            &self,
            _tx_hash: primitive_types::H256,
            _sender: primitive_types::H160,
            _to: Option<primitive_types::H160>,
            _data: &[u8],
        ) -> Result<AccessPattern, AnalysisError> {
            Ok(AccessPattern::default())
        }
    }

    /// Mock conflict detector that returns no conflicts
    pub struct MockConflictDetector;

    #[async_trait]
    impl ConflictDetector for MockConflictDetector {
        async fn detect_conflicts(
            &self,
            _transactions: &[AnnotatedTransaction],
        ) -> Result<Vec<Conflict>, ConflictError> {
            Ok(vec![])
        }
    }
}
