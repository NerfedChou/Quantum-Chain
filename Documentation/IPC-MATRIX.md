# BLOCKCHAIN INTER-PROCESS COMMUNICATION (IPC) MATRIX
## Strict Message Types, Access Control, and Security Boundaries

**Security Principle:** Each subsystem is an isolated compartment. Even if one subsystem is compromised, attackers cannot access others without the correct message types and authentication.

---

## SUBSYSTEM 1: PEER DISCOVERY & ROUTING

### I Am Allowed To Talk To:
- **Subsystem 5 (Block Propagation)** - Provide peer list
- **Subsystem 7 (Bloom Filters)** - Provide full node connections
- **Subsystem 13 (Light Clients)** - Provide full node connections

### Who Is Allowed To Talk To Me:
- **External Bootstrap Nodes** (Initial network entry)
- **Subsystem 5 (Block Propagation)** - Request peer list
- **Subsystem 7 (Bloom Filters)** - Request full nodes
- **Subsystem 13 (Light Clients)** - Request full nodes

### Strict Message Types:

**OUTGOING:**
```rust
struct PeerList {
    peers: Vec<PeerInfo>,
    timestamp: u64,
    signature: Signature,  // Signed by this node
}

struct PeerInfo {
    node_id: [u8; 32],      // 256-bit Kademlia ID
    ip_address: IpAddr,
    port: u16,
    reputation_score: u8,   // 0-100
    last_seen: u64,
}
```

**INCOMING:**
```rust
struct PeerListRequest {
    requester_id: SubsystemId,  // Must be 5, 7, or 13
    request_id: u64,
    timestamp: u64,
    signature: Signature,       // Signed by requester
}

struct BootstrapRequest {
    node_id: [u8; 32],
    ip_address: IpAddr,
    port: u16,
    proof_of_work: [u8; 32],   // Anti-Sybil
}
```

### Security Boundaries:
- ✅ Accept: PeerListRequest from Subsystems 5, 7, 13 only
- ✅ Accept: BootstrapRequest from external nodes with valid PoW
- ❌ Reject: Any message from Subsystems 2, 3, 4, 6, 8, 9, 10, 11, 12, 14, 15
- ❌ Reject: Unsigned messages
- ❌ Reject: Messages older than 60 seconds

---

## SUBSYSTEM 2: BLOCK STORAGE ENGINE

### I Am Allowed To Talk To:
- **None** (Pure storage layer, only responds to requests)

### Who Is Allowed To Talk To Me:
- **Subsystem 3 (Transaction Indexing)** - Store Merkle roots
- **Subsystem 4 (State Management)** - Store state roots
- **Subsystem 8 (Consensus)** - Store validated blocks
- **Subsystem 9 (Finality)** - Mark blocks as finalized

### Strict Message Types:

**OUTGOING:**
```rust
struct StorageResponse {
    request_id: u64,
    success: bool,
    data: Option<Vec<u8>>,
    error: Option<StorageError>,
}

enum StorageError {
    NotFound,
    Corrupted,
    DiskFull,
    PermissionDenied,
}
```

**INCOMING:**
```rust
struct WriteBlockRequest {
    requester_id: SubsystemId,  // Must be 8
    block: ValidatedBlock,       // From Consensus only
    merkle_root: [u8; 32],      // From Subsystem 3
    state_root: [u8; 32],       // From Subsystem 4
    signature: Signature,
}

struct WriteMerkleRootRequest {
    requester_id: SubsystemId,  // Must be 3
    block_number: u64,
    merkle_root: [u8; 32],
    signature: Signature,
}

struct WriteStateRootRequest {
    requester_id: SubsystemId,  // Must be 4
    block_number: u64,
    state_root: [u8; 32],
    signature: Signature,
}

struct ReadBlockRequest {
    requester_id: SubsystemId,
    block_number: u64,
    signature: Signature,
}

struct MarkFinalizedRequest {
    requester_id: SubsystemId,  // Must be 9
    block_number: u64,
    finality_proof: FinalityProof,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: WriteBlockRequest from Subsystem 8 only
- ✅ Accept: WriteMerkleRootRequest from Subsystem 3 only
- ✅ Accept: WriteStateRootRequest from Subsystem 4 only
- ✅ Accept: MarkFinalizedRequest from Subsystem 9 only
- ✅ Accept: ReadBlockRequest from any subsystem (read-only)
- ❌ Reject: Write requests without valid Consensus signature
- ❌ Reject: Duplicate block writes
- ❌ Reject: Writes when disk >95% full

---

## SUBSYSTEM 3: TRANSACTION INDEXING

### I Am Allowed To Talk To:
- **Subsystem 2 (Block Storage)** - Store Merkle roots
- **Subsystem 7 (Bloom Filters)** - Provide transaction hashes
- **Subsystem 13 (Light Clients)** - Provide Merkle proofs

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Add transactions to Merkle tree
- **Subsystem 7 (Bloom Filters)** - Request transaction hashes
- **Subsystem 13 (Light Clients)** - Request Merkle proofs

### Strict Message Types:

**OUTGOING:**
```rust
struct MerkleRootStored {
    block_number: u64,
    merkle_root: [u8; 32],
    transaction_count: u32,
    timestamp: u64,
}

struct MerkleProof {
    transaction_hash: [u8; 32],
    block_number: u64,
    proof_path: Vec<[u8; 32]>,  // Sibling hashes from leaf to root
    merkle_root: [u8; 32],
    signature: Signature,
}

struct TransactionHashList {
    block_number: u64,
    hashes: Vec<[u8; 32]>,
    timestamp: u64,
}
```

**INCOMING:**
```rust
struct BuildMerkleTreeRequest {
    requester_id: SubsystemId,     // Must be 8
    block_number: u64,
    transactions: Vec<ValidatedTransaction>,
    signature: Signature,
}

struct MerkleProofRequest {
    requester_id: SubsystemId,     // Must be 13
    transaction_hash: [u8; 32],
    block_number: u64,
    signature: Signature,
}

struct TransactionHashRequest {
    requester_id: SubsystemId,     // Must be 7
    block_number: u64,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: BuildMerkleTreeRequest from Subsystem 8 only
- ✅ Accept: MerkleProofRequest from Subsystem 13 only
- ✅ Accept: TransactionHashRequest from Subsystem 7 only
- ❌ Reject: BuildMerkleTreeRequest with transactions from Subsystem 6 (unvalidated)
- ❌ Reject: Requests for non-existent blocks
- ❌ Reject: Tree depth >20 (1M+ transactions)

---

## SUBSYSTEM 4: STATE MANAGEMENT

### I Am Allowed To Talk To:
- **Subsystem 2 (Block Storage)** - Store state roots
- **Subsystem 6 (Mempool)** - Provide balance/nonce checks
- **Subsystem 11 (Smart Contracts)** - Provide state reads
- **Subsystem 12 (Transaction Ordering)** - Provide conflict detection

### Who Is Allowed To Talk To Me:
- **Subsystem 6 (Mempool)** - Check balance/nonce
- **Subsystem 11 (Smart Contracts)** - Read/write state
- **Subsystem 12 (Transaction Ordering)** - Detect conflicts
- **Subsystem 14 (Sharding)** - Access partitioned state

### Strict Message Types:

**OUTGOING:**
```rust
struct StateRootStored {
    block_number: u64,
    state_root: [u8; 32],
    accounts_modified: u32,
    timestamp: u64,
}

struct AccountState {
    address: [u8; 20],
    balance: u256,
    nonce: u64,
    code_hash: [u8; 32],
    storage_root: [u8; 32],
}

struct StateReadResponse {
    request_id: u64,
    value: Option<Vec<u8>>,
    proof: Vec<[u8; 32]>,  // Patricia trie proof
}

struct ConflictDetectionResponse {
    request_id: u64,
    conflicts: Vec<(usize, usize)>,  // (tx_index_1, tx_index_2)
    conflict_type: Vec<ConflictType>,
}

enum ConflictType {
    ReadWrite,
    WriteWrite,
    NonceConflict,
}
```

**INCOMING:**
```rust
struct StateReadRequest {
    requester_id: SubsystemId,     // Must be 6, 11, 12, or 14
    address: [u8; 20],
    storage_key: Option<[u8; 32]>, // None for account balance/nonce
    block_number: u64,
    signature: Signature,
}

struct StateWriteRequest {
    requester_id: SubsystemId,     // Must be 11 only
    address: [u8; 20],
    storage_key: [u8; 32],
    value: Vec<u8>,
    signature: Signature,
}

struct BalanceCheckRequest {
    requester_id: SubsystemId,     // Must be 6
    address: [u8; 20],
    required_balance: u256,
    signature: Signature,
}

struct ConflictDetectionRequest {
    requester_id: SubsystemId,     // Must be 12
    transactions: Vec<TransactionAccessPattern>,
    signature: Signature,
}

struct TransactionAccessPattern {
    tx_hash: [u8; 32],
    reads: Vec<(Address, StorageKey)>,
    writes: Vec<(Address, StorageKey)>,
}
```

### Security Boundaries:
- ✅ Accept: StateReadRequest from Subsystems 6, 11, 12, 14 only
- ✅ Accept: StateWriteRequest from Subsystem 11 only
- ✅ Accept: BalanceCheckRequest from Subsystem 6 only
- ✅ Accept: ConflictDetectionRequest from Subsystem 12 only
- ❌ Reject: Direct state writes from any subsystem except 11
- ❌ Reject: Unsigned state modifications
- ❌ Reject: Negative balances
- ❌ Reject: Nonce decrements

---

## SUBSYSTEM 5: BLOCK PROPAGATION

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request peer list
- **External Network Peers** - Broadcast blocks

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Send validated blocks to propagate
- **External Network Peers** - Receive blocks

### Strict Message Types:

**OUTGOING:**
```rust
struct BlockAnnouncement {
    block_hash: [u8; 32],
    block_number: u64,
    parent_hash: [u8; 32],
    timestamp: u64,
    signature: Signature,
}

struct CompactBlock {
    header: BlockHeader,
    short_txids: Vec<u64>,      // Short transaction IDs
    prefilled_txs: Vec<Transaction>,
    signature: Signature,
}

struct BlockRequest {
    block_hash: [u8; 32],
    request_id: u64,
    timestamp: u64,
}
```

**INCOMING:**
```rust
struct PropagateBlockRequest {
    requester_id: SubsystemId,     // Must be 8
    block: ValidatedBlock,
    validation_proof: ConsensusProof,
    signature: Signature,
}

struct BlockReceived {
    sender_peer_id: [u8; 32],
    block_hash: [u8; 32],
    full_block: Option<Block>,
    compact_block: Option<CompactBlock>,
    timestamp: u64,
}

struct BlockRequestReceived {
    sender_peer_id: [u8; 32],
    block_hash: [u8; 32],
    timestamp: u64,
}
```

### Security Boundaries:
- ✅ Accept: PropagateBlockRequest from Subsystem 8 only
- ✅ Accept: BlockReceived from known peers in Subsystem 1
- ✅ Accept: BlockRequestReceived from known peers
- ❌ Reject: Blocks without ConsensusProof
- ❌ Reject: Blocks from unknown peers
- ❌ Reject: >1 block announcement per peer per second (rate limit)
- ❌ Reject: Blocks >10MB

---

## SUBSYSTEM 6: TRANSACTION POOL (MEMPOOL)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Check balance/nonce
- **Subsystem 8 (Consensus)** - Provide transactions for blocks

### Who Is Allowed To Talk To Me:
- **Subsystem 10 (Signature Verification)** - Add verified transactions
- **Subsystem 8 (Consensus)** - Request transactions for block

### Strict Message Types:

**OUTGOING:**
```rust
struct TransactionBatch {
    transactions: Vec<ValidatedTransaction>,
    total_gas: u64,
    highest_fee: u256,
    timestamp: u64,
}

struct MempoolStatus {
    pending_count: u32,
    total_gas: u64,
    memory_usage: u64,
}
```

**INCOMING:**
```rust
struct AddTransactionRequest {
    requester_id: SubsystemId,        // Must be 10
    transaction: SignedTransaction,
    signature_valid: bool,            // Pre-verified by Subsystem 10
    signature: Signature,
}

struct GetTransactionsRequest {
    requester_id: SubsystemId,        // Must be 8
    max_count: u32,
    max_gas: u64,
    signature: Signature,
}

struct RemoveTransactionsRequest {
    requester_id: SubsystemId,        // Must be 8
    transaction_hashes: Vec<[u8; 32]>,
    reason: RemovalReason,
    signature: Signature,
}

enum RemovalReason {
    Included,
    Invalid,
    Expired,
}
```

### Security Boundaries:
- ✅ Accept: AddTransactionRequest from Subsystem 10 only (must be pre-verified)
- ✅ Accept: GetTransactionsRequest from Subsystem 8 only
- ✅ Accept: RemoveTransactionsRequest from Subsystem 8 only
- ❌ Reject: Transactions with signature_valid=false
- ❌ Reject: Transactions with gas price <1 gwei
- ❌ Reject: >16 pending transactions per account
- ❌ Reject: Total mempool size >5000 transactions
- ❌ Reject: Direct transaction additions from Subsystem 11 or others

---

## SUBSYSTEM 7: TRANSACTION FILTERING (BLOOM FILTERS)

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request full nodes
- **Subsystem 13 (Light Clients)** - Provide filtered transactions

### Who Is Allowed To Talk To Me:
- **Subsystem 3 (Transaction Indexing)** - Receive transaction hashes
- **Subsystem 13 (Light Clients)** - Request filter updates

### Strict Message Types:

**OUTGOING:**
```rust
struct BloomFilter {
    filter_id: u64,
    bit_array: Vec<u8>,          // m-bit array
    hash_count: u8,              // k hash functions
    false_positive_rate: f32,
    block_range: (u64, u64),     // (start, end)
    signature: Signature,
}

struct FilteredTransactions {
    block_number: u64,
    transactions: Vec<[u8; 32]>,  // Matching transaction hashes
    false_positives_included: bool,
}
```

**INCOMING:**
```rust
struct BuildFilterRequest {
    requester_id: SubsystemId,       // Must be 13
    watched_addresses: Vec<[u8; 20]>,
    start_block: u64,
    end_block: u64,
    target_fpr: f32,                 // Target false positive rate
    signature: Signature,
}

struct UpdateFilterRequest {
    requester_id: SubsystemId,       // Must be 13
    filter_id: u64,
    add_addresses: Vec<[u8; 20]>,
    remove_addresses: Vec<[u8; 20]>,
    signature: Signature,
}

struct TransactionHashUpdate {
    sender_id: SubsystemId,          // Must be 3
    block_number: u64,
    hashes: Vec<[u8; 32]>,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: BuildFilterRequest from Subsystem 13 only
- ✅ Accept: UpdateFilterRequest from Subsystem 13 only
- ✅ Accept: TransactionHashUpdate from Subsystem 3 only
- ❌ Reject: Filters with >1000 watched addresses (privacy risk)
- ❌ Reject: FPR <0.01 or >0.1 (too precise or too noisy)
- ❌ Reject: >1 filter update per 10 blocks per client
- ❌ Reject: Direct filter access from external nodes

---

## SUBSYSTEM 8: CONSENSUS MECHANISM

### I Am Allowed To Talk To:
- **Subsystem 2 (Block Storage)** - Store validated blocks
- **Subsystem 3 (Transaction Indexing)** - Verify Merkle roots
- **Subsystem 5 (Block Propagation)** - Propagate validated blocks
- **Subsystem 6 (Mempool)** - Get transactions for blocks
- **Subsystem 9 (Finality)** - Provide attestations
- **Subsystem 12 (Transaction Ordering)** - Order transactions (optional)
- **Subsystem 14 (Sharding)** - Coordinate shards (optional)
- **Subsystem 15 (Cross-Chain)** - Provide finality proofs (optional)

### Who Is Allowed To Talk To Me:
- **Subsystem 5 (Block Propagation)** - Receive new blocks
- **Subsystem 10 (Signature Verification)** - Provide validator signatures
- **External Validators** - Receive attestations (PoS) or block proposals (PBFT)

### Strict Message Types:

**OUTGOING:**
```rust
struct ValidatedBlock {
    header: BlockHeader,
    transactions: Vec<ValidatedTransaction>,
    merkle_root: [u8; 32],
    state_root: [u8; 32],
    consensus_proof: ConsensusProof,
    validator_signatures: Vec<Signature>,
}

struct ConsensusProof {
    proof_type: ConsensusType,
    validator_set: Vec<[u8; 20]>,
    attestations: Vec<Attestation>,  // For PoS
    pbft_votes: Option<PBFTVotes>,   // For PBFT
}

enum ConsensusType {
    ProofOfStake,
    PBFT,
}

struct BlockHeader {
    parent_hash: [u8; 32],
    block_number: u64,
    timestamp: u64,
    merkle_root: [u8; 32],
    state_root: [u8; 32],
    difficulty: u256,
    gas_used: u64,
    gas_limit: u64,
}
```

**INCOMING:**
```rust
struct ValidateBlockRequest {
    requester_id: SubsystemId,          // Must be 5
    block: Block,
    received_from: [u8; 32],            // Peer ID
    timestamp: u64,
}

struct AttestationReceived {
    sender_id: SubsystemId,             // Must be 10 (verified signature)
    validator: [u8; 20],
    block_hash: [u8; 32],
    slot: u64,
    signature: Signature,
    signature_valid: bool,
}

struct PBFTMessage {
    sender_id: SubsystemId,             // Must be 10 (verified signature)
    message_type: PBFTPhase,
    block_hash: [u8; 32],
    view: u64,
    sender_validator: [u8; 20],
    signature: Signature,
}

enum PBFTPhase {
    PrePrepare,
    Prepare,
    Commit,
    ViewChange,
}
```

### Security Boundaries:
- ✅ Accept: ValidateBlockRequest from Subsystem 5 only
- ✅ Accept: AttestationReceived from Subsystem 10 only (pre-verified)
- ✅ Accept: PBFTMessage from Subsystem 10 only (pre-verified)
- ❌ Reject: Blocks without valid transactions
- ❌ Reject: Blocks with invalid Merkle root
- ❌ Reject: Blocks with invalid state transitions
- ❌ Reject: Attestations without signature_valid=true
- ❌ Reject: >33% Byzantine validators (safety threshold)
- ❌ Reject: Blocks older than 2 epochs

---

## SUBSYSTEM 9: FINALITY MECHANISM

### I Am Allowed To Talk To:
- **Subsystem 2 (Block Storage)** - Mark blocks as finalized
- **Subsystem 15 (Cross-Chain)** - Provide finality proofs

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Provide attestations for finality

### Strict Message Types:

**OUTGOING:**
```rust
struct FinalityProof {
    checkpoint: u64,                // Epoch boundary
    block_hash: [u8; 32],
    justification: Vec<Attestation>,
    supermajority_reached: bool,     // >2/3 validators
    timestamp: u64,
    signature: Signature,
}

struct FinalizedNotification {
    block_number: u64,
    block_hash: [u8; 32],
    finalized_at: u64,
}
```

**INCOMING:**
```rust
struct FinalityCheckRequest {
    requester_id: SubsystemId,      // Must be 8
    checkpoint: u64,
    attestations: Vec<Attestation>,
    signature: Signature,
}

struct FinalityProofRequest {
    requester_id: SubsystemId,      // Must be 15
    block_hash: [u8; 32],
    signature: Signature,
}

struct Attestation {
    validator: [u8; 20],
    source_checkpoint: u64,
    target_checkpoint: u64,
    block_hash: [u8; 32],
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: FinalityCheckRequest from Subsystem 8 only
- ✅ Accept: FinalityProofRequest from Subsystem 15 only
- ✅ Accept: Attestations with >2/3 validator support
- ❌ Reject: Attestations without 67%+ stake participation
- ❌ Reject: Conflicting finalizations (slashable offense)
- ❌ Reject: Finalization of non-consecutive checkpoints
- ❌ Reject: Attestations from unknown validators

---

## SUBSYSTEM 10: SIGNATURE VERIFICATION

### I Am Allowed To Talk To:
- **Subsystem 6 (Mempool)** - Send verified transactions
- **Subsystem 8 (Consensus)** - Send verified validator signatures

### Who Is Allowed To Talk To Me:
- **External Network** - Receive signed transactions (via P2P gateway)
- **Subsystem 5 (Block Propagation)** - Verify block signatures from network peers
- **Subsystem 6 (Mempool)** - Verify transaction signatures before pool entry
- **Subsystem 8 (Consensus)** - Verify block/validator signatures
- **Subsystem 9 (Finality)** - Verify attestation signatures

### FORBIDDEN Consumers (Principle of Least Privilege):
The following subsystems are EXPLICITLY FORBIDDEN from accessing SignatureVerification:
- ❌ Subsystem 1 (Peer Discovery) - No cryptographic verification needs
- ❌ Subsystem 2 (Block Storage) - Storage only, receives pre-verified data
- ❌ Subsystem 3 (Transaction Indexing) - Indexing only, receives pre-verified data
- ❌ Subsystem 4 (State Management) - State only, receives pre-verified data
- ❌ Subsystem 7 (Bloom Filters) - Filtering only, no signature needs
- ❌ Subsystem 11 (Smart Contracts) - Execution only, receives pre-verified transactions
- ❌ Subsystem 12 (Transaction Ordering) - Ordering only, receives pre-verified data
- ❌ Subsystem 13 (Light Clients) - Receives proofs, does not verify signatures directly
- ❌ Subsystem 14 (Sharding) - Coordination only, uses Consensus for verification
- ❌ Subsystem 15 (Cross-Chain) - Uses Finality proofs, not direct signature verification

**Security Rationale:** Restricting access to SignatureVerification minimizes the attack surface. If a low-priority subsystem (e.g., BloomFilters) is compromised, the attacker cannot use it as a vector to DoS the signature verification service. Only transaction ingestion points (P2P, Mempool) and consensus-critical paths have access.

### Strict Message Types:

**OUTGOING:**
```rust
struct VerifiedTransaction {
    version: u16,                    // Protocol version
    transaction: SignedTransaction,
    signer_address: [u8; 20],
    signature_valid: bool,
    verification_timestamp: u64,
}

struct VerifiedSignature {
    version: u16,                    // Protocol version
    message_hash: [u8; 32],
    signature: Signature,
    signer: [u8; 20],
    valid: bool,
    verification_timestamp: u64,
}

struct BatchVerificationResult {
    version: u16,                    // Protocol version
    request_id: u64,
    correlation_id: [u8; 16],        // Maps to original request
    results: Vec<bool>,              // One per signature
    total_valid: u32,
    total_invalid: u32,
}
```

**INCOMING:**
```rust
struct VerifyTransactionRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 5, 6, 8, or 9 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    transaction: SignedTransaction,
    expected_signer: Option<[u8; 20]>,
}

struct SignedTransaction {
    nonce: u64,
    to: [u8; 20],
    value: u256,
    data: Vec<u8>,
    gas_limit: u64,
    gas_price: u256,
    signature: Signature,
}

struct Signature {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

struct VerifySignatureRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 5, 8, or 9 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    message_hash: [u8; 32],
    signature: Signature,
    expected_signer: [u8; 20],
}

struct BatchVerifyRequest {
    version: u16,                      // Protocol version - MUST be validated first
    requester_id: SubsystemId,         // MUST be 8 ONLY
    correlation_id: [u8; 16],          // For async response correlation
    reply_to: Topic,                   // Where to send response
    signatures: Vec<(MessageHash, Signature, ExpectedSigner)>,
}
```

### Security Boundaries:
- ✅ Accept: VerifyTransactionRequest from Subsystems 5, 6, 8, 9 ONLY
- ✅ Accept: VerifySignatureRequest from Subsystems 5, 8, 9 ONLY
- ✅ Accept: BatchVerifyRequest from Subsystem 8 ONLY
- ❌ **REJECT: ALL requests from Subsystems 1, 2, 3, 4, 7, 11, 12, 13, 14, 15**
- ❌ Reject: Signatures with low s value (malleability)
- ❌ Reject: Signatures with v ∉ {27, 28}
- ❌ Reject: Batch verification with >1000 signatures (DoS risk)
- ❌ Reject: Invalid ECDSA points
- ❌ Reject: Messages with unsupported version field

---

## SUBSYSTEM 11: SMART CONTRACT EXECUTION

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Read/write contract state
- **Subsystem 15 (Cross-Chain)** - Execute HTLC contracts

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Execute transactions in validated block
- **Subsystem 12 (Transaction Ordering)** - Execute ordered transactions

### Strict Message Types:

**OUTGOING:**
```rust
struct ExecutionResult {
    transaction_hash: [u8; 32],
    success: bool,
    gas_used: u64,
    return_data: Vec<u8>,
    logs: Vec<Log>,
    state_changes: Vec<StateChange>,
    signature: Signature,
}

struct StateChange {
    address: [u8; 20],
    storage_key: [u8; 32],
    old_value: Vec<u8>,
    new_value: Vec<u8>,
}

struct Log {
    address: [u8; 20],
    topics: Vec<[u8; 32]>,
    data: Vec<u8>,
}

struct ContractDeployed {
    contract_address: [u8; 20],
    deployer: [u8; 20],
    code_hash: [u8; 32],
    gas_used: u64,
}
```

**INCOMING:**
```rust
struct ExecuteTransactionRequest {
    requester_id: SubsystemId,         // Must be 8 or 12
    transaction: ValidatedTransaction,
    block_context: BlockContext,
    gas_limit: u64,
    signature: Signature,
}

struct BlockContext {
    block_number: u64,
    timestamp: u64,
    coinbase: [u8; 20],
    difficulty: u256,
    gas_limit: u64,
}

struct ValidatedTransaction {
    from: [u8; 20],
    to: Option<[u8; 20]>,  // None for contract creation
    value: u256,
    data: Vec<u8>,
    nonce: u64,
    gas_price: u256,
    gas_limit: u64,
}

struct ExecuteHTLCRequest {
    requester_id: SubsystemId,         // Must be 15
    htlc_contract: [u8; 20],
    operation: HTLCOperation,
    signature: Signature,
}

enum HTLCOperation {
    Claim { secret: [u8; 32] },
    Refund,
}
```

### Security Boundaries:
- ✅ Accept: ExecuteTransactionRequest from Subsystems 8, 12 only
- ✅ Accept: ExecuteHTLCRequest from Subsystem 15 only
- ✅ Accept: Only transactions with validated signatures
- ❌ Reject: Execution without valid BlockContext
- ❌ Reject: Gas limit >30M (block gas limit)
- ❌ Reject: Recursive calls >1024 depth
- ❌ Reject: Execution time >5 seconds (timeout)
- ❌ Reject: Direct execution requests from external sources

---

## SUBSYSTEM 12: TRANSACTION ORDERING (DAG)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Detect conflicts
- **Subsystem 11 (Smart Contracts)** - Execute ordered transactions

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Provide transactions to order

### Strict Message Types:

**OUTGOING:**
```rust
struct OrderedTransactions {
    transactions: Vec<ValidatedTransaction>,
    dependency_graph: DependencyGraph,
    parallel_batches: Vec<Vec<usize>>,  // Indices of parallelizable txs
    signature: Signature,
}

struct DependencyGraph {
    nodes: Vec<[u8; 32]>,        // Transaction hashes
    edges: Vec<(usize, usize)>,  // (from, to) dependencies
}

struct OrderingMetrics {
    total_transactions: u32,
    parallel_batches: u32,
    sequential_transactions: u32,
    conflicts_detected: u32,
    ordering_time_ms: u64,
}
```

**INCOMING:**
```rust
struct OrderTransactionsRequest {
    requester_id: SubsystemId,      // Must be 8
    transactions: Vec<ValidatedTransaction>,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: OrderTransactionsRequest from Subsystem 8 only
- ❌ Reject: Transactions with circular dependencies
- ❌ Reject: Dependency graphs with >10,000 edges (complexity attack)
- ❌ Reject: Requests from subsystems other than Consensus

---

## SUBSYSTEM 13: LIGHT CLIENT SYNC

### I Am Allowed To Talk To:
- **Subsystem 1 (Peer Discovery)** - Request full nodes
- **Subsystem 7 (Bloom Filters)** - Setup filters
- **External Full Nodes** - Request headers and data

### Who Is Allowed To Talk To Me:
- **Subsystem 3 (Transaction Indexing)** - Receive Merkle proofs
- **External Full Nodes** - Receive headers and proofs

### Strict Message Types:

**OUTGOING:**
```rust
struct SyncRequest {
    start_block: u64,
    end_block: u64,
    checkpoint_hash: [u8; 32],
    signature: Signature,
}

struct ProofVerificationRequest {
    transaction_hash: [u8; 32],
    block_number: u64,
    signature: Signature,
}

struct FilterSetupRequest {
    watched_addresses: Vec<[u8; 20]>,
    target_fpr: f32,
    signature: Signature,
}
```

**INCOMING:**
```rust
struct HeaderChain {
    headers: Vec<BlockHeader>,
    checkpoint_proof: Vec<[u8; 32]>,
    signature: Signature,
}

struct MerkleProofReceived {
    sender_id: SubsystemId,           // Must be 3
    proof: MerkleProof,
    verified: bool,
}

struct FilterCreated {
    sender_id: SubsystemId,           // Must be 7
    filter: BloomFilter,
}
```

### Security Boundaries:
- ✅ Accept: MerkleProofReceived from Subsystem 3 only
- ✅ Accept: FilterCreated from Subsystem 7 only
- ✅ Accept: HeaderChain from trusted full nodes
- ❌ Reject: Headers without valid PoW/PoS proof
- ❌ Reject: Proofs that don't match stored Merkle root
- ❌ Reject: Headers from unknown full nodes
- ❌ Reject: Sync requests spanning >10,000 blocks

---

## SUBSYSTEM 14: SHARDING (OPTIONAL)

### I Am Allowed To Talk To:
- **Subsystem 4 (State Management)** - Access partitioned state
- **Subsystem 8 (Consensus)** - Report shard status

### Who Is Allowed To Talk To Me:
- **Subsystem 8 (Consensus)** - Beacon chain coordination

### Strict Message Types:

**OUTGOING:**
```rust
struct ShardAssignment {
    account: [u8; 20],
    shard_id: u16,
    signature: Signature,
}

struct CrossShardTransaction {
    from_shard: u16,
    to_shard: u16,
    transaction: ValidatedTransaction,
    two_phase_commit_id: u64,
    signature: Signature,
}

struct ShardStatus {
    shard_id: u16,
    validator_count: u32,
    transaction_count: u64,
    state_size: u64,
    load_percentage: u8,
}
```

**INCOMING:**
```rust
struct AssignShardRequest {
    requester_id: SubsystemId,        // Must be 8
    account: [u8; 20],
    signature: Signature,
}

struct CrossShardCommitRequest {
    requester_id: SubsystemId,        // Must be 8
    commit_id: u64,
    phase: CommitPhase,
    participating_shards: Vec<u16>,
    signature: Signature,
}

enum CommitPhase {
    Prepare,
    Commit,
    Abort,
}

struct RebalanceRequest {
    requester_id: SubsystemId,        // Must be 8
    target_load: u8,
    signature: Signature,
}
```

### Security Boundaries:
- ✅ Accept: AssignShardRequest from Subsystem 8 only
- ✅ Accept: CrossShardCommitRequest from Subsystem 8 only
- ✅ Accept: RebalanceRequest from Subsystem 8 only
- ❌ Reject: Cross-shard transactions without 2PC
- ❌ Reject: Shards with <128 validators (security threshold)
- ❌ Reject: Shard assignments from non-Consensus subsystems
- ❌ Reject: Rebalancing that creates load imbalance >20%

---

## SUBSYSTEM 15: CROSS-CHAIN COMMUNICATION

### I Am Allowed To Talk To:
- **Subsystem 8 (Consensus)** - Request finality proofs
- **Subsystem 11 (Smart Contracts)** - Execute HTLC operations

### Who Is Allowed To Talk To Me:
- **Subsystem 9 (Finality)** - Provide finality proofs
- **External Blockchains** - Receive cross-chain messages

### Strict Message Types:

**OUTGOING:**
```rust
struct HTLCInitiate {
    hashlock: [u8; 32],          // H(secret)
    timelock: u64,               // Expiration timestamp
    amount: u256,
    recipient_chain: ChainId,
    recipient: [u8; 20],
    signature: Signature,
}

struct HTLCClaim {
    htlc_id: [u8; 32],
    secret: [u8; 32],            // Reveals the secret
    signature: Signature,
}

struct HTLCRefund {
    htlc_id: [u8; 32],
    reason: RefundReason,
    signature: Signature,
}

enum RefundReason {
    Timeout,
    Failed,
}

struct ChainId {
    chain_name: String,
    chain_id: u64,
}
```

**INCOMING:**
```rust
struct InitiateSwapRequest {
    requester_id: SubsystemId,        // External or validated
    amount: u256,
    target_chain: ChainId,
    target_recipient: [u8; 20],
    hashlock: [u8; 32],
    timelock: u64,
    signature: Signature,
}

struct ClaimSwapRequest {
    requester_id: SubsystemId,        // External or validated
    htlc_id: [u8; 32],
    secret: [u8; 32],
    signature: Signature,
}

struct FinalityProofReceived {
    sender_id: SubsystemId,           // Must be 9
    proof: FinalityProof,
    chain: ChainId,
}

struct CrossChainMessage {
    source_chain: ChainId,
    message_type: CrossChainMessageType,
    payload: Vec<u8>,
    signature: Signature,
}

enum CrossChainMessageType {
    HTLCInitiated,
    HTLCClaimed,
    HTLCRefunded,
}
```

### Security Boundaries:
- ✅ Accept: FinalityProofReceived from Subsystem 9 only
- ✅ Accept: InitiateSwapRequest from validated external sources
- ✅ Accept: ClaimSwapRequest with valid secret reveal
- ❌ Reject: HTLCs with timelock <6 hours
- ❌ Reject: HTLCs without finality proof on both chains
- ❌ Reject: Secret reveals after timelock expiration
- ❌ Reject: Cross-chain messages without valid signatures
- ❌ Reject: Claims without valid hash preimage (H(secret) == hashlock)

---

## COMMUNICATION FLOW DIAGRAM

```
┌─────────────────────────────────────────────────────────────────┐
│                     EXTERNAL SOURCES                            │
├─────────────────────────────────────────────────────────────────┤
│  • Bootstrap Nodes → [1]                                        │
│  • Network Peers → [1, 5]                                       │
│  • Transactions → [10]                                          │
│  • Validators → [8, 10]                                         │
│  • Full Nodes → [13]                                            │
│  • Other Blockchains → [15]                                     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    SUBSYSTEM INTERACTIONS                       │
└─────────────────────────────────────────────────────────────────┘

[1] Peer Discovery
     ↓ PeerList
     ├──→ [5] Block Propagation
     ├──→ [7] Bloom Filters
     └──→ [13] Light Clients

[10] Signature Verification
     ↓ VerifiedTransaction
     ├──→ [6] Mempool
     │
     ↓ VerifiedSignature
     └──→ [8] Consensus

[6] Mempool
     ↓ BalanceCheckRequest
     ├──→ [4] State Management
     │
     ↓ TransactionBatch
     └──→ [8] Consensus

[8] Consensus
     ↓ BuildMerkleTreeRequest
     ├──→ [3] Transaction Indexing
     │
     ↓ StateReadRequest
     ├──→ [4] State Management
     │
     ↓ ValidatedBlock
     ├──→ [2] Block Storage
     ├──→ [5] Block Propagation
     │
     ↓ Attestations
     ├──→ [9] Finality
     │
     ↓ OrderTransactionsRequest (optional)
     ├──→ [12] Transaction Ordering
     │
     └──→ [14] Sharding (optional)

[3] Transaction Indexing
     ↓ MerkleRootStored
     ├──→ [2] Block Storage
     │
     ↓ TransactionHashList
     ├──→ [7] Bloom Filters
     │
     ↓ MerkleProof
     └──→ [13] Light Clients

[4] State Management
     ↓ StateRootStored
     ├──→ [2] Block Storage
     │
     ↓ AccountState / StateReadResponse
     ├──→ [6] Mempool
     ├──→ [11] Smart Contracts
     ├──→ [12] Transaction Ordering
     └──→ [14] Sharding

[9] Finality
     ↓ FinalityProof
     ├──→ [2] Block Storage
     └──→ [15] Cross-Chain

[11] Smart Contracts
     ↓ StateWriteRequest
     ├──→ [4] State Management
     │
     ↓ ExecutionResult
     └──→ [15] Cross-Chain (HTLC)

[12] Transaction Ordering
     ↓ ConflictDetectionRequest
     ├──→ [4] State Management
     │
     ↓ OrderedTransactions
     └──→ [11] Smart Contracts

[7] Bloom Filters
     ↓ BloomFilter
     └──→ [13] Light Clients

[13] Light Clients
     ↓ PeerListRequest
     ├──→ [1] Peer Discovery
     │
     ↓ MerkleProofRequest
     ├──→ [3] Transaction Indexing
     │
     └──→ [7] Bloom Filters

[14] Sharding
     ↓ ShardStatus
     ├──→ [8] Consensus
     │
     └──→ [4] State Management

[15] Cross-Chain
     ↓ FinalityProofRequest
     ├──→ [9] Finality
     │
     ↓ ExecuteHTLCRequest
     └──→ [11] Smart Contracts
```

---

## IPC SECURITY SUMMARY TABLE

| Subsystem | Allowed Senders | Allowed Recipients | Critical Security Check |
|-----------|----------------|-------------------|------------------------|
| 1 (Peer Discovery) | 5, 7, 13, External | 5, 7, 13 | PoW for bootstrap, reputation scoring |
| 2 (Block Storage) | 3, 4, 8, 9 | None (responses only) | Write permissions enforced |
| 3 (Transaction Indexing) | 7, 8, 13 | 2, 7, 13 | Only validated transactions |
| 4 (State Management) | 6, 11, 12, 14 | 2, 6, 11, 12 | Only Subsystem 11 can write |
| 5 (Block Propagation) | 8, External Peers | 1, 10, External Peers | ConsensusProof required |
| 6 (Mempool) | 8, 10 | 4, 8, 10 | Only pre-verified transactions |
| 7 (Bloom Filters) | 3, 13 | 1, 13 | FPR limits, address count limits |
| 8 (Consensus) | 5, 10, External | 2, 3, 5, 6, 9, 10, 12, 14, 15 | Signature validation, >67% threshold |
| 9 (Finality) | 8 | 2, 10, 15 | Supermajority (>2/3) required |
| 10 (Signature Verification) | **5, 6, 8, 9 ONLY** | 6, 8 | **Least Privilege: 11 subsystems FORBIDDEN** |
| 11 (Smart Contracts) | 8, 12 | 4, 15 | Gas limits, depth limits |
| 12 (Transaction Ordering) | 8 | 4, 11 | Cycle detection |
| 13 (Light Clients) | 1, 3, 7, External | 1, 7 | Merkle proof validation |
| 14 (Sharding) | 8 | 4, 8 | Minimum validator count |
| 15 (Cross-Chain) | 9, 11, External | 8, 11 | Finality proofs, timelock enforcement |

---

## IMPLEMENTATION CHECKLIST

For each subsystem, implement:

### 1. Message Type Validation
```rust
fn validate_message_type<T: MessageType>(msg: &T) -> Result<(), ValidationError> {
    // Check message type matches expected struct
    // Verify all required fields present
    // Validate field constraints (ranges, lengths)
}
```

### 2. Subsystem ID Verification
```rust
fn verify_sender(sender_id: SubsystemId, allowed: &[SubsystemId]) -> Result<(), AuthError> {
    if !allowed.contains(&sender_id) {
        return Err(AuthError::UnauthorizedSender);
    }
    Ok(())
}
```

### 3. Signature Verification
```rust
fn verify_signature(msg: &SignedMessage) -> Result<(), CryptoError> {
    // Verify message signature
    // Check signature timestamp (reject if >60s old)
    // Verify signer authority
}
```

### 4. Rate Limiting
```rust
struct RateLimiter {
    limits: HashMap<SubsystemId, (u32, Duration)>,
    counters: HashMap<SubsystemId, (u32, Instant)>,
}

impl RateLimiter {
    fn check_limit(&mut self, sender: SubsystemId) -> Result<(), RateLimitError> {
        // Implement token bucket or sliding window
    }
}
```

### 5. Input Sanitization
```rust
fn sanitize_input<T>(input: &T) -> Result<T, SanitizationError> {
    // Check bounds (array lengths, numeric ranges)
    // Validate addresses (20 bytes)
    // Validate hashes (32 bytes)
    // Check for null bytes, invalid UTF-8, etc.
}
```

---

## ATTACK SCENARIO EXAMPLES

### Scenario 1: Compromised Mempool
**Attack:** Attacker gains control of Subsystem 6 (Mempool)
**Attempt:** Try to add malicious transactions directly to Consensus

**Defense:**
- ❌ Subsystem 8 REJECTS: `AddTransactionRequest` from Subsystem 6
- ✅ Only accepts `TransactionBatch` when requested via `GetTransactionsRequest`
- ✅ All transactions must come through Subsystem 10 (Signature Verification) first

**Result:** Attack contained to Mempool, cannot affect consensus

---

### Scenario 2: Compromised Smart Contract Executor
**Attack:** Attacker gains control of Subsystem 11
**Attempt:** Directly modify state without transaction validation

**Defense:**
- ❌ Subsystem 4 REJECTS: `StateWriteRequest` without valid `ExecuteTransactionRequest` from Subsystem 8
- ✅ Only Subsystem 11 can write, but only after receiving execution request from Consensus
- ✅ All state changes signed and auditable

**Result:** Cannot modify state without going through Consensus

---

### Scenario 3: Compromised Peer Discovery
**Attack:** Attacker controls Subsystem 1
**Attempt:** Provide malicious peers to isolate node (Eclipse attack)

**Defense:**
- ✅ Subsystem 5 validates peer reputation before connecting
- ✅ Maintains connections to known good peers (checkpoint peers)
- ✅ Random peer selection prevents full eclipse
- ✅ Peer diversity requirements (IP subnet limits)

**Result:** Partial mitigation, cannot fully eclipse node

---

## CROSS-SUBSYSTEM AUTHENTICATION

Every message between subsystems must include:

```rust
struct AuthenticatedMessage<T> {
    // === VERSION (MANDATORY - MUST BE FIRST) ===
    version: u16,                    // Protocol version - deserialize and validate FIRST
    
    // === ROUTING ===
    sender_id: SubsystemId,
    recipient_id: SubsystemId,
    
    // === CORRELATION (FOR REQUEST/RESPONSE) ===
    correlation_id: [u8; 16],        // UUID v4 for request/response mapping
    reply_to: Option<Topic>,         // Topic for async response delivery
    
    // === PAYLOAD ===
    payload: T,
    
    // === SECURITY ===
    timestamp: u64,
    nonce: u64,
    signature: [u8; 32],             // HMAC-SHA256 with shared secret
}

struct Topic {
    subsystem_id: SubsystemId,
    channel: String,                 // e.g., "responses", "dlq.errors"
}

impl<T> AuthenticatedMessage<T> {
    fn verify(&self, expected_sender: SubsystemId, shared_secret: &[u8]) -> Result<(), AuthError> {
        // 0. Check version FIRST (before any deserialization of payload)
        if self.version < MIN_SUPPORTED_VERSION || self.version > MAX_SUPPORTED_VERSION {
            return Err(AuthError::UnsupportedVersion { 
                received: self.version,
                supported_range: (MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION),
            });
        }
        
        // 1. Check sender_id matches expected
        if self.sender_id != expected_sender {
            return Err(AuthError::InvalidSender);
        }
        
        // 2. Check timestamp (reject if >60s old)
        let now = current_timestamp();
        if now - self.timestamp > 60 {
            return Err(AuthError::MessageTooOld);
        }
        
        // 3. Check nonce (prevent replay)
        if !nonce_tracker.check_and_mark(self.nonce) {
            return Err(AuthError::NonceReused);
        }
        
        // 4. Verify HMAC
        let computed_hmac = compute_hmac(shared_secret, &self.serialize_without_sig());
        if !constant_time_eq(&computed_hmac, &self.signature) {
            return Err(AuthError::InvalidSignature);
        }
        
        Ok(())
    }
}
```

---

## DEFENSE IN DEPTH SUMMARY

1. **Type Safety** - Strict message types prevent wrong data structures
2. **Sender Verification** - Only authorized subsystems can communicate
3. **Signature Validation** - Cryptographic proof of authenticity
4. **Rate Limiting** - Prevents flooding attacks
5. **Input Sanitization** - Bounds checking on all inputs
6. **Nonce Tracking** - Prevents replay attacks
7. **Timestamp Validation** - Prevents old message replay
8. **Compartmentalization** - Breaching one subsystem doesn't compromise others
9. **Version Validation** - Protocol version checked BEFORE payload deserialization
10. **Correlation IDs** - Enables non-blocking request/response without deadlocks
11. **Dead Letter Queues** - Critical events never lost, always recoverable
12. **Principle of Least Privilege** - SignatureVerification restricted to 4 subsystems only

**Result:** Even if an attacker fully compromises one subsystem, they are trapped in that compartment and cannot spread to others without breaking multiple layers of authentication. Additionally, the system is resilient to cascading failures through non-blocking patterns and recoverable from data loss through DLQ infrastructure.