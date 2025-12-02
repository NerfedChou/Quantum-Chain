# TODO: Subsystem 10 - Signature Verification

**Specification:** `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md` v2.3  
**Crate:** `crates/qc-10-signature-verification`  
**Created:** 2025-12-02  
**Last Updated:** 2025-12-02  
**Status:** 🟢 Service Layer Complete, Integration Pending

---

## CURRENT PHASE

```
[x] Phase 1: RED    - Domain tests ✅ COMPLETE (34 tests)
[x] Phase 2: GREEN  - Domain implementation ✅ COMPLETE
[x] Phase 3: REFACTOR - Code cleanup ✅ COMPLETE
[x] Phase 4: SERVICE - SignatureVerificationService ✅ COMPLETE (9 new tests)
[ ] Phase 5: INTEGRATION - Wire to runtime (IPC, rate limiting) ← NEXT
```

**Test Results:** 43 tests passing
- 34 domain tests (7 BLS + 27 ECDSA)
- 9 service layer tests
- ✅ Clippy clean with `-D warnings`
- ✅ cargo fmt applied

---

## COMPLETED COMPONENTS

### Domain Layer ✅
| Component | File | SPEC Reference |
|-----------|------|----------------|
| Entities | `domain/entities.rs` | Section 2.1 |
| ECDSA Logic | `domain/ecdsa.rs` | Section 3.1 |
| BLS Logic | `domain/bls.rs` | Section 3.1 |
| Errors | `domain/errors.rs` | Section 6 |
| `EcdsaVerifier` struct | `domain/ecdsa.rs` | Section 5.1 |
| Test helpers | `domain/ecdsa.rs` | Section 5.1 |

### Ports Layer ✅
| Component | File | SPEC Reference |
|-----------|------|----------------|
| `SignatureVerificationApi` trait | `ports/inbound.rs` | Section 3.1 |
| `MempoolGateway` trait | `ports/outbound.rs` | Section 3.2 |

### Service Layer ✅
| Component | File | SPEC Reference |
|-----------|------|----------------|
| `SignatureVerificationService` | `service.rs` | Section 3.1 |
| `MockMempoolGateway` | `service.rs` (tests) | Section 5.2 |
| All 8 API methods implemented | `service.rs` | Section 3.1 |
| Service delegation tests | `service.rs` | Section 5.1 |

---

## NEXT: Integration Phase (Tasks 12-17)

### Task 12: Integration Tests
**Reference:** SPEC-10 Section 5.2

- [ ] `test_verify_and_forward_to_mempool` - Mock mempool, verify tx, assert forwarded
- [ ] `test_invalid_tx_not_forwarded` - Corrupt sig, verify fails, nothing forwarded

### Task 13: IPC Message Handling
**Reference:** SPEC-10 Section 4, IPC-MATRIX.md

- [ ] Handle `VerifySignatureRequestPayload`
- [ ] Handle `VerifyNodeIdentityPayload` (DDoS defense)
- [ ] Security boundary checks (authorized senders: 1, 5, 6, 8, 9 ONLY)

### Task 14: Rate Limiting
**Reference:** IPC-MATRIX.md, SPEC-10 Appendix B.3

- [ ] Per-subsystem rate limiter
  - Subsystem 1: Max 100/sec
  - Subsystems 5, 6: Max 1000/sec
  - Subsystems 8, 9: No limit (consensus-critical)

### Task 15: Configuration
**Reference:** SPEC-10 Section 7

- [ ] Define config struct
- [ ] Load from TOML

### Task 16: Final Refactor
- [ ] Review all code
- [ ] Ensure all tests pass

### Task 17: Documentation
- [ ] Rustdoc for all public items
- [ ] Security considerations documented

---

## BLOCKERS

- None currently identified

---

## ARCHITECTURE

```
┌─────────────────────────────────────────────────────────┐
│                    PORTS LAYER ✅                        │
│  SignatureVerificationApi (inbound)                     │
│  MempoolGateway (outbound)                              │
├─────────────────────────────────────────────────────────┤
│                   SERVICE LAYER ✅                       │
│  SignatureVerificationService                           │
│  (implements inbound port, uses outbound port)          │
├─────────────────────────────────────────────────────────┤
│                   DOMAIN LAYER ✅                        │
│  entities.rs, ecdsa.rs, bls.rs, errors.rs              │
└─────────────────────────────────────────────────────────┘
```

## SECURITY NOTES (from SPEC-10)

1. ✅ **Malleability Prevention (EIP-2):** S ≤ half curve order enforced
2. **Zero-Trust:** Subsystems 8, 9 should re-verify independently
3. **Envelope-Only Identity:** Use `AuthenticatedMessage.sender_id` only
4. **Rate Limiting:** Per-subsystem limits to prevent DoS
