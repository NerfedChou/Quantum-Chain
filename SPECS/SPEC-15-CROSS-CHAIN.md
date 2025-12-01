# SPECIFICATION: CROSS-CHAIN COMMUNICATION

**Version:** 2.3  
**Subsystem ID:** 15  
**Bounded Context:** Interoperability  
**Crate Name:** `crates/cross-chain`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Cross-Chain Communication** subsystem enables secure asset transfers and message passing between different blockchain networks using Hash Time-Locked Contracts (HTLCs) and relay mechanisms.

### 1.2 Responsibility Boundaries

**In Scope:**
- HTLC contract deployment and management
- Cross-chain message relay
- Proof verification from external chains
- Atomic swap coordination

**Out of Scope:**
- Smart contract execution (Subsystem 11)
- Consensus on external chains
- External chain state storage

### 1.3 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED:                                                       │
│  ├─ HTLC contracts from Subsystem 11                            │
│  └─ Finality from Subsystem 8                                   │
│                                                                 │
│  UNTRUSTED (cryptographically verified):                        │
│  └─ Proofs from external chains (SPV/relay verification)        │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Supported external chains
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainId {
    QuantumChain,
    Ethereum,
    Bitcoin,
    Polygon,
    Arbitrum,
}

/// Hash Time-Locked Contract state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HTLC {
    pub id: Hash,
    /// Chain where HTLC is deployed
    pub chain: ChainId,
    /// Sender of funds
    pub sender: ChainAddress,
    /// Recipient of funds
    pub recipient: ChainAddress,
    /// Amount locked
    pub amount: U256,
    /// Hash of the secret
    pub hash_lock: Hash,
    /// Expiry timestamp
    pub time_lock: u64,
    /// Current state
    pub state: HTLCState,
    /// Secret (revealed on claim)
    pub secret: Option<[u8; 32]>,
}

/// Chain-agnostic address
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainAddress {
    pub chain: ChainId,
    pub address: Vec<u8>,  // Variable length for different chains
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HTLCState {
    Pending,
    Locked,
    Claimed,
    Refunded,
    Expired,
}

/// Atomic swap between two chains
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtomicSwap {
    pub id: Hash,
    /// HTLC on source chain
    pub source_htlc: HTLCId,
    /// HTLC on target chain
    pub target_htlc: HTLCId,
    /// Swap state
    pub state: SwapState,
    /// Creation timestamp
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapState {
    Initiated,
    SourceLocked,
    TargetLocked,
    Completed,
    Refunded,
}

/// Cross-chain message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainMessage {
    pub id: Hash,
    pub source_chain: ChainId,
    pub target_chain: ChainId,
    pub sender: ChainAddress,
    pub recipient: ChainAddress,
    pub payload: Vec<u8>,
    pub proof: CrossChainProof,
}

/// Proof of inclusion on source chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainProof {
    pub block_hash: Hash,
    pub block_height: u64,
    pub merkle_proof: Vec<Hash>,
    pub finality_proof: FinalityProof,
}

/// Cross-chain configuration
#[derive(Clone, Debug)]
pub struct CrossChainConfig {
    /// HTLC time-lock duration
    pub htlc_timeout_hours: u64,
    /// Required confirmations per chain
    pub confirmations: HashMap<ChainId, u64>,
    /// Relay fee percentage
    pub relay_fee_percent: f64,
}

impl Default for CrossChainConfig {
    fn default() -> Self {
        let mut confirmations = HashMap::new();
        confirmations.insert(ChainId::Ethereum, 12);
        confirmations.insert(ChainId::Bitcoin, 6);
        confirmations.insert(ChainId::Polygon, 128);
        confirmations.insert(ChainId::Arbitrum, 1);
        
        Self {
            htlc_timeout_hours: 24,
            confirmations,
            relay_fee_percent: 0.1,
        }
    }
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: HTLC Atomicity
/// Either both HTLCs complete (claimed) or both refund.
fn invariant_atomic_swap(swap: &AtomicSwap, source: &HTLC, target: &HTLC) -> bool {
    match swap.state {
        SwapState::Completed => {
            source.state == HTLCState::Claimed && target.state == HTLCState::Claimed
        }
        SwapState::Refunded => {
            source.state == HTLCState::Refunded && target.state == HTLCState::Refunded
        }
        _ => true,  // Intermediate states
    }
}

/// INVARIANT-2: Time-Lock Ordering
/// Target HTLC expires before source HTLC.
fn invariant_timelock_ordering(source: &HTLC, target: &HTLC) -> bool {
    target.time_lock < source.time_lock
}

/// INVARIANT-3: Hash Lock Consistency
/// Both HTLCs use the same hash lock.
fn invariant_hashlock_match(source: &HTLC, target: &HTLC) -> bool {
    source.hash_lock == target.hash_lock
}

/// INVARIANT-4: Secret Reveals Atomically
/// If secret is revealed on one chain, it can be used on the other.
fn invariant_secret_atomic(source: &HTLC, target: &HTLC) -> bool {
    source.secret == target.secret
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Cross-Chain API
#[async_trait]
pub trait CrossChainApi: Send + Sync {
    /// Initiate atomic swap
    async fn initiate_swap(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        sender: ChainAddress,
        recipient: ChainAddress,
        amount: U256,
    ) -> Result<AtomicSwap, CrossChainError>;
    
    /// Lock funds on source chain (step 1)
    async fn lock_source(&self, swap_id: Hash, secret_hash: Hash) -> Result<HTLC, CrossChainError>;
    
    /// Lock funds on target chain (step 2)
    async fn lock_target(&self, swap_id: Hash, htlc_proof: CrossChainProof) -> Result<HTLC, CrossChainError>;
    
    /// Claim funds with secret (step 3)
    async fn claim(&self, htlc_id: Hash, secret: [u8; 32]) -> Result<(), CrossChainError>;
    
    /// Refund expired HTLC
    async fn refund(&self, htlc_id: Hash) -> Result<(), CrossChainError>;
    
    /// Relay cross-chain message
    async fn relay_message(&self, message: CrossChainMessage) -> Result<(), CrossChainError>;
    
    /// Verify proof from external chain
    async fn verify_proof(&self, proof: CrossChainProof, chain: ChainId) -> Result<bool, CrossChainError>;
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// HTLC contract interface (uses Subsystem 11)
#[async_trait]
pub trait HTLCContract: Send + Sync {
    /// Deploy HTLC on local chain
    async fn deploy_htlc(&self, params: HTLCParams) -> Result<Address, ContractError>;
    
    /// Call claim on HTLC
    async fn claim_htlc(&self, htlc_address: Address, secret: [u8; 32]) -> Result<(), ContractError>;
    
    /// Call refund on HTLC
    async fn refund_htlc(&self, htlc_address: Address) -> Result<(), ContractError>;
    
    /// Get HTLC state
    async fn get_htlc_state(&self, htlc_address: Address) -> Result<HTLCState, ContractError>;
}

/// External chain light client
#[async_trait]
pub trait ExternalChainClient: Send + Sync {
    /// Get block header
    async fn get_header(&self, chain: ChainId, height: u64) -> Result<BlockHeader, ChainError>;
    
    /// Verify Merkle proof
    async fn verify_proof(&self, chain: ChainId, proof: &CrossChainProof) -> Result<bool, ChainError>;
    
    /// Get finality status
    async fn is_finalized(&self, chain: ChainId, block_hash: Hash) -> Result<bool, ChainError>;
}

/// Finality checker (uses Subsystem 8)
#[async_trait]
pub trait FinalityChecker: Send + Sync {
    /// Check if block is finalized
    async fn is_finalized(&self, block_hash: Hash) -> Result<bool, FinalityError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Messages

```rust
/// Atomic swap initiation request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitiateSwapRequest {
    pub correlation_id: CorrelationId,
    pub source_chain: ChainId,
    pub target_chain: ChainId,
    pub sender: ChainAddress,
    pub recipient: ChainAddress,
    pub amount: U256,
}

/// HTLC event (emitted when state changes)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HTLCEvent {
    pub htlc_id: Hash,
    pub event_type: HTLCEventType,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HTLCEventType {
    Locked { hash_lock: Hash, time_lock: u64 },
    Claimed { secret: [u8; 32] },
    Refunded,
    Expired,
}

/// Cross-chain relay request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayRequest {
    pub correlation_id: CorrelationId,
    pub message: CrossChainMessage,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === HTLC Tests ===
    
    #[test]
    fn test_htlc_secret_verification() {
        let secret = [0xAB; 32];
        let hash_lock = sha256(&secret);
        
        let htlc = create_htlc(hash_lock, now() + 3600);
        
        assert!(verify_secret(&htlc, &secret));
        assert!(!verify_secret(&htlc, &[0xCD; 32]));
    }
    
    #[test]
    fn test_htlc_expiry() {
        let htlc = create_htlc([0; 32], now() - 100);  // Already expired
        
        assert!(htlc.is_expired(now()));
        assert!(!htlc.can_claim(now()));
        assert!(htlc.can_refund(now()));
    }
    
    #[test]
    fn test_htlc_claim_before_expiry() {
        let htlc = create_htlc([0; 32], now() + 3600);  // Expires in 1 hour
        
        assert!(!htlc.is_expired(now()));
        assert!(htlc.can_claim(now()));
        assert!(!htlc.can_refund(now()));
    }
    
    // === Atomic Swap Tests ===
    
    #[test]
    fn test_timelock_ordering() {
        let source_timelock = now() + 7200;  // 2 hours
        let target_timelock = now() + 3600;  // 1 hour
        
        assert!(target_timelock < source_timelock);
    }
    
    #[test]
    fn test_swap_state_transitions() {
        let mut swap = create_atomic_swap();
        
        assert_eq!(swap.state, SwapState::Initiated);
        
        swap.lock_source();
        assert_eq!(swap.state, SwapState::SourceLocked);
        
        swap.lock_target();
        assert_eq!(swap.state, SwapState::TargetLocked);
        
        swap.complete();
        assert_eq!(swap.state, SwapState::Completed);
    }
    
    // === Proof Verification Tests ===
    
    #[test]
    fn test_merkle_proof_verification() {
        let proof = create_valid_proof();
        
        assert!(verify_merkle_proof_external(&proof));
    }
    
    #[test]
    fn test_invalid_merkle_proof() {
        let mut proof = create_valid_proof();
        proof.merkle_proof[0][0] ^= 0xFF;  // Tamper
        
        assert!(!verify_merkle_proof_external(&proof));
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_complete_atomic_swap() {
        // Setup
        let (htlc_contract, _) = create_mock_htlc_contract();
        let (external_client, _) = create_mock_external_client();
        let service = CrossChainService::new(htlc_contract, external_client);
        
        // Generate secret
        let secret = generate_random_secret();
        let hash_lock = sha256(&secret);
        
        // Step 1: Initiate swap
        let swap = service.initiate_swap(
            ChainId::QuantumChain,
            ChainId::Ethereum,
            ALICE_QC,
            BOB_ETH,
            U256::from(100),
        ).await.unwrap();
        
        // Step 2: Lock on source (QuantumChain)
        let source_htlc = service.lock_source(swap.id, hash_lock).await.unwrap();
        assert_eq!(source_htlc.state, HTLCState::Locked);
        
        // Step 3: Lock on target (Ethereum) with proof
        let proof = generate_lock_proof(&source_htlc);
        let target_htlc = service.lock_target(swap.id, proof).await.unwrap();
        assert_eq!(target_htlc.state, HTLCState::Locked);
        
        // Step 4: Bob claims on target (reveals secret)
        service.claim(target_htlc.id, secret).await.unwrap();
        
        // Step 5: Alice claims on source (uses revealed secret)
        service.claim(source_htlc.id, secret).await.unwrap();
        
        // Verify final state
        let final_swap = service.get_swap(swap.id).await.unwrap();
        assert_eq!(final_swap.state, SwapState::Completed);
    }
    
    #[tokio::test]
    async fn test_swap_timeout_refund() {
        let service = create_test_service();
        
        let swap = service.initiate_swap(
            ChainId::QuantumChain,
            ChainId::Ethereum,
            ALICE_QC,
            BOB_ETH,
            U256::from(100),
        ).await.unwrap();
        
        // Lock source with short timeout for testing
        let source_htlc = lock_with_short_timeout(&service, swap.id).await;
        
        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Refund
        service.refund(source_htlc.id).await.unwrap();
        
        let final_htlc = service.get_htlc(source_htlc.id).await.unwrap();
        assert_eq!(final_htlc.state, HTLCState::Refunded);
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum CrossChainError {
    #[error("Unsupported chain: {0:?}")]
    UnsupportedChain(ChainId),
    
    #[error("HTLC not found: {0:?}")]
    HTLCNotFound(Hash),
    
    #[error("Invalid secret")]
    InvalidSecret,
    
    #[error("HTLC expired")]
    HTLCExpired,
    
    #[error("HTLC not expired (cannot refund)")]
    HTLCNotExpired,
    
    #[error("Invalid proof")]
    InvalidProof,
    
    #[error("Not finalized on source chain")]
    NotFinalized,
    
    #[error("Contract error: {0}")]
    ContractError(#[from] ContractError),
    
    #[error("External chain error: {0}")]
    ChainError(#[from] ChainError),
}
```

---

## 7. CONFIGURATION

```toml
[cross_chain]
htlc_timeout_hours = 24
relay_fee_percent = 0.1

[cross_chain.confirmations]
ethereum = 12
bitcoin = 6
polygon = 128
arbitrum = 1
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 15

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Cross-Chain (15) | Subsystem 11 (Smart Contracts) | Uses | HTLC contracts | System.md Subsystem 15 |
| Cross-Chain (15) | Subsystem 9 (Finality) | Uses | Finality proofs | IPC-MATRIX.md Subsystem 15 |
| Cross-Chain (15) | External Chains | Bridge | Light client verification | System.md Subsystem 15 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 15 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `CreateHTLCRequest` | External relayers (verified) | IPC-MATRIX.md Subsystem 15 |
| `ClaimHTLCRequest` | External relayers (verified) | IPC-MATRIX.md Subsystem 15 |
| `FinalityProofRequest` | This subsystem to Subsystem 9 | IPC-MATRIX.md Subsystem 15 |

### B.2 HTLC Security

**Reference:** System.md, Subsystem 15 Security Defenses

```rust
/// HTLC security verification
/// 
/// Reference: System.md, Subsystem 15 HTLC Protocol
fn verify_htlc_claim(
    htlc: &HTLC,
    secret: &[u8; 32],
    claimer: Address,
) -> Result<(), CrossChainError> {
    // Step 1: Verify HTLC not expired
    if current_time() > htlc.expiration {
        return Err(CrossChainError::HTLCExpired);
    }
    
    // Step 2: Verify secret matches hashlock
    let hash = sha256(secret);
    if hash != htlc.hashlock {
        return Err(CrossChainError::InvalidSecret);
    }
    
    // Step 3: Verify claimer matches receiver
    if claimer != htlc.receiver {
        return Err(CrossChainError::InvalidClaimer);
    }
    
    // Step 4: Verify HTLC is finalized on source chain
    let proof = finality.get_proof(htlc.source_tx_hash).await?;
    if !proof.is_finalized() {
        return Err(CrossChainError::NotFinalized);
    }
    
    Ok(())
}

/// HTLC refund verification
fn verify_htlc_refund(
    htlc: &HTLC,
    refunder: Address,
) -> Result<(), CrossChainError> {
    // Step 1: Verify HTLC IS expired
    if current_time() <= htlc.expiration {
        return Err(CrossChainError::HTLCNotExpired);
    }
    
    // Step 2: Verify refunder matches sender
    if refunder != htlc.sender {
        return Err(CrossChainError::InvalidRefunder);
    }
    
    // Step 3: Verify HTLC not already claimed
    if htlc.claimed {
        return Err(CrossChainError::AlreadyClaimed);
    }
    
    Ok(())
}
```

### B.3 Light Client Bridge Security

**Reference:** System.md, Subsystem 15 Chain-Specific Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CROSS-CHAIN VERIFICATION MODEL                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  EACH EXTERNAL CHAIN requires:                                              │
│  1. Light client implementation for header verification                     │
│  2. Finality rules specific to that chain                                  │
│  3. Merkle proof verification for transaction inclusion                     │
│                                                                             │
│  CHAIN-SPECIFIC FINALITY:                                                   │
│  ├── Bitcoin:   6 confirmations (PoW)                                       │
│  ├── Ethereum:  12 confirmations (PoS, 2 epochs)                            │
│  ├── Polygon:   128 confirmations (fast finality)                           │
│  └── Arbitrum:  1 confirmation (L2, verified by L1)                         │
│                                                                             │
│  TRUST MODEL:                                                               │
│  ├── NO trusted relayers (trustless bridge)                                 │
│  ├── Cryptographic verification only                                        │
│  └── HTLC atomicity guarantees                                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| IPC-MATRIX.md | Subsystem 15 | Cross-chain message types |
| System.md | Subsystem 15 | HTLC protocol, chain-specific finality |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-09-FINALITY.md | Dependency | Finality proofs for cross-chain verification |
| SPEC-11-SMART-CONTRACTS.md | Dependency | HTLC contract execution |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 4 (Future - Post V1)** because:
- Requires per-chain light client implementations
- Complex security model
- High research risk

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
