# TODO: Subsystem 10 - Signature Verification

**Specification:** `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md` v2.3  
**Crate:** `crates/qc-10-signature-verification`  
**Created:** 2025-12-02  
**Status:** 🔴 Not Started

---

## DOCUMENT PURPOSE

This TODO tracks implementation progress for Subsystem 10 (Signature Verification).

**Reference Documents:**
- `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md` - Technical specification (SINGLE SOURCE OF TRUTH)
- `Documentation/Architecture.md` v2.3 - Hexagonal architecture, TDD workflow
- `Documentation/IPC-MATRIX.md` v2.3 - Security boundaries, authorized consumers
- `Documentation/System.md` v2.3 - Subsystem dependencies

**TDD Enforcement:** No implementation code without a failing test first.

---

## CURRENT PHASE

```
[x] Phase 1: RED    - Write failing tests ✅ COMPLETE
[x] Phase 2: GREEN  - Write minimum code to pass tests ✅ COMPLETE
[x] Phase 3: REFACTOR - Clean up while keeping tests green ✅ COMPLETE
[ ] Phase 4: INTEGRATION - Wire to runtime
```

**Test Results:** 34 tests passing (7 BLS + 27 ECDSA)
- All SPEC-10 Section 5.1 tests implemented
- All SPEC-10 Section 2.2 invariants enforced
- Security edge cases for blockchain (zero values, boundary conditions, malleability)
- Clippy clean with -D warnings
- cargo fmt applied

---

## TASK BREAKDOWN

### TASK 1: Project Setup ✅ COMPLETE
**Reference:** SPEC-10 Section 7 (Configuration)

- [x] 1.1 Add dependencies to `Cargo.toml`
  - `k256` (secp256k1 ECDSA)
  - `blst` (BLS signatures)
  - `sha3` (keccak256 hashing)
  - `thiserror` (error handling)
  - `serde`, `serde_with` (serialization)
  - `shared-types` (workspace dependency)
  - `async-trait` (async port traits)
  - `rayon` (parallel batch verification)

- [x] 1.2 Create module structure per hexagonal architecture
  ```
  src/
  ├── lib.rs           # Public API exports
  ├── domain/          # Inner layer (pure logic)
  │   ├── mod.rs
  │   ├── entities.rs  # EcdsaSignature, BlsSignature, etc.
  │   ├── ecdsa.rs     # ECDSA verification logic
  │   ├── bls.rs       # BLS verification logic
  │   └── errors.rs    # SignatureError enum
  └── ports/           # Middle layer (traits)
      ├── mod.rs
      ├── inbound.rs   # SignatureVerificationApi trait
      └── outbound.rs  # MempoolGateway trait
  ```

---

### TASK 2: Domain Entities ✅ COMPLETE
**Reference:** SPEC-10 Section 2.1 (Core Entities)

- [x] 2.1 Define `EcdsaSignature` struct
  - Fields: `r: [u8; 32]`, `s: [u8; 32]`, `v: u8`
  - Derive: `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`

- [x] 2.2 Define `BlsSignature` struct
  - Field: `bytes: [u8; 96]` (G2 point compressed, min_pk variant)

- [x] 2.3 Define `BlsPublicKey` struct
  - Field: `bytes: [u8; 48]` (G1 point compressed, min_pk variant)

- [x] 2.4 Define `EcdsaPublicKey` struct
  - Field: `bytes: [u8; 65]` (uncompressed: 0x04 || x || y)

- [x] 2.5 Define `Address` type alias
  - `pub type Address = [u8; 20];`

- [x] 2.6 Define `VerificationRequest` struct
  - Fields: `message_hash: Hash`, `signature: EcdsaSignature`, `expected_signer: Option<Address>`

- [x] 2.7 Define `VerificationResult` struct
  - Fields: `valid: bool`, `recovered_address: Option<Address>`, `error: Option<SignatureError>`

- [x] 2.8 Define `BatchVerificationRequest` and `BatchVerificationResult` structs
  - Per SPEC-10 Section 2.1

---

### TASK 3: Domain Invariants ✅ COMPLETE
**Reference:** SPEC-10 Section 2.2 (Invariants)

- [x] 3.1 Write test: `test_invariant_deterministic`
  - Same inputs always produce same output

- [x] 3.2 Write test: `test_invariant_no_false_positives`
  - Invalid signatures never verify as valid

- [x] 3.3 Write test: `test_invariant_no_malleability`
  - Signatures with high S values are rejected (EIP-2)
  - S must be ≤ SECP256K1_HALF_ORDER

---

### TASK 4: ECDSA Unit Tests ✅ COMPLETE
**Reference:** SPEC-10 Section 5.1 (Unit Tests)

- [x] 4.1 Write test: `test_verify_valid_signature`
  - Generate keypair, sign message, verify succeeds
  - Assert `result.valid == true`
  - Assert `result.recovered_address == expected`

- [x] 4.2 Write test: `test_verify_invalid_signature`
  - Use garbage signature `[0xFF; 32]` for r and s
  - Assert `result.valid == false`

- [x] 4.3 Write test: `test_verify_wrong_message`
  - Sign message1, verify against message2
  - Signature recovers different address

- [x] 4.4 Write test: `test_signature_malleability_rejected`
  - Invert S value (s' = n - s) to make it high
  - Assert `result.valid == false`
  - Assert `result.error == Some(SignatureError::MalleableSignature)`

- [x] 4.5 Write test: `test_recover_address`
  - Sign message, recover address, compare to expected

- [x] 4.6 Write test: `test_parse_recovery_id` (helper function test)
  - Use v value not in {0, 1, 27, 28}
  - Assert error `SignatureError::InvalidRecoveryId`

---

### TASK 5: BLS Unit Tests ✅ COMPLETE
**Reference:** SPEC-10 Section 5.1 (Unit Tests - BLS Tests)

- [x] 5.1 Write test: `test_bls_verify_valid`
  - Generate BLS keypair, sign message, verify succeeds

- [x] 5.2 Write test: `test_bls_verify_invalid_wrong_key`
  - Use wrong public key, verify fails

- [x] 5.3 Write test: `test_bls_aggregate_and_verify`
  - Generate 5 keypairs, sign same message
  - Aggregate signatures, verify with all public keys

- [x] 5.4 Write test: `test_bls_aggregate_empty_fails`
  - Aggregating empty list returns `SignatureError::EmptyAggregation`

---

### TASK 6: Batch Verification Tests ✅ COMPLETE
**Reference:** SPEC-10 Section 5.1 (Batch Verification Tests)

- [x] 6.1 Write test: `test_batch_verify_all_valid`
  - 100 valid requests
  - Assert `result.all_valid == true`
  - Assert `result.valid_count == 100`

- [x] 6.2 Write test: `test_batch_verify_mixed`
  - 90 valid + 10 invalid requests
  - Assert `result.all_valid == false`
  - Assert `result.valid_count == 90`
  - Assert `result.invalid_count == 10`

- [x] 6.3 Write test: `test_batch_faster_than_sequential`
  - 1000 requests batch vs sequential
  - Batch uses rayon parallel processing

---

### TASK 7: ECDSA Domain Implementation ✅ COMPLETE
**Reference:** SPEC-10 Section 2.1, 3.1

- [x] 7.1 Implement `verify_ecdsa` function
  - Use `k256` crate for secp256k1
  - Check signature format validity
  - Check malleability (S ≤ half order)
  - Recover public key and derive address

- [x] 7.2 Implement `verify_ecdsa_signer` function
  - Call `verify_ecdsa`, compare recovered address to expected

- [x] 7.3 Implement `recover_address` function
  - Recover public key from signature
  - Keccak256 hash of public key, take last 20 bytes

- [x] 7.4 Implement `batch_verify_ecdsa` function
  - Use rayon for parallel verification
  - Collect results, compute statistics

- [x] 7.5 Implement `EcdsaVerifier` struct (per SPEC-10 Section 5.1)
  - `EcdsaVerifier::new()` constructor
  - All verification methods as instance methods

- [x] 7.6 Implement test helpers (per SPEC-10 Section 5.1)
  - `generate_keypair()`
  - `sign()`
  - `keccak256()`
  - `address_from_pubkey()`
  - `invert_s()`
  - `create_valid_verification_request()`
  - `create_invalid_verification_request()`

---

### TASK 8: BLS Domain Implementation ✅ COMPLETE
**Reference:** SPEC-10 Section 2.1, 3.1

- [x] 8.1 Implement `verify_bls` function
  - Use `blst` crate with `min_sig` variant (per SPEC-10)
  - Pairing check for G1 signatures / G2 public keys

- [x] 8.2 Implement `verify_bls_aggregate` function
  - Aggregate pairing check for multiple public keys

- [x] 8.3 Implement `aggregate_bls_signatures` function
  - Point addition on G1 curve
  - Return error if empty input

---

### TASK 9: Error Types ✅ COMPLETE
**Reference:** SPEC-10 Section 6 (Error Handling)

- [x] 9.1 Implement `SignatureError` enum with thiserror
  - `InvalidFormat`
  - `VerificationFailed`
  - `MalleableSignature`
  - `InvalidRecoveryId(u8)`
  - `RecoveryFailed`
  - `BlsPairingFailed`
  - `EmptyAggregation`
  - `SignerMismatch` (added for signer verification)

---

### TASK 10: Inbound Port ✅ COMPLETE
**Reference:** SPEC-10 Section 3.1 (Driving Ports)

- [x] 10.1 Define `SignatureVerificationApi` trait
  - `verify_ecdsa(&self, message_hash: &Hash, signature: &EcdsaSignature) -> VerificationResult`
  - `verify_ecdsa_signer(&self, ..., expected: Address) -> VerificationResult`
  - `recover_address(&self, ...) -> Result<Address, SignatureError>`
  - `batch_verify_ecdsa(&self, requests: &BatchVerificationRequest) -> BatchVerificationResult`
  - `verify_bls(&self, ...) -> bool`
  - `verify_bls_aggregate(&self, ...) -> bool`
  - `aggregate_bls_signatures(&self, ...) -> Result<BlsSignature, SignatureError>`
  - `verify_transaction(&self, tx: Transaction) -> Result<VerifiedTransaction, SignatureError>`

- [ ] 10.2 Implement `SignatureVerificationService` struct that implements the trait

---

### TASK 11: Outbound Port ✅ COMPLETE
**Reference:** SPEC-10 Section 3.2 (Driven Ports)

- [x] 11.1 Define `MempoolGateway` trait
  - `async fn submit_verified_transaction(&self, tx: VerifiedTransaction) -> Result<(), MempoolError>`

- [ ] 11.2 Create mock implementation for testing

---

### TASK 12: Integration Tests (GREEN Phase)
**Reference:** SPEC-10 Section 5.2 (Integration Tests)

- [ ] 12.1 Write test: `test_verify_and_forward_to_mempool`
  - Mock mempool, verify transaction, assert forwarded

- [ ] 12.2 Write test: `test_invalid_tx_not_forwarded`
  - Corrupt signature, verify fails, nothing forwarded to mempool

---

### TASK 13: IPC Message Handling
**Reference:** SPEC-10 Section 4 (Event Schema), IPC-MATRIX.md Subsystem 10

- [ ] 13.1 Verify alignment with `shared-types/src/ipc.rs`
  - `VerifySignatureRequestPayload` exists
  - `VerifySignatureResponsePayload` exists
  - `VerifyNodeIdentityPayload` exists
  - `VerifyNodeIdentityResponse` exists

- [ ] 13.2 Implement message handlers
  - Handle `VerifySignatureRequestPayload`
  - Handle `VerifyNodeIdentityPayload` (for DDoS defense)

- [ ] 13.3 Implement security boundary checks
  - Verify sender_id is in authorized list per IPC-MATRIX.md
  - Authorized: Subsystems 1, 5, 6, 8, 9 ONLY
  - REJECT requests from: 2, 3, 4, 7, 11, 12, 13, 14, 15

---

### TASK 14: Rate Limiting
**Reference:** IPC-MATRIX.md Subsystem 10 Rate Limiting, SPEC-10 Appendix B.3

- [ ] 14.1 Implement rate limiter per subsystem
  - Subsystem 1 (Peer Discovery): Max 100/sec
  - Subsystems 5, 6: Max 1000/sec
  - Subsystems 8, 9: No limit (consensus-critical)

- [ ] 14.2 Write tests for rate limiting enforcement

---

### TASK 15: Configuration
**Reference:** SPEC-10 Section 7 (Configuration)

- [ ] 15.1 Define configuration struct
  - `enable_batch_parallel: bool` (default: true)
  - `batch_thread_count: u32` (default: 4)
  - `reject_malleable_signatures: bool` (default: true)
  - `use_precomputed_tables: bool` (default: true)

- [ ] 15.2 Load configuration from TOML

---

### TASK 16: Refactor Phase

- [ ] 16.1 Review code for clarity and naming
- [ ] 16.2 Extract common patterns into helper functions
- [ ] 16.3 Ensure all tests still pass
- [ ] 16.4 Run clippy and fix warnings
- [ ] 16.5 Run cargo fmt

---

### TASK 17: Documentation

- [ ] 17.1 Add rustdoc comments to all public items
- [ ] 17.2 Document security considerations
- [ ] 17.3 Document Zero-Trust warning for consumers (8, 9)

---

## BLOCKERS

- None currently identified

---

## NOTES

### Security Considerations (from SPEC-10)
1. **Malleability Prevention (EIP-2):** Always check S ≤ half curve order
2. **Zero-Trust:** Subsystems 8 and 9 should re-verify signatures independently
3. **Envelope-Only Identity:** Use `AuthenticatedMessage.sender_id` only, never payload identity
4. **Rate Limiting:** Enforce per-subsystem limits to prevent DoS

### Dependencies to Add (Cargo.toml)
```toml
[dependencies]
shared-types = { path = "../shared-types" }
k256 = { version = "0.13", features = ["ecdsa", "ecdsa-core"] }
blst = "0.3"
sha3 = "0.10"
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
rayon = "1.8"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
rand = "0.8"
```

---

## COMPLETION CHECKLIST

- [ ] All tests in SPEC-10 Section 5.1 implemented and passing
- [ ] All tests in SPEC-10 Section 5.2 implemented and passing
- [ ] All invariants from SPEC-10 Section 2.2 enforced
- [ ] SignatureVerificationApi trait fully implemented
- [ ] MempoolGateway trait defined with mock
- [ ] Security boundaries enforced per IPC-MATRIX.md
- [ ] Rate limiting implemented per IPC-MATRIX.md
- [ ] Configuration loading implemented
- [ ] Clippy clean, cargo fmt applied
- [ ] Rustdoc complete

---

**END OF TODO**
