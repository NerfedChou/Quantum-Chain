# CI/CD Quick Start Guide

## 🚀 For Contributors

### Creating a PR

1. **Check your changes locally first**:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
cargo test --all
```

2. **Follow PR title format** (Conventional Commits):
```
✅ feat(qc-08): add BLS signature aggregation
✅ fix(mempool): resolve race condition in pool
✅ docs: update architecture diagrams
✅ refactor(consensus): extract validation logic
✅ test(qc-10): add signature verification tests

❌ Updated consensus module
❌ Fixed bugs
❌ Changes
```

3. **Keep PRs small**:
- Target: < 30 files, < 500 lines
- Hard limit: < 100 files, < 2000 lines
- Split large features into multiple PRs

4. **Push and wait for CI** (~8 minutes for small PRs):
```bash
git push origin feature/my-feature
```

### Understanding CI Results

Your PR will run through 3 phases:

```
Phase 1: Fast Feedback (~3 min)
  ├── PR size validation
  ├── Conventional Commits check
  ├── Architecture validation (DDD, EDA, Hexagonal)
  └── Code quality (Clippy, rustfmt)

Phase 2: Testing (~2-5 min)
  ├── Incremental tests (only changed crates)
  └── Integration tests

Phase 3: Security (~3 min)
  ├── Dependency review
  ├── cargo-audit (CVEs)
  └── cargo-deny (licenses)
```

### Common Failures & Fixes

#### "PR too large"
```bash
# Split into multiple PRs
git checkout -b my-feature-part1
git cherry-pick <commits>
git push origin my-feature-part1
```

#### "Commit format invalid"
```
Edit PR title to match format:
type(scope): description

Types: feat, fix, docs, refactor, test, chore
```

#### "Clippy errors"
```bash
cargo clippy --fix --all-targets --all-features
git add .
git commit -m "style: fix clippy warnings"
```

#### "Tests failed"
```bash
cargo test --all  # Run locally first
# Fix failing tests
git add .
git commit -m "test: fix failing tests"
```

---

## 🎯 For Reviewers

### What to Check

1. **Architecture Compliance**:
   - No direct subsystem imports (must use event bus)
   - Domain layer has no I/O (pure functions only)
   - Proper hexagonal structure (domain/ports/adapters)

2. **Code Quality**:
   - Clear variable names
   - Proper error handling (no `.unwrap()` in prod code)
   - Test coverage for new functionality

3. **Security**:
   - No secrets in code
   - Input validation present
   - No unsafe code (unless justified)

### Review Checklist

```markdown
- [ ] PR size is reasonable (< 100 files)
- [ ] Tests exist for new functionality
- [ ] Architecture compliance (check CI report)
- [ ] No security concerns
- [ ] Documentation updated if needed
- [ ] Conventional Commits format used
```

---

## 📊 CI/CD Metrics Dashboard

After workflows run, check the **Summary** tab for:

```markdown
## 📊 CI/CD Pipeline Metrics

### Phase Results
| Phase | Workflow | Status | Critical |
|-------|----------|--------|----------|
| 1️⃣ Fast Feedback | 01 • Architect | success | ✅ |
| 1️⃣ Fast Feedback | 02 • Quality | success | ✅ |
| 2️⃣ Core Testing | 03 • Security | success | ✅ |
| 3️⃣ Security | 05 • Zero-Day | success | ✅ |

### 🚀 Performance Insights
- Fast Feedback: ~3 min
- Incremental Testing: ~2 min (80% faster!)
- Security: ~3 min
```

---

## 🔒 Security Features

### Dependency Review (Automatic)
- Blocks high-severity vulnerabilities
- Enforces license compliance
- Comments on PR with findings

### License Enforcement
```
✅ Allowed: MIT, Apache-2.0, BSD-*, ISC
❌ Denied:  GPL-3.0, AGPL-3.0, SSPL-1.0
```

---

## 💡 Pro Tips

### Speed Up Your CI

1. **Make focused changes**: Only modify what's needed
2. **Run tests locally**: Catch issues before CI
3. **Small commits**: Easier to review and revert
4. **Rebase before push**: Keep history clean

### Incremental Testing

The system automatically detects which crates changed:
- Changed `qc-08-consensus`? → Only tests `qc-08-consensus`
- Changed `shared-bus`? → Tests everything (cascading)
- Push to `main`? → Always tests everything (safety)

This saves **60-80% CI time** on typical PRs!

---

## 📚 Documentation

| File | Purpose |
|------|---------|
| `QUICK-START.md` | This file (quick reference) |
| `ENHANCEMENTS.md` | Technical details & features |
| `BRANCH-PROTECTION.md` | Branch protection setup |
| `IMPLEMENTATION-SUMMARY.md` | Deployment summary |
| `ARCHITECTURE.md` | CI/CD architecture |
| `README.md` | Workflow usage guide |

---

## 🆘 Troubleshooting

### CI taking too long?

**Expected times**:
- Small PR (1-2 crates): ~8 minutes
- Large PR (10+ crates): ~15 minutes
- Full test suite: ~20 minutes

If longer, check:
1. Are you pushing to `main`? (full suite runs)
2. Did you change `shared-*` crates? (cascading tests)
3. Check GitHub Actions status page

### Status check not found?

1. Refresh the PR page
2. Re-run failed workflows from Actions tab
3. Check if workflow conditions are met

### Can't merge?

Required checks must pass:
- ✅ Architecture validation
- ✅ Code quality (Clippy, rustfmt)
- ✅ All tests pass
- ✅ Security checks pass (on `main`)

---

## 🎓 Learning Resources

### Conventional Commits
- Website: https://www.conventionalcommits.org/
- Format: `type(scope): description`
- Enables auto-changelog generation

### Architecture
Read `CLAUDE.md` for:
- LAW #1: Subsystem Isolation
- LAW #2: Event Bus Only Communication
- LAW #3: Envelope-Only Identity
- LAW #4: Test-Driven Development

### Rust Best Practices
```bash
# Format code
cargo fmt --all

# Lint code
cargo clippy --all-targets --all-features

# Test code
cargo test --all

# Check documentation
cargo doc --no-deps --open
```

---

## ✅ Checklist Before Push

```markdown
- [ ] Code formatted (cargo fmt)
- [ ] No clippy warnings (cargo clippy)
- [ ] Tests pass locally (cargo test)
- [ ] Tests added for new functionality
- [ ] PR title follows Conventional Commits
- [ ] PR is < 100 files (if not, split it)
- [ ] Documentation updated (if needed)
```

---

## 🎉 Success!

If all checks pass:
1. Your PR is ready for review
2. Request review from team members
3. Address review comments
4. Merge when approved!

**Typical timeline**: 
- CI: ~8 minutes
- Review: ~1 hour (depending on size)
- Total: ~1-2 hours to merge

---

**Questions?** Check the detailed docs or open an issue with the `ci/cd` label.
