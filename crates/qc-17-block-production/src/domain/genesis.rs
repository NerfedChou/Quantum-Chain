//! Genesis Block Creation
//!
//! Handles the creation and validation of the genesis block that bootstraps
//! the blockchain.

use sha2::{Digest, Sha256};
use shared_types::entities::{
    Address, BlockHeader, ConsensusProof, GenesisConfig, Hash, PublicKey, Transaction,
    ValidatedBlock, ValidatedTransaction, U256,
};

use crate::domain::difficulty::DifficultyConfig;

/// Creates the genesis block from configuration
pub fn create_genesis_block(config: &GenesisConfig) -> Result<ValidatedBlock, GenesisError> {
    // Create genesis transactions for initial allocations
    let mut transactions = Vec::new();

    for (address, amount) in &config.allocations {
        let genesis_tx = create_genesis_transaction(*address, *amount, config.timestamp)?;
        transactions.push(genesis_tx);
    }

    // Calculate merkle root of genesis transactions
    let merkle_root = if transactions.is_empty() {
        [0u8; 32]
    } else {
        calculate_merkle_root(&transactions)
    };

    let header = BlockHeader {
        version: 1,
        height: 0,
        parent_hash: [0u8; 32], // No parent for genesis
        merkle_root,
        state_root: [0u8; 32], // Initial state root
        timestamp: config.timestamp,
        proposer: [0u8; 32], // System genesis
        // Genesis uses same initial difficulty as DifficultyConfig - 2^220
        // This ensures proper difficulty adjustment from the start
        difficulty: DifficultyConfig::default().initial_difficulty,
        nonce: 0, // Genesis doesn't require mining
    };

    // Create empty consensus proof for genesis (trusted by definition)
    let consensus_proof = ConsensusProof {
        block_hash: calculate_block_hash(&header),
        attestations: vec![],
        total_stake: 0,
    };

    Ok(ValidatedBlock {
        header,
        transactions,
        consensus_proof,
    })
}

/// Create a genesis allocation transaction
fn create_genesis_transaction(
    recipient: Address,
    amount: U256,
    _timestamp: u64,
) -> Result<ValidatedTransaction, GenesisError> {
    // Genesis transactions have no sender (minted from nothing)
    let tx = Transaction {
        from: [0u8; 32], // System/Genesis sender
        to: Some(address_to_pubkey(recipient)),
        value: amount.as_u64(), // Safe for genesis allocations
        nonce: 0,
        data: b"GENESIS".to_vec(),
        signature: [0u8; 64], // No signature needed for genesis
    };

    let tx_hash = calculate_transaction_hash(&tx);

    Ok(ValidatedTransaction { inner: tx, tx_hash })
}

/// Creates a coinbase transaction for block mining reward
pub fn create_coinbase_transaction(
    block_height: u64,
    miner_address: Address,
    base_reward: U256,
    transaction_fees: U256,
    _timestamp: u64,
) -> Result<ValidatedTransaction, GenesisError> {
    let total_reward = base_reward + transaction_fees;

    if total_reward > U256::from(u64::MAX) {
        tracing::error!(
            "Coinbase reward overflow: base={}, fees={}, total={}, max={}",
            base_reward,
            transaction_fees,
            total_reward,
            U256::from(u64::MAX)
        );
        return Err(GenesisError::InvalidAmount);
    }

    // Create coinbase transaction
    let tx = Transaction {
        from: [0u8; 32], // Coinbase has no sender (minted)
        to: Some(address_to_pubkey(miner_address)),
        value: total_reward.as_u64(),
        nonce: block_height, // Use block height as nonce
        data: format!("COINBASE:HEIGHT:{}", block_height).into_bytes(),
        signature: [0u8; 64], // No signature for coinbase
    };

    let tx_hash = calculate_transaction_hash(&tx);

    Ok(ValidatedTransaction { inner: tx, tx_hash })
}

/// Calculate the block reward for a given height
///
/// Uses a halving schedule: starts at 50 coins, halves every 210,000 blocks
pub fn calculate_block_reward(height: u64) -> U256 {
    const INITIAL_REWARD: u64 = 50;
    const HALVING_INTERVAL: u64 = 210_000;
    const DECIMALS: u128 = 10u128.pow(8);

    let halvings = height / HALVING_INTERVAL;

    if halvings >= 64 {
        return U256::zero();
    }

    let reward = INITIAL_REWARD >> halvings; // Bit shift for halving
    U256::from(reward) * U256::from(DECIMALS)
}

/// Calculate transaction fees from a list of transactions
pub fn calculate_transaction_fees(transactions: &[ValidatedTransaction]) -> U256 {
    // Simple fee model: each transaction pays a base fee
    // Production: gas_used * gas_price per transaction
    const BASE_FEE: u64 = 1_000_000; // 0.000001 coin per tx

    let total_fees = transactions.len() as u64 * BASE_FEE;
    U256::from(total_fees)
}

// =============================================================================
// Helper Functions
// =============================================================================

fn calculate_block_hash(header: &BlockHeader) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(header.version.to_le_bytes());
    hasher.update(header.height.to_le_bytes());
    hasher.update(header.parent_hash);
    hasher.update(header.merkle_root);
    hasher.update(header.state_root);
    hasher.update(header.timestamp.to_le_bytes());
    hasher.update(header.proposer);
    hasher.finalize().into()
}

fn calculate_transaction_hash(tx: &Transaction) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(tx.from);
    if let Some(to) = tx.to {
        hasher.update(to);
    }
    hasher.update(tx.value.to_le_bytes());
    hasher.update(tx.nonce.to_le_bytes());
    hasher.update(&tx.data);
    hasher.finalize().into()
}

fn calculate_merkle_root(transactions: &[ValidatedTransaction]) -> Hash {
    if transactions.is_empty() {
        return [0u8; 32];
    }

    let mut hashes: Vec<Hash> = transactions.iter().map(|tx| tx.tx_hash).collect();

    while hashes.len() > 1 {
        let mut next_level = Vec::new();

        for chunk in hashes.chunks(2) {
            let combined_hash = if chunk.len() == 2 {
                let mut hasher = Sha256::new();
                hasher.update(chunk[0]);
                hasher.update(chunk[1]);
                hasher.finalize().into()
            } else {
                chunk[0]
            };
            next_level.push(combined_hash);
        }

        hashes = next_level;
    }

    hashes[0]
}

fn address_to_pubkey(address: Address) -> PublicKey {
    let mut pubkey = [0u8; 32];
    pubkey[..20].copy_from_slice(&address);
    pubkey
}

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during genesis block creation
#[derive(Debug, thiserror::Error)]
pub enum GenesisError {
    /// Invalid genesis configuration
    #[error("Invalid genesis configuration: {0}")]
    InvalidConfig(String),

    /// Genesis block already exists
    #[error("Genesis block already exists")]
    AlreadyExists,

    /// Invalid allocation amount
    #[error("Invalid allocation amount")]
    InvalidAmount,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_genesis_block() {
        let config = GenesisConfig::default_dev();
        let genesis = create_genesis_block(&config).expect("Should create genesis block");

        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.parent_hash, [0u8; 32]);
        assert!(genesis.transactions.is_empty()); // Default has no allocations
    }

    #[test]
    fn test_calculate_block_reward() {
        // Initial reward
        let reward_0 = calculate_block_reward(0);
        assert_eq!(reward_0, U256::from(50) * U256::from(10u128.pow(8)));

        // After first halving
        let reward_halving = calculate_block_reward(210_000);
        assert_eq!(reward_halving, U256::from(25) * U256::from(10u128.pow(8)));

        let reward_halving2 = calculate_block_reward(420_000);
        assert_eq!(reward_halving2, U256::from(12) * U256::from(10u128.pow(8)));
    }

    #[test]
    fn test_create_coinbase_transaction() {
        let miner_addr = [1u8; 20];
        // Use smaller rewards to fit in u64
        let base_reward = U256::from(50_000_000_000u64); // 50 Gwei
        let fees = U256::from(1_000_000);
        let timestamp = 1733494800;

        let coinbase = create_coinbase_transaction(1, miner_addr, base_reward, fees, timestamp)
            .expect("Should create coinbase");

        assert_eq!(coinbase.inner.from, [0u8; 32]); // No sender
        assert!(coinbase.inner.data.starts_with(b"COINBASE"));
    }

    #[test]
    fn test_merkle_root_single_tx() {
        let tx = ValidatedTransaction {
            inner: Transaction {
                from: [1u8; 32],
                to: Some([2u8; 32]),
                value: 100,
                nonce: 0,
                data: vec![],
                signature: [0u8; 64],
            },
            tx_hash: [3u8; 32],
        };

        let root = calculate_merkle_root(&[tx]);
        assert_eq!(root, [3u8; 32]);
    }

    #[test]
    fn test_genesis_difficulty_matches_config() {
        // This test verifies the fix for the "blocks too hard to mine" issue.
        // Genesis difficulty must match DifficultyConfig::default().initial_difficulty
        // to prevent runaway difficulty adjustment.
        let config = GenesisConfig::default_dev();
        let genesis = create_genesis_block(&config).expect("Should create genesis block");
        let expected_difficulty = DifficultyConfig::default().initial_difficulty;

        assert_eq!(
            genesis.header.difficulty, expected_difficulty,
            "Genesis difficulty must match DifficultyConfig::default().initial_difficulty to prevent difficulty adjustment issues"
        );

        // Also verify it's NOT the old incorrect value of 2^252
        let old_incorrect_value = U256::from(2).pow(U256::from(252));
        assert_ne!(
            genesis.header.difficulty, old_incorrect_value,
            "Genesis should not use the old incorrect difficulty of 2^252"
        );
    }
}
