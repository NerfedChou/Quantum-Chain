//! Validator Set Provider Adapter
//!
//! Implements `ValidatorSetProvider` port using State Management (qc-04).
//! Reference: SPEC-09-FINALITY.md Section 3.2, IPC-MATRIX.md

use crate::domain::{ValidatorId, ValidatorSet};
use crate::error::{FinalityError, FinalityResult};
use crate::ports::outbound::ValidatorSetProvider;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, info};

/// Adapter that queries State Management (qc-04) for validator stake information.
///
/// Per IPC-MATRIX.md, State Management is authoritative for stake data.
/// This adapter would integrate via the event bus for queries.
pub struct StateManagementValidatorProvider {
    /// Cache of validator sets by epoch (to avoid repeated queries).
    cache: parking_lot::RwLock<HashMap<u64, ValidatorSet>>,
    /// Default stake for testing.
    default_stake: u128,
}

impl StateManagementValidatorProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self {
            cache: parking_lot::RwLock::new(HashMap::new()),
            default_stake: 32_000_000_000, // 32 ETH in gwei
        }
    }

    /// Create with custom default stake.
    pub fn with_default_stake(default_stake: u128) -> Self {
        Self {
            cache: parking_lot::RwLock::new(HashMap::new()),
            default_stake,
        }
    }

    /// Pre-populate cache for testing.
    pub fn with_cached_set(epoch: u64, validator_set: ValidatorSet) -> Self {
        let provider = Self::new();
        provider.cache.write().insert(epoch, validator_set);
        provider
    }
}

impl Default for StateManagementValidatorProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidatorSetProvider for StateManagementValidatorProvider {
    async fn get_validator_set_at_epoch(&self, epoch: u64) -> FinalityResult<ValidatorSet> {
        // Check cache first
        if let Some(cached) = self.cache.read().get(&epoch) {
            debug!("[qc-09] Cache hit for validator set at epoch {}", epoch);
            return Ok(cached.clone());
        }

        info!(
            "[qc-09] 📥 Querying State Management for validator set at epoch {}",
            epoch
        );

        // TODO: Query qc-04 via event bus
        // For now, return a minimal test set using the proper constructor methods
        let mut validator_set = ValidatorSet::new(epoch);

        // Add test validators using the proper API
        validator_set.add_validator(ValidatorId([1u8; 32]), self.default_stake);
        validator_set.add_validator(ValidatorId([2u8; 32]), self.default_stake);
        validator_set.add_validator(ValidatorId([3u8; 32]), self.default_stake);

        // Cache the result
        self.cache.write().insert(epoch, validator_set.clone());

        Ok(validator_set)
    }

    async fn get_validator_stake(
        &self,
        validator_id: &ValidatorId,
        epoch: u64,
    ) -> FinalityResult<u128> {
        let validator_set = self.get_validator_set_at_epoch(epoch).await?;

        match validator_set.get_stake(validator_id) {
            Some(stake) => Ok(stake),
            None => Err(FinalityError::UnknownValidator {
                validator_id: validator_id.0,
            }),
        }
    }

    async fn get_total_active_stake(&self, epoch: u64) -> FinalityResult<u128> {
        let validator_set = self.get_validator_set_at_epoch(epoch).await?;
        Ok(validator_set.total_stake())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_validator_set() {
        let provider = StateManagementValidatorProvider::new();
        let set = provider.get_validator_set_at_epoch(1).await.unwrap();

        assert_eq!(set.epoch(), 1);
        assert_eq!(set.len(), 3);
    }

    #[tokio::test]
    async fn test_get_validator_stake() {
        let provider = StateManagementValidatorProvider::new();
        let stake = provider
            .get_validator_stake(&ValidatorId([1u8; 32]), 1)
            .await
            .unwrap();

        assert_eq!(stake, 32_000_000_000);
    }

    #[tokio::test]
    async fn test_unknown_validator_error() {
        let provider = StateManagementValidatorProvider::new();
        let result = provider
            .get_validator_stake(&ValidatorId([99u8; 32]), 1)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let mut custom_set = ValidatorSet::new(5);
        custom_set.add_validator(ValidatorId([10u8; 32]), 1000);

        let provider = StateManagementValidatorProvider::with_cached_set(5, custom_set);
        let set = provider.get_validator_set_at_epoch(5).await.unwrap();

        assert_eq!(set.len(), 1);
        assert_eq!(set.get_stake(&ValidatorId([10u8; 32])), Some(1000));
    }
}
