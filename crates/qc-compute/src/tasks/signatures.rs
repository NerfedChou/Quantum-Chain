//! Signature verification tasks

use crate::{ComputeEngine, ComputeError};
use std::sync::Arc;

/// Batch ECDSA signature verification
pub struct BatchEcdsaVerifyTask {
    pub messages: Vec<[u8; 32]>,
    pub signatures: Vec<[u8; 65]>,
    pub public_keys: Vec<[u8; 33]>,
}

/// Verification result for a batch
#[derive(Debug, Clone)]
pub struct BatchVerifyResult {
    pub results: Vec<bool>,
    pub valid_count: usize,
    pub invalid_count: usize,
}

impl BatchEcdsaVerifyTask {
    /// Execute batch verification
    pub async fn execute(
        self,
        engine: &Arc<dyn ComputeEngine>,
    ) -> Result<BatchVerifyResult, ComputeError> {
        let results = engine
            .batch_verify_ecdsa(&self.messages, &self.signatures, &self.public_keys)
            .await?;

        let valid_count = results.iter().filter(|&&v| v).count();
        let invalid_count = results.len() - valid_count;

        Ok(BatchVerifyResult {
            results,
            valid_count,
            invalid_count,
        })
    }
}

/// Single ECDSA verification (convenience wrapper)
pub struct EcdsaVerifyTask {
    pub message: [u8; 32],
    pub signature: [u8; 65],
    pub public_key: [u8; 33],
}

impl EcdsaVerifyTask {
    /// Execute single verification
    pub async fn execute(self, engine: &Arc<dyn ComputeEngine>) -> Result<bool, ComputeError> {
        let results = engine
            .batch_verify_ecdsa(&[self.message], &[self.signature], &[self.public_key])
            .await?;

        Ok(results.first().copied().unwrap_or(false))
    }
}
