# DATA ARCHITECTURE VISUALIZATION
## Quantum-Chain System Entities Class Diagram
**Version:** 2.2 | **Generated:** 2025-12-01 | **Status:** Architecture v2.2 Compliant

---

## Overview

This document provides Mermaid.js Class Diagrams representing all system entities
as defined in the IPC-MATRIX.md, System.md, and Architecture.md specifications.

**Key Architectural Compliance (v2.2):**
- All messages wrapped in `AuthenticatedMessage<T>` envelope
- Envelope `sender_id` is the SOLE source of truth for identity (no payload identity fields)
- Block Storage uses Stateful Assembler pattern (choreography, not orchestration)
- Time-bounded nonce cache for replay prevention

> **Note:** Diagrams are split by domain cluster for clarity.

---

## Diagram 1: The Envelope (Universal Message Container)

The `AuthenticatedMessage<T>` wraps ALL inter-subsystem communication.

```mermaid
classDiagram
    direction LR

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
        +verify() Result
        +is_replay() bool
        +extract_sender() SubsystemId
    }

    class SubsystemId {
        <<value object>>
        +id: u8
        --
        PEER_DISCOVERY = 1
        BLOCK_STORAGE = 2
        TRANSACTION_INDEXING = 3
        STATE_MANAGEMENT = 4
        BLOCK_PROPAGATION = 5
        MEMPOOL = 6
        BLOOM_FILTERS = 7
        CONSENSUS = 8
        FINALITY = 9
        SIGNATURE_VERIFICATION = 10
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
        +verify(pubkey, message) bool
    }

    class Priority {
        <<enumeration>>
        CRITICAL
        HIGH
        NORMAL
        LOW
    }

    AuthenticatedMessage~T~ --> SubsystemId : sender_id
    AuthenticatedMessage~T~ --> Topic : reply_to
    AuthenticatedMessage~T~ --> Signature : signature
    Topic --> SubsystemId : subsystem_id
    Topic --> Priority : priority
```

---

## Diagram 2: Cluster A - The Chain (Block & Transaction Structures)

Core blockchain data structures for persistence.

```mermaid
classDiagram
    direction TB

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

    Block "1" *-- "1" BlockHeader : contains
    Block "1" *-- "*" Transaction : contains
    ValidatedBlock "1" *-- "1" Block : wraps
    StoredBlock "1" *-- "1" ValidatedBlock : wraps
```

---

## Diagram 3: Cluster A (cont.) - Transactions

Transaction lifecycle from receipt to inclusion.

```mermaid
classDiagram
    direction TB

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

    ValidatedTransaction "1" *-- "1" Transaction : wraps
    PendingTransaction "1" *-- "1" ValidatedTransaction : wraps
    PendingTransaction --> InclusionState : state
```

---

## Diagram 4: Cluster B - Consensus

Validator and consensus proof structures.

```mermaid
classDiagram
    direction TB

    class Validator {
        <<entity>>
        +validator_id: [u8; 32]
        +pubkey: [u8; 33]
        +stake: u128
        +activation_epoch: u64
        +exit_epoch: Option~u64~
        +slashed: bool
        --
        +is_active(epoch) bool
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

    SlashingEvidence --> SlashingType : type
    Validator ..> ConsensusProof : produces
```

---

## Diagram 5: Cluster B (cont.) - Finality (Casper FFG)

Attestation and finality proof structures.

```mermaid
classDiagram
    direction TB

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

    FinalityProof "1" *-- "*" Attestation : aggregates
    Attestation ..> Checkpoint : source
    Attestation ..> Checkpoint : target
```

---

## Diagram 6: Cluster C - State Management

Account state and Merkle tree structures.

```mermaid
classDiagram
    direction LR

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

    AccountState "1" o-- "*" StorageSlot : storage
    MerkleProof ..> MerkleRoot : validates against
```

---

## Diagram 7: Cluster D - Networking (Peer Discovery)

Kademlia DHT peer management structures.

```mermaid
classDiagram
    direction TB

    class NodeId {
        <<value object>>
        +id: [u8; 32]
        +pubkey: [u8; 33]
        --
        +distance(other) [u8; 32]
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
        +is_stale(now) bool
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

    PeerInfo "1" *-- "1" NodeId : identity
    PeerInfo --> Capability : capabilities
    PeerList "1" o-- "*" PeerInfo : contains
    KBucket "1" o-- "*" PeerInfo : contains
```

---

## Diagram 8: IPC Payloads - Choreography Events (v2.2)

Event payloads for the decentralized block assembly flow.

```mermaid
classDiagram
    direction TB

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

    note for BlockValidatedPayload "Emitted by: Consensus (8)"
    note for MerkleRootComputedPayload "Emitted by: Transaction Indexing (3)"
    note for StateRootComputedPayload "Emitted by: State Management (4)"
    note for BlockStoredPayload "Emitted by: Block Storage (2)"
```

---

## Diagram 9: IPC Payloads - Request/Response

Request and response payloads using correlation_id pattern.

```mermaid
classDiagram
    direction LR

    class ReadBlockRequest {
        <<request payload>>
        +version: u16
        +block_hash: Option~[u8; 32]~
        +block_height: Option~u64~
    }

    class ReadBlockResponse {
        <<response payload>>
        +version: u16
        +block: Option~StoredBlock~
        +error: Option~StorageError~
    }

    class ReadBlockRangeRequest {
        <<request payload>>
        +version: u16
        +start_height: u64
        +limit: u64
    }

    class ProposeTransactionBatch {
        <<request payload>>
        +version: u16
        +transactions: Vec~ValidatedTransaction~
        +priority: Priority
        +deadline_slot: u64
    }

    class BlockStorageConfirmation {
        <<response payload>>
        +version: u16
        +block_hash: [u8; 32]
        +included_transactions: Vec~[u8; 32]~
        +confirmed_at: u64
    }

    class StorageError {
        <<enumeration>>
        NOT_FOUND
        DATA_CORRUPTION
        DISK_FULL
        TIMEOUT
    }

    ReadBlockRequest ..> ReadBlockResponse : correlation_id
    ReadBlockResponse --> StorageError : error
```

---

## Diagram 10: Stateful Assembler (Block Storage Internal - v2.2)

Internal state for choreographed block assembly.

```mermaid
classDiagram
    direction TB

    class AssemblyBuffer {
        <<internal aggregate>>
        +pending: HashMap~Hash, PendingBlockAssembly~
        +max_pending: usize
        +timeout_seconds: u64
        --
        +on_block_validated(payload)
        +on_merkle_root(payload)
        +on_state_root(payload)
        +garbage_collect(now)
    }

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
        +is_expired(now) bool
        +assemble() Option~StoredBlock~
    }

    AssemblyBuffer "1" o-- "*" PendingBlockAssembly : pending

    note for AssemblyBuffer "Waits for 3 events:\n1. BlockValidated\n2. MerkleRootComputed\n3. StateRootComputed"
    note for PendingBlockAssembly "Atomic write occurs\nONLY when is_complete() = true"
```

---

## Diagram 11: Security - Time-Bounded Nonce Cache (v2.1)

Replay prevention with garbage collection.

```mermaid
classDiagram
    direction LR

    class NonceCache {
        <<internal aggregate>>
        +entries: HashMap~Nonce, NonceEntry~
        +validity_window: u64
        +max_entries: usize
        --
        +check_and_add(nonce, timestamp) bool
        +garbage_collect(now)
        +is_replay(nonce) bool
    }

    class NonceEntry {
        <<internal state>>
        +nonce: [u8; 16]
        +timestamp: u64
        +sender_id: SubsystemId
    }

    NonceCache "1" o-- "*" NonceEntry : entries

    note for NonceCache "Prevents memory exhaustion:\n- Entries expire after validity_window\n- garbage_collect() runs periodically"
```

---

## Diagram 12: Circuit Breaker (Finality - v2.2 Deterministic)

Deterministic failure handling with testable thresholds.

```mermaid
classDiagram
    direction TB

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

    CircuitBreakerState --> NodeState : current_state
    CircuitBreakerState --> SyncReason : sync_reason
    SyncResult --> SyncFailureReason : failure_reason

    note for CircuitBreakerConfig "All thresholds are\nexplicit and testable"
    note for NodeState "HALTED state prevents\nCPU-draining livelock"
```

---

## Diagram 13: Data Flow Overview (Choreography Pattern)

High-level view of the v2.2 block assembly flow.

```mermaid
flowchart LR
    subgraph Consensus["Subsystem 8: Consensus"]
        CV[Block Validated]
    end

    subgraph TxIndex["Subsystem 3: Tx Indexing"]
        MR[Compute Merkle Root]
    end

    subgraph State["Subsystem 4: State Mgmt"]
        SR[Compute State Root]
    end

    subgraph Storage["Subsystem 2: Block Storage"]
        AB[Assembly Buffer]
        AW[Atomic Write]
    end

    CV -->|BlockValidatedPayload| AB
    CV -->|BlockValidatedPayload| MR
    CV -->|BlockValidatedPayload| SR

    MR -->|MerkleRootComputedPayload| AB
    SR -->|StateRootComputedPayload| AB

    AB -->|"is_complete() = true"| AW
    AW -->|BlockStoredPayload| OUT[Event Bus]
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

| Diagram | Subsystem ID | Owner | Responsibility |
|---------|--------------|-------|----------------|
| 1 | All | Architecture | Message authentication |
| 2-3 | 2, 6 | Persistence/Mempool | Block & transaction storage |
| 4-5 | 8, 9 | Consensus/Finality | Block validation & finalization |
| 6 | 3, 4 | Indexing/State | Merkle & state management |
| 7 | 1 | Networking | Peer discovery |
| 8-9 | Various | IPC | Message definitions |
| 10 | 2 | Persistence | Internal assembly state |
| 11 | All | Security | Replay prevention |
| 12 | 9 | Finality | Failure handling |
| 13 | 2, 3, 4, 8 | Multiple | Choreography flow |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-11-30 | Initial single diagram |
| 2.0 | 2025-12-01 | Added v2.0 amendments |
| 2.1 | 2025-12-01 | Added time-bounded nonce cache |
| 2.2 | 2025-12-01 | Split into 13 focused diagrams for readability |
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
