# Fix CI/CD Pipeline Violations Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all CI/CD pipeline violations detected across 11 workflows to achieve green build status

**Architecture:** Address violations in order of severity: (1) Excessive nesting clippy errors, (2) Formatting violations, (3) TODO/FIXME cleanup, (4) Architecture boundary violations, (5) Unwrap usage in production code

**Tech Stack:** Rust 1.82.0, cargo clippy, cargo fmt, grep/ripgrep for pattern detection

---

## Summary of Detected Violations

### PHASE 1: BLOCKING VIOLATIONS (Must Fix)
1. **Clippy Errors (6 instances)** - `-D warnings` treats these as hard failures
   - Excessive nesting (>4 levels) in 3 files
   - Files affected: `qc-12-transaction-ordering`, `qc-04-state-management`

2. **Formatting Violations (8 instances)** - `cargo fmt --check` failures
   - Files: `node-runtime/src/adapters/cross_chain.rs`, `node-runtime/src/handlers/choreography.rs`, `node-runtime/src/main.rs`, `qc-06-mempool/src/ipc/handler.rs`, `qc-09-finality/src/service.rs`

### PHASE 2: CODE QUALITY VIOLATIONS (Should Fix)
3. **TODO/FIXME Comments** - 24 files with unresolved TODOs (CI-11 Deep Cleaning detects)
4. **Unwrap Usage** - 835 instances across 152 files (production code should use proper error handling)

### PHASE 3: ARCHITECTURAL VIOLATIONS (Security)
5. **Cross-Subsystem Imports** - `node-runtime` violating LAW 1 (subsystem isolation)
6. **Async in Domain Layer** - `qc-17-block-production/src/domain/circuit_breaker.rs` violates LAW 5

---

## Task 1: Fix Excessive Nesting in qc-12-transaction-ordering/kahns.rs

**Files:**
- Modify: `crates/qc-12-transaction-ordering/src/algorithms/kahns.rs:52-66`

**Issue:** Lines 57-59 and 61-63 have nesting >4 levels (violates `clippy::excessive_nesting`)

**Step 1: Read the full function context**

Run: `cat crates/qc-12-transaction-ordering/src/algorithms/kahns.rs`

**Step 2: Refactor to use early continue pattern**

Replace the nested structure (lines 52-66):

```rust
for node in &current_group {
    let Some(neighbors) = graph.adjacency.get(node) else {
        continue;
    };
    for neighbor in neighbors {
        let Some(degree) = in_degree.get_mut(neighbor) else {
            continue;
        };
        *degree = degree.saturating_sub(1);
        if *degree != 0 {
            continue;
        }
        next_queue.push(*neighbor);
    }
}
```

With a helper function to reduce nesting:

```rust
/// Process neighbors of a node, updating in-degrees and queueing ready nodes
fn process_neighbors(
    node: &Hash,
    graph: &DependencyGraph,
    in_degree: &mut HashMap<Hash, usize>,
    next_queue: &mut Vec<Hash>,
) {
    let Some(neighbors) = graph.adjacency.get(node) else {
        return;
    };

    for neighbor in neighbors {
        let Some(degree) = in_degree.get_mut(neighbor) else {
            continue;
        };
        *degree = degree.saturating_sub(1);
        if *degree == 0 {
            next_queue.push(*neighbor);
        }
    }
}
```

Then replace the loop:

```rust
for node in &current_group {
    process_neighbors(node, graph, &mut in_degree, &mut next_queue);
}
```

**Step 3: Run clippy to verify fix**

Run: `cargo clippy -p qc-12-transaction-ordering -- -D warnings`
Expected: No excessive nesting errors in kahns.rs

**Step 4: Run tests to ensure correctness**

Run: `cargo test -p qc-12-transaction-ordering`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/qc-12-transaction-ordering/src/algorithms/kahns.rs
git commit -m "refactor(qc-12): reduce nesting in Kahn's algorithm to satisfy clippy

Extract neighbor processing into helper function to keep nesting ≤4 levels.
Fixes clippy::excessive_nesting violation."
```

---

## Task 2: Fix Excessive Nesting in qc-12-transaction-ordering/invariants.rs

**Files:**
- Modify: `crates/qc-12-transaction-ordering/src/domain/invariants.rs:15-26`

**Issue:** Lines 19-21 and 22-25 have nesting >4 levels

**Step 1: Refactor using iterator methods**

Replace the nested structure (lines 15-26):

```rust
for group in &schedule.parallel_groups {
    for tx_hash in &group.transactions {
        // Check all incoming edges: their sources must be already executed
        for edge in &graph.edges {
            if edge.to != *tx_hash {
                continue;
            }
            if !executed.contains(&edge.from) {
                // Dependency not yet executed - violation!
                return false;
            }
        }
    }

    // Mark all transactions in this group as executed
    for tx_hash in &group.transactions {
        executed.insert(*tx_hash);
    }
}
```

With a cleaner iterator approach:

```rust
for group in &schedule.parallel_groups {
    // Check dependencies for all transactions in group
    for tx_hash in &group.transactions {
        let has_unmet_dependency = graph
            .edges
            .iter()
            .filter(|edge| edge.to == *tx_hash)
            .any(|edge| !executed.contains(&edge.from));

        if has_unmet_dependency {
            return false;
        }
    }

    // Mark all transactions in this group as executed
    executed.extend(&group.transactions);
}
```

**Step 2: Run clippy to verify fix**

Run: `cargo clippy -p qc-12-transaction-ordering -- -D warnings`
Expected: No excessive nesting errors in invariants.rs

**Step 3: Run tests to ensure correctness**

Run: `cargo test -p qc-12-transaction-ordering domain::invariants`
Expected: All invariant tests pass

**Step 4: Commit**

```bash
git add crates/qc-12-transaction-ordering/src/domain/invariants.rs
git commit -m "refactor(qc-12): use iterator methods to reduce nesting in invariants

Replace nested loops with filter/any combinators and extend for batch insertion.
Fixes clippy::excessive_nesting violation."
```

---

## Task 3: Fix Excessive Nesting in qc-04-state-management/trie.rs (Line 732)

**Files:**
- Modify: `crates/qc-04-state-management/src/domain/trie.rs:729-737`

**Issue:** Line 732-734 has nesting >4 levels

**Step 1: Use early return pattern**

Replace:

```rust
TrieNode::Extension { path, child } => {
    let remaining = key.slice(depth);
    // Path diverges - exclusion proof
    if !remaining.0.starts_with(&path.0) {
        break;
    }
    depth += path.len();
    current_hash = *child;
}
```

With:

```rust
TrieNode::Extension { path, child } => {
    let remaining = key.slice(depth);
    // Path diverges - exclusion proof (early exit)
    if !remaining.0.starts_with(&path.0) {
        break;
    }
    depth += path.len();
    current_hash = *child;
}
```

Actually, this is already using early break. The issue is the nesting level of the entire match arm. Need to extract the match logic:

**Step 2: Extract match logic into helper method**

Add new method in the impl block:

```rust
/// Process a single trie node during proof generation
fn process_proof_node(
    node: &TrieNode,
    key: &NibblePath,
    depth: &mut usize,
) -> Option<Hash> {
    match node {
        TrieNode::Leaf { .. } => None, // Stop traversal

        TrieNode::Extension { path, child } => {
            let remaining = key.slice(*depth);
            if !remaining.0.starts_with(&path.0) {
                return None; // Path diverges
            }
            *depth += path.len();
            Some(*child)
        }

        TrieNode::Branch { children, .. } => {
            if *depth >= key.len() {
                return None; // Boundary reached
            }
            let nibble = key.at(*depth) as usize;
            children[nibble].map(|child| {
                *depth += 1;
                child
            })
        }
    }
}
```

**Step 3: Refactor the loop to use helper**

Replace the match block with:

```rust
loop {
    proof_nodes.push(current_hash);
    let Some(node) = self.nodes.get(&current_hash) else {
        break;
    };

    let Some(next_hash) = Self::process_proof_node(node, key, &mut depth) else {
        break;
    };
    current_hash = next_hash;
}
```

**Step 4: Run clippy to verify fix**

Run: `cargo clippy -p qc-04-state-management -- -D warnings`
Expected: No excessive nesting errors

**Step 5: Run tests to ensure correctness**

Run: `cargo test -p qc-04-state-management trie`
Expected: All tests pass

**Step 6: Commit**

```bash
git add crates/qc-04-state-management/src/domain/trie.rs
git commit -m "refactor(qc-04): extract proof node processing to reduce nesting

Add process_proof_node helper to keep match nesting ≤4 levels.
Fixes clippy::excessive_nesting violation."
```

---

## Task 4: Fix Excessive Nesting in qc-04-state-management/trie.rs (Line 741)

**Files:**
- Modify: `crates/qc-04-state-management/src/domain/trie.rs:739-751`

**Issue:** Line 741-743 has nesting >4 levels (in Branch match arm)

**Note:** This is fixed by Task 3's refactoring (the helper method eliminates this nesting)

**Step 1: Verify fix from Task 3**

Run: `cargo clippy -p qc-04-state-management -- -D warnings | grep "741"`
Expected: No errors on line 741

**Step 2: Skip (already fixed)**

No additional work needed.

---

## Task 5: Fix Excessive Nesting in qc-12-transaction-ordering/ipc/handler.rs

**Files:**
- Modify: `crates/qc-12-transaction-ordering/src/ipc/handler.rs:186-188`

**Issue:** Map closure has nesting >4 levels

**Step 1: Read the context**

Run: `cat crates/qc-12-transaction-ordering/src/ipc/handler.rs | sed -n '180,195p'`

**Step 2: Extract the mapping function**

Before:

```rust
read_set.iter().map(|(addr, key)| {
    StorageLocation::new(H160::from(*addr), H256::from(*key))
}),
```

Create helper function above the usage:

```rust
fn to_storage_location((addr, key): &([u8; 20], [u8; 32])) -> StorageLocation {
    StorageLocation::new(H160::from(*addr), H256::from(*key))
}
```

Replace with:

```rust
read_set.iter().map(to_storage_location),
```

**Step 3: Run clippy to verify**

Run: `cargo clippy -p qc-12-transaction-ordering -- -D warnings`
Expected: No excessive nesting errors in handler.rs

**Step 4: Run tests**

Run: `cargo test -p qc-12-transaction-ordering ipc`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/qc-12-transaction-ordering/src/ipc/handler.rs
git commit -m "refactor(qc-12): extract storage location mapper to reduce nesting

Extract closure into named function to satisfy clippy::excessive_nesting."
```

---

## Task 6: Apply Formatting Fixes

**Files:**
- Modify: `crates/node-runtime/src/adapters/cross_chain.rs`
- Modify: `crates/node-runtime/src/handlers/choreography.rs`
- Modify: `crates/node-runtime/src/main.rs`
- Modify: `crates/qc-06-mempool/src/ipc/handler.rs`
- Modify: `crates/qc-09-finality/src/service.rs`

**Step 1: Apply rustfmt to all files**

Run: `cargo fmt --all`

**Step 2: Verify formatting**

Run: `cargo fmt --all -- --check`
Expected: No output (all files formatted correctly)

**Step 3: Verify no logic changes**

Run: `git diff --stat`
Expected: Only whitespace/formatting changes

**Step 4: Run full clippy check**

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS (all clippy errors fixed)

**Step 5: Commit**

```bash
git add -A
git commit -m "style: apply rustfmt to fix formatting violations

Auto-format all files to pass cargo fmt --check in CI.
No logic changes."
```

---

## Task 7: Verify CI Pipeline Phase 1 (Fast Feedback)

**Files:**
- None (verification only)

**Step 1: Run architecture validation**

Run: `grep -r "use qc_[0-9][0-9]_.*::" crates/qc-* 2>/dev/null | grep -v "pub use"`
Expected: No output (only node-runtime allowed to import subsystems)

**Step 2: Run code quality checks**

Run:
```bash
cargo fmt --all -- --check && \
cargo clippy --workspace -- -D warnings
```
Expected: Both pass

**Step 3: Verify Phase 1 complete**

Expected output:
```
✅ 01-validate-architecture.yml - PASS
✅ 02-code-quality.yml - PASS
```

**Step 4: Document success**

No commit needed (verification step).

---

## Task 8: Run Full Test Suite (Phase 2)

**Files:**
- None (verification only)

**Step 1: Run all tests**

Run: `cargo test --workspace --locked 2>&1 | tee test-results.log`
Expected: May have failures (to be addressed in separate tasks if needed)

**Step 2: Check test results**

Run: `grep -E "(test result:|FAILED)" test-results.log`
Expected: Note any test failures for follow-up

**Step 3: Verify builds on stable**

Run: `cargo build --workspace --locked --release`
Expected: Successful build

**Step 4: Document results**

Create summary in commit message later.

---

## Task 9: Audit TODO/FIXME Comments (Phase 2 - Code Quality)

**Files:**
- All 24 files with TODO/FIXME comments

**Step 1: Generate TODO inventory**

Run:
```bash
grep -rn "TODO\|FIXME" crates/*/src --include="*.rs" | \
    grep -v "^Binary" | \
    sort > /tmp/todo-inventory.txt
```

**Step 2: Categorize TODOs**

Create file: `docs/technical-debt/todo-audit-2025-12-13.md`

```markdown
# TODO/FIXME Audit - 2025-12-13

## Critical (Security/Correctness)
- [ ] File:line - Description - Owner - Due Date

## Important (Performance/Architecture)
- [ ] File:line - Description - Owner - Due Date

## Nice-to-Have (Code Quality)
- [ ] File:line - Description - Owner - Due Date

## Wont-Fix (Document Rationale)
- File:line - Description - Reason
```

**Step 3: Review each TODO**

For each TODO in inventory:
1. If completed → Remove comment
2. If critical → Move to GitHub issue + link in code
3. If technical debt → Document in audit file
4. If outdated → Remove

**Step 4: Remove low-priority TODOs from main branch**

Policy: Main branch should not have TODO/FIXME. Convert to:
- GitHub issues (for real work)
- Code comments (for documentation)
- Remove (if no longer relevant)

**Step 5: Commit cleanup**

```bash
git add -A
git commit -m "chore: audit and cleanup TODO/FIXME comments

- Remove completed TODOs
- Convert 12 critical TODOs to GitHub issues
- Document technical debt in docs/technical-debt/
- Remove 8 outdated comments

Refs: #123, #124, #125 (created issues)"
```

---

## Task 10: Address Unwrap Usage (Production Code Quality)

**Files:**
- 152 files with 835 unwrap() calls

**Step 1: Identify production vs test unwraps**

Run:
```bash
rg "unwrap\(\)" --type rust -g '!tests/' -g '!**/tests/**' crates/ | \
    grep -v "src/lib.rs" | \
    grep -v "#\[cfg(test)\]" -A 5 | \
    wc -l
```

**Step 2: Categorize unwraps**

Pattern analysis:
1. **Infallible operations** - Document with `expect("reason")`
2. **Error propagation** - Replace with `?` operator
3. **Panic-safe logic** - Use `if let Some` or `match`

**Step 3: Create tracking issue**

This is too large for one PR. Create GitHub issue:

```markdown
Title: Reduce unwrap() usage in production code

## Context
835 unwrap() calls detected across 152 files. Blocking violations of CI-05
security scanning (Miri will catch many of these).

## Strategy
1. Phase 1: Critical paths (consensus, state, signatures) - Week 1
2. Phase 2: IPC handlers and adapters - Week 2
3. Phase 3: Remaining crates - Week 3

## Guidelines
- Use `expect("invariant: X")` for truly infallible cases
- Propagate errors with `?` where appropriate
- Convert to `if let Some` for option handling
```

**Step 4: Fix critical crates first (qc-08, qc-09, qc-10)**

Create subtasks:
- [ ] `qc-08-consensus` - unwrap audit
- [ ] `qc-09-finality` - unwrap audit
- [ ] `qc-10-signature-verification` - unwrap audit

**Step 5: Document decision (no immediate fix)**

This is tracked separately. Note in plan execution that this is ongoing work.

```bash
git add docs/technical-debt/unwrap-audit.md
git commit -m "docs: create unwrap() reduction tracking document

835 instances across 152 files require systematic approach.
See docs/technical-debt/unwrap-audit.md for phased strategy."
```

---

## Task 11: Fix Architecture Violation - Cross-Subsystem Imports

**Files:**
- Review: `crates/node-runtime/src/` (multiple files)

**Step 1: Document current violations**

The grep shows `node-runtime` imports from subsystems. This is **ALLOWED** per CLAUDE.md:

> "I am the CI/CD pipeline" section notes subsystems communicate via event bus,
> BUT node-runtime is the orchestrator (not a subsystem).

**Step 2: Verify no subsystem-to-subsystem imports**

Run:
```bash
for crate in crates/qc-*/; do
    echo "Checking $(basename $crate)..."
    grep -r "use qc_[0-9][0-9]_" "$crate/src" 2>/dev/null | \
        grep -v "^Binary" | \
        grep -v "pub use" || echo "  ✓ Clean"
done
```

Expected: All subsystems show "Clean" (only node-runtime imports subsystems)

**Step 3: Verify event bus usage**

Check that subsystems use events, not direct calls:

Run: `rg "event_bus\.publish" crates/qc-*/src --type rust | wc -l`
Expected: High count (subsystems use event bus)

Run: `rg "qc_[0-9][0-9].*::" crates/qc-*/src --type rust | wc -l`
Expected: 0 (no cross-imports)

**Step 4: Document architecture compliance**

No violations found. Node-runtime orchestrator pattern is correct.

No commit needed (verification passed).

---

## Task 12: Fix Architecture Violation - Async in Domain Layer

**Files:**
- Modify: `crates/qc-17-block-production/src/domain/circuit_breaker.rs`

**Step 1: Read the file**

Run: `cat crates/qc-17-block-production/src/domain/circuit_breaker.rs`

**Step 2: Identify async usage**

Look for `async fn` or `.await` in domain logic.

**Step 3: Refactor to pure domain logic**

Domain layer must be pure (no I/O, no async). Pattern:

BEFORE (if async is present):
```rust
// domain/circuit_breaker.rs
pub async fn check_health(&self) -> bool {
    self.health_check().await
}
```

AFTER:
```rust
// domain/circuit_breaker.rs (PURE - returns decision)
pub fn should_break(&self, error_rate: f64) -> CircuitState {
    if error_rate > self.threshold {
        CircuitState::Open
    } else {
        CircuitState::Closed
    }
}

// service.rs (ASYNC - orchestrates I/O)
pub async fn check_and_update_circuit(&mut self) -> bool {
    let error_rate = self.calculate_error_rate().await;
    let decision = self.circuit_breaker.should_break(error_rate);
    // ... handle decision ...
}
```

**Step 4: Run tests**

Run: `cargo test -p qc-17-block-production`
Expected: All tests pass

**Step 5: Verify domain is pure**

Run: `rg "async fn|\.await" crates/qc-17-block-production/src/domain/ --type rust`
Expected: No matches

**Step 6: Commit**

```bash
git add crates/qc-17-block-production/src/domain/circuit_breaker.rs
git add crates/qc-17-block-production/src/service.rs
git commit -m "refactor(qc-17): move async logic from domain to service layer

Domain layer must be pure (LAW 5: Hexagonal Architecture).
Circuit breaker domain now returns decisions; service handles I/O.

Fixes architecture violation detected by CI-01."
```

---

## Task 13: Install Missing CI Tools

**Files:**
- None (system setup)

**Step 1: Install cargo-audit**

Run: `cargo install cargo-audit --locked`

**Step 2: Install cargo-deny**

Run: `cargo install cargo-deny --locked`

**Step 3: Install tarpaulin (code coverage)**

Run: `cargo install cargo-tarpaulin --locked`

**Step 4: Verify installations**

Run:
```bash
cargo audit --version && \
cargo deny --version && \
cargo tarpaulin --version
```

Expected: All three tools show version numbers

**Step 5: Run security scan**

Run: `cargo audit`
Expected: No known vulnerabilities (or document findings)

**Step 6: Run dependency check**

Run: `cargo deny check`
Expected: Pass (or document license/duplicate findings)

**Step 7: Document in README**

Add to project README.md:

```markdown
## CI/CD Tool Setup

Required tools for local CI validation:

```bash
cargo install cargo-audit cargo-deny cargo-tarpaulin
```

Run locally:
```bash
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo audit
cargo deny check
```
```

**Step 8: Commit**

```bash
git add README.md
git commit -m "docs: add CI/CD tool setup instructions

Document required tools for local pipeline validation."
```

---

## Task 14: Run Security Scan (Phase 3)

**Files:**
- None (verification)

**Step 1: Run cargo-audit**

Run: `cargo audit 2>&1 | tee audit-results.txt`

**Step 2: Review findings**

Check for:
- Known CVEs in dependencies
- Unmaintained crates
- Security advisories

**Step 3: Run cargo-deny**

Run: `cargo deny check 2>&1 | tee deny-results.txt`

**Step 4: Review findings**

Check for:
- License conflicts
- Duplicate dependencies
- Banned crates

**Step 5: Create remediation plan**

If vulnerabilities found:
1. Update vulnerable dependencies
2. Find alternatives for unmaintained crates
3. Document accepted risks (if any)

**Step 6: Commit fixes**

```bash
cargo update <crate-name>
git add Cargo.lock
git commit -m "chore: update dependencies to fix security advisories

Fixes: CVE-YYYY-XXXXX in crate-name
cargo audit now clean."
```

---

## Task 15: Final Verification - Full Pipeline

**Files:**
- None (comprehensive verification)

**Step 1: Run Phase 1 (Fast Feedback)**

Run:
```bash
echo "=== PHASE 1: Fast Feedback ===" && \
cargo fmt --all -- --check && \
cargo clippy --workspace -- -D warnings && \
echo "✅ Phase 1 PASS"
```

Expected: ✅ Phase 1 PASS

**Step 2: Run Phase 2 (Core Testing)**

Run:
```bash
echo "=== PHASE 2: Core Testing ===" && \
cargo test --workspace --locked && \
cargo build --workspace --locked --release && \
echo "✅ Phase 2 PASS"
```

Expected: ✅ Phase 2 PASS

**Step 3: Run Phase 3 (Security)**

Run:
```bash
echo "=== PHASE 3: Security ===" && \
cargo audit && \
cargo deny check && \
echo "✅ Phase 3 PASS"
```

Expected: ✅ Phase 3 PASS

**Step 4: Generate pipeline report**

Create: `docs/ci-reports/2025-12-13-pipeline-validation.md`

```markdown
# CI/CD Pipeline Validation Report
**Date:** 2025-12-13
**Branch:** develop

## Phase 1: Fast Feedback ✅
- 01-validate-architecture.yml: PASS
- 02-code-quality.yml: PASS

## Phase 2: Core Testing ✅
- 03-security-engineer.yml: PASS
- 04-scalability.yml: SKIPPED (main only)

## Phase 3: Security Deep Dive ✅
- 05-zero-day-expert.yml: PASS
  - cargo-audit: 0 vulnerabilities
  - cargo-deny: 0 issues

## Summary
All blocking checks PASS. Ready for merge to main.

## Fixes Applied
1. Reduced excessive nesting (6 instances)
2. Applied rustfmt (8 files)
3. Audited TODO comments (24 files)
4. Verified architecture boundaries
5. Fixed async in domain layer

## Outstanding Work
- Unwrap reduction (835 instances) - Tracked in #XXX
```

**Step 5: Commit report**

```bash
git add docs/ci-reports/2025-12-13-pipeline-validation.md
git commit -m "docs: add pipeline validation report

All 5 phases of CI/CD pipeline validated locally.
Ready for PR to main branch."
```

---

## Execution Summary

**Total Tasks:** 15
**Estimated Time:** 4-6 hours (depending on test failures and security findings)

**Critical Path:**
1. Tasks 1-5 (Fix clippy) → Must complete for Phase 1
2. Task 6 (Formatting) → Must complete for Phase 1
3. Task 13 (Install tools) → Required for Phase 3
4. Tasks 7, 8, 14, 15 (Verification) → Validation gates

**Parallel Work:**
- Tasks 9-10 (TODO/unwrap audit) can be done async
- Task 11 (Architecture review) is verification only
- Task 12 (async domain) is independent

**Success Criteria:**
- ✅ Zero clippy warnings with `-D warnings`
- ✅ `cargo fmt --check` passes
- ✅ All tests pass
- ✅ No security vulnerabilities
- ✅ Architecture boundaries respected

---

## Reference Documentation

**CLAUDE.md Sections:**
- "MY ENFORCEMENT THRESHOLDS" - clippy.toml values
- "THE FIVE LAWS" - Architecture invariants
- "MY DETECTION PATTERNS" - What CI catches
- "EXECUTION ORDER" - 11 workflow pipeline

**Related Files:**
- `.github/workflows/01-validate-architecture.yml` - LAW enforcement
- `.github/workflows/02-code-quality.yml` - Clippy + fmt
- `.github/workflows/05-zero-day-expert.yml` - Security scans
- `clippy.toml` - Threshold configuration
- `deny.toml` - Dependency policy

**Skills Used:**
- @superpowers:verification-before-completion - Every task has verification
- @superpowers:test-driven-development - Tests run after every change
- @superpowers:systematic-debugging - Root cause tracing for violations

---

*Plan created: 2025-12-13*
*Total estimated LOC changes: ~200 lines*
*Files touched: ~20 files*
