---
description: Check for TODO comments and silenced code patterns
---

# Code Hygiene Check

This workflow checks for silenced code and TODO patterns to maintain code quality.

## Quick Check

```bash
# Check for silenced dead_code in production (should find NOTHING)
grep -rn '#\[allow(dead_code)\]' crates/ --include='*.rs' \
  | grep -v 'tests/' \
  | grep -v '#\[cfg(test)\]' \
  | grep -v 'mod tests'

# Check for silenced unused warnings
grep -rn '#\[allow(unused' crates/ --include='*.rs' \
  | grep -v 'tests/'

# Count TODOs
grep -rn 'TODO' crates/ --include='*.rs' | wc -l
```

## Policy

| Pattern | Production | Test Code |
|---------|------------|-----------|
| `#[allow(dead_code)]` | ❌ BLOCKED | ✅ Allowed |
| `#[allow(unused*)]` | ❌ BLOCKED | ✅ Allowed |
| `_field:` in structs | ⚠️ Warning | ✅ Allowed |
| `TODO` comments | ⚠️ Tracked | ⚠️ Tracked |
| `TODO CRITICAL` | ❌ BLOCKED | ❌ BLOCKED |

## When Violations Found

1. **Implement the code** - Make it work
2. **Remove it** - If truly not needed
3. **Move to test** - If only needed for testing

## Run Full Test Suite

// turbo-all
```bash
cargo test --all-features --workspace
```
