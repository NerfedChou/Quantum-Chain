//! # Genesis Block Builder
//!
//! Creates and validates the genesis block for chain initialization.

use std::time::{SystemTime, UNIX_EPOCH};

use sha3::{Digest, Keccak256};
use thiserror::Error;

/// Genesis block creation errors.
#[derive(Debug, Error)]
pub enum GenesisError {
    /// Genesis block already exists in storage.
    #[error("Genesis block already exists at height 0")]
    AlreadyExists,

    /// Failed to store genesis block.
    #[error("Failed to store genesis block: {0}")]
    StorageFailed(String),

    /// Invalid genesis configuration.
    #[error("Invalid genesis configuration: {0}")]
    InvalidConfig(String),

    /// State initialization failed.
    #[error("Failed to initialize genesis state: {0}")]
    StateInitFailed(String),
}

/// Genesis block configuration.
#[derive(Debug, Clone)]
pub struct GenesisConfig {
    /// Chain ID (e.g., 1 for mainnet, 5 for testnet).
    pub chain_id: u64,

    /// Genesis timestamp (Unix seconds).
    /// If None, uses current time.
    pub timestamp: Option<u64>,

    /// Initial validator set (public keys).
    pub initial_validators: Vec<[u8; 33]>,

    /// Initial validator stakes (in wei).
    pub initial_stakes: Vec<u128>,

    /// Protocol version.
    pub protocol_version: u32,

    /// Extra data (max 32 bytes, e.g., "Quantum-Chain Genesis").
    pub extra_data: Vec<u8>,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            timestamp: None,
            initial_validators: Vec::new(),
            initial_stakes: Vec::new(),
            protocol_version: 1,
            extra_data: b"Quantum-Chain Genesis".to_vec(),
        }
    }
}

impl GenesisConfig {
    /// Create a testnet configuration.
    pub fn testnet() -> Self {
        Self {
            chain_id: 5,
            extra_data: b"Quantum-Chain Testnet".to_vec(),
            ..Default::default()
        }
    }

    /// Create a devnet configuration with single validator.
    pub fn devnet(validator_pubkey: [u8; 33]) -> Self {
        Self {
            chain_id: 31337,
            initial_validators: vec![validator_pubkey],
            initial_stakes: vec![32_000_000_000_000_000_000], // 32 ETH equivalent
            extra_data: b"Quantum-Chain Devnet".to_vec(),
            ..Default::default()
        }
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), GenesisError> {
        if self.initial_validators.len() != self.initial_stakes.len() {
            return Err(GenesisError::InvalidConfig(
                "Validator count must match stake count".to_string(),
            ));
        }

        if self.extra_data.len() > 32 {
            return Err(GenesisError::InvalidConfig(
                "Extra data exceeds 32 bytes".to_string(),
            ));
        }

        Ok(())
    }
}

/// The genesis block structure.
#[derive(Debug, Clone)]
pub struct GenesisBlock {
    /// Block header.
    pub header: GenesisHeader,

    /// Initial validator set.
    pub validators: Vec<ValidatorInfo>,

    /// Genesis state root (empty trie).
    pub state_root: [u8; 32],

    /// Genesis transactions root (empty tree).
    pub transactions_root: [u8; 32],
}

/// Genesis block header.
#[derive(Debug, Clone)]
pub struct GenesisHeader {
    /// Always 0 for genesis.
    pub height: u64,

    /// Always 32 zero bytes for genesis.
    pub parent_hash: [u8; 32],

    /// Hash of the block content.
    pub block_hash: [u8; 32],

    /// Merkle root of transactions (empty for genesis).
    pub merkle_root: [u8; 32],

    /// State trie root after genesis state.
    pub state_root: [u8; 32],

    /// Genesis timestamp.
    pub timestamp: u64,

    /// Chain ID.
    pub chain_id: u64,

    /// Protocol version.
    pub protocol_version: u32,

    /// Extra data.
    pub extra_data: Vec<u8>,
}

/// Validator information in genesis.
#[derive(Debug, Clone)]
pub struct ValidatorInfo {
    /// Compressed public key (33 bytes).
    pub pubkey: [u8; 33],

    /// Initial stake in wei.
    pub stake: u128,

    /// Derived address (20 bytes).
    pub address: [u8; 20],
}

/// Empty Merkle tree root (Keccak256 of empty bytes).
pub const EMPTY_MERKLE_ROOT: [u8; 32] = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];

/// Empty Patricia trie root (Keccak256 of RLP-encoded empty string).
pub const EMPTY_STATE_ROOT: [u8; 32] = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];

/// Builder for creating genesis blocks.
pub struct GenesisBuilder {
    config: GenesisConfig,
}

impl GenesisBuilder {
    /// Create a new genesis builder with configuration.
    pub fn new(config: GenesisConfig) -> Self {
        Self { config }
    }

    /// Build the genesis block.
    pub fn build(self) -> Result<GenesisBlock, GenesisError> {
        self.config.validate()?;

        let timestamp = self.config.timestamp.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0) // Fallback to epoch if system time is before UNIX_EPOCH
        });

        // Build validator info
        let validators: Vec<ValidatorInfo> = self
            .config
            .initial_validators
            .iter()
            .zip(self.config.initial_stakes.iter())
            .map(|(pubkey, stake)| {
                let address = derive_address_from_pubkey(pubkey);
                ValidatorInfo {
                    pubkey: *pubkey,
                    stake: *stake,
                    address,
                }
            })
            .collect();

        // Compute state root (includes validator balances)
        let state_root = if validators.is_empty() {
            EMPTY_STATE_ROOT
        } else {
            // In production: compute actual Patricia trie root with validator states
            compute_genesis_state_root(&validators)
        };

        // Create header (without hash first)
        let mut header = GenesisHeader {
            height: 0,
            parent_hash: [0u8; 32],
            block_hash: [0u8; 32], // Will be computed
            merkle_root: EMPTY_MERKLE_ROOT,
            state_root,
            timestamp,
            chain_id: self.config.chain_id,
            protocol_version: self.config.protocol_version,
            extra_data: self.config.extra_data.clone(),
        };

        // Compute block hash
        header.block_hash = compute_genesis_hash(&header);

        Ok(GenesisBlock {
            header,
            validators,
            state_root,
            transactions_root: EMPTY_MERKLE_ROOT,
        })
    }
}

/// Derive address from compressed public key.
fn derive_address_from_pubkey(pubkey: &[u8; 33]) -> [u8; 20] {
    // Keccak256 of public key, take last 20 bytes
    let hash = Keccak256::digest(pubkey);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..32]);
    address
}

/// Compute genesis state root from validator set.
fn compute_genesis_state_root(validators: &[ValidatorInfo]) -> [u8; 32] {
    // Simplified: hash all validator data together
    // In production: build actual Patricia Merkle Trie
    let mut hasher = Keccak256::new();
    
    for validator in validators {
        hasher.update(validator.address);
        hasher.update(validator.stake.to_be_bytes());
    }
    
    let result = hasher.finalize();
    let mut root = [0u8; 32];
    root.copy_from_slice(&result);
    root
}

/// Compute genesis block hash.
fn compute_genesis_hash(header: &GenesisHeader) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    
    // Hash all header fields deterministically
    hasher.update(header.height.to_be_bytes());
    hasher.update(header.parent_hash);
    hasher.update(header.merkle_root);
    hasher.update(header.state_root);
    hasher.update(header.timestamp.to_be_bytes());
    hasher.update(header.chain_id.to_be_bytes());
    hasher.update(header.protocol_version.to_be_bytes());
    hasher.update(&header.extra_data);
    
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_builder_default() {
        let config = GenesisConfig::default();
        let builder = GenesisBuilder::new(config);
        let genesis = builder.build().unwrap();

        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.parent_hash, [0u8; 32]);
        assert_eq!(genesis.header.chain_id, 1);
        assert!(genesis.validators.is_empty());
    }

    #[test]
    fn test_genesis_builder_with_validators() {
        let validator_pubkey = [0x02u8; 33]; // Compressed pubkey prefix
        let config = GenesisConfig::devnet(validator_pubkey);
        let builder = GenesisBuilder::new(config);
        let genesis = builder.build().unwrap();

        assert_eq!(genesis.validators.len(), 1);
        assert_eq!(genesis.validators[0].pubkey, validator_pubkey);
        assert_ne!(genesis.header.state_root, EMPTY_STATE_ROOT);
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        let config = GenesisConfig {
            timestamp: Some(1700000000), // Fixed timestamp
            ..Default::default()
        };
        
        let genesis1 = GenesisBuilder::new(config.clone()).build().unwrap();
        let genesis2 = GenesisBuilder::new(config).build().unwrap();

        assert_eq!(genesis1.header.block_hash, genesis2.header.block_hash);
    }

    #[test]
    fn test_config_validation() {
        let mut config = GenesisConfig::default();
        config.initial_validators = vec![[0u8; 33]];
        // No matching stakes
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_extra_data_limit() {
        let mut config = GenesisConfig::default();
        config.extra_data = vec![0u8; 33]; // Too long
        
        let result = config.validate();
        assert!(result.is_err());
    }
}
