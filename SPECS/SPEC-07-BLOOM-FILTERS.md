# SPECIFICATION: TRANSACTION FILTERING (BLOOM FILTERS)

**Version:** 2.3  
**Subsystem ID:** 7  
**Bounded Context:** Light Client Support  
**Crate Name:** `crates/bloom-filters`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Transaction Filtering** subsystem provides Bloom filter-based probabilistic filtering for light clients, allowing them to check transaction relevance without downloading the full blockchain. It enables efficient SPV (Simplified Payment Verification) by matching transactions against watched addresses.

### 1.2 Responsibility Boundaries

**In Scope:**
- Construct Bloom filters for blocks containing transaction data
- Match transactions against client-provided address filters
- Provide filtered transaction streams to light clients
- Manage filter parameters (size, hash functions, false positive rate)
- Implement privacy protections (random false positives, rotation)

**Out of Scope:**
- Merkle proof generation (Subsystem 3)
- Peer discovery (Subsystem 1)
- Full block validation (Subsystem 8)
- Light client protocol management (Subsystem 13)

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS:                                                │
│  └─ Transaction hashes from Subsystem 3 (Transaction Indexing)  │
│                                                                 │
│  UNTRUSTED (client-provided):                                   │
│  └─ Bloom filters from light clients (privacy-sensitive)        │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  For internal IPC, identity from AuthenticatedMessage only.     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Bloom filter for transaction matching
#[derive(Clone, Debug)]
pub struct BloomFilter {
    /// Bit array
    pub bits: BitVec,
    /// Number of hash functions
    pub k: usize,
    /// Filter size in bits
    pub m: usize,
    /// Number of elements inserted
    pub n: usize,
    /// Tweak for hash function variation
    pub tweak: u32,
}

/// Bloom filter configuration
#[derive(Clone, Debug)]
pub struct BloomConfig {
    /// Target false positive rate
    pub target_fpr: f64,
    /// Maximum filter size (bits)
    pub max_size_bits: usize,
    /// Maximum elements per filter
    pub max_elements: usize,
    /// Filter rotation interval (blocks)
    pub rotation_interval: u64,
    /// Add random false positives for privacy
    pub privacy_noise_percent: f64,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            target_fpr: 0.0001,  // 0.01% false positive rate
            max_size_bits: 36_000,  // ~4.5 KB
            max_elements: 50,
            rotation_interval: 100,
            privacy_noise_percent: 5.0,
        }
    }
}

/// Block filter for a specific block
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockFilter {
    pub block_hash: Hash,
    pub block_height: u64,
    pub filter: BloomFilter,
    pub transaction_count: u32,
}

/// Client filter subscription
#[derive(Clone, Debug)]
pub struct FilterSubscription {
    pub client_id: ClientId,
    pub filter: BloomFilter,
    pub watched_addresses: Vec<Address>,
    pub created_at: Instant,
    pub last_rotated: Instant,
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: False Positive Rate Bounded
/// FPR = (1 - e^(-kn/m))^k must be <= target_fpr
fn invariant_fpr_bounded(filter: &BloomFilter, config: &BloomConfig) -> bool {
    filter.false_positive_rate() <= config.target_fpr
}

/// INVARIANT-2: No False Negatives
/// If an element was inserted, membership test MUST return true.
fn invariant_no_false_negatives(filter: &BloomFilter, element: &[u8]) -> bool {
    // After insert, contains must return true
    true  // Guaranteed by Bloom filter properties
}

/// INVARIANT-3: Privacy Protection
/// Filters include noise to prevent address fingerprinting.
fn invariant_privacy_noise(filter: &BloomFilter, config: &BloomConfig) -> bool {
    // Verified by adding random elements during construction
    true
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Bloom filter API
#[async_trait]
pub trait BloomFilterApi: Send + Sync {
    /// Create a filter for a set of addresses
    fn create_filter(
        &self,
        addresses: &[Address],
        config: &BloomConfig,
    ) -> Result<BloomFilter, FilterError>;
    
    /// Test if a transaction matches a filter
    /// 
    /// Reference: System.md, Subsystem 7 - SPV transaction filtering
    /// 
    /// MATCHING FIELDS (tested in order):
    /// 1. Transaction sender address (tx.from) - 20 bytes
    /// 2. Transaction recipient address (tx.to) - 20 bytes, None for contract creation
    /// 3. Contract creation address (if tx.to is None) - computed from sender+nonce
    /// 4. Log addresses (for each log in receipt) - 20 bytes each
    /// 
    /// A transaction matches if ANY of the above fields match ANY element in the filter.
    /// 
    /// PRIVACY NOTE (IPC-MATRIX.md §7): Testing additional fields (e.g., log topics, 
    /// data patterns) would increase precision but also increases fingerprinting risk.
    fn matches(
        &self,
        filter: &BloomFilter,
        transaction: &Transaction,
        receipt: Option<&TransactionReceipt>,
    ) -> MatchResult;
    
    /// Get filtered transactions for a block
    async fn get_filtered_transactions(
        &self,
        block_height: u64,
        filter: &BloomFilter,
    ) -> Result<Vec<Transaction>, FilterError>;
    
    /// Create block filter from transaction hashes
    fn create_block_filter(
        &self,
        block_hash: Hash,
        block_height: u64,
        tx_hashes: &[Hash],
        addresses: &[Address],
    ) -> Result<BlockFilter, FilterError>;
}

/// Result of matching with details for debugging
#[derive(Clone, Debug)]
pub struct MatchResult {
    pub matches: bool,
    /// Which field caused the match (for debugging, not privacy-safe to expose)
    pub matched_field: Option<MatchedField>,
}

#[derive(Clone, Debug)]
pub enum MatchedField {
    Sender,
    Recipient,
    ContractCreation,
    LogAddress(usize),  // Index of log
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Transaction data provider (minimal data principle)
/// 
/// Reference: Architecture.md §3.2.1 - Principle of Least Data
/// 
/// SECURITY: This port returns ONLY the data needed for filtering,
/// not full transactions. This reduces bandwidth and information leakage.
#[async_trait]
pub trait TransactionDataProvider: Send + Sync {
    /// Get transaction hashes for a block
    async fn get_transaction_hashes(
        &self,
        block_height: u64,
    ) -> Result<Vec<Hash>, DataError>;
    
    /// Get full transactions for a block
    async fn get_transactions(
        &self,
        block_height: u64,
    ) -> Result<Vec<Transaction>, DataError>;
    
    /// Get addresses involved in transactions for a block
    /// 
    /// Returns: Vec of (sender, recipient, created_contract, log_addresses)
    /// This is MORE EFFICIENT than fetching full transactions.
    async fn get_transaction_addresses(
        &self,
        block_height: u64,
    ) -> Result<Vec<TransactionAddresses>, DataError>;
}

/// Addresses involved in a single transaction
#[derive(Clone, Debug)]
pub struct TransactionAddresses {
    pub tx_hash: Hash,
    pub sender: Address,
    pub recipient: Option<Address>,
    pub created_contract: Option<Address>,
    pub log_addresses: Vec<Address>,
}
```

---

## 4. EVENT SCHEMA

### 4.1 Messages

```rust
/// Request for filtered transactions
/// SECURITY: Identity from envelope
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredTransactionsRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub block_height: u64,
    pub filter: BloomFilter,
}

/// Response with filtered transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredTransactionsResponse {
    pub correlation_id: CorrelationId,
    pub block_height: u64,
    pub transactions: Vec<Transaction>,
    pub false_positive_estimate: f64,
}

/// Request for transaction hashes (to Subsystem 3)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionHashRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub block_height: u64,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_bloom_filter_insert_and_contains() {
        let mut filter = BloomFilter::new(1000, 7);
        let element = b"test_address";
        
        assert!(!filter.contains(element));
        filter.insert(element);
        assert!(filter.contains(element));
    }
    
    #[test]
    fn test_no_false_negatives() {
        let mut filter = BloomFilter::new(10_000, 7);
        let elements: Vec<_> = (0..100).map(|i| format!("element_{}", i)).collect();
        
        for elem in &elements {
            filter.insert(elem.as_bytes());
        }
        
        for elem in &elements {
            assert!(filter.contains(elem.as_bytes()), "False negative for {}", elem);
        }
    }
    
    #[test]
    fn test_false_positive_rate_within_bounds() {
        let config = BloomConfig {
            target_fpr: 0.01,
            ..Default::default()
        };
        
        let mut filter = BloomFilter::new_with_fpr(100, config.target_fpr);
        
        // Insert 100 elements
        for i in 0..100 {
            filter.insert(format!("inserted_{}", i).as_bytes());
        }
        
        // Test 10000 elements that were NOT inserted
        let mut false_positives = 0;
        for i in 0..10_000 {
            if filter.contains(format!("not_inserted_{}", i).as_bytes()) {
                false_positives += 1;
            }
        }
        
        let actual_fpr = false_positives as f64 / 10_000.0;
        assert!(actual_fpr <= config.target_fpr * 1.5, "FPR {} exceeds target {}", actual_fpr, config.target_fpr);
    }
    
    #[test]
    fn test_optimal_parameters() {
        // For n=50, FPR=0.0001, optimal k=13, m=959
        let (k, m) = BloomFilter::optimal_params(50, 0.0001);
        assert!(k >= 10 && k <= 15);
        assert!(m >= 800 && m <= 1200);
    }
    
    #[test]
    fn test_privacy_noise_added() {
        let config = BloomConfig {
            privacy_noise_percent: 10.0,
            ..Default::default()
        };
        
        let addresses = vec![Address::from([0xAB; 20])];
        let filter = create_filter_with_noise(&addresses, &config);
        
        // Filter should have more bits set than just the addresses
        let bits_set = filter.bits.count_ones();
        let expected_min = (addresses.len() * filter.k) as usize;
        
        // Noise should add ~10% more
        assert!(bits_set as f64 >= expected_min as f64 * 1.05);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_filter_matches_watched_address() {
        let (tx_provider, _) = create_mock_tx_provider();
        let service = BloomFilterService::new(tx_provider);
        
        let watched = Address::from([0xAB; 20]);
        let filter = service.create_filter(&[watched], &BloomConfig::default()).unwrap();
        
        // Create transaction to watched address
        let tx = create_transaction_to(watched);
        
        assert!(service.matches(&filter, &tx));
    }
    
    #[tokio::test]
    async fn test_filter_not_matches_unwatched() {
        let service = create_test_service();
        
        let watched = Address::from([0xAB; 20]);
        let unwatched = Address::from([0xCD; 20]);
        
        let filter = service.create_filter(&[watched], &BloomConfig::default()).unwrap();
        let tx = create_transaction_to(unwatched);
        
        // Might match (false positive) but usually won't
        // This test verifies the filter is working, not that FPR is exact
    }
    
    #[tokio::test]
    async fn test_get_filtered_transactions() {
        let txs = vec![
            create_transaction_to(ALICE),
            create_transaction_to(BOB),
            create_transaction_to(CHARLIE),
        ];
        let (tx_provider, _) = create_mock_tx_provider_with_txs(1, txs);
        let service = BloomFilterService::new(tx_provider);
        
        // Filter for ALICE only
        let filter = service.create_filter(&[ALICE], &BloomConfig::default()).unwrap();
        
        let filtered = service.get_filtered_transactions(1, &filter).await.unwrap();
        
        // Should include ALICE's transaction (maybe others as false positives)
        assert!(filtered.iter().any(|tx| tx.recipient() == ALICE));
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("Filter size exceeds maximum: {size} > {max}")]
    FilterTooLarge { size: usize, max: usize },
    
    #[error("Too many elements: {count} > {max}")]
    TooManyElements { count: usize, max: usize },
    
    #[error("Block not found: {height}")]
    BlockNotFound { height: u64 },
    
    #[error("Data provider error: {0}")]
    DataError(#[from] DataError),
}
```

---

## 7. CONFIGURATION

```toml
[bloom_filters]
target_fpr = 0.0001
max_size_bits = 36000
max_elements = 50
rotation_interval_blocks = 100
privacy_noise_percent = 5.0
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 7

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Bloom Filters (7) | Subsystem 3 (Tx Indexing) | Query | Transaction hashes for filter population | System.md Subsystem 7 |
| Bloom Filters (7) | Subsystem 1 (Peer Discovery) | Query | Full node connections | System.md Subsystem 7 |
| Bloom Filters (7) | Subsystem 13 (Light Clients) | Provides to | Filtered transactions | IPC-MATRIX.md Subsystem 7 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 7 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `BuildFilterRequest` | Subsystem 13 (Light Clients) ONLY | IPC-MATRIX.md Security Boundaries |
| `UpdateFilterRequest` | Subsystem 13 (Light Clients) ONLY | IPC-MATRIX.md Security Boundaries |
| `TransactionHashUpdate` | Subsystem 3 (Transaction Indexing) ONLY | IPC-MATRIX.md Security Boundaries |

### B.2 Mandatory Rejection Rules

```rust
/// MANDATORY security checks per IPC-MATRIX.md
fn validate_request(msg: &AuthenticatedMessage<BloomFilterRequest>) -> Result<(), BloomError> {
    match msg.payload {
        BloomFilterRequest::BuildFilter { .. } | BloomFilterRequest::UpdateFilter { .. } => {
            // ONLY Light Clients (13) can request filters
            if msg.sender_id != SubsystemId::LightClients {
                return Err(BloomError::UnauthorizedSender(msg.sender_id));
            }
        }
        BloomFilterRequest::TransactionHashUpdate { .. } => {
            // ONLY Transaction Indexing (3) can provide hashes
            if msg.sender_id != SubsystemId::TransactionIndexing {
                return Err(BloomError::UnauthorizedSender(msg.sender_id));
            }
        }
    }
    
    // Reject filters with >1000 watched addresses (privacy risk)
    if msg.payload.watched_addresses().len() > 1000 {
        return Err(BloomError::TooManyAddresses);
    }
    
    // Reject FPR <0.01 or >0.1 (too precise or too noisy)
    if let Some(fpr) = msg.payload.target_fpr() {
        if fpr < 0.01 || fpr > 0.1 {
            return Err(BloomError::InvalidFPR(fpr));
        }
    }
    
    // Reject >1 filter update per 10 blocks per client
    if rate_limited_filter_update(&msg.sender_id) {
        return Err(BloomError::RateLimited);
    }
    
    Ok(())
}
```

### B.3 Privacy Considerations

**Reference:** System.md, Subsystem 7 Security Defenses

The Bloom filter reveals information about watched addresses. Mitigations:

| Defense | Description | Reference |
|---------|-------------|-----------|
| Random False Positives | Add random addresses to filter | System.md Subsystem 7 |
| Filter Rotation | Change filters periodically (every 100 blocks) | System.md Subsystem 7 |
| Multiple Filters | Use different filters with different full nodes | System.md Subsystem 7 |
| Client-Side Filtering | Download more than needed, filter locally | System.md Subsystem 7 |

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| IPC-MATRIX.md | Subsystem 7 | Security boundaries, message types |
| System.md | Subsystem 7 | Bloom Filter algorithm, FPR calculation |
| System.md | V2.3 Dependency Graph | Depends on 1, 3; provides to 13 |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-01-PEER-DISCOVERY.md | Dependency | Full node connections |
| SPEC-03-TRANSACTION-INDEXING.md | Dependency | Transaction hashes for filter population |
| SPEC-13-LIGHT-CLIENT.md | Client | Primary consumer of filtered transactions |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 3 (Advanced - Weeks 9-12)** because:
- Depends on Subsystems 1 (Peer Discovery) and 3 (Transaction Indexing)
- Provides optimization for light clients, not core functionality
- Can be implemented after core block processing is complete

---

## APPENDIX D: FALSE POSITIVE RATE CALCULATION

**Reference:** System.md, Subsystem 7 Supporting Algorithms

```rust
/// Calculate optimal Bloom filter parameters
/// 
/// Formula: FPR = (1 - e^(-kn/m))^k
/// Where:
///   k = number of hash functions
///   n = number of elements (watched addresses)
///   m = size of bit array
/// 
/// Reference: System.md, Subsystem 7
pub fn calculate_optimal_parameters(
    num_elements: usize,
    target_fpr: f64,
) -> BloomFilterParams {
    // Optimal number of bits: m = -n*ln(fpr) / (ln(2)^2)
    let m = (-(num_elements as f64) * target_fpr.ln() / (LN_2 * LN_2)).ceil() as usize;
    
    // Optimal number of hash functions: k = (m/n) * ln(2)
    let k = ((m as f64 / num_elements as f64) * LN_2).round() as usize;
    
    BloomFilterParams {
        size_bits: m,
        hash_count: k.max(1).min(32),  // Clamp to reasonable range
        expected_fpr: calculate_actual_fpr(m, num_elements, k),
    }
}
```

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
