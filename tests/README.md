# Test Structure - Clean Organization

## 📁 Final Structure

```
tests/
├── Cargo.toml                    # Test crate config
├── benches/                      # Criterion benchmark entry points
│   ├── subsystem_benchmarks.rs   # Standard benchmarks
│   └── brutal_benchmarks.rs      # Stress test benchmarks
│
└── src/
    ├── lib.rs                    # Crate entry point
    │
    ├── benchmarks/               # Performance tests (by subsystem)
    │   ├── mod.rs
    │   ├── qc_01_peer_discovery.rs
    │   ├── qc_02_block_storage.rs
    │   ├── qc_03_tx_indexing.rs
    │   ├── qc_04_state_mgmt.rs
    │   ├── qc_06_mempool.rs
    │   ├── qc_07_bloom_filters.rs
    │   ├── qc_08_consensus.rs
    │   └── qc_10_signature.rs
    │
    ├── exploits/                 # Attack simulations
    │   ├── mod.rs
    │   ├── helpers.rs            # Shared test utilities
    │   │
    │   ├── historical/           # Famous past attacks
    │   │   ├── mod.rs
    │   │   ├── phase1_exploits.rs  # Mt Gox, Wormhole, Dust, Eclipse
    │   │   ├── qc_01/            # Peer Discovery attacks
    │   │   │   └── eclipse.rs
    │   │   ├── qc_06/            # Mempool attacks
    │   │   │   └── penny_flooding.rs
    │   │   └── qc_08/            # Consensus attacks
    │   │       └── timejacking.rs
    │   │
    │   ├── modern/               # Current threat landscape
    │   │   ├── mod.rs
    │   │   ├── qc_02/            # Block Storage
    │   │   │   └── block_storage.rs
    │   │   ├── qc_03/            # Transaction Indexing
    │   │   │   └── merkle_proofs.rs
    │   │   ├── qc_04/            # State Management
    │   │   │   └── state_management.rs
    │   │   ├── qc_05/            # Block Propagation
    │   │   │   └── block_propagation.rs
    │   │   ├── qc_06/            # Mempool
    │   │   │   ├── data_exhaustion.rs
    │   │   │   └── staging_flood.rs
    │   │   ├── qc_07/            # Bloom Filters
    │   │   │   └── bloom_filters.rs
    │   │   ├── qc_08/            # Consensus
    │   │   │   └── consensus.rs
    │   │   ├── qc_09/            # Finality
    │   │   │   └── finality.rs
    │   │   ├── qc_10/            # Signature Verification
    │   │   │   └── legit_vs_fake.rs
    │   │   └── qc_16/            # API Gateway
    │   │       └── api_gateway.rs
    │   │
    │   └── architectural/        # System-level attacks
    │       ├── mod.rs
    │       ├── qc_03/            # Transaction Indexing
    │       │   └── ghost_transaction.rs
    │       ├── qc_06/            # Mempool
    │       │   └── zombie_assembler.rs
    │       └── cross_cutting/    # Multi-subsystem attacks
    │           ├── breach_isolation.rs
    │           ├── crash_recovery.rs
    │           ├── ipc_authentication.rs
    │           ├── under_pressure.rs
    │           └── zero_day.rs
    │
    └── integration/              # Cross-subsystem choreography
        ├── mod.rs
        ├── e2e_choreography.rs   # Full event flow
        ├── flows.rs              # Business logic flows
        └── runtime_simulation.rs # Node simulation
```

## 🎯 Test Categories

### **benchmarks/** - Performance Tests
- Per-subsystem performance validation
- Criterion-based measurements
- SPEC claim verification

### **exploits/** - Security Tests

| Category | Purpose | Example Attacks |
|----------|---------|-----------------|
| **historical/** | Famous past attacks | Eclipse, Mt Gox, Penny Flooding |
| **modern/** | Current threats | Memory exhaustion, Merkle attacks |
| **architectural/** | System-level | IPC bypass, Crash recovery |

### **integration/** - Choreography Tests
- Cross-subsystem event flow
- DDD/EDA pattern validation
- Runtime behavior simulation

## 🚀 Running Tests

```bash
# All tests
cargo test -p qc-tests

# By category
cargo test -p qc-tests integration::
cargo test -p qc-tests exploits::historical::
cargo test -p qc-tests exploits::modern::
cargo test -p qc-tests exploits::architectural::

# By subsystem
cargo test -p qc-tests exploits::modern::qc_02::
cargo test -p qc-tests exploits::modern::qc_06::

# Benchmarks
cargo bench -p qc-tests
cargo bench -p qc-tests -- qc_01
```

## 📊 Test Results (Verified)

| Category | Tests | Status |
|----------|-------|--------|
| Historical | 21 | ✅ PASS |
| Modern | 156 | ✅ PASS |
| Architectural | 69 | ✅ PASS |
| Integration | 35 | ✅ PASS |
| **TOTAL** | **281** | ✅ **ALL PASS** |

## 🔗 CI Integration

Tests are integrated into `ci-main.yml`:

```yaml
test:
  steps:
    - cargo test --all --lib              # Unit tests
    - cargo test --doc --all              # Doc tests
    - cargo test -p qc-tests integration:: # Integration
    - cargo test -p qc-tests exploits::    # Security
```

## 📈 Coverage by Subsystem

| Subsystem | Benchmarks | Historical | Modern | Architectural |
|-----------|------------|------------|--------|---------------|
| QC-01 | ✅ | ✅ Eclipse | - | - |
| QC-02 | ✅ | - | ✅ | - |
| QC-03 | ✅ | - | ✅ Merkle | ✅ Ghost TX |
| QC-04 | ✅ | - | ✅ State | - |
| QC-05 | - | - | ✅ Propagation | - |
| QC-06 | ✅ | ✅ Penny | ✅ Exhaustion | ✅ Zombie |
| QC-07 | ✅ | - | ✅ Bloom | - |
| QC-08 | ✅ | ✅ Timejack | ✅ Consensus | - |
| QC-09 | - | - | ✅ Finality | - |
| QC-10 | ✅ | - | ✅ Spoofing | - |
| QC-16 | - | - | ✅ API | - |
| Cross | - | ✅ Mt Gox | - | ✅ IPC, Crash |
