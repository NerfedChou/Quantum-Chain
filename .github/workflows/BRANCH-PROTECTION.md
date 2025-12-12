# Branch Protection Configuration

## Overview

This document defines the **required status checks** for branch protection on `main` and `develop` branches. These checks ensure code quality, security, and architectural compliance before merge.

---

## 🔒 Required Status Checks

### Critical Checks (Always Required)

These checks **MUST PASS** before any PR can be merged to `main` or `develop`:

| Check Name | Workflow | Purpose | Block Merge |
|------------|----------|---------|-------------|
| `01 • Architect / PR: Governance Checks` | validate-architecture.yml | PR size & commit format | ✅ YES |
| `01 • Architect / DDD: Bounded Contexts` | validate-architecture.yml | Subsystem isolation | ✅ YES |
| `01 • Architect / EDA: Choreography Pattern` | validate-architecture.yml | Event-driven communication | ✅ YES |
| `01 • Architect / Hexagonal: Ports & Adapters` | validate-architecture.yml | Hexagonal structure | ✅ YES |
| `01 • Architect / IPC: Security Boundaries` | validate-architecture.yml | Envelope-only identity | ✅ YES |
| `02 • Quality / Lint: Clippy (Strict)` | code-quality.yml | Code linting | ✅ YES |
| `02 • Quality / Format: rustfmt` | code-quality.yml | Code formatting | ✅ YES |
| `03 • Security / Unit: Domain Layer` | unit-integration-tests.yml | Domain logic tests | ✅ YES |
| `03 • Security / Unit: Service Layer` | unit-integration-tests.yml | Service tests | ✅ YES |
| `03 • Security / Integration: Subsystems` | unit-integration-tests.yml | Cross-subsystem tests | ✅ YES |

### Security Checks (Main Branch Only)

Required only for merges to `main` (production):

| Check Name | Workflow | Purpose | Block Merge |
|------------|----------|---------|-------------|
| `05 • Zero-Day / Dependency Review` | vulnerability-scanning.yml | Malicious dependencies | ✅ YES |
| `05 • Zero-Day / Audit: Known CVEs` | vulnerability-scanning.yml | Known vulnerabilities | ✅ YES |
| `05 • Zero-Day / cargo-deny` | vulnerability-scanning.yml | License & advisory checks | ✅ YES |

### Optional Checks (Run But Don't Block)

These provide valuable feedback but don't block merge:

| Check Name | Workflow | Purpose | Block Merge |
|------------|----------|---------|-------------|
| `03 • Security / Validation: Test Isolation` | unit-integration-tests.yml | Test independence | ❌ NO |
| `03 • Security / Validation: TDD Compliance` | unit-integration-tests.yml | Test coverage metrics | ❌ NO |
| `07 • Optimization / Benchmarks` | benchmarks.yml | Performance tracking | ❌ NO |
| `09 • QA / Quality Assurance` | quality-assurance.yml | Additional QA checks | ❌ NO |

---

## ⚙️ GitHub Configuration

### Setting Up Branch Protection

1. Navigate to: **Settings** → **Branches** → **Branch protection rules**
2. Add rule for `main`:

```yaml
Branch name pattern: main

# Require status checks to pass before merging
✅ Require status checks to pass before merging
✅ Require branches to be up to date before merging

Status checks that are required:
  - "01 • Architect / PR: Governance Checks"
  - "01 • Architect / DDD: Bounded Contexts"
  - "01 • Architect / EDA: Choreography Pattern"
  - "01 • Architect / Hexagonal: Ports & Adapters"
  - "01 • Architect / IPC: Security Boundaries"
  - "02 • Quality / Lint: Clippy (Strict)"
  - "02 • Quality / Format: rustfmt"
  - "03 • Security / Unit: Domain Layer"
  - "03 • Security / Unit: Service Layer"
  - "03 • Security / Integration: Subsystems"
  - "05 • Zero-Day / Dependency Review"
  - "05 • Zero-Day / Audit: Known CVEs"
  - "05 • Zero-Day / cargo-deny"

# Additional protections
✅ Require a pull request before merging
  - Required approvals: 1
  - Dismiss stale PR approvals when new commits are pushed
  
✅ Require conversation resolution before merging
✅ Require linear history
✅ Include administrators
✅ Allow force pushes: DISABLED
✅ Allow deletions: DISABLED
```

3. Add rule for `develop`:

```yaml
Branch name pattern: develop

# Same as main but without Zero-Day checks
Status checks that are required:
  - "01 • Architect / PR: Governance Checks"
  - "01 • Architect / DDD: Bounded Contexts"
  - "01 • Architect / EDA: Choreography Pattern"
  - "01 • Architect / Hexagonal: Ports & Adapters"
  - "01 • Architect / IPC: Security Boundaries"
  - "02 • Quality / Lint: Clippy (Strict)"
  - "02 • Quality / Format: rustfmt"
  - "03 • Security / Unit: Domain Layer"
  - "03 • Security / Unit: Service Layer"
  - "03 • Security / Integration: Subsystems"

# Less strict approvals for develop
✅ Require a pull request before merging
  - Required approvals: 1
  
✅ Require conversation resolution before merging
✅ Include administrators
```

---

## 🚨 What Happens on Failure?

### PR Size Violation
```
❌ PR too large (120 files). Must be < 100 files.
   Split into multiple PRs for better review quality.

Action Required: Split your PR into smaller chunks
```

### Conventional Commit Violation
```
❌ PR title must follow Conventional Commits format

Format: type(scope): description

Examples:
  - feat(qc-08): add BLS signature aggregation
  - fix(consensus): prevent duplicate attestations

Action Required: Edit PR title
```

### Architecture Violation
```
❌ VIOLATION: qc-08-consensus has direct subsystem dependency!
   Found: use qc_02_block_storage::internal::StorageBlock;

Action Required: Use event bus for inter-subsystem communication
Reference: CLAUDE.md - LAW #2: Event Bus Only Communication
```

### Security Violation
```
❌ High severity vulnerability found in dependency: tokio v1.28.0
   CVE-2023-XXXXX: Denial of service vulnerability

Action Required: Update dependency to safe version
```

### Test Failure
```
❌ Unit tests failed for qc-08-consensus

Action Required: Fix failing tests or add tests for new functionality
Reference: CLAUDE.md - LAW #4: Test-Driven Development
```

---

## 📊 Merge Requirements Matrix

| Branch | Arch | Quality | Tests | Security | Approvals | Time |
|--------|------|---------|-------|----------|-----------|------|
| `main` | ✅ | ✅ | ✅ | ✅ | 1+ | ~10 min |
| `develop` | ✅ | ✅ | ✅ | ⚠️ | 1 | ~5 min |
| `feature/*` | - | - | - | - | 0 | - |

Legend:
- ✅ Required and blocking
- ⚠️ Run but not blocking
- `-` Not run

---

## 🔄 CI/CD Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Developer Pushes PR                      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              Phase 1: Fast Feedback (~5 min)                │
├─────────────────────────────────────────────────────────────┤
│  • PR Governance (size, commit format)          [BLOCKING]  │
│  • Architecture Validation (DDD, EDA, Hex)      [BLOCKING]  │
│  • Code Quality (Clippy, rustfmt)               [BLOCKING]  │
└─────────────────────────────────────────────────────────────┘
                            │
                     ✅ All Pass? │ ❌ Fail → Block Merge
                            ▼
┌─────────────────────────────────────────────────────────────┐
│             Phase 2: Core Testing (~5-10 min)               │
├─────────────────────────────────────────────────────────────┤
│  • Unit Tests (Domain)                          [BLOCKING]  │
│  • Unit Tests (Service)                         [BLOCKING]  │
│  • Integration Tests                            [BLOCKING]  │
│  • Test Isolation                            [NON-BLOCKING] │
└─────────────────────────────────────────────────────────────┘
                            │
                     ✅ All Pass? │ ❌ Fail → Block Merge
                            ▼
┌─────────────────────────────────────────────────────────────┐
│             Phase 3: Security (~3-5 min)                    │
├─────────────────────────────────────────────────────────────┤
│  • Dependency Review (main only)                [BLOCKING]  │
│  • Cargo Audit (CVEs)                           [BLOCKING]  │
│  • cargo-deny (licenses)                        [BLOCKING]  │
└─────────────────────────────────────────────────────────────┘
                            │
                     ✅ All Pass? │ ❌ Fail → Block Merge
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    ✅ READY TO MERGE                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 🎯 Best Practices

### For Contributors

1. **Before Creating PR**:
   ```bash
   # Run locally first
   cargo clippy --all-targets --all-features
   cargo fmt --all -- --check
   cargo test --all
   ```

2. **PR Title Format**:
   ```
   ✅ feat(qc-08): add signature aggregation
   ✅ fix(mempool): resolve race condition
   ❌ Updated consensus module
   ```

3. **Keep PRs Small**:
   - Target: < 30 files, < 500 lines
   - Hard limit: < 100 files, < 2000 lines
   - Split large features into multiple PRs

4. **Test Coverage**:
   - Write tests BEFORE implementation (TDD)
   - Aim for > 80% domain logic coverage
   - All tests must be isolated (no shared state)

### For Reviewers

1. **Check Architecture**:
   - No direct subsystem imports
   - Event bus used for communication
   - Domain layer has no I/O

2. **Check Security**:
   - No secrets in code
   - Proper error handling
   - Input validation

3. **Check Tests**:
   - Tests exist for new functionality
   - Tests are isolated and deterministic
   - Edge cases covered

---

## 🔧 Troubleshooting

### "Status check not found"

**Cause**: Check name changed or workflow not running

**Fix**:
1. Verify workflow file exists
2. Check if workflow conditions are met
3. Re-run workflows from GitHub UI

### "Branch protection prevents merge"

**Cause**: Required checks haven't run or failed

**Fix**:
1. Wait for all checks to complete
2. Fix any failing checks
3. Push new commits to trigger re-run

### "Stale branch"

**Cause**: `main` has new commits since PR created

**Fix**:
```bash
git fetch origin
git rebase origin/main
git push --force-with-lease
```

---

## 📈 Metrics Tracking

Track these metrics monthly:

| Metric | Target | Current |
|--------|--------|---------|
| PR merge time (median) | < 1 hour | - |
| CI pass rate (first run) | > 80% | - |
| Security violations blocked | Track | - |
| Architecture violations blocked | Track | - |
| Average PR size | < 30 files | - |

---

## 📚 References

- [GitHub Branch Protection Docs](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [CLAUDE.md](../../CLAUDE.md) - Architectural Laws
- [ARCHITECTURE.md](./ARCHITECTURE.md) - CI/CD Architecture

---

**Version**: 1.0  
**Last Updated**: 2025-12-12  
**Maintained By**: DevOps Team
