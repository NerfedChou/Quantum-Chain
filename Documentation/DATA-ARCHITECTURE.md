# DATA ARCHITECTURE VISUALIZATION
## Quantum-Chain System Entities Class Diagram
**Version:** 2.2 | **Generated:** 2025-12-01 | **Status:** Architecture v2.2 Compliant

---

## Overview

This document provides a comprehensive Mermaid.js Class Diagram representing all system entities
as defined in the IPC-MATRIX.md, System.md, and Architecture.md specifications.

**Key Architectural Compliance (v2.2):**
- All messages wrapped in `AuthenticatedMessage<T>` envelope
- Envelope `sender_id` is the SOLE source of truth for identity (no payload identity fields)
- Block Storage uses Stateful Assembler pattern (choreography, not orchestration)
- Time-bounded nonce cache for replay prevention

---

## Complete System Entity Diagram

```mermaid
classDiagram
    direction TB

    %% ═══════════════════════════════════════════════════════════════════════════
    %% THE ENVELOPE - Universal Message Container (Architecture.md Section 3.2)
    %% ═══════════════════════════════════════════════════════════════════════════
    
    namespace Core_Envelope {
        class AuthenticatedMessage~T~ {
            <<generic>>
            +version: u16
            +sender_id: SubsystemId
            +recipient_id: SubsystemId
            +correlation_id: [u8; 16]
            +reply_to: Option~Topic~
            +payload: T
            +timestamp: u64
            +nonce: [u8; 16]
            +signature: Signature
            --
            +verify() Result~(), SecurityError~
            +is_replay() bool
            +extract_sender() SubsystemId
        }

        class SubsystemId {
            <<value object>>
            +id: u8
            --
            +PEER_DISCOVERY: 1
            +BLOCK_STORAGE: 2
            +TRANSACTION_INDEXING: 3
            +STATE_MANAGEMENT: 4
            +BLOCK_PROPAGATION: 5
            +MEMPOOL: 6
            +BLOOM_FILTERS: 7
            +CONSENSUS: 8
            +FINALITY: 9
            +SIGNATURE_VERIFICATION: 10
        }

        class Topic {
            <<value object>>
            +subsystem_id: SubsystemId
            +channel: String
            +priority: Priority
        }

        class Signature {
            <<value object>>
            +r: [u8; 32]
            +s: [u8; 32]
            +v: u8
            --
            +verify(pubkey: PublicKey, message: &[u8]) bool
        }

        class Priority {
            <<enumeration>>
            CRITICAL
            HIGH
            NORMAL
            LOW
        }
    }

    AuthenticatedMessage~T~ --> SubsystemId : sender_id
    AuthenticatedMessage~T~ --> SubsystemId : recipient_id
    AuthenticatedMessage~T~ --> Topic : reply_to
    AuthenticatedMessage~T~ --> Signature : signature

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CLUSTER A: THE CHAIN - Core Blockchain Data Structures
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_2_BlockStorage {
        class Block {
            <<aggregate root>>
            +header: BlockHeader
            +transactions: Vec~Transaction~
            +uncle_headers: Vec~BlockHeader~
            --
            +hash() [u8; 32]
            +verify_integrity() bool
        }

        class BlockHeader {
            <<entity>>
            +version: u16
            +parent_hash: [u8; 32]
            +merkle_root: [u8; 32]
            +state_root: [u8; 32]
            +receipts_root: [u8; 32]
            +timestamp: u64
            +height: u64
            +difficulty: u64
            +nonce: u64
            +extra_data: Vec~u8~
            +proposer: [u8; 32]
            +signature: Signature
            --
            +hash() [u8; 32]
        }

        class ValidatedBlock {
            <<refined type>>
            +block: Block
            +consensus_proof: ConsensusProof
            +validation_timestamp: u64
            --
            +is_valid() bool
        }

        class StoredBlock {
            <<entity>>
            +block: ValidatedBlock
            +merkle_root: [u8; 32]
            +state_root: [u8; 32]
            +checksum: u32
            +stored_at: u64
            --
            +verify_checksum() bool
        }

        class BlockIndex {
            <<value object>>
            +height: u64
            +block_hash: [u8; 32]
        }

        class StorageMetadata {
            <<entity>>
            +genesis_hash: [u8; 32]
            +finalized_height: u64
            +chain_id: u64
            +last_updated: u64
        }
    }

    Block "1" *-- "1" BlockHeader : header
    Block "1" *-- "*" Transaction : transactions
    ValidatedBlock "1" *-- "1" Block : block
    ValidatedBlock "1" --> "1" ConsensusProof : consensus_proof
    StoredBlock "1" *-- "1" ValidatedBlock : block
    BlockHeader ..> Block : parent_hash refers to

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CLUSTER A (continued): TRANSACTIONS
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_6_Mempool {
        class Transaction {
            <<entity>>
            +version: u16
            +nonce: u64
            +from: [u8; 32]
            +to: Option~[u8; 32]~
            +value: u128
            +gas_limit: u64
            +gas_price: u64
            +data: Vec~u8~
            +signature: Signature
            --
            +hash() [u8; 32]
            +sender() [u8; 32]
        }

        class ValidatedTransaction {
            <<refined type>>
            +transaction: Transaction
            +signature_valid: bool
            +nonce_valid: bool
            +balance_sufficient: bool
            +validated_at: u64
        }

        class PendingTransaction {
            <<entity>>
            +transaction: ValidatedTransaction
            +received_at: u64
            +priority_score: u64
            +inclusion_state: InclusionState
        }

        class InclusionState {
            <<enumeration>>
            PENDING
            PROPOSED
            CONFIRMED
            REJECTED
        }
    }

    Transaction "1" --> "1" Signature : signature
    ValidatedTransaction "1" *-- "1" Transaction : transaction
    PendingTransaction "1" *-- "1" ValidatedTransaction : transaction
    PendingTransaction --> InclusionState : inclusion_state

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CLUSTER B: CONSENSUS & FINALITY
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_8_Consensus {
        class Validator {
            <<entity>>
            +validator_id: [u8; 32]
            +pubkey: [u8; 33]
            +stake: u128
            +activation_epoch: u64
            +exit_epoch: Option~u64~
            +slashed: bool
            --
            +is_active(epoch: u64) bool
            +voting_power() u128
        }

        class ConsensusProof {
            <<value object>>
            +block_hash: [u8; 32]
            +slot: u64
            +epoch: u64
            +proposer_index: u64
            +proposer_signature: Signature
            +attestation_count: u32
        }

        class SlashingEvidence {
            <<entity>>
            +validator_id: [u8; 32]
            +evidence_type: SlashingType
            +block_1: [u8; 32]
            +block_2: [u8; 32]
            +signature_1: Signature
            +signature_2: Signature
        }

        class SlashingType {
            <<enumeration>>
            DOUBLE_VOTE
            SURROUND_VOTE
        }
    }

    ConsensusProof --> Signature : proposer_signature
    SlashingEvidence --> SlashingType : evidence_type

    namespace Subsystem_9_Finality {
        class Attestation {
            <<entity>>
            +validator_id: [u8; 32]
            +source_epoch: u64
            +source_root: [u8; 32]
            +target_epoch: u64
            +target_root: [u8; 32]
            +signature: Signature
            --
            +is_valid() bool
        }

        class FinalityProof {
            <<aggregate root>>
            +checkpoint_epoch: u64
            +checkpoint_root: [u8; 32]
            +attestations: Vec~Attestation~
            +supermajority_stake: u128
            +total_stake: u128
            +finalized_at: u64
            --
            +is_supermajority() bool
            +stake_ratio() f64
        }

        class Checkpoint {
            <<value object>>
            +epoch: u64
            +root: [u8; 32]
        }

        class FinalityState {
            <<enumeration>>
            PENDING
            JUSTIFIED
            FINALIZED
        }
    }

    FinalityProof "1" *-- "*" Attestation : attestations
    Attestation --> Signature : signature
    Attestation --> Checkpoint : source (via epoch+root)
    Attestation --> Checkpoint : target (via epoch+root)

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CLUSTER C: STATE & STORAGE
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_4_StateManagement {
        class AccountState {
            <<entity>>
            +address: [u8; 32]
            +balance: u128
            +nonce: u64
            +code_hash: [u8; 32]
            +storage_root: [u8; 32]
            --
            +is_contract() bool
        }

        class StateRoot {
            <<value object>>
            +root: [u8; 32]
            +block_height: u64
            +account_count: u64
        }

        class StorageSlot {
            <<value object>>
            +key: [u8; 32]
            +value: [u8; 32]
        }

        class StateTransition {
            <<event>>
            +block_hash: [u8; 32]
            +pre_state_root: [u8; 32]
            +post_state_root: [u8; 32]
            +modified_accounts: Vec~[u8; 32]~
        }
    }

    AccountState o-- "*" StorageSlot : storage

    namespace Subsystem_3_TransactionIndexing {
        class MerkleRoot {
            <<value object>>
            +root: [u8; 32]
            +transaction_count: u32
            +block_hash: [u8; 32]
        }

        class MerkleProof {
            <<value object>>
            +leaf_hash: [u8; 32]
            +leaf_index: u32
            +siblings: Vec~[u8; 32]~
            +root: [u8; 32]
            --
            +verify() bool
        }

        class MerkleNode {
            <<value object>>
            +hash: [u8; 32]
            +left: Option~Box~MerkleNode~~
            +right: Option~Box~MerkleNode~~
        }
    }

    MerkleProof --> MerkleRoot : root
    MerkleNode o-- MerkleNode : left
    MerkleNode o-- MerkleNode : right

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CLUSTER D: NETWORKING
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_1_PeerDiscovery {
        class NodeId {
            <<value object>>
            +id: [u8; 32]
            +pubkey: [u8; 33]
            --
            +distance(other: NodeId) [u8; 32]
        }

        class PeerInfo {
            <<entity>>
            +node_id: NodeId
            +ip_address: IpAddr
            +port: u16
            +reputation_score: u8
            +last_seen: u64
            +capabilities: Vec~Capability~
            --
            +is_stale(now: u64) bool
        }

        class PeerList {
            <<aggregate>>
            +version: u16
            +peers: Vec~PeerInfo~
            +timestamp: u64
            +signature: Signature
        }

        class Capability {
            <<enumeration>>
            FULL_NODE
            LIGHT_CLIENT
            ARCHIVE_NODE
            VALIDATOR
        }

        class KBucket {
            <<entity>>
            +distance_prefix: u8
            +peers: Vec~PeerInfo~
            +max_size: usize
        }
    }

    PeerInfo "1" *-- "1" NodeId : node_id
    PeerInfo --> Capability : capabilities
    PeerList "1" o-- "*" PeerInfo : peers
    PeerList --> Signature : signature
    KBucket o-- "*" PeerInfo : peers

    %% ═══════════════════════════════════════════════════════════════════════════
    %% IPC MESSAGE PAYLOADS - Data in Motion (v2.2 Choreography Pattern)
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace IPC_Payloads_Choreography {
        class BlockValidatedPayload {
            <<event payload>>
            +version: u16
            +block: ValidatedBlock
            +consensus_proof: ConsensusProof
            +validation_timestamp: u64
        }

        class MerkleRootComputedPayload {
            <<event payload>>
            +version: u16
            +block_hash: [u8; 32]
            +merkle_root: [u8; 32]
            +transaction_count: u32
            +computed_at: u64
        }

        class StateRootComputedPayload {
            <<event payload>>
            +version: u16
            +block_hash: [u8; 32]
            +state_root: [u8; 32]
            +modified_accounts: u32
            +computed_at: u64
        }

        class BlockStoredPayload {
            <<event payload>>
            +version: u16
            +block_hash: [u8; 32]
            +block_height: u64
            +stored_at: u64
        }
    }

    BlockValidatedPayload --> ValidatedBlock : block
    BlockValidatedPayload --> ConsensusProof : consensus_proof

    namespace IPC_Payloads_Requests {
        class VerifyNodeIdentityRequest {
            <<request payload>>
            +version: u16
            +node_id: [u8; 32]
            +claimed_pubkey: [u8; 33]
            +challenge_response: Signature
        }

        class ProposeTransactionBatch {
            <<request payload>>
            +version: u16
            +transactions: Vec~ValidatedTransaction~
            +priority: Priority
            +deadline_slot: u64
        }

        class ReadBlockRequest {
            <<request payload>>
            +version: u16
            +block_hash: Option~[u8; 32]~
            +block_height: Option~u64~
        }

        class ReadBlockRangeRequest {
            <<request payload>>
            +version: u16
            +start_height: u64
            +limit: u64
        }
    }

    ProposeTransactionBatch o-- "*" ValidatedTransaction : transactions

    namespace IPC_Payloads_Responses {
        class NodeIdentityVerificationResult {
            <<response payload>>
            +version: u16
            +node_id: [u8; 32]
            +identity_valid: bool
            +verification_timestamp: u64
        }

        class BlockStorageConfirmation {
            <<response payload>>
            +version: u16
            +block_hash: [u8; 32]
            +included_transactions: Vec~[u8; 32]~
            +confirmed_at: u64
        }

        class ReadBlockResponse {
            <<response payload>>
            +version: u16
            +block: Option~StoredBlock~
            +error: Option~StorageError~
        }

        class StorageError {
            <<enumeration>>
            NOT_FOUND
            DATA_CORRUPTION
            DISK_FULL
            TIMEOUT
        }
    }

    ReadBlockResponse --> StoredBlock : block
    ReadBlockResponse --> StorageError : error

    %% ═══════════════════════════════════════════════════════════════════════════
    %% STATEFUL ASSEMBLER - Block Storage Internal State (v2.2)
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_2_Internal {
        class PendingBlockAssembly {
            <<internal state>>
            +block_hash: [u8; 32]
            +validated_block: Option~ValidatedBlock~
            +merkle_root: Option~[u8; 32]~
            +state_root: Option~[u8; 32]~
            +created_at: u64
            +timeout_at: u64
            --
            +is_complete() bool
            +is_expired(now: u64) bool
            +assemble() Option~StoredBlock~
        }

        class AssemblyBuffer {
            <<internal aggregate>>
            +pending: HashMap~[u8; 32], PendingBlockAssembly~
            +max_pending: usize
            +timeout_seconds: u64
            --
            +on_block_validated(payload: BlockValidatedPayload)
            +on_merkle_root(payload: MerkleRootComputedPayload)
            +on_state_root(payload: StateRootComputedPayload)
            +garbage_collect(now: u64)
        }
    }

    AssemblyBuffer "1" o-- "*" PendingBlockAssembly : pending
    PendingBlockAssembly --> ValidatedBlock : validated_block

    %% ═══════════════════════════════════════════════════════════════════════════
    %% SECURITY ENTITIES - Replay Prevention (v2.1)
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Security_TimeBoundedNonce {
        class NonceEntry {
            <<internal state>>
            +nonce: [u8; 16]
            +timestamp: u64
            +sender_id: SubsystemId
        }

        class NonceCache {
            <<internal aggregate>>
            +entries: HashMap~[u8; 16], NonceEntry~
            +validity_window: u64
            +max_entries: usize
            --
            +check_and_add(nonce: [u8; 16], timestamp: u64) bool
            +garbage_collect(now: u64)
            +is_replay(nonce: [u8; 16]) bool
        }
    }

    NonceCache "1" o-- "*" NonceEntry : entries
    NonceEntry --> SubsystemId : sender_id

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CIRCUIT BREAKER STATE (v2.2 Deterministic)
    %% ═══════════════════════════════════════════════════════════════════════════

    namespace Subsystem_9_CircuitBreaker {
        class CircuitBreakerConfig {
            <<configuration>>
            +sync_timeout: Duration
            +max_sync_attempts: u8
            +min_peer_quorum: usize
            +peer_response_threshold: f64
            +min_checkpoint_advantage: u64
        }

        class CircuitBreakerState {
            <<internal state>>
            +current_state: NodeState
            +sync_failure_counter: u8
            +last_sync_attempt: u64
            +sync_reason: Option~SyncReason~
        }

        class NodeState {
            <<enumeration>>
            SYNCING
            FINALIZING
            PRODUCING
            HALTED_AWAITING_INTERVENTION
        }

        class SyncReason {
            <<enumeration>>
            MINORITY_FORK
            NETWORK_PARTITION
            NODE_BEHIND
            BYZANTINE_BEHAVIOR
        }

        class SyncResult {
            <<result type>>
            +success: bool
            +new_checkpoint: Option~u64~
            +peer_id: Option~NodeId~
            +failure_reason: Option~SyncFailureReason~
        }

        class SyncFailureReason {
            <<enumeration>>
            INSUFFICIENT_PEERS
            NO_PEER_DATA
            NO_SUPERIOR_CHAIN
            TIMEOUT
        }
    }

    CircuitBreakerState --> NodeState : current_state
    CircuitBreakerState --> SyncReason : sync_reason
    SyncResult --> SyncFailureReason : failure_reason

    %% ═══════════════════════════════════════════════════════════════════════════
    %% CROSS-CLUSTER RELATIONSHIPS
    %% ═══════════════════════════════════════════════════════════════════════════

    %% Envelope wraps all payloads
    AuthenticatedMessage~T~ ..> BlockValidatedPayload : wraps
    AuthenticatedMessage~T~ ..> MerkleRootComputedPayload : wraps
    AuthenticatedMessage~T~ ..> StateRootComputedPayload : wraps
    AuthenticatedMessage~T~ ..> ProposeTransactionBatch : wraps
    AuthenticatedMessage~T~ ..> ReadBlockRequest : wraps
    AuthenticatedMessage~T~ ..> ReadBlockResponse : wraps

    %% Hash References (The Blockchain Links)
    BlockHeader ..> Block : parent_hash references
    StateRoot ..> Block : block_height references
    MerkleRoot ..> Block : block_hash references
    Attestation ..> Block : target_root references
    FinalityProof ..> Block : checkpoint_root references
```

---

## Relationship Legend

| Symbol | Meaning | Example |
|--------|---------|---------|
| `*--` | **Composition** (strong has-a, lifecycle bound) | Block *-- BlockHeader |
| `o--` | **Aggregation** (weak has-a, independent lifecycle) | PeerList o-- PeerInfo |
| `-->` | **Association** (refers to, uses) | ValidatedBlock --> ConsensusProof |
| `..>` | **Dependency** (depends on, hash reference) | BlockHeader ..> Block (parent_hash) |

---

## Architecture v2.2 Compliance Notes

### 1. Envelope-Only Identity (Amendment 4.2)
All IPC payloads in this diagram have **NO** `requester_id` or `sender_id` fields.
Identity is derived ONLY from the `AuthenticatedMessage<T>` envelope's `sender_id` field.

### 2. Choreography Pattern (Amendment 4.1)
The `Subsystem_2_Internal` namespace shows the **Stateful Assembler** pattern:
- `PendingBlockAssembly` buffers incoming components
- `AssemblyBuffer` coordinates the three events: `BlockValidated`, `MerkleRootComputed`, `StateRootComputed`
- Atomic write occurs ONLY when all three components are present

### 3. Deterministic Circuit Breaker (Amendment 4.3)
The `Subsystem_9_CircuitBreaker` namespace shows:
- `CircuitBreakerConfig` with explicit, testable thresholds
- `SyncResult` and `SyncFailureReason` for deterministic outcomes
- `NodeState::HALTED_AWAITING_INTERVENTION` for livelock prevention

### 4. Time-Bounded Nonce Cache (Amendment 2.3 v2.1)
The `Security_TimeBoundedNonce` namespace shows:
- `NonceCache` with garbage collection
- `NonceEntry` with timestamp for expiration
- Prevents memory exhaustion attacks

---

## Subsystem Ownership Matrix

| Namespace | Subsystem ID | Owner | Responsibility |
|-----------|--------------|-------|----------------|
| `Core_Envelope` | All | Architecture | Message authentication |
| `Subsystem_1_PeerDiscovery` | 1 | Networking | Peer management |
| `Subsystem_2_BlockStorage` | 2 | Persistence | Block persistence |
| `Subsystem_2_Internal` | 2 | Persistence | Assembly coordination |
| `Subsystem_3_TransactionIndexing` | 3 | Indexing | Merkle proofs |
| `Subsystem_4_StateManagement` | 4 | State | Account state |
| `Subsystem_6_Mempool` | 6 | Transactions | Transaction pool |
| `Subsystem_8_Consensus` | 8 | Consensus | Block validation |
| `Subsystem_9_Finality` | 9 | Finality | Casper FFG |
| `Subsystem_9_CircuitBreaker` | 9 | Finality | Failure handling |
| `IPC_Payloads_*` | Various | IPC | Message definitions |
| `Security_*` | All | Security | Attack prevention |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-11-30 | Initial diagram |
| 2.0 | 2025-12-01 | Added v2.0 amendments |
| 2.1 | 2025-12-01 | Added time-bounded nonce cache |
| 2.2 | 2025-12-01 | Added choreography pattern, deterministic triggers, envelope-only identity |
