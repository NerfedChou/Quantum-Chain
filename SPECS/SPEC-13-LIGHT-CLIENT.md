# SPECIFICATION: LIGHT CLIENT SYNC

**Version:** 2.3  
**Subsystem ID:** 13  
**Bounded Context:** SPV & Mobile Clients  
**Crate Name:** `crates/light-client`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Light Client Sync** subsystem enables verification of blockchain state without downloading the full chain. It implements SPV (Simplified Payment Verification) using block headers, Merkle proofs, and Bloom filters.

### 1.2 Responsibility Boundaries

**In Scope:**
- Download and verify block headers
- Request and verify Merkle proofs for transactions
- Use Bloom filters for transaction filtering
- Connect to multiple full nodes for security
- Maintain local header chain

**Out of Scope:**
- Full block validation (Subsystem 8)
- Block storage (Subsystem 2)
- Merkle proof generation (Subsystem 3)
- Bloom filter construction (Subsystem 7)

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED:                                                       │
│  ├─ Merkle proofs from Subsystem 3 (cryptographically verified) │
│  └─ Bloom filters from Subsystem 7                              │
│                                                                 │
│  UNTRUSTED:                                                     │
│  └─ Full node responses (verify with multiple sources)          │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Light client header chain
#[derive(Debug)]
pub struct HeaderChain {
    /// Headers indexed by hash
    pub headers: HashMap<Hash, BlockHeader>,
    /// Headers indexed by height
    pub by_height: BTreeMap<u64, Hash>,
    /// Current chain tip
    pub tip: Hash,
    /// Tip height
    pub height: u64,
    /// Trusted checkpoints
    pub checkpoints: Vec<Checkpoint>,
}

/// Trusted checkpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub height: u64,
    pub hash: Hash,
    pub source: CheckpointSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CheckpointSource {
    Hardcoded,
    MultiNodeConsensus { node_count: usize },
    External { url: String },
}

/// Transaction with SPV proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenTransaction {
    pub transaction: Transaction,
    pub block_hash: Hash,
    pub block_height: u64,
    pub merkle_proof: MerkleProof,
    pub confirmations: u64,
}

/// Light client configuration
#[derive(Clone, Debug)]
pub struct LightClientConfig {
    /// Minimum full nodes to query
    pub min_full_nodes: usize,
    /// Maximum headers to sync per request
    pub max_headers_per_request: usize,
    /// Required confirmations for transactions
    pub required_confirmations: u64,
    /// Enable Bloom filter privacy mode
    pub privacy_mode: bool,
    /// Checkpoint verification
    pub verify_checkpoints: bool,
}

impl Default for LightClientConfig {
    fn default() -> Self {
        Self {
            min_full_nodes: 3,
            max_headers_per_request: 2000,
            required_confirmations: 6,
            privacy_mode: true,
            verify_checkpoints: true,
        }
    }
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Proof Verification
/// All Merkle proofs must be cryptographically verified.
fn invariant_proof_verified(proof: &MerkleProof, header: &BlockHeader) -> bool {
    verify_merkle_proof(
        &proof.transaction_hash,
        &proof.proof_path,
        &header.transactions_root.unwrap(),
    )
}

/// INVARIANT-2: Multi-Node Consensus
/// Critical data must be verified by multiple nodes.
fn invariant_multi_node(responses: &[FullNodeResponse], config: &LightClientConfig) -> bool {
    let agreeing = responses.iter()
        .filter(|r| r.data == responses[0].data)
        .count();
    agreeing >= config.min_full_nodes
}

/// INVARIANT-3: Checkpoint Chain
/// Header chain must include trusted checkpoints.
fn invariant_checkpoint_chain(chain: &HeaderChain) -> bool {
    chain.checkpoints.iter().all(|cp| {
        chain.headers.get(&cp.hash)
            .map(|h| h.block_height == cp.height)
            .unwrap_or(false)
    })
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Light Client API
#[async_trait]
pub trait LightClientApi: Send + Sync {
    /// Sync headers from network
    async fn sync_headers(&mut self) -> Result<SyncResult, LightClientError>;
    
    /// Get transaction with proof
    async fn get_proven_transaction(
        &self,
        tx_hash: Hash,
    ) -> Result<ProvenTransaction, LightClientError>;
    
    /// Verify transaction inclusion
    async fn verify_transaction(
        &self,
        tx_hash: Hash,
        block_hash: Hash,
    ) -> Result<bool, LightClientError>;
    
    /// Get transactions for addresses using Bloom filter
    async fn get_filtered_transactions(
        &self,
        addresses: &[Address],
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<ProvenTransaction>, LightClientError>;
    
    /// Get current chain tip
    fn get_chain_tip(&self) -> ChainTip;
    
    /// Check if synced
    fn is_synced(&self) -> bool;
}

/// Sync result
#[derive(Clone, Debug)]
pub struct SyncResult {
    pub headers_synced: u64,
    pub new_tip_height: u64,
    pub new_tip_hash: Hash,
    pub sync_time_ms: u64,
}

/// Chain tip info
#[derive(Clone, Debug)]
pub struct ChainTip {
    pub hash: Hash,
    pub height: u64,
    pub timestamp: u64,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Full node connection
#[async_trait]
pub trait FullNodeConnection: Send + Sync {
    /// Get block headers
    async fn get_headers(
        &self,
        from_hash: Hash,
        count: usize,
    ) -> Result<Vec<BlockHeader>, NetworkError>;
    
    /// Get Merkle proof for transaction
    async fn get_merkle_proof(
        &self,
        tx_hash: Hash,
    ) -> Result<MerkleProofResponse, NetworkError>;
    
    /// Get filtered transactions
    async fn get_filtered_transactions(
        &self,
        filter: &BloomFilter,
        from_height: u64,
        to_height: u64,
    ) -> Result<Vec<Transaction>, NetworkError>;
}

/// Peer discovery (uses Subsystem 1)
#[async_trait]
pub trait PeerDiscovery: Send + Sync {
    /// Get full node peers
    async fn get_full_nodes(&self, count: usize) -> Result<Vec<PeerInfo>, NetworkError>;
}

/// Merkle proof provider (uses Subsystem 3)
#[async_trait]
pub trait MerkleProofProvider: Send + Sync {
    /// Request Merkle proof
    async fn request_proof(
        &self,
        tx_hash: Hash,
    ) -> Result<MerkleProof, ProofError>;
}

/// Bloom filter provider (uses Subsystem 7)
#[async_trait]
pub trait BloomFilterProvider: Send + Sync {
    /// Create Bloom filter for addresses
    fn create_filter(&self, addresses: &[Address]) -> BloomFilter;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Messages

```rust
/// Merkle proof request (to Subsystem 3)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProofRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub transaction_hash: Hash,
}

/// Merkle proof response (from Subsystem 3)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProofResponse {
    pub correlation_id: CorrelationId,
    pub transaction_hash: Hash,
    pub found: bool,
    pub block_hash: Option<Hash>,
    pub block_height: Option<u64>,
    pub proof: Option<MerkleProof>,
}

/// Peer list request (to Subsystem 1)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetFullNodesRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub count: usize,
}

/// Filtered transactions request (to Subsystem 7)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredTransactionsRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub filter: BloomFilter,
    pub from_height: u64,
    pub to_height: u64,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === Proof Verification Tests ===
    
    #[test]
    fn test_merkle_proof_verification_valid() {
        let header = create_test_header_with_txs(10);
        let tx = &header.transactions[5];
        let proof = generate_merkle_proof(&header, &tx.hash());
        
        let valid = verify_merkle_proof(
            &tx.hash(),
            &proof.proof_path,
            &header.transactions_root.unwrap(),
        );
        
        assert!(valid);
    }
    
    #[test]
    fn test_merkle_proof_verification_invalid() {
        let header = create_test_header_with_txs(10);
        let tx = &header.transactions[5];
        let mut proof = generate_merkle_proof(&header, &tx.hash());
        
        // Tamper with proof
        proof.proof_path[0][0] ^= 0xFF;
        
        let valid = verify_merkle_proof(
            &tx.hash(),
            &proof.proof_path,
            &header.transactions_root.unwrap(),
        );
        
        assert!(!valid);
    }
    
    // === Header Chain Tests ===
    
    #[test]
    fn test_header_chain_append() {
        let mut chain = HeaderChain::new();
        let headers = create_header_chain(100);
        
        for header in headers {
            chain.append(header.clone()).unwrap();
        }
        
        assert_eq!(chain.height, 99);
    }
    
    #[test]
    fn test_header_chain_fork_handling() {
        let mut chain = HeaderChain::new();
        let main_chain = create_header_chain(100);
        
        for header in &main_chain[..50] {
            chain.append(header.clone()).unwrap();
        }
        
        // Create fork at height 40
        let fork_chain = create_fork_at(&main_chain, 40, 15);
        
        for header in fork_chain {
            chain.append(header).unwrap();
        }
        
        // Fork should not be main chain (shorter)
        assert_eq!(chain.height, 49);
    }
    
    // === Multi-Node Consensus Tests ===
    
    #[test]
    fn test_multi_node_consensus_agreement() {
        let config = LightClientConfig {
            min_full_nodes: 3,
            ..Default::default()
        };
        
        let responses = vec![
            create_node_response([0xAB; 32]),
            create_node_response([0xAB; 32]),
            create_node_response([0xAB; 32]),
        ];
        
        assert!(check_multi_node_consensus(&responses, &config));
    }
    
    #[test]
    fn test_multi_node_consensus_disagreement() {
        let config = LightClientConfig {
            min_full_nodes: 3,
            ..Default::default()
        };
        
        let responses = vec![
            create_node_response([0xAB; 32]),
            create_node_response([0xCD; 32]),  // Different!
            create_node_response([0xAB; 32]),
        ];
        
        assert!(!check_multi_node_consensus(&responses, &config));
    }
    
    // === Checkpoint Tests ===
    
    #[test]
    fn test_checkpoint_verification() {
        let mut chain = HeaderChain::new();
        chain.add_checkpoint(Checkpoint {
            height: 50,
            hash: [0xAB; 32],
            source: CheckpointSource::Hardcoded,
        });
        
        // Try to sync headers that don't include checkpoint
        let headers = create_header_chain(100);  // Different hashes
        
        let result = chain.verify_against_checkpoints(&headers);
        assert!(result.is_err());
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_sync_headers_from_network() {
        let (peer_discovery, _) = create_mock_peer_discovery(5);
        let (full_nodes, _) = create_mock_full_nodes(5);
        let mut client = LightClientService::new(peer_discovery, full_nodes);
        
        let result = client.sync_headers().await.unwrap();
        
        assert!(result.headers_synced > 0);
        assert!(client.is_synced());
    }
    
    #[tokio::test]
    async fn test_get_proven_transaction() {
        let (proof_provider, _) = create_mock_proof_provider();
        let client = create_synced_light_client(proof_provider);
        
        let tx_hash = [0xAB; 32];
        let proven = client.get_proven_transaction(tx_hash).await.unwrap();
        
        assert_eq!(proven.transaction.hash(), tx_hash);
        assert!(proven.confirmations >= 1);
        
        // Verify the proof
        let header = client.get_header(proven.block_hash).unwrap();
        assert!(verify_merkle_proof(
            &tx_hash,
            &proven.merkle_proof.proof_path,
            &header.transactions_root.unwrap(),
        ));
    }
    
    #[tokio::test]
    async fn test_filtered_transactions() {
        let (bloom_provider, _) = create_mock_bloom_provider();
        let client = create_synced_light_client(bloom_provider);
        
        let my_addresses = vec![ALICE, BOB];
        let txs = client.get_filtered_transactions(&my_addresses, 0, 100).await.unwrap();
        
        // All returned transactions should be relevant
        for tx in &txs {
            assert!(tx.transaction.involves(&my_addresses));
        }
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum LightClientError {
    #[error("Not enough full nodes: {got} < {required}")]
    InsufficientNodes { got: usize, required: usize },
    
    #[error("Multi-node consensus failed")]
    ConsensusFailed,
    
    #[error("Merkle proof verification failed")]
    InvalidProof,
    
    #[error("Transaction not found: {0:?}")]
    TransactionNotFound(Hash),
    
    #[error("Checkpoint mismatch at height {height}")]
    CheckpointMismatch { height: u64 },
    
    #[error("Header chain fork detected")]
    ForkDetected,
    
    #[error("Network error: {0}")]
    NetworkError(#[from] NetworkError),
}
```

---

## 7. CONFIGURATION

```toml
[light_client]
min_full_nodes = 3
max_headers_per_request = 2000
required_confirmations = 6
privacy_mode = true
verify_checkpoints = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 13

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Light Client (13) | Subsystem 1 (Peer Discovery) | Query | Full node discovery | System.md Subsystem 13 |
| Light Client (13) | Subsystem 3 (Tx Indexing) | Query | Merkle proofs | System.md Subsystem 13 |
| Light Client (13) | Subsystem 7 (Bloom Filters) | Query | Filtered transactions | System.md Subsystem 13 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 13 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| Internal only | Light clients run on mobile/desktop devices | IPC-MATRIX.md Subsystem 13 |

### B.2 Multi-Node Consensus

**Reference:** System.md, Subsystem 13 Security Defenses

Light clients do NOT trust any single full node. They query multiple nodes and require consensus:

```rust
/// Multi-node consensus for light client security
/// 
/// Reference: System.md, Subsystem 13
async fn verify_with_multi_node_consensus(
    &self,
    request: VerificationRequest,
) -> Result<VerifiedData, LightClientError> {
    let nodes = self.peer_discovery.get_full_nodes(MIN_NODES)?;
    
    if nodes.len() < MIN_NODES {
        return Err(LightClientError::InsufficientNodes {
            got: nodes.len(),
            required: MIN_NODES,
        });
    }
    
    // Query all nodes in parallel
    let responses: Vec<_> = join_all(
        nodes.iter().map(|node| self.query_node(node, &request))
    ).await;
    
    // Require 2/3 agreement
    let valid_responses: Vec<_> = responses.iter()
        .filter_map(|r| r.as_ref().ok())
        .collect();
    
    if valid_responses.len() * 3 < nodes.len() * 2 {
        return Err(LightClientError::ConsensusFailed);
    }
    
    // Verify all valid responses match
    if !all_equal(&valid_responses) {
        return Err(LightClientError::ForkDetected);
    }
    
    // Return verified data
    Ok(valid_responses[0].clone())
}
```

### B.3 Privacy Considerations

**Reference:** System.md, Subsystem 13 Privacy

| Privacy Feature | Description |
|-----------------|-------------|
| Multiple full nodes | Prevents single node from tracking all requests |
| Bloom filters | Hide exact addresses from full nodes |
| Request obfuscation | Add random addresses to filters |
| Connection rotation | Change full node connections periodically |

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| IPC-MATRIX.md | Subsystem 13 | Light client design |
| System.md | Subsystem 13 | SPV, multi-node consensus |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-01-PEER-DISCOVERY.md | Dependency | Full node discovery |
| SPEC-03-TRANSACTION-INDEXING.md | Dependency | Merkle proofs |
| SPEC-07-BLOOM-FILTERS.md | Dependency | Filtered transactions |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 3 (Advanced - Weeks 9-12)** because:
- Depends on Subsystems 1, 3, 7
- Not required for full node operation
- Client-side implementation

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
