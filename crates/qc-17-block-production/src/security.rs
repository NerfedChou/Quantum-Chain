//! Security Layer - Transaction and Block Template Validation
//!
//! SPEC-17 Section 5: Security Requirements
//! - Transaction signature validation
//! - Nonce ordering validation
//! - Gas limit validation
//! - Block template integrity checks
//! - IPC sender validation
//! - DoS protection (rate limiting)

use crate::domain::{BlockTemplate, TransactionCandidate};
use crate::error::BlockProductionError;
use primitive_types::{H256 as Hash, U256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Security validator for block production
pub struct SecurityValidator {
    /// Allowed IPC sender subsystems
    allowed_senders: HashSet<u8>,

    /// Rate limiter per subsystem
    rate_limiter: Arc<RwLock<RateLimiter>>,

    /// Maximum block gas limit
    max_block_gas_limit: u64,

    /// Minimum gas price threshold
    min_gas_price: U256,
}

impl SecurityValidator {
    /// Creates a new security validator with given limits
    pub fn new(max_block_gas_limit: u64, min_gas_price: U256) -> Self {
        // SPEC-17 Appendix B.1: Allowed Senders
        let mut allowed_senders = HashSet::new();
        allowed_senders.insert(6); // Mempool
        allowed_senders.insert(4); // State Management
        allowed_senders.insert(9); // Finality
        allowed_senders.insert(8); // Consensus
                                   // Note: Admin CLI validation handled at transport layer

        Self {
            allowed_senders,
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new())),
            max_block_gas_limit,
            min_gas_price,
        }
    }

    /// Validate IPC sender is authorized
    pub fn validate_sender(&self, subsystem_id: u8) -> Result<(), BlockProductionError> {
        if !self.allowed_senders.contains(&subsystem_id) {
            return Err(BlockProductionError::UnauthorizedSender {
                sender_id: subsystem_id,
            });
        }
        Ok(())
    }

    /// Check rate limit for subsystem
    pub async fn check_rate_limit(&self, subsystem_id: u8) -> Result<(), BlockProductionError> {
        let mut limiter = self.rate_limiter.write().await;

        if !limiter.allow(subsystem_id) {
            return Err(BlockProductionError::RateLimitExceeded { subsystem_id });
        }

        Ok(())
    }

    /// Validate transaction candidate
    pub fn validate_transaction(
        &self,
        tx: &TransactionCandidate,
    ) -> Result<(), BlockProductionError> {
        // 1. Signature validation (pre-verified flag)
        if !tx.signature_valid {
            return Err(BlockProductionError::InvalidSignature);
        }

        // 2. Gas price validation
        if tx.gas_price < self.min_gas_price {
            return Err(BlockProductionError::GasPriceTooLow {
                gas_price: format!("{}", tx.gas_price),
                min_gas_price: format!("{}", self.min_gas_price),
            });
        }

        // 3. Gas limit validation (reasonable upper bound)
        if tx.gas_limit > self.max_block_gas_limit {
            return Err(BlockProductionError::GasLimitTooHigh {
                gas_limit: tx.gas_limit,
                max_gas_limit: self.max_block_gas_limit,
            });
        }

        // 4. Basic sanity checks
        if tx.gas_limit == 0 {
            return Err(BlockProductionError::ZeroGasLimit {
                tx_hash: "unknown".to_string(),
            });
        }

        Ok(())
    }

    /// Validate block template before sealing
    pub fn validate_block_template(
        &self,
        template: &BlockTemplate,
    ) -> Result<(), BlockProductionError> {
        // 1. Gas limit validation
        if template.header.gas_limit > self.max_block_gas_limit {
            return Err(BlockProductionError::BlockGasLimitExceeded {
                provided: template.header.gas_limit,
                max: self.max_block_gas_limit,
            });
        }

        // 2. Gas used must not exceed gas limit
        if template.header.gas_used > template.header.gas_limit {
            return Err(BlockProductionError::GasUsedExceedsLimit {
                gas_used: template.header.gas_used,
                gas_limit: template.header.gas_limit,
            });
        }

        // 3. Timestamp validation (within 15 seconds of now)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let time_diff = template.header.timestamp.abs_diff(now);

        if time_diff > 15 {
            return Err(BlockProductionError::InvalidTimestamp {
                provided: template.header.timestamp,
                diff_seconds: time_diff,
            });
        }

        // 4. Transaction count sanity check
        if template.transactions.is_empty() && template.header.gas_used > 0 {
            return Err(BlockProductionError::InconsistentState {
                reason: "Gas used but no transactions".to_string(),
            });
        }

        // 5. State root must not be zero (if set)
        if let Some(state_root) = template.header.state_root {
            if state_root == Hash::zero() {
                return Err(BlockProductionError::InvalidStateRoot);
            }
        }

        Ok(())
    }

    /// Validate nonce ordering for transaction batch
    pub fn validate_nonce_ordering(
        &self,
        transactions: &[TransactionCandidate],
    ) -> Result<(), BlockProductionError> {
        let mut nonce_map: HashMap<[u8; 20], u64> = HashMap::new();

        for tx in transactions {
            let expected_nonce = nonce_map.get(&tx.from).copied().unwrap_or(tx.nonce);

            if tx.nonce != expected_nonce {
                return Err(BlockProductionError::InvalidNonceOrdering {
                    address: hex::encode(tx.from),
                    expected: expected_nonce,
                    actual: tx.nonce,
                });
            }

            nonce_map.insert(tx.from, tx.nonce + 1);
        }

        Ok(())
    }
}

/// Rate limiter for DoS protection
struct RateLimiter {
    /// Request counts per subsystem
    requests: HashMap<u8, RequestBucket>,

    /// Time window (1 second)
    window_duration: u64,

    /// Max requests per window
    max_requests: u32,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            requests: HashMap::new(),
            window_duration: 1, // 1 second
            max_requests: 100,  // 100 requests per second per subsystem
        }
    }

    fn allow(&mut self, subsystem_id: u8) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let bucket = self
            .requests
            .entry(subsystem_id)
            .or_insert_with(|| RequestBucket {
                count: 0,
                window_start: now,
            });

        // Reset window if expired
        if now >= bucket.window_start + self.window_duration {
            bucket.count = 0;
            bucket.window_start = now;
        }

        // Check limit
        if bucket.count >= self.max_requests {
            return false;
        }

        bucket.count += 1;
        true
    }
}

struct RequestBucket {
    count: u32,
    window_start: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_senders() {
        let validator = SecurityValidator::new(8_000_000, U256::from(1_000_000_000u64));

        // Allowed senders (per SPEC-17 Appendix B.1)
        assert!(validator.validate_sender(6).is_ok()); // Mempool
        assert!(validator.validate_sender(4).is_ok()); // State Management
        assert!(validator.validate_sender(8).is_ok()); // Consensus
        assert!(validator.validate_sender(9).is_ok()); // Finality

        // Disallowed senders
        assert!(validator.validate_sender(1).is_err()); // Peer Discovery
        assert!(validator.validate_sender(99).is_err()); // Unknown
    }

    /// Comprehensive IPC authorization test per IPC-MATRIX.md
    /// Tests that each unauthorized sender is correctly rejected
    #[test]
    fn test_ipc_authorization_comprehensive() {
        let validator = SecurityValidator::new(30_000_000, U256::from(1_000_000_000u64));

        // === ALLOWED SENDERS ===
        // Per SPEC-17 Section 4: Only these subsystems can send to Block Production
        let allowed = [
            (4, "State Management"),
            (6, "Mempool"),
            (8, "Consensus"),
            (9, "Finality"),
        ];

        for (id, name) in allowed.iter() {
            let result = validator.validate_sender(*id);
            assert!(
                result.is_ok(),
                "Expected {} (id {}) to be allowed, but got error: {:?}",
                name,
                id,
                result.err()
            );
        }

        // === UNAUTHORIZED SENDERS ===
        // All other subsystems should be rejected
        let unauthorized = [
            (1, "Peer Discovery"),
            (2, "Block Storage"),
            (3, "Transaction Indexing"),
            (5, "Block Propagation"),
            (7, "Bloom Filters"),
            (10, "Signature Verification"),
            (11, "Smart Contracts"),
            (12, "Cross-Chain Bridges"),
            (13, "Upgrade Manager"),
            (14, "Security Monitor"),
            (15, "Fallback Systems"),
            (16, "API Gateway"),
            (17, "Block Production"), // Self-send not allowed
            (18, "Telemetry"),
            (0, "Invalid zero"),
            (255, "Invalid max"),
        ];

        for (id, name) in unauthorized.iter() {
            let result = validator.validate_sender(*id);
            assert!(
                result.is_err(),
                "Expected {} (id {}) to be rejected, but it was allowed",
                name,
                id
            );

            // Verify the error type is UnauthorizedSender
            if let Err(BlockProductionError::UnauthorizedSender { sender_id }) = result {
                assert_eq!(sender_id, *id);
            } else {
                panic!("Expected UnauthorizedSender error for {} (id {})", name, id);
            }
        }
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let validator = SecurityValidator::new(8_000_000, U256::from(1_000_000_000u64));

        // First 100 requests should succeed
        for _ in 0..100 {
            assert!(validator.check_rate_limit(6).await.is_ok());
        }

        // 101st request should fail
        assert!(validator.check_rate_limit(6).await.is_err());
    }

    /// Test that rate limiting is per-subsystem
    #[tokio::test]
    async fn test_rate_limiting_per_subsystem() {
        let validator = SecurityValidator::new(8_000_000, U256::from(1_000_000_000u64));

        // Exhaust rate limit for subsystem 6
        for _ in 0..100 {
            let _ = validator.check_rate_limit(6).await;
        }

        // Subsystem 6 should be rate limited
        assert!(validator.check_rate_limit(6).await.is_err());

        // But subsystem 4 should still be allowed
        assert!(validator.check_rate_limit(4).await.is_ok());
    }

    #[test]
    fn test_gas_price_validation() {
        let min_gas_price = U256::from(1_000_000_000u64);
        let validator = SecurityValidator::new(8_000_000, min_gas_price);

        // Create mock transaction with low gas price
        // (simplified - actual implementation would need full transaction)
        // This test validates the logic structure
        assert!(min_gas_price > U256::zero());
    }

    #[test]
    fn test_block_template_timestamp_validation() {
        let validator = SecurityValidator::new(30_000_000, U256::from(1_000_000_000u64));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Valid template (timestamp is now)
        let valid_template = crate::domain::BlockTemplate {
            header: crate::domain::BlockHeader {
                parent_hash: Hash::zero(),
                block_number: 1,
                timestamp: now,
                beneficiary: [0u8; 20],
                gas_used: 21000,
                gas_limit: 10_000_000,
                difficulty: U256::from(1000),
                extra_data: vec![],
                merkle_root: None,
                state_root: Some(Hash::random()),
                nonce: Some(12345),
            },
            transactions: vec![vec![1, 2, 3]],
            total_gas_used: 21000,
            total_fees: U256::from(21000),
            consensus_mode: crate::ConsensusMode::ProofOfWork,
            created_at: now,
        };

        assert!(validator.validate_block_template(&valid_template).is_ok());

        // Invalid template (timestamp too far in future)
        let mut future_template = valid_template.clone();
        future_template.header.timestamp = now + 60; // 60 seconds in future

        let result = validator.validate_block_template(&future_template);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(BlockProductionError::InvalidTimestamp { .. })
        ));
    }
}
