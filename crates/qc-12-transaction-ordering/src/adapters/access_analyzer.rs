//! Access Pattern Analyzer Adapter
//!
//! Implements `AccessPatternAnalyzer` port using State Management (qc-04).
//! Reference: SPEC-12 Section 3.2

use crate::domain::errors::AnalysisError;
use crate::domain::value_objects::AccessPattern;
use crate::ports::outbound::AccessPatternAnalyzer;
use async_trait::async_trait;
use primitive_types::{H160, H256};
use std::collections::HashSet;
use tracing::debug;

/// Analyzes transaction access patterns using State Management (qc-04).
///
/// Per SPEC-12, this determines read/write sets for dependency analysis.
pub struct StateManagementAccessAnalyzer {
    /// Whether to perform static bytecode analysis.
    enable_static_analysis: bool,
}

impl StateManagementAccessAnalyzer {
    /// Create a new analyzer.
    pub fn new() -> Self {
        Self {
            enable_static_analysis: true,
        }
    }

    /// Create with configurable static analysis.
    pub fn with_static_analysis(enable: bool) -> Self {
        Self {
            enable_static_analysis: enable,
        }
    }

    /// Analyze EVM bytecode for storage access patterns.
    fn analyze_bytecode(&self, data: &[u8]) -> (HashSet<H256>, HashSet<H256>) {
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();

        if !self.enable_static_analysis || data.is_empty() {
            return (reads, writes);
        }

        // Simple bytecode pattern detection
        // SLOAD = 0x54 (read), SSTORE = 0x55 (write)
        let mut i = 0;
        while i < data.len() {
            match data[i] {
                0x54 => {
                    // SLOAD - read storage
                    // The slot is on the stack, we can't determine it statically
                    // but we mark that storage reads occur
                    debug!("[qc-12] Detected SLOAD at offset {}", i);
                }
                0x55 => {
                    // SSTORE - write storage
                    debug!("[qc-12] Detected SSTORE at offset {}", i);
                }
                _ => {}
            }
            i += 1;
        }

        (reads, writes)
    }
}

impl Default for StateManagementAccessAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AccessPatternAnalyzer for StateManagementAccessAnalyzer {
    async fn analyze_access_pattern(
        &self,
        tx_hash: H256,
        sender: H160,
        to: Option<H160>,
        data: &[u8],
    ) -> Result<AccessPattern, AnalysisError> {
        debug!(
            "[qc-12] Analyzing access pattern for tx {:02x}{:02x}...",
            tx_hash[0], tx_hash[1]
        );

        // Basic pattern: all transactions read sender balance
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();

        // Sender balance is always read (for gas payment)
        let sender_balance_slot = H256::from_slice(&sender.0);
        reads.insert(sender_balance_slot);

        // If contract call, analyze the contract
        if let Some(contract) = to {
            let contract_slot = H256::from_slice(&contract.0);
            reads.insert(contract_slot);

            // Analyze bytecode for storage patterns
            let (code_reads, code_writes) = self.analyze_bytecode(data);
            reads.extend(code_reads);
            writes.extend(code_writes);
        } else {
            // Contract creation - writes to new address
            let create_slot = H256::from_low_u64_be(tx_hash.low_u64());
            writes.insert(create_slot);
        }

        // Sender nonce is always written
        let sender_nonce_slot = {
            let mut slot = [0u8; 32];
            slot[..20].copy_from_slice(&sender.0);
            slot[31] = 1; // Nonce slot indicator
            H256::from(slot)
        };
        writes.insert(sender_nonce_slot);

        Ok(AccessPattern {
            tx_hash,
            reads: reads.into_iter().collect(),
            writes: writes.into_iter().collect(),
            balance_reads: vec![sender],
            balance_writes: if to.is_some() {
                vec![sender, to.unwrap()]
            } else {
                vec![sender]
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_analyze_simple_transfer() {
        let analyzer = StateManagementAccessAnalyzer::new();

        let tx_hash = H256::from([1u8; 32]);
        let sender = H160::from([2u8; 20]);
        let to = Some(H160::from([3u8; 20]));

        let pattern = analyzer
            .analyze_access_pattern(tx_hash, sender, to, &[])
            .await
            .unwrap();

        assert_eq!(pattern.tx_hash, tx_hash);
        assert!(!pattern.reads.is_empty());
        assert!(!pattern.writes.is_empty());
        assert!(pattern.balance_reads.contains(&sender));
    }

    #[tokio::test]
    async fn test_analyze_contract_creation() {
        let analyzer = StateManagementAccessAnalyzer::new();

        let tx_hash = H256::from([1u8; 32]);
        let sender = H160::from([2u8; 20]);

        let pattern = analyzer
            .analyze_access_pattern(tx_hash, sender, None, &[0x60, 0x80])
            .await
            .unwrap();

        assert!(!pattern.writes.is_empty());
        assert_eq!(pattern.balance_writes.len(), 1); // Only sender
    }
}
