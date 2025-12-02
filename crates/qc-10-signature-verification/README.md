# QC-10: Signature Verification

**Subsystem ID:** 10  
**Bounded Context:** Cryptographic Primitives  
**Specification:** [`SPECS/SPEC-10-SIGNATURE-VERIFICATION.md`](../../SPECS/SPEC-10-SIGNATURE-VERIFICATION.md)

---

## Purpose

The Signature Verification subsystem provides cryptographic signature verification for transaction authenticity. It is a **pure cryptographic service** with NO external dependencies, ensuring isolation and testability.

```
Input: (message, signature, public_key) → Output: bool
```

## Capabilities

| Feature | Algorithm | Status |
|---------|-----------|--------|
| ECDSA Verification | secp256k1 | ✅ |
| Address Recovery | keccak256 | ✅ |
| BLS Verification | BLS12-381 | ✅ |
| BLS Aggregation | BLS12-381 | ✅ |
| Batch Verification | secp256k1 | ✅ |

## Quick Start

```rust
use qc_10_signature_verification::{
    SignatureVerificationService, SignatureVerificationApi,
    EcdsaSignature, keccak256,
};

// Create the service
let service = SignatureVerificationService::new();

// Hash a message
let message = b"Transfer 100 tokens to Alice";
let message_hash = keccak256(message);

// Verify signature (in real usage, signature comes from transaction)
let result = service.verify_ecdsa(&message_hash, &signature);

if result.valid {
    println!("Signer: {:?}", result.recovered_address);
} else {
    println!("Invalid: {:?}", result.error);
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   ADAPTERS LAYER                         │
│  IpcHandler (security boundaries, rate limiting)        │
├─────────────────────────────────────────────────────────┤
│                    PORTS LAYER                           │
│  SignatureVerificationApi (inbound)                     │
│  MempoolGateway (outbound)                              │
├─────────────────────────────────────────────────────────┤
│                   SERVICE LAYER                          │
│  SignatureVerificationService                           │
├─────────────────────────────────────────────────────────┤
│                   DOMAIN LAYER                           │
│  entities.rs, ecdsa.rs, bls.rs, errors.rs              │
└─────────────────────────────────────────────────────────┘
```

## Security

### ⚠️ CRITICAL: Zero-Trust Policy

**Subsystems 8 (Consensus) and 9 (Finality) MUST NOT trust the `signature_valid` flag.**

They MUST re-verify signatures independently before making consensus or finality decisions. A compromised Subsystem 10 could otherwise fake transaction validity.

```rust
// WRONG - Trusting pre-validation flag
if msg.signature_valid {
    process_transaction(msg);  // VULNERABLE!
}

// CORRECT - Zero-trust re-verification
let recovered = ecrecover(&msg.hash, &msg.signature)?;
if recovered != msg.claimed_signer {
    return Err(Error::SignatureMismatch);
}
process_transaction(msg);
```

### Malleability Prevention (EIP-2)

All ECDSA signatures are checked for malleability. Signatures with S values in the upper half of the curve order are **rejected**. This prevents:

- Transaction hash manipulation
- Double-spend attacks via modified signatures
- Replay attacks with mutated signatures

### Authorized Consumers

Per [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md), only these subsystems may request verification:

| Subsystem | Allowed Operations |
|-----------|-------------------|
| 1 (Peer Discovery) | `VerifyNodeIdentity` only |
| 5 (Block Propagation) | `VerifySignature` |
| 6 (Mempool) | `VerifyTransaction` |
| 8 (Consensus) | All + `BatchVerify` |
| 9 (Finality) | `VerifySignature` |

**All other subsystems are FORBIDDEN** (2, 3, 4, 7, 11-15).

### Rate Limiting

| Subsystem | Limit | Rationale |
|-----------|-------|-----------|
| 1 (Peer Discovery) | 100/sec | Network edge protection |
| 5, 6 | 1000/sec | Internal traffic |
| 8, 9 | Unlimited | Consensus-critical |

## Testing

```bash
# Run all tests
cargo test -p qc-10-signature-verification

# Run with output
cargo test -p qc-10-signature-verification -- --nocapture

# Run specific test
cargo test -p qc-10-signature-verification test_verify_valid_signature
```

**Test Coverage:** 52 tests
- Domain (ECDSA): 27 tests
- Domain (BLS): 7 tests  
- Service: 9 tests
- IPC/Security: 9 tests

## Dependencies

This subsystem has **NO dependencies on other subsystems**. It depends only on:

- `shared-types` (common types, envelope)
- `k256` (secp256k1 implementation)
- `blst` (BLS12-381 implementation)
- `sha3` (keccak256)

## Files

```
src/
├── lib.rs              # Public API & re-exports
├── domain/
│   ├── mod.rs          # Domain module
│   ├── entities.rs     # Core types (EcdsaSignature, etc.)
│   ├── ecdsa.rs        # ECDSA verification logic
│   ├── bls.rs          # BLS verification logic
│   └── errors.rs       # SignatureError enum
├── ports/
│   ├── mod.rs          # Ports module
│   ├── inbound.rs      # SignatureVerificationApi trait
│   └── outbound.rs     # MempoolGateway trait
├── adapters/
│   ├── mod.rs          # Adapters module
│   └── ipc.rs          # IPC handler with security
└── service.rs          # SignatureVerificationService
```

## Related Documentation

- [SPEC-10-SIGNATURE-VERIFICATION.md](../../SPECS/SPEC-10-SIGNATURE-VERIFICATION.md) - Full technical specification
- [Architecture.md](../../Documentation/Architecture.md) - System architecture
- [IPC-MATRIX.md](../../Documentation/IPC-MATRIX.md) - Security boundaries
- [System.md](../../Documentation/System.md) - Subsystem overview
