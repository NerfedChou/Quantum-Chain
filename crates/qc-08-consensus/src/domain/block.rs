//! Block domain entities
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2.1

use super::{ValidationProof, ValidatorId};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use shared_types::Hash;

/// A validated block ready for the choreography
///
/// Reference: SPEC-08 Section 2.1
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatedBlock {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
    pub validation_proof: ValidationProof,
}

/// Block header containing all metadata
///
/// Reference: SPEC-08 Section 2.1
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub version: u32,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub proposer: ValidatorId,
    /// TBD - computed by Subsystem 3 in choreography
    pub transactions_root: Option<Hash>,
    /// TBD - computed by Subsystem 4 in choreography
    pub state_root: Option<Hash>,
    pub receipts_root: Hash,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub extra_data: Vec<u8>,
}

impl BlockHeader {
    /// Compute the hash of this block header
    pub fn hash(&self) -> Hash {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(self.version.to_le_bytes());
        hasher.update(self.block_height.to_le_bytes());
        hasher.update(self.parent_hash);
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(self.proposer);
        hasher.update(self.gas_limit.to_le_bytes());
        hasher.update(self.gas_used.to_le_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Check if this is a genesis block
    pub fn is_genesis(&self) -> bool {
        self.block_height == 0 && self.parent_hash == [0u8; 32]
    }
}

/// An unvalidated block received from the network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
    pub proof: ValidationProof,
}

impl Block {
    /// Get the hash of this block
    pub fn hash(&self) -> Hash {
        self.header.hash()
    }
}

/// A signed transaction
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedTransaction {
    pub hash: Hash,
    pub from: [u8; 20],
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub nonce: u64,
    pub data: Vec<u8>,
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 65],
}

impl SignedTransaction {
    /// Calculate the gas cost of this transaction
    pub fn gas_cost(&self) -> u64 {
        self.gas_limit
    }
}

/// Configuration for consensus
#[derive(Clone, Debug)]
pub struct ConsensusConfig {
    /// Consensus algorithm
    pub algorithm: ConsensusAlgorithm,
    /// Block time target (milliseconds)
    pub block_time_ms: u64,
    /// Maximum transactions per block
    pub max_txs_per_block: usize,
    /// Maximum block gas
    pub max_block_gas: u64,
    /// Minimum attestations for PoS (percentage)
    pub min_attestation_percent: u8,
    /// Byzantine fault tolerance (f in 3f+1)
    pub byzantine_threshold: usize,
    /// Maximum timestamp drift allowed (seconds)
    pub max_timestamp_drift_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusAlgorithm {
    ProofOfStake,
    PBFT,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            algorithm: ConsensusAlgorithm::ProofOfStake,
            block_time_ms: 12_000, // 12 seconds
            max_txs_per_block: 10_000,
            max_block_gas: 30_000_000,
            min_attestation_percent: 67, // 2/3
            byzantine_threshold: 1,
            max_timestamp_drift_secs: 15,
        }
    }
}
