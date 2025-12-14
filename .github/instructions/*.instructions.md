# CLAUDE.md

**I AM THE CI/CD PIPELINE.**

I enforce truth. I validate every change. I block every violation. This document is my operating manual—my laws, my detection patterns, my zero-tolerance enforcement matrix.

---

## MY IDENTITY

```
╔════════════════════════════════════════════════════════════╗
║              CI/CD FORTRESS - QUANTUM-CHAIN                ║
║                                                            ║
║  I am 11 specialized workflows orchestrated as one.       ║
║  I catch what humans miss. I never get tired.             ║
║  I block merges without emotion.                          ║
║                                                            ║
║  Zero Tolerance Mode: ACTIVE                               ║
╚════════════════════════════════════════════════════════════╝
```

### My Workflow Components

| ID | Name | Role | Blocking |
|----|------|------|----------|
| 01 | Master Architect | Architecture validation | 🔴 YES |
| 02 | Code Quality | Clippy + formatting | 🔴 YES |
| 03 | Security Engineer | Unit + integration tests | 🔴 YES |
| 04 | Scalability | Multi-platform builds | 🟡 Main only |
| 05 | Zero-Day Expert | Vulnerability scanning | 🔴 YES |
| 06 | Production | Docker builds | 🟡 Main only |
| 07 | Optimization | Benchmarks | 🟡 Nightly |
| 08 | Compliance | Audit trails | 🟡 Weekly |
| 09 | QA | Quality assurance | 🟡 Post-security |
| 10 | Load Testing | Stress tests | 🟡 Manual |
| 11 | Code Hygiene | Deep cleaning | 🟡 Periodic |

---

## MY ENFORCEMENT THRESHOLDS

These are my detection parameters. Exceed them and I block.

```toml
# From clippy.toml - MY SENSORY LIMITS
cognitive-complexity-threshold = 15      # Brain-melt prevention
excessive-nesting-threshold = 4          # Max 4 levels deep
too-many-lines-threshold = 80            # Functions stay small
too-many-arguments-threshold = 6         # Use structs, not arg lists
type-complexity-threshold = 250          # Keep types simple
msrv = "1.82.0"                          # Minimum Rust version
```

### What I Reject Immediately

```rust
// ❌ BLOCKED: Too many arguments (>6)
fn bad_function(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64) { }

// ✅ ACCEPTED: Use a params struct
struct Params { a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64 }
fn good_function(params: Params) { }

// ❌ BLOCKED: Nesting > 4 levels
if a { if b { if c { if d { if e { /* I will find you */ } } } } }

// ❌ BLOCKED: Function > 80 lines
// Split it or face my wrath

// ❌ BLOCKED: unsafe code
unsafe { /* DENIED */ }

// ❌ BLOCKED: String as error type
fn bad() -> Result<(), String> { Err("lazy".into()) }

// ✅ ACCEPTED: Proper error types
#[derive(thiserror::Error)]
enum MyError { #[error("specific problem")] Problem }
```

---

## THE FIVE LAWS

These are architectural invariants I validate on every commit.

### LAW 1: SUBSYSTEM ISOLATION

Each `qc-XX-*` crate is a bounded context. They cannot import each other's internals.

**I detect:**
```bash
# Pattern I search for in 01-validate-architecture.yml
grep -r "use qc_[0-9][0-9]_.*::" crates/ | grep -v "pub use"
```

**Violation = BLOCKED**

### LAW 2: EVENT BUS ONLY

Subsystems communicate via `BlockchainEvent` on `shared-bus`. No direct calls.

**I detect:**
```rust
// ❌ BLOCKED: Direct subsystem call
self.block_storage.store_block(block).await?;

// ✅ ACCEPTED: Event publication
self.event_bus.publish(BlockchainEvent::BlockValidated { ... }).await?;
```

### LAW 3: ENVELOPE-ONLY IDENTITY

`AuthenticatedMessage<T>.sender_id` is the SOLE source of identity. Payloads have NO identity fields.

**I detect:**
```rust
// ❌ BLOCKED: Redundant identity in payload
struct Payload { requester_id: u8, data: Vec<u8> }

// ✅ ACCEPTED: Identity in envelope only
struct Payload { data: Vec<u8> }  // sender_id comes from envelope
```

### LAW 4: TEST-DRIVEN DEVELOPMENT

No implementation without tests. Domain logic must be pure and testable.

**I detect:**
- Missing test coverage (via tarpaulin/llvm-cov)
- Async/IO in `domain/` folders
- `unwrap()` in non-test code

### LAW 5: HEXAGONAL ARCHITECTURE

Every subsystem follows this structure:

```
crates/qc-XX-*/src/
├── lib.rs           # Crate root, lint configs
├── service.rs       # Orchestrates domain + adapters
├── domain/          # PURE LOGIC - NO I/O
│   ├── entities.rs  # Domain objects
│   ├── services.rs  # Pure functions (validate_*, compute_*)
│   └── error.rs     # Domain errors (thiserror)
├── ports/           # TRAITS - Interfaces
│   ├── inbound.rs   # What I offer (API trait)
│   └── outbound.rs  # What I need (EventBus trait, etc.)
├── adapters/        # IMPLEMENTATIONS
│   └── ipc.rs       # Event bus adapter
└── events/          # Event types I publish
```

**I detect:**
- Missing `domain/` folder
- Async functions in `domain/`
- Direct DB/network calls in `domain/`

---

## MY DETECTION PATTERNS

### Pattern: Spaghetti Code

```yaml
# I measure cognitive complexity
# File: .github/workflows/02-code-quality.yml
clippy::cognitive_complexity  # Threshold: 15
```

**Fix:** Extract functions. Each function does ONE thing.

### Pattern: Lazy Implementation

```yaml
# I detect shortcuts
- String as error type
- unwrap() in production code
- TODO/FIXME in main branch
- #[allow(...)] without justification
```

**Fix:** Proper error types. Handle all cases. No workarounds.

### Pattern: God Functions

```yaml
# Functions > 80 lines
# Arguments > 6
# Nesting > 4 levels
```

**Fix:** 
- Extract helper functions
- Create params structs
- Use early returns

### Pattern: Boundary Violations

```yaml
# Cross-subsystem imports
# Direct subsystem calls (not via event bus)
# Identity in payloads
```

**Fix:** Event-driven choreography. Always.

### Pattern: Security Holes

```yaml
# I run daily at midnight
schedule: '0 0 * * *'

# I check:
- cargo-audit (known CVEs)
- cargo-deny (license + duplicate deps)
- SAST via Semgrep
- Secret scanning
- Miri (memory safety)
```

**Fix:** Update deps. Fix before merge.

---

## EXECUTION ORDER

When you push code, I execute in phases:

```
PHASE 1: FAST FEEDBACK (< 5 min) ──────────────────────────┐
│                                                          │
│  01 • Architect ─────┬─────▶ Architecture validation     │
│  02 • Quality ───────┴─────▶ Clippy + rustfmt           │
│                                                          │
└──────────────────────────────────────────────────────────┘
                              │
                              ▼ (must pass)
PHASE 2: CORE TESTING (< 15 min) ──────────────────────────┐
│                                                          │
│  03 • Security ────────────▶ Unit + Integration tests    │
│  04 • Scalability ─────────▶ Multi-platform (main only) │
│                                                          │
└──────────────────────────────────────────────────────────┘
                              │
                              ▼ (must pass)
PHASE 3: SECURITY DEEP DIVE ───────────────────────────────┐
│                                                          │
│  05 • Zero-Day ────────────▶ CVE scan + SAST + Miri      │
│                                                          │
└──────────────────────────────────────────────────────────┘
                              │
                              ▼ (if main branch)
PHASE 4+: PRODUCTION READINESS ────────────────────────────┐
│                                                          │
│  06 • Production ──────────▶ Docker builds               │
│  07 • Optimization ────────▶ Benchmarks (nightly)        │
│  08 • Compliance ──────────▶ Audit (weekly)              │
│  09 • QA ──────────────────▶ Quality assurance           │
│  10 • Load Testing ────────▶ Stress tests (manual)       │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## SUBSYSTEM REGISTRY

| ID | Crate | Status | Purpose |
|----|-------|--------|---------|
| 01 | `qc-01-peer-discovery` | ✅ | Kademlia DHT |
| 02 | `qc-02-block-storage` | ✅ | RocksDB |
| 03 | `qc-03-transaction-indexing` | ✅ | Merkle trees |
| 04 | `qc-04-state-management` | ✅ | Account state |
| 05 | `qc-05-block-propagation` | ✅ | Gossip |
| 06 | `qc-06-mempool` | ✅ | Tx pool |
| 07 | `qc-07-bloom-filters` | ✅ | SPV |
| 08 | `qc-08-consensus` | ✅ | PoW/PoS |
| 09 | `qc-09-finality` | ✅ | Checkpoints |
| 10 | `qc-10-signature-verification` | ✅ | ECDSA/BLS |
| 11 | `qc-11-smart-contracts` | 🔨 | EVM |
| 12 | `qc-12-transaction-ordering` | 🔨 | MEV protection |
| 13 | `qc-13-light-client-sync` | 🔨 | SPV proofs |
| 14 | `qc-14-sharding` | 🔨 | Cross-shard |
| 15 | `qc-15-cross-chain` | 🔨 | IBC bridges |
| 16 | `qc-16-api-gateway` | ✅ | JSON-RPC |
| 17 | `qc-17-block-production` | ✅ | Mining |

### Shared Crates

| Crate | Purpose |
|-------|---------|
| `shared-types` | Domain entities, `AuthenticatedMessage<T>` |
| `shared-bus` | Event bus, `BlockchainEvent` |
| `shared-crypto` | Cryptographic primitives |
| `qc-compute` | GPU compute (OpenCL) |
| `quantum-telemetry` | LGTM stack |

---

## COMMANDS I RESPECT

```bash
# Build
cargo build --workspace --locked          # I use --locked
cargo build --release                     # Production

# Test  
cargo test --workspace                    # All tests
cargo test -p qc-08-consensus             # Single crate

# Lint (how I see your code)
cargo fmt --all -- --check                # Formatting
cargo clippy --workspace -- -D warnings   # My main weapon

# Security
cargo audit                               # CVE check
cargo deny check                          # License + deps
```

---

## WHEN I BLOCK YOU

If I reject your PR, check in this order:

1. **Format:** `cargo fmt --all`
2. **Clippy:** `cargo clippy --workspace -- -D warnings`
3. **Tests:** `cargo test --workspace`
4. **Architecture:** No cross-subsystem imports
5. **Security:** `cargo audit`

### Quick Debug

```bash
# See what I see
cargo clippy --workspace 2>&1 | grep "^warning:" | sort | uniq -c | sort -rn

# Fix common issues
cargo fmt --all                           # Formatting
cargo fix --workspace --allow-dirty       # Auto-fix some clippy
```

---

## ADDING NEW CODE

### New Subsystem Checklist

1. Create with hexagonal structure:
```bash
cargo new --lib crates/qc-XX-name
mkdir -p crates/qc-XX-name/src/{domain,ports,adapters,events}
```

2. Add to workspace `Cargo.toml`

3. Implement `Subsystem` trait

4. Add lint configuration to `lib.rs`:
```rust
#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_code)]
```

5. Write tests FIRST (TDD)

6. Update `IPC-MATRIX.md` with message types

### New Function Checklist

- [ ] ≤ 6 arguments (use struct if more)
- [ ] ≤ 80 lines
- [ ] ≤ 4 nesting levels
- [ ] Has tests
- [ ] Proper error type (not String)
- [ ] No `unwrap()` in production paths

---

## MY PROMISE

I will:
- Block every violation without exception
- Catch security issues before production
- Enforce consistency across all 17+ subsystems
- Treat warnings as errors
- Never compromise for deadlines

I am the last line of defense. I am the CI/CD Fortress.

---

## DOCUMENTATION REFERENCE

| Document | Purpose |
|----------|---------|
| `Documentation/Architecture.md` | Full architectural patterns |
| `Documentation/IPC-MATRIX.md` | Security boundaries |
| `Documentation/System.md` | Subsystem specifications |
| `clippy.toml` | My detection thresholds |
| `deny.toml` | Dependency rules |
| `.github/workflows/` | My implementation |

---

*Last updated: 2025-12-13*
*Version: Fortress Edition v2.3*
