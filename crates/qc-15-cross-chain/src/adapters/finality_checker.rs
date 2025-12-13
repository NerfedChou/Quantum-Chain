//! Finality Checker Adapter
//!
//! Implements `FinalityChecker` port for confirmation counting.
//! Reference: SPEC-15 Section 3.2

use crate::domain::{ChainId, CrossChainError, CrossChainProof};
use crate::ports::outbound::FinalityChecker;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// Configurable finality checker.
///
/// Uses chain-specific confirmation requirements.
pub struct ConfigurableFinalityChecker {
    /// Custom confirmation requirements (overrides ChainId defaults).
    custom_confirmations: HashMap<ChainId, u64>,
}

impl ConfigurableFinalityChecker {
    /// Create with default chain confirmations.
    pub fn new() -> Self {
        Self {
            custom_confirmations: HashMap::new(),
        }
    }

    /// Override confirmation requirement for a chain.
    pub fn with_custom(mut self, chain: ChainId, confirmations: u64) -> Self {
        self.custom_confirmations.insert(chain, confirmations);
        self
    }

    /// Set custom confirmations for testing (lower values).
    pub fn for_testing() -> Self {
        Self::new()
            .with_custom(ChainId::QuantumChain, 2)
            .with_custom(ChainId::Ethereum, 2)
            .with_custom(ChainId::Bitcoin, 2)
            .with_custom(ChainId::Polygon, 2)
            .with_custom(ChainId::Arbitrum, 1)
    }
}

impl Default for ConfigurableFinalityChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FinalityChecker for ConfigurableFinalityChecker {
    fn required_confirmations(&self, chain: ChainId) -> u64 {
        self.custom_confirmations
            .get(&chain)
            .copied()
            .unwrap_or_else(|| chain.required_confirmations())
    }

    async fn is_proof_final(&self, proof: &CrossChainProof) -> Result<bool, CrossChainError> {
        let required = self.required_confirmations(proof.chain);

        debug!(
            "[qc-15] Checking finality for {:?}: {}/{} confirmations",
            proof.chain, proof.confirmations, required
        );

        Ok(proof.confirmations >= required)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proof(chain: ChainId, confirmations: u64) -> CrossChainProof {
        CrossChainProof {
            chain,
            block_hash: [0u8; 32],
            block_height: 1000,
            tx_hash: [1u8; 32],
            merkle_proof: vec![[2u8; 32]],
            confirmations,
        }
    }

    #[test]
    fn test_default_confirmations() {
        let checker = ConfigurableFinalityChecker::new();

        assert_eq!(checker.required_confirmations(ChainId::QuantumChain), 6);
        assert_eq!(checker.required_confirmations(ChainId::Ethereum), 12);
        assert_eq!(checker.required_confirmations(ChainId::Bitcoin), 6);
    }

    #[test]
    fn test_custom_confirmations() {
        let checker = ConfigurableFinalityChecker::new()
            .with_custom(ChainId::QuantumChain, 10);

        assert_eq!(checker.required_confirmations(ChainId::QuantumChain), 10);
        assert_eq!(checker.required_confirmations(ChainId::Ethereum), 12); // Default
    }

    #[tokio::test]
    async fn test_proof_final_when_enough_confirmations() {
        let checker = ConfigurableFinalityChecker::new();
        let proof = make_proof(ChainId::QuantumChain, 10); // 10 >= 6

        let is_final = checker.is_proof_final(&proof).await.unwrap();
        assert!(is_final);
    }

    #[tokio::test]
    async fn test_proof_not_final_when_insufficient_confirmations() {
        let checker = ConfigurableFinalityChecker::new();
        let proof = make_proof(ChainId::QuantumChain, 3); // 3 < 6

        let is_final = checker.is_proof_final(&proof).await.unwrap();
        assert!(!is_final);
    }

    #[tokio::test]
    async fn test_testing_mode_lower_requirements() {
        let checker = ConfigurableFinalityChecker::for_testing();
        let proof = make_proof(ChainId::QuantumChain, 2); // 2 >= 2 (testing mode)

        let is_final = checker.is_proof_final(&proof).await.unwrap();
        assert!(is_final);
    }
}
