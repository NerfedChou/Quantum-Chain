# BLOCKCHAIN SUBSYSTEMS: STANDALONE ARCHITECTURE
## Each subsystem defined with Main Algorithm, Supporting Algorithms, Dependencies, and Security
**Version:** 2.3 | **Last Updated:** 2025-12-01

---

## V2.3 GLOBAL SECURITY MANDATES

These security rules apply to ALL subsystems and are non-negotiable:

1. **Envelope-Only Identity (V2.2 Amendment):** 
   - All inter-subsystem messages use `AuthenticatedMessage<T>` envelope
   - The envelope's `sender_id` is the SOLE source of truth for identity
   - Payloads MUST NOT contain `requester_id` or similar identity fields
   - See: Architecture.md Section 3.2.1

2. **Choreography over Orchestration (V2.2):**
   - No single subsystem "orchestrates" others
   - Subsystems publish events to Event Bus; consumers react independently
   - See: Architecture.md Section 5.1

3. **Time-Bounded Nonce (Replay Prevention):**
   - All authenticated messages include nonce + timestamp
   - Receivers maintain `TimeBoundedNonceCache` to reject replays
   - See: Architecture.md Section 3.2.2

---

## SUBSYSTEM 1: PEER DISCOVERY & ROUTING
**Purpose:** Find and connect to other nodes in the network

### Main Algorithm: Kademlia DHT
**Why:** O(log n) node lookup complexity, self-organizing, resilient to node churn

### Supporting Algorithms:
1. **XOR Distance Metric** - Calculate node proximity in 160-bit ID space
2. **k-Bucket Management** - Organize known peers by distance (k=20 typical)
3. **Iterative Node Lookup** - Query α closest nodes iteratively (α=3 typical)
4. **STUN/TURN Protocol** - NAT traversal for nodes behind firewalls

### Dependencies:
- **Subsystem 10** (Signature Verification) - Verify node identity for DDoS defense
- **Bootstrap Nodes** - Required for initial network entry

### DDoS Defense (Network Edge Protection):
Peer Discovery can now verify node signatures before accepting peers into the system.
This blocks malicious actors at the network edge, preventing Mempool spam attacks.

### Security & Robustness:
**Attack Vectors:**
- Sybil attacks where attackers create numerous fake peers to take over routing
- Eclipse attacks isolating nodes from the honest network
- Routing table poisoning with malicious node IDs
- **Mempool spam via unverified peers (NOW PREVENTED)**

**Defenses:**
1. **Proof of Work for Node IDs** - Require computational effort to generate valid node IDs
2. **IP Address Diversity** - Limit nodes per /24 subnet in routing table
3. **Reputation Scoring** - Track successful vs failed interactions per peer
4. **Random Peer Selection** - Mix deterministic routing with random peer connections
5. **Bootstrap Node Diversity** - Use multiple bootstrap nodes from different entities
6. **Signature Verification at Edge** - Verify node identity before accepting (via Subsystem 10)

**Robustness Measures:**
- Periodic bucket refresh (every 1 hour)
- Redundant peer storage (k peers per bucket)
- Graceful degradation when peers go offline

---

## SUBSYSTEM 2: BLOCK STORAGE ENGINE
**Purpose:** Persist blockchain data to disk efficiently

### Main Algorithm: LSM Tree (Log-Structured Merge Tree)
**Why:** Optimized for write-heavy workloads, fast sequential writes, good compression

### Supporting Algorithms:
1. **Memtable (Skip List)** - In-memory buffer for recent writes, O(log n) operations
2. **SSTable Compaction** - Merge sorted string tables, eliminate tombstones
3. **Bloom Filter** - Quick key existence checks before disk read, 1% false positive rate
4. **Snappy Compression** - Fast compression/decompression (~250 MB/s)

### Dependencies:
- **Event Bus** - Subscribes to `BlockValidated`, `MerkleRootComputed`, `StateRootComputed`
- **Subsystem 9** (Finality) - Marks blocks as finalized

### Provides To (V2.3):
- **Subsystem 3** (Transaction Indexing) - Transaction hashes for Merkle proof generation
- **Subsystem 6** (Mempool) - BlockStorageConfirmation for Two-Phase Commit

**CRITICAL DESIGN DECISION (v2.3 Stateful Assembler + Data Provider Pattern):**
Block Storage acts as a **Stateful Assembler** in the choreography pattern AND as
a **Data Provider** for downstream subsystems:

**Write Path (Choreography):**
1. Subscribes to three independent events:
   - `BlockValidated` from Consensus (8)
   - `MerkleRootComputed` from Transaction Indexing (3)
   - `StateRootComputed` from State Management (4)
2. Buffers components keyed by `block_hash`
3. Only writes when ALL THREE components for a block_hash are received
4. Times out incomplete assemblies after 30 seconds

**Read Path (V2.3 - Data Retrieval):**
1. Responds to `GetTransactionHashesRequest` from Transaction Indexing (3)
2. Returns transaction hashes for a given block_hash
3. Enables Merkle proof generation on cache miss

**Why Choreography, Not Orchestration (v2.2):**
The previous "Orchestrator" pattern made Consensus a bottleneck and single point of failure.
The Stateful Assembler pattern enables:
- Parallel processing (each subsystem works independently)
- Observable metrics (each component emits timing data)
- Fault isolation (one slow subsystem doesn't block Consensus)

This still prevents the "Dual Write" vulnerability because the atomic write only
occurs when all components are assembled.

### Security & Robustness:
**Attack Vectors:**
- Disk space exhaustion (blockchain bloat)
- Data corruption from hardware failures
- Malicious transactions causing crashes that halt all nodes
- Incomplete assembly timeout (component never arrives)

**Defenses:**
1. **Write-Ahead Logging (WAL)** - Survive crashes mid-write
2. **Checksums on Every Block** - Detect corruption using CRC32C
3. **Disk Space Monitoring** - Alert when 85% full, reject writes at 95%
4. **Database Versioning** - Snapshot every 10,000 blocks for recovery
5. **Atomic Writes** - Either full block write or rollback
6. **Assembly Timeout** - Drop incomplete assemblies after 30s, emit alert

**Robustness Measures:**
- Background compaction to prevent read amplification
- LRU cache for hot blocks (default 256 MB)
- Separate data/metadata storage paths
- Garbage collection of stale pending assemblies

---

## SUBSYSTEM 3: TRANSACTION INDEXING
**Purpose:** Efficiently prove transaction inclusion in blocks

### Main Algorithm: Merkle Tree (Binary Hash Tree)
**Why:** O(log n) proof size, cryptographically secure inclusion proofs

### Supporting Algorithms:
1. **SHA-256 / Keccak-256** - Cryptographic hash function for nodes
2. **Bottom-Up Tree Construction** - Build tree from leaves to root
3. **Merkle Proof Generation** - Extract sibling path from leaf to root
4. **Proof Verification** - Recompute root from leaf + proof path

### Dependencies:
- **Subsystem 10** (Signature Verification) - Validates transactions before indexing
- **Subsystem 2** (Block Storage) - Transaction hashes for proof generation (V2.3)
- **Event Bus** - Subscribes to `BlockValidated` events

### Provides To (via Event Bus):
- **Event Bus** - Emits `MerkleRootComputed` event (block_hash + merkle_root)
- **Subsystem 13** (Light Clients) - Provides Merkle proofs for SPV

**v2.3 Choreography + Data Retrieval Pattern:**

**Write Path (Choreography):**
1. Subscribes to `BlockValidated` events from Event Bus
2. Computes merkle_root for the block's transactions
3. Emits `MerkleRootComputed` event with block_hash as key
4. Block Storage assembles this with other components

**Read Path (V2.3 - Proof Generation):**
1. Receives `MerkleProofRequest` from Light Clients (13)
2. Checks local cache for transaction location
3. On cache miss: Queries Block Storage (2) via `GetTransactionHashesRequest`
4. Rebuilds Merkle tree from transaction hashes
5. Generates and returns Merkle proof

### Security & Robustness:
**Attack Vectors:**
- Second preimage attacks (find different transaction with same hash)
- Hash collision vulnerabilities if using weak algorithms like MD5 or SHA-1
- Malformed proof attacks

**Defenses:**
1. **Strong Hash Functions** - Use SHA-256 (Bitcoin) or Keccak-256 (Ethereum), avoid MD5/SHA-1
2. **Proof Size Limits** - Max depth 20 (1M transactions per block)
3. **Canonical Serialization** - Deterministic transaction encoding before hashing
4. **Duplicate Transaction Prevention** - Reject identical transaction hashes in same block
5. **Root Verification** - Always verify computed root matches stored root

**Robustness Measures:**
- Cache frequently accessed proofs
- Parallel proof generation for large blocks
- Proof compression for SPV clients

---

## SUBSYSTEM 4: STATE MANAGEMENT
**Purpose:** Store current account balances and smart contract state

### Main Algorithm: Patricia Merkle Trie (Modified Merkle Patricia Trie)
**Why:** Efficient state lookups O(log n), cryptographic proof of state, path compression

### Supporting Algorithms:
1. **Radix Tree Structure** - Path compression reduces tree depth
2. **RLP Encoding** - Recursive Length Prefix serialization for Ethereum
3. **Node Type Handling** - Branch (16 children), Extension (path compression), Leaf (value)
4. **State Root Calculation** - Hash all nodes to produce single root hash

### Dependencies:
- **Subsystem 11** (Smart Contract Execution) - Updates state after transactions
- **Event Bus** - Subscribes to `BlockValidated` events

### Provides To (via Event Bus):
- **Event Bus** - Emits `StateRootComputed` event (block_hash + state_root)

**v2.2 Choreography Pattern:**
1. Subscribes to `BlockValidated` events from Event Bus
2. Computes state_root for the block's state transitions
3. Emits `StateRootComputed` event with block_hash as key
4. Block Storage assembles this with other components

### Security & Robustness:
**Attack Vectors:**
- State bloat attacks (create many accounts/storage slots)
- Logic errors processing negative balances or incorrect values
- Trie poisoning (malicious state roots)

**Defenses:**
1. **Gas Fees for State Changes** - Charge for creating accounts/storage (EIP-2929)
2. **State Rent** - Charge ongoing fees for state storage (proposed)
3. **Balance Validation** - Reject negative balances, check overflow
4. **State Root Checkpoints** - Verify state root against consensus
5. **Pruning Old State** - Keep only recent N states (archive nodes keep all)

**Robustness Measures:**
- State snapshots every 128 blocks
- Incremental state sync for fast node startup
- Separate trie for accounts vs contract storage

### Future Scalability Considerations (V2 Architecture)

**Identified Limitation:** The current single, lock-protected state trie will become a 
serialization bottleneck at enterprise scale. All state reads and writes funnel through
a single trie structure, limiting parallelism.

**Planned V2 Solution:**
- **Sharded State Model:** Partition state into multiple independent tries based on
  address ranges (e.g., first 2 bytes of address)
- **Parallel Access:** Each shard can be read/written concurrently without locking others
- **Merkle Forest:** Root hash computed as merkle tree of shard roots
- **Cross-Shard Transactions:** Handled via atomic commit protocol across affected shards

**Migration Path:** V1 single-trie → V1.5 read-only sharding → V2 full sharded writes

---

## SUBSYSTEM 5: BLOCK PROPAGATION
**Purpose:** Distribute new blocks to all network nodes quickly

### Main Algorithm: Gossip Protocol (Epidemic Broadcast)
**Why:** O(log n) message complexity, resilient to node failures, scalable

### Supporting Algorithms:
1. **Random Peer Selection** - Choose random subset of peers (fan-out = 8)
2. **Message Deduplication** - Track seen block hashes using Bloom filter
3. **Fan-out Control** - Tune infection rate vs bandwidth
4. **Compact Block Relay** - Send block header + short transaction IDs (BIP152)

### Dependencies:
- **Subsystem 1** (Peer Discovery) - Provides list of connected peers
- **Subsystem 8** (Consensus) - Validates block before propagation

### Security & Robustness:
**Attack Vectors:**
- DDoS attacks on blockchain network and exchanges
- Selfish mining (delay block propagation)
- Eclipse attacks (isolate nodes)

**Defenses:**
1. **Rate Limiting** - Max 1 block announcement per peer per second
2. **Block Priority Queue** - Process higher difficulty blocks first
3. **Peer Reputation** - Track timely vs delayed blocks per peer
4. **Multiple Propagation Paths** - Send to both random and high-reputation peers
5. **Header-First Propagation** - Validate header before requesting full block

**Robustness Measures:**
- Retry failed sends up to 3 times
- Adaptive fan-out based on network conditions
- Compact blocks reduce bandwidth 90%

---

## SUBSYSTEM 6: TRANSACTION POOL (MEMPOOL)
**Purpose:** Queue and prioritize unconfirmed transactions

### Main Algorithm: Priority Queue (Binary Min/Max Heap)
**Why:** O(log n) insert/extract, efficient priority-based ordering

### Supporting Algorithms:
1. **Gas Price Sorting** - Order by transaction fee (higher = higher priority)
2. **Nonce Tracking** - Maintain sequential order per account
3. **LRU Eviction Policy** - Remove lowest fee transactions when full
4. **Replace-by-Fee (RBF)** - Allow higher-fee transaction to replace existing
5. **Two-Phase Commit for Transaction Removal** - Transactions only deleted upon storage confirmation

### Two-Phase Transaction Removal Protocol

**CRITICAL:** Transactions are NEVER deleted when proposed for a block. They are only 
deleted when Block Storage confirms the block was successfully written.

**Transaction States:**
```
[PENDING] ──propose for block──→ [PENDING_INCLUSION] ──storage confirmed──→ [DELETED]
                                         │
                                         └── block rejected/timeout ──→ [PENDING] (rollback)
```

**Protocol:**
1. **Proposal Phase:** When Consensus requests transactions, move them to `pending_inclusion` state
2. **Confirmation Phase:** Upon receiving `BlockStorageConfirmation`, permanently delete
3. **Rollback Phase:** If block rejected or 30-second timeout, move back to `pending`

This prevents the "Transaction Loss" vulnerability where transactions could be deleted
from mempool but the block fails to store, resulting in permanent transaction loss.

### Dependencies:
- **Subsystem 10** (Signature Verification) - Validates transactions before mempool entry
- **Subsystem 4** (State Management) - Checks account balance/nonce
- **Subsystem 2** (Block Storage) - Receives BlockStorageConfirmation for two-phase commit

### Security & Robustness:
**Attack Vectors:**
- Mempool spam (flood with low-fee transactions)
- Nonce gap attacks (block transactions with missing nonces)
- Front-running attacks by reordering transactions
- **Transaction loss from storage failures (NOW PREVENTED)**

**Defenses:**
1. **Minimum Gas Price** - Reject transactions below threshold (e.g., 1 gwei)
2. **Per-Account Limits** - Max 16 pending transactions per account
3. **Mempool Size Cap** - Max 5000 transactions (configurable)
4. **Nonce Gap Timeout** - Drop transactions with nonce gaps after 10 minutes
5. **Private Mempools** - Flashbots-style private transaction ordering
6. **Two-Phase Commit** - Never delete until storage confirmed

**Robustness Measures:**
- Periodic mempool cleanup (every 5 minutes)
- Concurrent transaction validation
- Separate pools for high/low fee transactions
- **Automatic rollback on block rejection/timeout**

---

## SUBSYSTEM 7: TRANSACTION FILTERING (SPV)
**Purpose:** Allow light clients to check transaction relevance without full blockchain

### Main Algorithm: Bloom Filter
**Why:** O(1) probabilistic membership test, compact size, false positives acceptable

### Supporting Algorithms:
1. **Multiple Hash Functions** - k=3 to 7 independent hash functions
2. **Bit Array Operations** - Set/test bits in m-bit array
3. **False Positive Rate Calculation** - FPR = (1 - e^(-kn/m))^k, tune m and k
4. **Dynamic Filter Resizing** - Adjust size based on watched addresses

### Dependencies:
- **Subsystem 3** (Transaction Indexing) - Provides transaction hashes to test
- **Subsystem 1** (Peer Discovery) - Connects to full nodes for filtered data

### Security & Robustness:
**Attack Vectors:**
- Privacy leakage (filter reveals watched addresses)
- Attackers dictionary-attack filters to uncover watched addresses
- Filter stuffing (send many false positives)

**Defenses:**
1. **Random False Positives** - Add random addresses to filter
2. **Filter Rotation** - Change filters periodically (every 100 blocks)
3. **Multiple Filters** - Use different filters with different full nodes
4. **Client-Side Filtering** - Download more than needed, filter locally
5. **Rate Limit Filter Updates** - Max 1 filter update per 10 blocks

**Robustness Measures:**
- Graceful degradation with higher FPR
- Backup full nodes if primary fails
- Filter caching to reduce recomputation

---

## SUBSYSTEM 8: CONSENSUS MECHANISM
**Purpose:** Achieve agreement on valid blocks across all nodes

### Main Algorithm: Proof of Stake (Gasper) OR PBFT
**Why:** PoS for public chains (energy efficient, finality), PBFT for private chains

### Supporting Algorithms (PoS):
1. **Stake-Weighted Randomness** - Select validators proportional to stake using VRF
2. **LMD-GHOST Fork Choice** - Choose heaviest subtree for canonical chain
3. **Attestation Aggregation** - Combine BLS signatures from validators
4. **Slashing Conditions** - Penalize double-signing and surround voting
5. **Epoch Transitions** - Rotate validator committees every 32 slots (6.4 minutes)

### Supporting Algorithms (PBFT):
1. **Pre-Prepare Phase** - Leader broadcasts block proposal
2. **Prepare Phase** - Nodes broadcast agreement, wait for 2f+1 prepares
3. **Commit Phase** - Nodes broadcast commit, execute after 2f+1 commits
4. **View Change** - Leader rotation if timeout (Byzantine leader detected)

### Dependencies:
- **Subsystem 5** (Block Propagation) - Receives blocks from network
- **Subsystem 6** (Mempool) - Source of transactions for block building
- **Subsystem 10** (Signature Verification) - Validates block signatures

### Role (v2.2 UPDATED - Validation Only, NOT Orchestration):
Consensus performs **validation only**. It does NOT orchestrate block storage writes.

**v2.2 Change:** The "Orchestrator" anti-pattern was rejected because it created:
- Single point of failure
- Performance bottleneck
- Hidden latency sources
- Complex retry logic ("god object")

**Current Responsibility:**
1. Validates block cryptographically
2. Emits `BlockValidated` event to the bus
3. Other subsystems react independently (choreography pattern)

### Provides To (via Event Bus):
- **Event Bus** - Emits `BlockValidated` event (block + consensus proof)
- **Subsystem 9** (Finality) - Validated blocks for finalization checking

### Security & Robustness:
**Attack Vectors:**
- 51% attack where single entity dominates staking or computational power
- Long-range attack forking and altering chain history
- Nothing-at-stake problem (validators sign multiple chains)
- Sybil attacks where attacker controls many dishonest nodes in PBFT

**Defenses (PoS):**
1. **Slashing** - Burn 1 ETH minimum for provable misbehavior
2. **Weak Subjectivity Checkpoints** - Nodes must sync from recent checkpoint
3. **Finality Gadget (Casper FFG)** - Economic finality after 2 epochs
4. **Validator Set Limits** - Cap individual stake at 5% total
5. **Inactivity Leak** - Penalize offline validators during chain split
6. **Zero-Trust Signature Verification** - Independently re-verify all critical signatures; does not trust the `valid` flag from Signature Verification subsystem

**Defenses (PBFT):**
1. **Node Authentication** - PKI-based identity verification
2. **Message Signing** - Every message signed to prevent tampering
3. **Byzantine Fault Tolerance** - Tolerate f Byzantine nodes where n > 3f
4. **View Change Timeout** - Replace leader after 30 seconds of inactivity
5. **Cryptographic Commitments** - Bind nodes to their votes
6. **Zero-Trust Signature Verification** - All validator signatures re-verified independently

**Robustness Measures:**
- PBFT handles high throughput but struggles with scalability as nodes increase
- Validator diversity (geographic, client, hardware)
- Parallel block proposal for higher throughput
- Graceful degradation under 33% faults (PoS) or 33% Byzantine (PBFT)

---

## SUBSYSTEM 9: FINALITY MECHANISM
**Purpose:** Guarantee transactions won't be reverted (economic finality)

### Main Algorithm: Casper FFG (Friendly Finality Gadget)
**Why:** Provides explicit finality, unlike probabilistic PoW finality

### Supporting Algorithms:
1. **Checkpoint System** - Mark epoch boundaries (every 32 blocks)
2. **Justification Logic** - 2/3+ validators attest to checkpoint
3. **Finalization Logic** - Two consecutive justified checkpoints finalize first
4. **Accountable Safety** - Slashing for provable equivocations

### Dependencies:
- **Subsystem 8** (Consensus) - Uses PoS attestations for finality votes
- **Subsystem 10** (Signature Verification) - Validates finality signatures

### Security & Robustness:
**Attack Vectors:**
- Finality reversion (fork after finalized block)
- Validators refusing to finalize due to censorship
- Supermajority collusion
- **Livelock from repeated sync failures (NOW PREVENTED)**

**Defenses:**
1. **Slashing for Double Finality** - Burn all stake if validator finalizes conflicting chains
2. **Minimum Deposit** - 32 ETH to become validator (skin in the game)
3. **Inactivity Leak** - Bleed stake from offline validators during no-finality
4. **Social Consensus** - Community coordination for extreme scenarios
5. **Fork Choice Rule** - Always follow finalized chain
6. **Zero-Trust Signature Verification** - Independently re-verify all attestation signatures; does not trust pre-validation flags from Signature Verification subsystem
7. **Circuit Breaker with Livelock Prevention** - Halts node after 3 failed sync attempts to await manual intervention or network recovery

### Circuit Breaker Behavior (See Architecture.md Section 5.4):
- If finality cannot be achieved, node enters SYNC mode
- If sync fails 3 consecutive times, node enters HALTED_AWAITING_INTERVENTION
- In HALTED state, node ceases block production/validation
- Requires manual intervention OR significant network recovery to resume

**Robustness Measures:**
- Finality delay up to 2 epochs acceptable
- Graceful recovery from <33% participation
- Automatic fork pruning of non-finalized chains
- **Livelock prevention via circuit breaker**

---

## SUBSYSTEM 10: SIGNATURE VERIFICATION
**Purpose:** Verify transaction authenticity using cryptographic signatures

### Main Algorithm: ECDSA (Elliptic Curve Digital Signature Algorithm)
**Why:** Industry standard, 256-bit security with 64-byte signatures

### Supporting Algorithms:
1. **secp256k1 Curve** - Bitcoin/Ethereum curve parameters
2. **Signature Generation** - Sign(message, private_key) → (r, s, v)
3. **Public Key Recovery** - Recover public key from signature + message
4. **Batch Verification (Schnorr)** - Verify n signatures in one operation

### Dependencies:
- **None** (Pure cryptographic operation)

### Security & Robustness:
**Attack Vectors:**
- Private key prediction through brute-force or dictionary attacks
- Insufficient entropy in key generation causing duplicate keys
- Length extension attacks on hash functions

**Defenses:**
1. **CSPRNG for Key Generation** - Use cryptographically secure random number generator
2. **RFC6979 Deterministic Nonces** - Prevent nonce reuse attacks
3. **Public Key Hashing** - Use addresses (hash of pubkey), not raw pubkeys
4. **Multi-Signature Wallets** - Require m-of-n signatures (e.g., 2-of-3)
5. **Hardware Security Modules** - Store keys in tamper-proof hardware

**Robustness Measures:**
- Signature malleability prevention (EIP-2)
- Batch verification for throughput (100x faster)
- Pre-computed tables for faster verification

---

## SUBSYSTEM 11: SMART CONTRACT EXECUTION
**Purpose:** Execute deterministic code for programmable transactions

### Main Algorithm: Virtual Machine (EVM / WASM)
**Why:** Sandboxed execution, Turing-complete, deterministic

### Supporting Algorithms:
1. **Stack Machine Operations** - PUSH, POP, DUP, SWAP, arithmetic, logic
2. **Gas Metering** - Charge gas per opcode to prevent infinite loops
3. **Memory Management** - Expandable byte array with quadratic cost
4. **Storage Operations** - Persistent key-value store, SLOAD/SSTORE
5. **Message Calls** - CALL, DELEGATECALL, STATICCALL between contracts

### Dependencies:
- **Subsystem 4** (State Management) - Reads/writes contract storage
- **Subsystem 10** (Signature Verification) - Verifies transaction sender

### Security & Robustness:
**Attack Vectors:**
- Reentrancy attacks where contract calls untrusted external contract
- Integer overflow vulnerabilities in Solidity code
- Gas griefing (out-of-gas in sub-call)
- Unverified return value vulnerabilities

**Defenses:**
1. **Checks-Effects-Interactions Pattern** - Update state before external calls
2. **Reentrancy Guards** - Mutex locks on sensitive functions
3. **SafeMath Libraries** - Overflow/underflow protection (Solidity 0.8+ built-in)
4. **Gas Stipends** - Limit forwarded gas (2300 for transfers)
5. **Static Analysis Tools** - Slither, Mythril for vulnerability detection
6. **Formal Verification** - Mathematical proof of correctness

**Robustness Measures:**
- Execution timeout (block gas limit)
- Memory/storage limits per contract
- Precompiled contracts for expensive operations (ecrecover, sha256)

---

## SUBSYSTEM 12: TRANSACTION ORDERING (DAG-based)
**Purpose:** Order transactions correctly for parallel execution in DAG chains

### Main Algorithm: Topological Sort (Kahn's Algorithm)
**Why:** O(V + E) complexity, detects cycles, enables parallelism

### Supporting Algorithms:
1. **Dependency Graph Construction** - Build directed graph from transaction reads/writes
2. **Zero In-Degree Selection** - Pick transactions with no dependencies
3. **Conflict Resolution** - Determine order for conflicting transactions (timestamp, fee)
4. **Parallel Execution Scheduling** - Assign independent transactions to threads

### Dependencies:
- **Subsystem 11** (Smart Contract Execution) - Executes ordered transactions
- **Subsystem 4** (State Management) - Detects read/write conflicts

### Security & Robustness:
**Attack Vectors:**
- Dependency manipulation (create false dependencies)
- Front-running through transaction reordering
- Cycle creation (deadlock)

**Defenses:**
1. **Automatic Conflict Detection** - Analyze storage access patterns
2. **Deterministic Ordering** - Timestamp + hash for tie-breaking
3. **Cycle Detection** - Reject transactions creating cycles
4. **Execution Isolation** - Each thread operates on separate state fork
5. **Rollback on Conflict** - Re-execute conflicting transactions sequentially

**Robustness Measures:**
- Fallback to sequential execution if conflicts exceed threshold
- Speculative execution with rollback
- Parallel validation for independent transactions

---

## SUBSYSTEM 13: LIGHT CLIENT SYNC
**Purpose:** Verify blockchain without downloading full chain data

### Main Algorithm: SPV (Simplified Payment Verification)
**Why:** Download only headers (~80 bytes/block), verify via Merkle proofs

### Supporting Algorithms:
1. **Header Chain Sync** - Download and verify block headers only
2. **Merkle Proof Verification** - Validate transaction inclusion using proofs
3. **Bloom Filter Setup** - Request filtered transactions matching addresses
4. **Checkpoint Verification** - Trust recent hardcoded checkpoints

### Dependencies:
- **Subsystem 1** (Peer Discovery) - Connect to full nodes
- **Subsystem 3** (Transaction Indexing) - Provides Merkle proofs
- **Subsystem 7** (Bloom Filters) - Filters relevant transactions

### Security & Robustness:
**Attack Vectors:**
- Malicious full nodes lying about chain state
- Eclipse attacks (connect only to attacker nodes)
- Invalid Merkle proofs

**Defenses:**
1. **Multiple Full Node Connections** - Query 3+ independent nodes
2. **Proof Verification** - Validate every Merkle proof cryptographically
3. **Checkpoint System** - Trust checkpoints from multiple sources
4. **Fraud Proofs** - Full nodes can prove invalid state transitions
5. **Random Peer Selection** - Avoid relying on single full node

**Robustness Measures:**
- Header download parallelization
- Graceful fallback to full sync if SPV fails
- Local proof caching

### Future Scalability Considerations (V2 Architecture)

**Identified Limitation:** The current SPV model requires a number of Merkle proofs 
proportional to transaction volume. As the chain grows and users monitor more addresses,
the proof overhead becomes unsustainable for mobile and IoT clients.

**Planned V2 Solution:**
- **Proof Aggregation:** Use cryptographic accumulators (Utreexo-style) to achieve O(log n) 
  or even O(1) proof sizes regardless of transaction volume
- **ZK-SNARK Proofs:** State validity proofs that compress entire block verification into 
  a single constant-size proof (~200 bytes)
- **Stateless Verification:** Clients verify proofs without maintaining any local state
- **Incremental Updates:** Proof updates are small and efficient

**Migration Path:** V1 SPV → V1.5 Utreexo accumulators → V2 ZK-validity proofs

---

## SUBSYSTEM 14: SHARDING (ADVANCED)
**Purpose:** Split blockchain state across multiple shards for horizontal scaling

### Main Algorithm: Consistent Hashing (Rendezvous Hashing)
**Why:** Minimal data movement on shard addition/removal, load balancing

### Supporting Algorithms:
1. **Shard Assignment** - Hash(account) % num_shards determines shard
2. **Cross-Shard Transactions** - Two-phase commit protocol
3. **Beacon Chain Coordination** - Central chain manages shard metadata
4. **Validator Rotation** - Random shuffling prevents shard takeover

### Dependencies:
- **Subsystem 8** (Consensus) - Each shard runs own consensus
- **Subsystem 4** (State Management) - Partitioned state across shards

### Security & Robustness:
**Attack Vectors:**
- Shard takeover (1% attack, not 51%)
- Cross-shard fraud
- Data availability attacks hiding shard data

**Defenses:**
1. **Validator Shuffling** - Random rotation every epoch
2. **Cross-Links** - Beacon chain validates shard block headers
3. **Data Availability Sampling** - Random chunk verification
4. **Fraud Proofs** - Challenge invalid cross-shard transactions
5. **Minimum Shard Size** - Require 128+ validators per shard

**Robustness Measures:**
- Shard rebalancing on load imbalance
- Emergency shard merging if validator count drops
- Redundant shard storage (erasure coding)

### Future Scalability Considerations (V2 Architecture)

**Identified Limitation:** The current conceptual Two-Phase Commit (2PC) protocol for 
cross-shard transactions is synchronous and creates significant latency. As shard count
increases, 2PC becomes a coordination bottleneck that limits throughput.

**Planned V2 Solution:**
- **Asynchronous Receipt-Based Protocol:** Cross-shard transactions are non-blocking
  - Source shard commits transaction and emits a receipt
  - Receipt is relayed to destination shard asynchronously
  - Destination shard processes receipt in next block
- **Fraud Proofs:** If a receipt is invalid, anyone can submit a fraud proof to slash
  the malicious validator
- **Optimistic Execution:** Destination assumes receipt is valid, executes immediately
- **Challenge Period:** Short window for fraud proof submission before finality

**Migration Path:** V1 synchronous 2PC → V1.5 batched 2PC → V2 async receipts + fraud proofs

**Expected Improvement:** 10x throughput increase for cross-shard transactions

---

## SUBSYSTEM 15: CROSS-CHAIN COMMUNICATION
**Purpose:** Enable asset transfers between independent blockchains

### Main Algorithm: HTLC (Hash Time-Locked Contracts)
**Why:** Trustless atomic swaps, no third-party escrow

### Supporting Algorithms:
1. **Hashlock Creation** - Alice generates secret S, shares H(S) with Bob
2. **Timelock Setup** - Set refund deadlines (24 hours typical)
3. **Atomic Swap Protocol** - Either both chains execute or both refund
4. **Secret Reveal** - Alice claims Bob's chain by revealing S

### Dependencies:
- **Subsystem 11** (Smart Contracts) - Implements HTLC logic on each chain
- **Subsystem 8** (Consensus) - Finalizes transactions on both chains

### Security & Robustness:
**Attack Vectors:**
- Timing attacks (claim on one chain, let other timeout)
- Hash collision (find S' where H(S) = H(S'))
- Relay censorship (prevent secret reveal transaction)

**Defenses:**
1. **Timelock Margins** - Chain A timeout > Chain B timeout + 6 hours
2. **Strong Hash Functions** - Use SHA-256, avoid weak hashes
3. **Monitoring Services** - Watch for timeouts, auto-refund
4. **Multiple Relay Paths** - Redundant transaction submission
5. **Proof of Secret Reveal** - Incentivize relayers to publish secret

**Robustness Measures:**
- Grace period before timelock expiration
- Automatic refund execution
- Cross-chain state verification

---

## DEPENDENCY GRAPH (V2.3 - UNIFIED WORKFLOW)

**CRITICAL UPDATE (V2.3 - Choreography + Data Retrieval):**
The dependency graph has been updated to reflect the V2.2 Choreography Pattern AND
the V2.3 Data Retrieval Path. Block Storage (Subsystem 2) now:
1. Subscribes to events from Subsystems 3, 4, 8 (Choreography - Write Path)
2. Provides transaction data to Subsystem 3 (Data Retrieval - Read Path)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                   V2.3 UNIFIED DEPENDENCY GRAPH                              │
├──────────────────────────────────────────────────────────────────────────────┤

SUBSYSTEM 1: Peer Discovery
    ├── Depends on: Subsystem 10 (DDoS defense - verify node identity at edge)
    └── Provides to: Subsystems 5, 7, 13 (peer lists)

SUBSYSTEM 2: Block Storage (Stateful Assembler + Data Provider)
    ├── Subscribes to: Subsystem 8 (BlockValidated event)
    ├── Subscribes to: Subsystem 3 (MerkleRootComputed event)
    ├── Subscribes to: Subsystem 4 (StateRootComputed event)
    ├── Depends on: Subsystem 9 (Finality marking)
    ├── Provides to: Subsystem 6 (BlockStorageConfirmation)
    └── Provides to: Subsystem 3 (Transaction hashes for proof generation) [V2.3]

SUBSYSTEM 3: Transaction Indexing
    ├── Depends on: Subsystem 10 (Signature verification)
    ├── Subscribes to: Subsystem 8 (BlockValidated event)
    ├── Provides to: Event Bus (MerkleRootComputed)
    ├── Provides to: Subsystem 13 (Merkle proofs)
    └── Depends on: Subsystem 2 (Transaction hashes for proof generation) [V2.3]

SUBSYSTEM 4: State Management
    ├── Depends on: Subsystem 11 (State updates)
    ├── Subscribes to: Subsystem 8 (BlockValidated event)
    └── Provides to: Event Bus (StateRootComputed)

SUBSYSTEM 5: Block Propagation
    ├── Depends on: Subsystem 1 (Peer list)
    └── Depends on: Subsystem 8 (Block validation)

SUBSYSTEM 6: Mempool
    ├── Depends on: Subsystem 10 (Signature check)
    ├── Depends on: Subsystem 4 (Balance/nonce check)
    ├── Provides to: Subsystem 8 (ProposeTransactionBatch)
    └── Subscribes to: Subsystem 2 (BlockStorageConfirmation)

SUBSYSTEM 7: Bloom Filters
    ├── Depends on: Subsystem 3 (Transaction hashes)
    └── Depends on: Subsystem 1 (Full node connections)

SUBSYSTEM 8: Consensus (Validation Only - NOT Orchestrator)
    ├── Depends on: Subsystem 5 (Receive blocks from network)
    ├── Depends on: Subsystem 6 (ProposeTransactionBatch)
    ├── Depends on: Subsystem 10 (Signature verification)
    ├── Provides to: Event Bus (BlockValidated)
    └── Provides to: Subsystem 9 (Attestations)

SUBSYSTEM 9: Finality
    ├── Depends on: Subsystem 8 (PoS attestations)
    ├── Depends on: Subsystem 10 (Signature verification)
    ├── Provides to: Subsystem 2 (MarkFinalizedRequest)
    └── NOTE: Uses circuit breaker on failure, not retries

SUBSYSTEM 10: Signature Verification
    └── No Dependencies (Pure crypto)

SUBSYSTEM 11: Smart Contracts
    ├── Depends on: Subsystem 4 (State read/write)
    └── Depends on: Subsystem 10 (Sender verification)

SUBSYSTEM 12: Transaction Ordering
    ├── Depends on: Subsystem 11 (Execution)
    └── Depends on: Subsystem 4 (Conflict detection)

SUBSYSTEM 13: Light Clients
    ├── Depends on: Subsystem 1 (Full node connections)
    ├── Depends on: Subsystem 3 (Merkle proofs)
    └── Depends on: Subsystem 7 (Bloom filters)

SUBSYSTEM 14: Sharding
    ├── Depends on: Subsystem 8 (Per-shard consensus)
    └── Depends on: Subsystem 4 (Partitioned state)

SUBSYSTEM 15: Cross-Chain
    ├── Depends on: Subsystem 11 (HTLC contracts)
    └── Depends on: Subsystem 8 (Finality on both chains)

SUBSYSTEM 16: API Gateway (External Interface)
    ├── Depends on: Subsystem 1 (Peer info queries)
    ├── Depends on: Subsystem 2 (Block queries)
    ├── Depends on: Subsystem 3 (Transaction/receipt queries)
    ├── Depends on: Subsystem 4 (State queries - eth_getBalance)
    ├── Depends on: Subsystem 6 (Transaction submission)
    ├── Depends on: Subsystem 10 (Transaction signature validation)
    ├── Depends on: Subsystem 11 (eth_call, estimateGas)
    ├── Subscribes to: Event Bus (WebSocket subscriptions)
    └── Exposed to: External HTTP/WS clients (wallets, dApps, explorers)
```

**V2.3 Data Flow Diagrams:**

**1. Block Creation (Choreography - Write Path):**
```
Consensus (8) ─────BlockValidated──────→ [Event Bus]
                                              │
                    ┌─────────────────────────┼─────────────────────────┐
                    ↓                         ↓                         ↓
           Tx Indexing (3)           State Management (4)      Block Storage (2)
                    │                         │                  [Stateful Assembler]
                    ↓                         ↓                         ↑
         MerkleRootComputed          StateRootComputed                  │
                    └─────────────────────────┴─────────────────────────┘
                                              │
                                              ↓
                                   [All 3 components received]
                                              │
                                              ↓
                                   Block Storage: Atomic Write
```

**2. Proof Generation (V2.3 - Read Path):**
```
Light Client (13) ──MerkleProofRequest──→ Transaction Indexing (3)
                                                    │
                                          [Check local cache]
                                                    │
                              ┌─────────────────────┴─────────────────────┐
                              ↓                                           ↓
                        [Cache Hit]                                 [Cache Miss]
                              │                                           │
                              ↓                                           ↓
                    [Generate Proof]              GetTransactionHashesRequest
                              │                            │              ↓
                              │                            └───→ Block Storage (2)
                              │                                           │
                              │                    TransactionHashesResponse
                              │                                           │
                              │                            ←──────────────┘
                              │                                           │
                              ↓                                           ↓
                     [Return Proof]                          [Rebuild Merkle Tree]
                                                                          │
                                                                          ↓
                                                               [Generate Proof]
                                                                          │
                                                                          ↓
                                                                 [Return Proof]
```

**3. External API Request Flow (V2.4 - API Gateway):**
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      EXTERNAL API REQUEST FLOW                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [External Client] (MetaMask, dApp, CLI)                                    │
│         │                                                                   │
│         │ eth_sendRawTransaction / eth_getBalance / eth_call                │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │ API Gateway (16)                                                  │      │
│  │ ├─ Rate Limiting (Tower middleware)                              │      │
│  │ ├─ Request Validation                                            │      │
│  │ ├─ Method Whitelist Check                                        │      │
│  │ └─ Route to Internal Subsystem                                   │      │
│  └──────────────────────────────────────────────────────────────────┘      │
│         │                                                                   │
│    ┌────┴────┬────────────┬────────────┬────────────┐                      │
│    ▼         ▼            ▼            ▼            ▼                      │
│ [qc-04]  [qc-06]      [qc-02]      [qc-03]      [qc-11]                    │
│ State    Mempool      Storage      TxIndex      Contracts                  │
│ (balance) (submit)   (blocks)     (receipts)   (eth_call)                  │
│    │         │            │            │            │                      │
│    └────┬────┴────────────┴────────────┴────────────┘                      │
│         ▼                                                                   │
│  [API Gateway (16)] ← Response                                              │
│         │                                                                   │
│         ▼                                                                   │
│  [External Client] ← JSON-RPC Response                                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## IMPLEMENTATION PRIORITY

**Phase 1 (Core - Weeks 1-4):**
- Subsystem 10 (Signatures) - No dependencies
- Subsystem 1 (Peer Discovery) - Depends on 10 for DDoS defense
- Subsystem 3 (Merkle Trees) - Needs 10, and 2 for proof generation (V2.3)
- Subsystem 6 (Mempool) - Needs 10, 4

**Phase 2 (Consensus - Weeks 5-8):**
- Subsystem 4 (State) - Needs 11
- Subsystem 8 (Consensus) - Publishes BlockValidated, needs 5, 6, 10
- Subsystem 2 (Storage) - Stateful Assembler, subscribes to 3, 4, 8
- Subsystem 5 (Propagation) - Needs 1, 8
- Subsystem 9 (Finality) - Needs 8, 10

**Phase 3 (Advanced - Weeks 9-12):**
- Subsystem 11 (Smart Contracts) - Needs 4, 10
- Subsystem 7 (Bloom Filters) - Needs 3, 1
- Subsystem 13 (Light Clients) - Needs 1, 3, 7
- Subsystem 12 (Ordering) - Optional, needs 11, 4

**Phase 4 (External Interface - Weeks 13-14):**
- Subsystem 16 (API Gateway) - Needs 1, 2, 3, 4, 6, 10, 11 (Axum + Tower + jsonrpsee)
- LGTM Telemetry Integration (quantum-telemetry crate)

**Phase 5 (Optional Scaling - Weeks 15+):**
- Subsystem 14 (Sharding) - Needs 8, 4
- Subsystem 15 (Cross-Chain) - Needs 11, 8