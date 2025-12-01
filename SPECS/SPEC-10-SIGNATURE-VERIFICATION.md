# SPECIFICATION: SIGNATURE VERIFICATION

**Version:** 2.3  
**Subsystem ID:** 10  
**Bounded Context:** Cryptographic Primitives  
**Crate Name:** `crates/signature-verification`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Signature Verification** subsystem provides cryptographic signature verification for transaction authenticity. It is a pure cryptographic service with NO external dependencies, ensuring isolation and testability.

### 1.2 Responsibility Boundaries

**In Scope:**
- ECDSA signature verification (secp256k1)
- BLS signature verification (for PoS attestations)
- Public key recovery from signatures
- Batch signature verification
- Pre-verified transaction forwarding to Mempool

**Out of Scope:**
- Key generation (client-side)
- Key storage (HSM or wallet)
- Transaction validation logic
- State management

### 1.3 Key Design Principle: Pure Cryptographic Service

This subsystem has **NO dependencies on other subsystems**. It is a pure function:

```
Input: (message, signature, public_key) → Output: bool
```

This isolation ensures:
- Testability (no mocking required)
- Security (minimal attack surface)
- Performance (can be parallelized)

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  THIS SUBSYSTEM IS THE TRUST ROOT FOR SIGNATURES                │
│                                                                 │
│  INPUTS:                                                        │
│  ├─ Raw messages (untrusted, from network)                      │
│  └─ Signatures (untrusted, to be verified)                      │
│                                                                 │
│  OUTPUTS:                                                       │
│  ├─ Verification result (trusted)                               │
│  └─ Pre-verified transactions → Mempool                         │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  Identity from AuthenticatedMessage.sender_id only.             │
│                                                                 │
│  ZERO-TRUST WARNING:                                            │
│  Other subsystems (8, 9) MAY re-verify signatures independently │
│  and should NOT blindly trust the `signature_valid` flag.       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// ECDSA signature (secp256k1)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EcdsaSignature {
    /// R component (32 bytes)
    pub r: [u8; 32],
    /// S component (32 bytes)
    pub s: [u8; 32],
    /// Recovery ID (0, 1, 27, or 28)
    pub v: u8,
}

/// BLS signature (for aggregation)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlsSignature {
    /// G1 point (48 bytes compressed)
    pub bytes: [u8; 48],
}

/// BLS public key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlsPublicKey {
    /// G2 point (96 bytes compressed)
    pub bytes: [u8; 96],
}

/// ECDSA public key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EcdsaPublicKey {
    /// Uncompressed public key (65 bytes: 0x04 || x || y)
    pub bytes: [u8; 65],
}

/// Ethereum-style address (derived from public key)
pub type Address = [u8; 20];

/// Verification request
#[derive(Clone, Debug)]
pub struct VerificationRequest {
    pub message_hash: Hash,
    pub signature: EcdsaSignature,
    pub expected_signer: Option<Address>,
}

/// Verification result
#[derive(Clone, Debug)]
pub struct VerificationResult {
    pub valid: bool,
    pub recovered_address: Option<Address>,
    pub error: Option<SignatureError>,
}

/// Batch verification request
#[derive(Clone, Debug)]
pub struct BatchVerificationRequest {
    pub requests: Vec<VerificationRequest>,
}

/// Batch verification result
#[derive(Clone, Debug)]
pub struct BatchVerificationResult {
    pub results: Vec<VerificationResult>,
    pub all_valid: bool,
    pub valid_count: usize,
    pub invalid_count: usize,
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Deterministic Verification
/// Same inputs always produce same output.
fn invariant_deterministic(
    message: &[u8],
    signature: &EcdsaSignature,
) -> bool {
    let result1 = verify(message, signature);
    let result2 = verify(message, signature);
    result1 == result2
}

/// INVARIANT-2: No False Positives
/// Invalid signatures never verify as valid.
fn invariant_no_false_positives(
    message: &[u8],
    invalid_signature: &EcdsaSignature,
) -> bool {
    // By construction of ECDSA
    !verify(message, invalid_signature).valid
}

/// INVARIANT-3: Signature Malleability Prevention
/// Signatures with high S values are rejected (EIP-2).
fn invariant_no_malleability(signature: &EcdsaSignature) -> bool {
    // S must be in lower half of curve order
    let s = U256::from_big_endian(&signature.s);
    s <= SECP256K1_HALF_ORDER
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary Signature Verification API
pub trait SignatureVerificationApi: Send + Sync {
    // === ECDSA Operations ===
    
    /// Verify an ECDSA signature
    fn verify_ecdsa(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> VerificationResult;
    
    /// Verify ECDSA and check expected signer
    fn verify_ecdsa_signer(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
        expected: Address,
    ) -> VerificationResult;
    
    /// Recover address from signature
    fn recover_address(
        &self,
        message_hash: &Hash,
        signature: &EcdsaSignature,
    ) -> Result<Address, SignatureError>;
    
    /// Batch verify ECDSA signatures (parallel)
    fn batch_verify_ecdsa(
        &self,
        requests: &[VerificationRequest],
    ) -> BatchVerificationResult;
    
    // === BLS Operations ===
    
    /// Verify a BLS signature
    fn verify_bls(
        &self,
        message: &[u8],
        signature: &BlsSignature,
        public_key: &BlsPublicKey,
    ) -> bool;
    
    /// Verify aggregated BLS signature
    fn verify_bls_aggregate(
        &self,
        message: &[u8],
        aggregate_signature: &BlsSignature,
        public_keys: &[BlsPublicKey],
    ) -> bool;
    
    /// Aggregate multiple BLS signatures
    fn aggregate_bls_signatures(
        &self,
        signatures: &[BlsSignature],
    ) -> Result<BlsSignature, SignatureError>;
    
    // === Transaction Verification ===
    
    /// Verify a signed transaction and forward to Mempool
    fn verify_transaction(
        &self,
        transaction: SignedTransaction,
    ) -> Result<VerifiedTransaction, SignatureError>;
}

/// A transaction with verified signature
#[derive(Clone, Debug)]
pub struct VerifiedTransaction {
    pub transaction: SignedTransaction,
    pub sender: Address,
    pub signature_valid: bool,
}
```

### 3.2 Driven Ports (SPI - Outbound)

```rust
/// Mempool interface for forwarding verified transactions
#[async_trait]
pub trait MempoolGateway: Send + Sync {
    /// Forward verified transaction to Mempool
    async fn submit_verified_transaction(
        &self,
        tx: VerifiedTransaction,
    ) -> Result<(), MempoolError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Outgoing Messages

```rust
/// Verified transaction forwarded to Mempool
/// SECURITY (Envelope-Only Identity): sender_id = 10
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddTransactionRequest {
    pub correlation_id: CorrelationId,
    pub transaction: SignedTransaction,
    pub signature_valid: bool,
    pub recovered_sender: Address,
}
```

### 4.2 Incoming Messages

```rust
/// Raw transaction for verification (from network)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifyTransactionRequest {
    pub correlation_id: CorrelationId,
    pub transaction: SignedTransaction,
}

/// Batch verification request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchVerifyRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub requests: Vec<VerificationRequest>,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    // === ECDSA Tests ===
    
    #[test]
    fn test_verify_valid_signature() {
        let verifier = EcdsaVerifier::new();
        
        let (private_key, public_key) = generate_keypair();
        let message_hash = keccak256(b"test message");
        let signature = sign(&message_hash, &private_key);
        
        let result = verifier.verify_ecdsa(&message_hash, &signature);
        
        assert!(result.valid);
        assert_eq!(result.recovered_address, Some(address_from_pubkey(&public_key)));
    }
    
    #[test]
    fn test_verify_invalid_signature() {
        let verifier = EcdsaVerifier::new();
        
        let message_hash = keccak256(b"test message");
        let invalid_signature = EcdsaSignature {
            r: [0xFF; 32],
            s: [0xFF; 32],
            v: 27,
        };
        
        let result = verifier.verify_ecdsa(&message_hash, &invalid_signature);
        
        assert!(!result.valid);
    }
    
    #[test]
    fn test_verify_wrong_message() {
        let verifier = EcdsaVerifier::new();
        
        let (private_key, _) = generate_keypair();
        let message1 = keccak256(b"message 1");
        let message2 = keccak256(b"message 2");
        let signature = sign(&message1, &private_key);
        
        // Verify against wrong message
        let result = verifier.verify_ecdsa(&message2, &signature);
        
        assert!(!result.valid);
    }
    
    #[test]
    fn test_signature_malleability_rejected() {
        let verifier = EcdsaVerifier::new();
        
        let (private_key, _) = generate_keypair();
        let message_hash = keccak256(b"test");
        let mut signature = sign(&message_hash, &private_key);
        
        // Make S value high (malleable)
        signature.s = invert_s(&signature.s);  // s' = n - s
        
        let result = verifier.verify_ecdsa(&message_hash, &signature);
        
        assert!(!result.valid);
        assert!(matches!(result.error, Some(SignatureError::MalleableSignature)));
    }
    
    #[test]
    fn test_recover_address() {
        let verifier = EcdsaVerifier::new();
        
        let (private_key, public_key) = generate_keypair();
        let expected_address = address_from_pubkey(&public_key);
        let message_hash = keccak256(b"test");
        let signature = sign(&message_hash, &private_key);
        
        let recovered = verifier.recover_address(&message_hash, &signature).unwrap();
        
        assert_eq!(recovered, expected_address);
    }
    
    // === BLS Tests ===
    
    #[test]
    fn test_bls_verify_valid() {
        let verifier = BlsVerifier::new();
        
        let (secret_key, public_key) = bls_generate_keypair();
        let message = b"test message";
        let signature = bls_sign(message, &secret_key);
        
        assert!(verifier.verify_bls(message, &signature, &public_key));
    }
    
    #[test]
    fn test_bls_aggregate_verify() {
        let verifier = BlsVerifier::new();
        
        let keypairs: Vec<_> = (0..10).map(|_| bls_generate_keypair()).collect();
        let message = b"aggregate test";
        
        let signatures: Vec<_> = keypairs.iter()
            .map(|(sk, _)| bls_sign(message, sk))
            .collect();
        
        let public_keys: Vec<_> = keypairs.iter()
            .map(|(_, pk)| pk.clone())
            .collect();
        
        let aggregate = verifier.aggregate_bls_signatures(&signatures).unwrap();
        
        assert!(verifier.verify_bls_aggregate(message, &aggregate, &public_keys));
    }
    
    // === Batch Verification Tests ===
    
    #[test]
    fn test_batch_verify_all_valid() {
        let verifier = EcdsaVerifier::new();
        
        let requests: Vec<_> = (0..100)
            .map(|_| create_valid_verification_request())
            .collect();
        
        let result = verifier.batch_verify_ecdsa(&requests);
        
        assert!(result.all_valid);
        assert_eq!(result.valid_count, 100);
        assert_eq!(result.invalid_count, 0);
    }
    
    #[test]
    fn test_batch_verify_mixed() {
        let verifier = EcdsaVerifier::new();
        
        let mut requests: Vec<_> = (0..90)
            .map(|_| create_valid_verification_request())
            .collect();
        
        // Add 10 invalid
        requests.extend((0..10).map(|_| create_invalid_verification_request()));
        
        let result = verifier.batch_verify_ecdsa(&requests);
        
        assert!(!result.all_valid);
        assert_eq!(result.valid_count, 90);
        assert_eq!(result.invalid_count, 10);
    }
    
    // === Performance Tests ===
    
    #[test]
    fn test_batch_faster_than_sequential() {
        let verifier = EcdsaVerifier::new();
        
        let requests: Vec<_> = (0..1000)
            .map(|_| create_valid_verification_request())
            .collect();
        
        let batch_start = Instant::now();
        verifier.batch_verify_ecdsa(&requests);
        let batch_time = batch_start.elapsed();
        
        let seq_start = Instant::now();
        for req in &requests {
            verifier.verify_ecdsa(&req.message_hash, &req.signature);
        }
        let seq_time = seq_start.elapsed();
        
        // Batch should be at least 2x faster
        assert!(batch_time < seq_time / 2);
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_verify_and_forward_to_mempool() {
        let (mempool, mempool_rx) = create_mock_mempool();
        let service = SignatureVerificationService::new(mempool);
        
        let tx = create_signed_transaction();
        
        service.verify_and_forward(tx.clone()).await.unwrap();
        
        let forwarded = mempool_rx.recv().await.unwrap();
        assert!(forwarded.signature_valid);
        assert_eq!(forwarded.transaction.hash(), tx.hash());
    }
    
    #[tokio::test]
    async fn test_invalid_tx_not_forwarded() {
        let (mempool, mempool_rx) = create_mock_mempool();
        let service = SignatureVerificationService::new(mempool);
        
        let mut tx = create_signed_transaction();
        tx.signature.r = [0xFF; 32];  // Corrupt signature
        
        let result = service.verify_and_forward(tx).await;
        
        assert!(result.is_err());
        assert!(mempool_rx.try_recv().is_err());  // Nothing forwarded
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Invalid signature format")]
    InvalidFormat,
    
    #[error("Signature verification failed")]
    VerificationFailed,
    
    #[error("Malleable signature (high S value)")]
    MalleableSignature,
    
    #[error("Invalid recovery ID: {0}")]
    InvalidRecoveryId(u8),
    
    #[error("Failed to recover public key")]
    RecoveryFailed,
    
    #[error("BLS pairing check failed")]
    BlsPairingFailed,
    
    #[error("Cannot aggregate empty signature list")]
    EmptyAggregation,
}
```

---

## 7. CONFIGURATION

```toml
[signature_verification]
# Enable batch verification parallelism
enable_batch_parallel = true
batch_thread_count = 4

# Malleability protection (EIP-2)
reject_malleable_signatures = true

# Pre-computed tables for faster verification
use_precomputed_tables = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 10

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Sig Verify (10) | None | - | Pure crypto (no deps) | System.md Subsystem 10 |
| Sig Verify (10) | Subsystem 6 (Mempool) | Sends to | Verified transactions | IPC-MATRIX.md Subsystem 10 |
| Sig Verify (10) | Subsystem 8 (Consensus) | Sends to | Verified signatures | IPC-MATRIX.md Subsystem 10 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 10 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `VerifyTransactionRequest` | Subsystems 1, 5, 6, 8, 9 ONLY | IPC-MATRIX.md Security Boundaries |
| `VerifyNodeIdentityRequest` | Subsystem 1 (Peer Discovery) ONLY | IPC-MATRIX.md DDoS Defense |
| `VerifySignatureRequest` | Subsystems 1, 5, 8, 9 ONLY | IPC-MATRIX.md Security Boundaries |
| `BatchVerifyRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |

### B.2 FORBIDDEN Consumers

**Reference:** IPC-MATRIX.md, "FORBIDDEN Consumers (Principle of Least Privilege)"

The following subsystems are EXPLICITLY FORBIDDEN from accessing SignatureVerification:

| Subsystem | Why Forbidden |
|-----------|---------------|
| 2 (Block Storage) | Storage only, receives pre-verified data |
| 3 (Transaction Indexing) | Indexing only, receives pre-verified data |
| 4 (State Management) | State only, receives pre-verified data |
| 7 (Bloom Filters) | Filtering only, no signature needs |
| 11 (Smart Contracts) | Execution only, receives pre-verified transactions |
| 12 (Transaction Ordering) | Ordering only, receives pre-verified data |
| 13 (Light Clients) | Receives proofs, does not verify signatures directly |
| 14 (Sharding) | Coordination only, uses Consensus for verification |
| 15 (Cross-Chain) | Uses Finality proofs, not direct signature verification |

### B.3 DDoS Edge Defense

**Reference:** IPC-MATRIX.md, Subsystem 1 → Subsystem 10 Flow

Subsystem 1 (Peer Discovery) is NOW ALLOWED to verify signatures for **DDoS defense at the network edge**:

```rust
/// Rate Limiting per IPC-MATRIX.md
/// 
/// Reference: IPC-MATRIX.md, Subsystem 10 Rate Limiting section
const RATE_LIMITS: &[(SubsystemId, u32)] = &[
    (SubsystemId::PeerDiscovery, 100),     // Max 100/sec (network edge)
    (SubsystemId::BlockPropagation, 1000), // Max 1000/sec (internal)
    (SubsystemId::Mempool, 1000),          // Max 1000/sec (internal)
    (SubsystemId::Consensus, u32::MAX),    // No limit (consensus-critical)
    (SubsystemId::Finality, u32::MAX),     // No limit (consensus-critical)
];
```

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| IPC-MATRIX.md | Subsystem 10 | Authorized consumers, rate limiting |
| System.md | Subsystem 10 | ECDSA/BLS algorithms, no dependencies |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-01-PEER-DISCOVERY.md | Consumer | DDoS edge defense (verify node identity) |
| SPEC-05-BLOCK-PROPAGATION.md | Consumer | Block signature verification |
| SPEC-06-MEMPOOL.md | Consumer | Transaction signature verification |
| SPEC-08-CONSENSUS.md | Consumer | Validator signature verification (but Zero-Trust) |
| SPEC-09-FINALITY.md | Consumer | Attestation signature verification (but Zero-Trust) |

### C.3 Zero-Trust Warning

**Reference:** IPC-MATRIX.md, Subsystems 8 and 9

Subsystems 8 (Consensus) and 9 (Finality) SHOULD NOT blindly trust the `signature_valid` flag. They MUST re-verify signatures independently because:

1. If this subsystem is compromised, attackers could inject fake attestations
2. Economic finality requires cryptographic certainty, not trust
3. Zero-Trust is a defense-in-depth security layer

### C.4 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 1 (Core - Weeks 1-4)** because:
- Has NO dependencies (pure cryptographic service)
- Required by all other subsystems for signature verification
- Should be implemented first to enable parallel development

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
