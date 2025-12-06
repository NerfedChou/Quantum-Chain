//! Domain entities for block production

use primitive_types::{H256, U256};
use serde::{Deserialize, Serialize};

/// Block template created by this subsystem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockTemplate {
    /// Block header (to be filled/signed)
    pub header: BlockHeader,

    /// Selected transactions in optimal order
    pub transactions: Vec<Vec<u8>>, // TODO: Replace with SignedTransaction type

    /// Total gas used by all transactions
    pub total_gas_used: u64,

    /// Total fee revenue
    pub total_fees: U256,

    /// Consensus mode this block is for
    pub consensus_mode: ConsensusMode,

    /// Creation timestamp
    pub created_at: u64,
}

/// Block header (partially filled by this subsystem)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Parent block hash
    pub parent_hash: H256,

    /// Block number (height)
    pub block_number: u64,

    /// Unix timestamp
    pub timestamp: u64,

    /// Beneficiary address (coinbase/validator)
    pub beneficiary: [u8; 20],

    /// Gas used in this block
    pub gas_used: u64,

    /// Gas limit for this block
    pub gas_limit: u64,

    /// Difficulty target (PoW only)
    pub difficulty: U256,

    /// Extra data (client ID, version)
    pub extra_data: Vec<u8>,

    /// Merkle root (filled by Transaction Indexing - Subsystem 3)
    pub merkle_root: Option<H256>,

    /// State root (filled by State Management - Subsystem 4)
    pub state_root: Option<H256>,

    /// Nonce (filled by PoW miner, omitted in PoS/PBFT)
    pub nonce: Option<u64>,
}

/// Consensus mode enumeration
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMode {
    /// Proof of Work - parallel nonce search
    ProofOfWork,

    /// Proof of Stake - VRF-based proposer selection
    ProofOfStake,

    /// PBFT - leader-based proposal
    PBFT,
}

/// Mining job for PoW mode
#[derive(Clone, Debug)]
pub struct MiningJob {
    /// Block template to mine
    pub template: BlockTemplate,

    /// Difficulty target
    pub difficulty_target: U256,

    /// Number of threads to use
    pub num_threads: u8,

    /// Nonce range per thread
    pub nonce_ranges: Vec<(u64, u64)>,
}

/// PoS proposer duty assignment
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposerDuty {
    /// Slot number
    pub slot: u64,

    /// Epoch number
    pub epoch: u64,

    /// Validator index in active set
    pub validator_index: u32,

    /// VRF proof of selection
    pub vrf_proof: VRFProof,
}

/// VRF proof for PoS proposer selection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VRFProof {
    /// VRF output (32 bytes)
    pub output: [u8; 32],

    /// VRF proof (80 bytes stored as Vec for serde compatibility)
    pub proof: Vec<u8>,
}

impl VRFProof {
    /// Create new VRF proof from fixed-size arrays
    pub fn new(output: [u8; 32], proof: [u8; 80]) -> Self {
        Self {
            output,
            proof: proof.to_vec(),
        }
    }

    /// Get proof as fixed-size array
    pub fn proof_array(&self) -> Option<[u8; 80]> {
        if self.proof.len() == 80 {
            let mut arr = [0u8; 80];
            arr.copy_from_slice(&self.proof);
            Some(arr)
        } else {
            None
        }
    }
}

/// Transaction with metadata for selection
#[derive(Clone, Debug)]
pub struct TransactionCandidate {
    /// The signed transaction (raw bytes for now)
    pub transaction: Vec<u8>, // TODO: Replace with SignedTransaction type

    /// Recovered sender address
    pub from: [u8; 20],

    /// Transaction nonce
    pub nonce: u64,

    /// Gas price (priority)
    pub gas_price: U256,

    /// Gas limit (maximum gas)
    pub gas_limit: u64,

    /// Pre-verified signature validity
    pub signature_valid: bool,
}

/// State simulation result
#[derive(Clone, Debug)]
pub struct SimulationResult {
    /// Transaction hash
    pub tx_hash: H256,

    /// Simulation succeeded
    pub success: bool,

    /// Actual gas used (if success)
    pub gas_used: u64,

    /// State changes (for cache)
    pub state_changes: Vec<StateChange>,

    /// Error message (if failed)
    pub error: Option<String>,
}

/// State change from simulation
#[derive(Clone, Debug)]
pub struct StateChange {
    /// Address affected
    pub address: [u8; 20],

    /// Storage key (None for balance/nonce)
    pub storage_key: Option<H256>,

    /// Old value
    pub old_value: Vec<u8>,

    /// New value
    pub new_value: Vec<u8>,
}

/// Transaction bundle for MEV
#[derive(Clone, Debug)]
pub struct TransactionBundle {
    /// Transactions in bundle (must be executed sequentially)
    pub transactions: Vec<Vec<u8>>, // TODO: Replace with SignedTransaction type

    /// Bundle profitability
    pub profit: U256,

    /// Bundle type
    pub bundle_type: BundleType,
}

/// MEV bundle types
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BundleType {
    /// Simple bundle (user intent)
    Simple,

    /// Front-running bundle (detected MEV)
    FrontRunning,

    /// Back-running bundle (detected MEV)
    BackRunning,

    /// Sandwich attack (detected MEV)
    Sandwich,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_mode() {
        assert_eq!(ConsensusMode::ProofOfWork, ConsensusMode::ProofOfWork);
        assert_ne!(ConsensusMode::ProofOfWork, ConsensusMode::ProofOfStake);
    }

    #[test]
    fn test_bundle_type() {
        assert_eq!(BundleType::Simple, BundleType::Simple);
        assert_ne!(BundleType::FrontRunning, BundleType::BackRunning);
    }
}
