//! Mining task abstraction

use crate::{ComputeEngine, ComputeError};
use primitive_types::U256;
use std::sync::Arc;

/// Mining task configuration
pub struct MiningTask {
    /// Block header template (without nonce)
    pub header_template: Vec<u8>,
    /// Difficulty target
    pub target: U256,
    /// Starting nonce
    pub nonce_start: u64,
    /// Number of nonces to search
    pub nonce_count: u64,
}

/// Mining result
#[derive(Debug, Clone)]
pub struct MiningResult {
    /// Found nonce
    pub nonce: u64,
    /// Resulting hash
    pub hash: [u8; 32],
    /// Hash as U256
    pub hash_value: U256,
}

impl MiningTask {
    /// Execute the mining task on the given compute engine
    pub async fn execute(
        self,
        engine: &Arc<dyn ComputeEngine>,
    ) -> Result<Option<MiningResult>, ComputeError> {
        let result = engine
            .pow_mine(
                &self.header_template,
                self.target,
                self.nonce_start,
                self.nonce_count,
            )
            .await?;

        Ok(result.map(|(nonce, hash)| MiningResult {
            nonce,
            hash,
            hash_value: U256::from_big_endian(&hash),
        }))
    }
}

/// Batch mining across multiple nonce ranges
pub struct BatchMiningTask {
    pub header_template: Vec<u8>,
    pub target: U256,
    pub ranges: Vec<(u64, u64)>, // (start, count) pairs
}

impl BatchMiningTask {
    /// Execute all ranges in parallel, return first result
    pub async fn execute(
        self,
        engine: &Arc<dyn ComputeEngine>,
    ) -> Result<Option<MiningResult>, ComputeError> {
        for (start, count) in self.ranges {
            let result = engine
                .pow_mine(&self.header_template, self.target, start, count)
                .await?;

            if let Some((nonce, hash)) = result {
                return Ok(Some(MiningResult {
                    nonce,
                    hash,
                    hash_value: U256::from_big_endian(&hash),
                }));
            }
        }

        Ok(None)
    }
}
