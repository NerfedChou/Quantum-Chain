# qc-09-finality

**Finality Gadget** implementing Casper FFG for economic finality guarantees.

## Overview

This subsystem provides cryptographic-economic finality through a two-phase protocol:

1. **Justification**: 2/3+ of validators attest to a checkpoint
2. **Finalization**: Two consecutive justified checkpoints

Once finalized, a block cannot be reverted without burning at least 1/3 of total stake.

## Architecture

Reference: `SPEC-09-FINALITY.md`, `Architecture.md v2.3`

```
Consensus (8) ──AttestationBatch──→ Finality (9)
                                        │
                                        ├── MarkFinalizedRequest ──→ Block Storage (2)
                                        │
                                        └── FinalityProof ──→ Cross-Chain (15)
```

## Key Features

### Casper FFG Implementation

- **Checkpoint Tracking**: Tracks checkpoints at epoch boundaries
- **Stake-Weighted Voting**: Validators vote proportionally to their stake
- **Two-Phase Finality**: Justified → Finalized progression

### Circuit Breaker (Livelock Prevention)

Reference: `Architecture.md Section 5.4.1`

```
[RUNNING] ──failure──→ [SYNC {1}] ──fail──→ [SYNC {2}] ──fail──→ [SYNC {3}] ──fail──→ [HALTED]
    ↑                      │                    │                    │                    │
    └──────────────────────┴────────────────────┴────────────────────┘                    │
                                 (sync success)                                           │
    ↑                                                                                     │
    └─────────────────────────── manual intervention ─────────────────────────────────────┘
```

### Zero-Trust Signature Verification

Every attestation signature is re-verified independently, even if pre-validated.

## Security Model

Reference: `IPC-MATRIX.md Subsystem 9`

| Message | Authorized Sender | 
|---------|-------------------|
| AttestationBatch | Consensus (8) |
| FinalityCheckRequest | Consensus (8) |
| FinalityProofRequest | Cross-Chain (15) |

## Usage

```rust
use qc_09_finality::{FinalityService, FinalityConfig};
use qc_09_finality::ports::inbound::FinalityApi;

// Create service
let service = FinalityService::new(
    FinalityConfig::default(),
    block_storage,
    verifier,
    validator_provider,
);

// Process attestations
let result = service.process_attestations(attestations).await?;

// Check if block is finalized
let is_final = service.is_finalized(block_hash).await;

// Get circuit breaker state
let state = service.get_state().await;
```

## Configuration

```toml
[finality]
epoch_length = 32
justification_threshold_percent = 67
max_sync_attempts = 3
sync_timeout_secs = 60
inactivity_leak_epochs = 4
always_reverify_signatures = true
```

## Invariants

| ID | Invariant | Description |
|----|-----------|-------------|
| INVARIANT-1 | Finalization requires 2 consecutive justified | Casper FFG rule |
| INVARIANT-2 | Justification requires 2/3 stake | Supermajority threshold |
| INVARIANT-3 | No conflicting finality | Only one block per height |
| INVARIANT-4 | Circuit breaker determinism | Testable state machine |

## Testing

```bash
cargo test -p qc-09-finality
```

Current: **30 tests passing**

## Dependencies

- `shared-types` - Common types and security module
- `qc-04-state-management` - Validator stake queries (via port)
- `qc-10-signature-verification` - BLS verification (via port)
- `qc-02-block-storage` - MarkFinalizedRequest target (via port)

## Status

🟢 **IMPLEMENTED** - Core domain logic complete, IPC security integrated, tests passing.

Remaining: Brutal tests for integration-tests suite.
