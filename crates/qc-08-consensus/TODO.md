# qc-08-consensus Implementation TODO

**Status:** 🟢 IMPLEMENTED  
**Subsystem ID:** 8  
**Bounded Context:** Block Validation & Agreement  
**Architecture Pattern:** TDD + DDD + Hexagonal + EDA  
**Last Updated:** 2024-12-03

---

## Architecture Alignment Summary

### From SPEC-08-CONSENSUS.md
- **Role:** Block validation ONLY (NOT orchestrator) ✅
- **Pattern:** V2.3 Choreography - publishes `BlockValidated` to Event Bus ✅
- **Algorithms:** PoS (2/3 attestations) or PBFT (2f+1 votes) ✅
- **Zero-Trust:** MUST re-verify all signatures independently ✅
- **Proposer Verification:** Validate proposer is in validator set ✅

### From IPC-MATRIX.md (Subsystem 8)
| Direction | Subsystem | Message Type | Status |
|-----------|-----------|--------------|--------|
| RECEIVES FROM | 5 (Block Propagation) | `ValidateBlockRequest` | ✅ |
| RECEIVES FROM | 6 (Mempool) | `TransactionBatchResponse` | ✅ |
| RECEIVES FROM | 10 (Signature Verify) | `VerifiedSignature` (but ZERO-TRUST re-verify) | ✅ |
| PUBLISHES TO | Event Bus | `BlockValidatedPayload` (triggers choreography) | ✅ |
| QUERIES | 6 (Mempool) | `GetTransactionsRequest` for block building | ✅ |
| QUERIES | 4 (State Mgmt) | `ValidatorSet` at epoch boundary | ✅ |
| PROVIDES TO | 9 (Finality) | Attestations for finality | ⬜ (Phase 8) |
| PROVIDES TO | 5 (Block Propagation) | `PropagateBlockRequest` | ⬜ (Phase 8) |

---

## Implementation Phases

### Phase 1: RED - Write Failing Tests ✅
- [x] Create `crates/qc-08-consensus/tests/` directory
- [x] Write invariant tests (in service.rs module tests)
- [x] Write validation tests
- [x] Write IPC authorization tests
- [x] Write zero-trust tests

### Phase 2: Domain Entities ✅
- [x] Create `src/domain/mod.rs`
- [x] Implement `ValidatedBlock`, `BlockHeader` in `block.rs`
- [x] Implement `ValidationProof`, `PoSProof`, `PBFTProof` in `proof.rs`
- [x] Implement `ValidatorSet`, `ValidatorInfo` in `validator.rs`
- [x] Implement `ChainState`, `ChainHead` in `chain.rs`
- [x] Implement `ConsensusError` in `error.rs`

### Phase 3: Ports (Hexagonal) ✅
- [x] Create `src/ports/mod.rs`
- [x] Define `ConsensusApi` (driving port) in `inbound.rs`
- [x] Define `EventBus`, `MempoolGateway`, `SignatureVerifier`, `ValidatorSetProvider` in `outbound.rs`

### Phase 4: Events (EDA) ✅
- [x] Create `src/events/mod.rs`
- [x] Define `BlockValidatedEvent` in `published.rs`
- [x] Define `ValidateBlockRequest`, `AttestationReceived` in `consumed.rs`

### Phase 5: GREEN - Domain Service ✅
- [x] Create `src/service.rs`
- [x] Implement `ConsensusService` with full validation logic
- [x] Implement `validate_structure()` - size, parent, timestamp
- [x] Implement `validate_signatures()` - ZERO-TRUST re-verify all
- [x] Implement `validate_proposer()` - proposer in validator set
- [x] Implement `verify_attestation_threshold()` - 2/3 for PoS
- [x] Implement `verify_pbft_quorum()` - 2f+1 for PBFT
- [x] Implement block building via mempool
- [x] Implement choreography publishing to Event Bus

### Phase 6: IPC Handler ✅
- [x] Create `src/ipc/mod.rs`
- [x] Implement `IpcHandler` with centralized security (shared-types)
- [x] Implement sender authorization checks
- [x] Implement HMAC/nonce/timestamp verification

### Phase 7: Adapters ✅
- [x] Create `src/adapters/mod.rs`
- [x] Implement `InMemoryEventBus` adapter for testing

### Phase 8: Integration ⬜
- [ ] Wire service to actual event bus
- [ ] Test choreography flow with Subsystems 2, 3, 4
- [ ] Verify BlockValidated triggers downstream processing

---

## INVARIANTS (From SPEC-08) ✅

| ID | Invariant | Test | Status |
|----|-----------|------|--------|
| **INVARIANT-1** | Valid Parent - parent_hash must exist (except genesis) | `test_validate_block_unknown_parent` | ✅ |
| **INVARIANT-2** | Sufficient Attestations - 2/3 validators for PoS | `test_validate_block_insufficient_attestations` | ✅ |
| **INVARIANT-3** | Valid Signatures - zero-trust re-verify all | Domain logic | ✅ |
| **INVARIANT-4** | Sequential Height - height = parent + 1 | `test_validate_block_height_skip` | ✅ |
| **INVARIANT-5** | Timestamp Ordering - timestamp > parent | `test_validate_timestamp` | ✅ |
| **INVARIANT-6** | Valid Proposer - proposer in validator set | `validate_proposer()` | ✅ |

---

## Test Results

```
Unit Tests: 15 passed; 0 failed
Brutal Tests: 15 passed; 0 failed
```

---

## BRUTAL TEST COVERAGE ✅

Tests in `integration-tests/src/exploits/brutal/consensus.rs`:

| Test | Attack Vector | Expected Defense | Status |
|------|---------------|------------------|--------|
| `brutal_fake_attestation_injection` | Forge attestation from unknown validator | Reject unknown validator | ✅ |
| `brutal_pre_validated_flag_bypass` | Zero signatures with signature_valid=true | Zero-trust re-verify rejects | ✅ |
| `brutal_insufficient_attestation_threshold` | Only 33% attestations (< 67%) | Reject block | ✅ |
| `brutal_orphan_block_injection` | Block with non-existent parent | Reject - INVARIANT-1 | ✅ |
| `brutal_height_skip_attack` | Block at height N+2 (skip N+1) | Detect skip | ✅ |
| `brutal_timestamp_regression_attack` | Block timestamp far in future | Reject - INVARIANT-5 | ✅ |
| `brutal_unauthorized_sender_forgery` | ValidateBlockRequest from Subsystem 6 | Reject - not Subsystem 5 | ✅ |
| `brutal_validator_set_manipulation` | Claim attestations from non-validators | Reject unknown validators | ✅ |
| `brutal_pbft_view_mismatch` | PBFT votes from wrong view | Reject view mismatch | ✅ |
| `brutal_double_vote_attack` | Same validator votes twice | Detect and reject duplicate | ✅ |
| `brutal_stale_block_replay` | Replay block from 99 epochs ago | Reject stale block | ✅ |
| `brutal_gas_limit_overflow` | Block exceeds max_block_gas | Reject gas limit exceeded | ✅ |
| `brutal_invalid_proposer` | Proposer not in validator set | Reject invalid proposer | ✅ |
| `brutal_proposer_did_not_attest` | Proposer missing from attestations | Reject - proposer must attest | ✅ |
| `brutal_concurrent_block_flood` | 1000 blocks from 10 threads | Handle without panic | ✅ |

---

## PROGRESS TRACKING

| Phase | Status | Tests Passing | Notes |
|-------|--------|---------------|-------|
| Phase 1: RED | ✅ | 15/15 | Tests written and passing |
| Phase 2: Domain | ✅ | - | All entities implemented |
| Phase 3: Ports | ✅ | - | All interfaces defined |
| Phase 4: Events | ✅ | - | Event payloads defined |
| Phase 5: GREEN | ✅ | 15/15 | Domain logic complete |
| Phase 6: IPC | ✅ | - | Using shared-types security |
| Phase 7: Adapters | ✅ | - | InMemoryEventBus ready |
| Phase 8: Integration | ⬜ | - | Pending other subsystems |
| Brutal Tests | ✅ | 15/15 | All attacks defended |

---

## Security Fixes Applied

1. **BLS Signature Verification** - Now uses actual attestation signatures instead of placeholder
2. **Proposer Verification** - Added `validate_proposer()` to check proposer is in validator set
3. **Proposer Attestation Requirement** - Proposer must include their own attestation
4. **Zero-Trust Enforcement** - Removed `cfg!(test)` bypass for signature verification

---

**Next Action:** Implement qc-09 (Finality) for integration with Consensus

