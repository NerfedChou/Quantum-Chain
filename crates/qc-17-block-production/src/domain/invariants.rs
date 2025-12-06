//! Invariant checkers for block production
//!
//! These functions enforce the 6 critical invariants that MUST hold
//! for every produced block.

use super::entities::*;
use crate::error::{BlockProductionError, Result};
use std::collections::HashSet;

/// INVARIANT-1: Gas Limit Enforcement
/// The sum of all transaction gas_used MUST NOT exceed block_gas_limit.
pub fn check_gas_limit(block: &BlockTemplate) -> Result<()> {
    if block.total_gas_used > block.header.gas_limit {
        return Err(BlockProductionError::GasLimitExceeded {
            used: block.total_gas_used,
            limit: block.header.gas_limit,
        });
    }
    Ok(())
}

/// INVARIANT-2: Nonce Ordering
/// All transactions from the same sender MUST have sequential nonces.
pub fn check_nonce_ordering(transactions: &[TransactionCandidate]) -> Result<()> {
    use std::collections::HashMap;

    let mut sender_nonces: HashMap<[u8; 20], Vec<u64>> = HashMap::new();

    // Group nonces by sender
    for tx in transactions {
        sender_nonces.entry(tx.from).or_default().push(tx.nonce);
    }

    // Check each sender has sequential nonces
    for (address, mut nonces) in sender_nonces {
        nonces.sort_unstable();

        for i in 1..nonces.len() {
            if nonces[i] != nonces[i - 1] + 1 {
                return Err(BlockProductionError::NonceMismatch {
                    address: hex::encode(address),
                    expected: nonces[i - 1] + 1,
                    actual: nonces[i],
                });
            }
        }
    }

    Ok(())
}

/// INVARIANT-3: State Validity
/// All included transactions MUST simulate successfully.
pub fn check_state_validity(simulations: &[SimulationResult]) -> Result<()> {
    for sim in simulations {
        if !sim.success {
            return Err(BlockProductionError::InternalError(format!(
                "Transaction {:?} failed simulation: {}",
                sim.tx_hash,
                sim.error.as_deref().unwrap_or("unknown error")
            )));
        }
    }
    Ok(())
}

/// INVARIANT-4: No Duplicates
/// No transaction hash appears more than once.
pub fn check_no_duplicates(transactions: &[Vec<u8>]) -> Result<()> {
    use sha2::{Digest, Sha256};

    let mut seen = HashSet::new();

    for tx in transactions {
        let mut hasher = Sha256::new();
        hasher.update(tx);
        let hash = hasher.finalize();

        if !seen.insert(hash) {
            return Err(BlockProductionError::InternalError(format!(
                "Duplicate transaction hash: {:?}",
                hash
            )));
        }
    }

    Ok(())
}

/// INVARIANT-5: Timestamp Monotonicity
/// Block timestamp MUST be >= parent timestamp and <= current time + 15s.
pub fn check_timestamp_validity(
    block_timestamp: u64,
    parent_timestamp: u64,
    current_time: u64,
    max_skew: u64,
) -> Result<()> {
    if block_timestamp < parent_timestamp {
        return Err(BlockProductionError::InvalidConfig(format!(
            "Block timestamp {} is before parent timestamp {}",
            block_timestamp, parent_timestamp
        )));
    }

    if block_timestamp > current_time + max_skew {
        return Err(BlockProductionError::InvalidConfig(format!(
            "Block timestamp {} is too far in future (current: {}, max skew: {})",
            block_timestamp, current_time, max_skew
        )));
    }

    Ok(())
}

/// INVARIANT-6: Fee Profitability
/// Selected transactions SHOULD be ordered by gas_price descending (greedy).
///
/// Note: This is a SHOULD not MUST - MEV bundles may violate this.
pub fn check_fee_ordering(transactions: &[TransactionCandidate]) -> bool {
    for i in 1..transactions.len() {
        if transactions[i].gas_price > transactions[i - 1].gas_price {
            return false; // Higher gas price should come first
        }
    }
    true
}

/// Validate all invariants for a block template
pub fn validate_block_template(template: &BlockTemplate) -> Result<()> {
    // Check gas limit
    check_gas_limit(template)?;

    // Check timestamp (require parent timestamp and current time from context)
    // This will be called by the service with proper context

    // Note: Other invariants checked during transaction selection
    // - Nonce ordering: checked during selection
    // - State validity: checked during simulation
    // - No duplicates: checked during selection
    // - Fee ordering: best-effort during selection

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::U256;

    #[test]
    fn test_gas_limit_enforcement() {
        let mut block = BlockTemplate {
            header: BlockHeader {
                parent_hash: primitive_types::H256::zero(),
                block_number: 1,
                timestamp: 0,
                beneficiary: [0u8; 20],
                gas_used: 30_000_000,
                gas_limit: 30_000_000,
                difficulty: U256::zero(),
                extra_data: vec![],
                merkle_root: None,
                state_root: None,
                nonce: None,
            },
            transactions: vec![],
            total_gas_used: 30_000_000,
            total_fees: U256::zero(),
            consensus_mode: ConsensusMode::ProofOfWork,
            created_at: 0,
        };

        // Should pass at limit
        assert!(check_gas_limit(&block).is_ok());

        // Should fail above limit
        block.total_gas_used = 30_000_001;
        assert!(check_gas_limit(&block).is_err());
    }

    #[test]
    fn test_timestamp_validity() {
        let current = 1000;
        let parent = 900;

        // Valid timestamp
        assert!(check_timestamp_validity(950, parent, current, 15).is_ok());

        // Too old (before parent)
        assert!(check_timestamp_validity(800, parent, current, 15).is_err());

        // Too far in future
        assert!(check_timestamp_validity(1100, parent, current, 15).is_err());
    }

    #[test]
    fn test_fee_ordering() {
        let txs = vec![
            TransactionCandidate {
                transaction: vec![],
                from: [0u8; 20],
                nonce: 0,
                gas_price: U256::from(200),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 0,
                gas_price: U256::from(150),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [2u8; 20],
                nonce: 0,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
        ];

        assert!(check_fee_ordering(&txs));

        // Invalid ordering
        let bad_txs = vec![
            TransactionCandidate {
                transaction: vec![],
                from: [0u8; 20],
                nonce: 0,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 0,
                gas_price: U256::from(200), // Higher price after lower
                gas_limit: 21000,
                signature_valid: true,
            },
        ];

        assert!(!check_fee_ordering(&bad_txs));
    }

    #[test]
    fn test_nonce_ordering_valid() {
        let txs = vec![
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 0,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 1,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [2u8; 20],
                nonce: 5,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [2u8; 20],
                nonce: 6,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
        ];

        assert!(check_nonce_ordering(&txs).is_ok());
    }

    #[test]
    fn test_nonce_ordering_gap() {
        let txs = vec![
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 0,
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 2, // Gap! Should be 1
                gas_price: U256::from(100),
                gas_limit: 21000,
                signature_valid: true,
            },
        ];

        assert!(check_nonce_ordering(&txs).is_err());
    }

    #[test]
    fn test_state_validity() {
        let valid_sims = vec![
            SimulationResult {
                tx_hash: primitive_types::H256::zero(),
                success: true,
                gas_used: 21000,
                state_changes: vec![],
                error: None,
            },
            SimulationResult {
                tx_hash: primitive_types::H256::zero(),
                success: true,
                gas_used: 50000,
                state_changes: vec![],
                error: None,
            },
        ];

        assert!(check_state_validity(&valid_sims).is_ok());

        let invalid_sims = vec![
            SimulationResult {
                tx_hash: primitive_types::H256::zero(),
                success: true,
                gas_used: 21000,
                state_changes: vec![],
                error: None,
            },
            SimulationResult {
                tx_hash: primitive_types::H256::zero(),
                success: false, // Failed!
                gas_used: 0,
                state_changes: vec![],
                error: Some("out of gas".to_string()),
            },
        ];

        assert!(check_state_validity(&invalid_sims).is_err());
    }

    #[test]
    fn test_no_duplicates() {
        let unique_txs = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];

        assert!(check_no_duplicates(&unique_txs).is_ok());

        let duplicate_txs = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![1, 2, 3], // Duplicate!
        ];

        assert!(check_no_duplicates(&duplicate_txs).is_err());
    }

    #[test]
    fn test_validate_block_template() {
        let template = BlockTemplate {
            header: BlockHeader {
                parent_hash: primitive_types::H256::zero(),
                block_number: 1,
                timestamp: 0,
                beneficiary: [0u8; 20],
                gas_used: 20_000_000,
                gas_limit: 30_000_000,
                difficulty: U256::zero(),
                extra_data: vec![],
                merkle_root: None,
                state_root: None,
                nonce: None,
            },
            transactions: vec![],
            total_gas_used: 20_000_000,
            total_fees: U256::zero(),
            consensus_mode: ConsensusMode::ProofOfWork,
            created_at: 0,
        };

        assert!(validate_block_template(&template).is_ok());
    }
}
