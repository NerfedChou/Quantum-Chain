//! Domain services for the Mempool subsystem.
//!
//! Provides business logic functions that operate on domain entities.

use super::entities::{Hash, MempoolTransaction, U256};
use super::value_objects::ShortTxId;

/// Calculates the minimum gas price required for RBF replacement.
///
/// Formula: old_price * (100 + bump_percent) / 100
pub fn calculate_rbf_min_price(old_price: U256, bump_percent: u64) -> U256 {
    old_price * U256::from(100 + bump_percent) / U256::from(100)
}

/// Checks if a new gas price is sufficient for RBF.
pub fn is_valid_rbf_bump(old_price: U256, new_price: U256, bump_percent: u64) -> bool {
    new_price >= calculate_rbf_min_price(old_price, bump_percent)
}

/// Calculates short transaction IDs for a list of transactions.
///
/// Per System.md Subsystem 5 - Compact Block Relay.
pub fn calculate_short_ids(tx_hashes: &[Hash], nonce: u64) -> Vec<ShortTxId> {
    tx_hashes
        .iter()
        .map(|hash| ShortTxId::from_hash(hash, nonce))
        .collect()
}

/// Estimates memory usage for a transaction.
pub fn estimate_tx_memory(tx: &MempoolTransaction) -> usize {
    std::mem::size_of::<MempoolTransaction>() + tx.transaction.data.len()
}

/// Validates that transactions from a sender maintain nonce ordering.
///
/// Returns true if all nonces form a contiguous sequence starting from `start_nonce`.
pub fn validate_nonce_sequence(transactions: &[&MempoolTransaction], start_nonce: u64) -> bool {
    let mut sorted: Vec<_> = transactions.iter().map(|t| t.nonce).collect();
    sorted.sort();

    for (i, nonce) in sorted.iter().enumerate() {
        if *nonce != start_nonce + i as u64 {
            return false;
        }
    }
    true
}

/// Computes the total gas for a list of transactions.
pub fn total_gas(transactions: &[&MempoolTransaction]) -> u64 {
    transactions.iter().map(|tx| tx.gas_limit).sum()
}

/// Computes the total value for a list of transactions.
pub fn total_value(transactions: &[&MempoolTransaction]) -> U256 {
    transactions
        .iter()
        .fold(U256::zero(), |acc, tx| acc + tx.transaction.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::MempoolTransaction;
    use shared_types::SignedTransaction;

    fn create_signed_tx(nonce: u64, gas_price: U256, value: U256) -> SignedTransaction {
        SignedTransaction {
            from: [0xAA; 20],
            to: Some([0xBB; 20]),
            value,
            nonce,
            gas_price,
            gas_limit: 21000,
            data: vec![1, 2, 3, 4],
            signature: [0u8; 64],
        }
    }

    fn create_tx(nonce: u64, gas_price: u64) -> MempoolTransaction {
        let signed_tx = create_signed_tx(nonce, U256::from(gas_price), U256::from(1000u64));
        MempoolTransaction::new(signed_tx, 1000)
    }

    #[test]
    fn test_calculate_rbf_min_price() {
        // 10% bump on 1 gwei
        let min = calculate_rbf_min_price(U256::from(1_000_000_000u64), 10);
        assert_eq!(min, U256::from(1_100_000_000u64));

        // 25% bump on 2 gwei
        let min = calculate_rbf_min_price(U256::from(2_000_000_000u64), 25);
        assert_eq!(min, U256::from(2_500_000_000u64));
    }

    #[test]
    fn test_is_valid_rbf_bump() {
        // Exact bump
        assert!(is_valid_rbf_bump(
            U256::from(1_000_000_000u64),
            U256::from(1_100_000_000u64),
            10
        ));

        // Above bump
        assert!(is_valid_rbf_bump(
            U256::from(1_000_000_000u64),
            U256::from(1_200_000_000u64),
            10
        ));

        // Below bump
        assert!(!is_valid_rbf_bump(
            U256::from(1_000_000_000u64),
            U256::from(1_050_000_000u64),
            10
        ));
    }

    #[test]
    fn test_calculate_short_ids() {
        let hashes = vec![[0xAA; 32], [0xBB; 32], [0xCC; 32]];
        let nonce = 12345u64;

        let short_ids = calculate_short_ids(&hashes, nonce);
        assert_eq!(short_ids.len(), 3);

        // Each should be unique
        assert_ne!(short_ids[0], short_ids[1]);
        assert_ne!(short_ids[1], short_ids[2]);
    }

    #[test]
    fn test_validate_nonce_sequence_valid() {
        let tx0 = create_tx(0, 1_000_000_000);
        let tx1 = create_tx(1, 1_000_000_000);
        let tx2 = create_tx(2, 1_000_000_000);

        let txs: Vec<&MempoolTransaction> = vec![&tx0, &tx1, &tx2];
        assert!(validate_nonce_sequence(&txs, 0));
    }

    #[test]
    fn test_validate_nonce_sequence_with_gap() {
        let tx0 = create_tx(0, 1_000_000_000);
        let tx2 = create_tx(2, 1_000_000_000); // Gap at nonce 1

        let txs: Vec<&MempoolTransaction> = vec![&tx0, &tx2];
        assert!(!validate_nonce_sequence(&txs, 0));
    }

    #[test]
    fn test_validate_nonce_sequence_wrong_start() {
        let tx1 = create_tx(1, 1_000_000_000);
        let tx2 = create_tx(2, 1_000_000_000);

        let txs: Vec<&MempoolTransaction> = vec![&tx1, &tx2];
        assert!(!validate_nonce_sequence(&txs, 0)); // Expected to start at 0
        assert!(validate_nonce_sequence(&txs, 1)); // But starts at 1
    }

    #[test]
    fn test_total_gas() {
        let tx1 = create_tx(0, 1_000_000_000);
        let tx2 = create_tx(1, 1_000_000_000);

        let txs: Vec<&MempoolTransaction> = vec![&tx1, &tx2];
        assert_eq!(total_gas(&txs), 42000); // 21000 * 2
    }

    #[test]
    fn test_total_value() {
        let tx1 = create_tx(0, 1_000_000_000);
        let tx2 = create_tx(1, 1_000_000_000);

        let txs: Vec<&MempoolTransaction> = vec![&tx1, &tx2];
        assert_eq!(total_value(&txs), U256::from(2000u64)); // 1000 * 2
    }

    #[test]
    fn test_estimate_tx_memory() {
        let tx = create_tx(0, 1_000_000_000);
        let mem = estimate_tx_memory(&tx);

        // Should include base struct size + data
        assert!(mem > std::mem::size_of::<MempoolTransaction>());
    }
}
