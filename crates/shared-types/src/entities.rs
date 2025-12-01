//! # Core Domain Entities
//!
//! Defines the core blockchain entities as specified in System.md and
//! the Data Architecture diagram.
//!
//! ## Clusters
//!
//! - **Chain**: Block, `BlockHeader`, Transaction, `ValidatedTransaction`
//! - **Consensus & Finality**: Validator, Attestation, `FinalityProof`
//! - **State & Storage**: `AccountState`, `StateRoot`, `MerkleRoot`
//! - **Networking**: `PeerInfo`, `NodeId`

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

// =============================================================================
// CLUSTER A: THE CHAIN
// =============================================================================

/// A 32-byte hash (e.g., SHA-256 or Blake3).
pub type Hash = [u8; 32];

/// A 64-byte Ed25519 signature.
pub type Signature = [u8; 64];

/// A 32-byte Ed25519 public key.
pub type PublicKey = [u8; 32];

/// Unique identifier for a node in the network.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

/// The header of a block containing metadata and root hashes.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Protocol version for this block.
    pub version: u16,
    /// Block height in the chain.
    pub height: u64,
    /// Hash of the parent block (creates the chain linkage).
    pub parent_hash: Hash,
    /// Merkle root of all transactions in the block.
    pub merkle_root: Hash,
    /// Root hash of the state trie after applying this block.
    pub state_root: Hash,
    /// Unix timestamp when the block was proposed.
    pub timestamp: u64,
    /// The validator who proposed this block.
    pub proposer: PublicKey,
}

/// A validated block ready for storage.
///
/// This is the output of the Consensus subsystem and the input to
/// the Block Storage subsystem via the choreographed assembly pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedBlock {
    /// The block header.
    pub header: BlockHeader,
    /// All validated transactions in this block.
    pub transactions: Vec<ValidatedTransaction>,
    /// Consensus proof (signatures from validators).
    pub consensus_proof: ConsensusProof,
}

/// A raw transaction as received from the network.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Sender's public key.
    pub from: PublicKey,
    /// Recipient's public key (optional for contract creation).
    pub to: Option<PublicKey>,
    /// Transaction amount in base units.
    pub value: u64,
    /// Sender's nonce to prevent replay attacks.
    pub nonce: u64,
    /// Transaction payload (contract call data, etc.).
    pub data: Vec<u8>,
    /// Sender's signature over the transaction.
    #[serde_as(as = "Bytes")]
    pub signature: Signature,
}

/// A transaction that has passed signature and format validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedTransaction {
    /// The underlying transaction.
    pub inner: Transaction,
    /// Hash of the transaction for indexing.
    pub tx_hash: Hash,
}

// =============================================================================
// CLUSTER B: CONSENSUS & FINALITY
// =============================================================================

/// A validator in the consensus protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    /// The validator's public key (identity).
    pub public_key: PublicKey,
    /// Stake weight for voting power.
    pub stake: u64,
    /// Whether this validator is currently active.
    pub active: bool,
}

/// An attestation (vote) from a validator for a block.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// The block hash being attested to.
    pub block_hash: Hash,
    /// The epoch number for this attestation.
    pub epoch: u64,
    /// The validator's public key.
    pub validator: PublicKey,
    /// Signature over (`block_hash`, `epoch`).
    #[serde_as(as = "Bytes")]
    pub signature: Signature,
}

/// Proof that a block has reached consensus (2/3+ validators).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProof {
    /// The block hash this proof applies to.
    pub block_hash: Hash,
    /// Aggregated or individual attestations.
    pub attestations: Vec<Attestation>,
    /// Total stake weight of the attestations.
    pub total_stake: u64,
}

/// Proof of finality for a checkpoint (Casper FFG style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityProof {
    /// The finalized checkpoint block hash.
    pub checkpoint_hash: Hash,
    /// The epoch that was finalized.
    pub epoch: u64,
    /// Attestations forming the supermajority.
    pub attestations: Vec<Attestation>,
    /// Total stake that voted for finalization.
    pub total_stake: u64,
    /// Required stake threshold (2/3 of total).
    pub required_stake: u64,
}

// =============================================================================
// CLUSTER C: STATE & STORAGE
// =============================================================================

/// The state of an account in the state trie.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountState {
    /// Account balance in base units.
    pub balance: u64,
    /// Account nonce (number of transactions sent).
    pub nonce: u64,
    /// Optional code hash for contract accounts.
    pub code_hash: Option<Hash>,
    /// Optional storage root for contract accounts.
    pub storage_root: Option<Hash>,
}

/// A stored block with integrity checksum.
///
/// This is the format used by Block Storage (Subsystem 2) for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBlock {
    /// The validated block data.
    pub block: ValidatedBlock,
    /// CRC32C checksum computed at write time for integrity verification.
    pub checksum: u32,
}

/// Metadata about the storage state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    /// Hash of the genesis block.
    pub genesis_hash: Hash,
    /// Height of the last finalized block.
    pub finalized_height: u64,
    /// Height of the chain tip.
    pub chain_tip_height: u64,
}

// =============================================================================
// CLUSTER D: NETWORKING
// =============================================================================

/// Information about a peer in the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// The peer's node ID.
    pub node_id: NodeId,
    /// Network address (IP:Port).
    pub address: String,
    /// Reputation score (0-100).
    pub reputation: u8,
    /// Last seen timestamp.
    pub last_seen: u64,
    /// Protocol version supported.
    pub protocol_version: u16,
}

/// A list of peers for exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerList {
    /// The peers in this list.
    pub peers: Vec<PeerInfo>,
}
