# Branch Protection Configuration Guide - CI/CD Fortress

This document describes the required branch protection settings to complete the CI/CD Fortress upgrade.

## Required Branch Protection Rules for `main`

Configure these settings in GitHub Repository Settings → Branches → Add rule:

### Basic Settings

| Setting | Value |
|---------|-------|
| Branch name pattern | `main` |
| Require a pull request before merging | ✅ |
| Required approving reviews | 2 |
| Dismiss stale reviews | ✅ |
| Require review from Code Owners | ✅ |
| Require approval of most recent reviewable push | ✅ |

### Status Checks

| Setting | Value |
|---------|-------|
| Require status checks to pass | ✅ |
| Require branches to be up to date | ✅ |

**Required Status Checks:**

```
✅ 01 • Architect / Summary: Architecture Report
✅ 02 • Quality / Summary: Quality Report
✅ 03 • Security / Summary: Test Report
✅ 05 • Zero-Day / Summary: Security Report
✅ ✅ CI Success
```

### Additional Protection

| Setting | Value |
|---------|-------|
| Require signed commits | ✅ (Recommended) |
| Require linear history | ✅ |
| Include administrators | ✅ |
| Restrict who can push | ✅ (quantum-chain/release-managers) |
| Allow force pushes | ❌ |
| Allow deletions | ❌ |

### Lock Branch (Optional)

| Setting | Value |
|---------|-------|
| Lock branch | ❌ (unless release freeze) |

---

## Required Branch Protection Rules for `develop`

| Setting | Value |
|---------|-------|
| Branch name pattern | `develop` |
| Require a pull request before merging | ✅ |
| Required approving reviews | 1 |
| Require status checks to pass | ✅ |

**Required Status Checks:**

```
✅ 01 • Architect / Summary: Architecture Report
✅ 02 • Quality / Summary: Quality Report
✅ ✅ CI Success
```

---

## GitHub Environment Configuration

### Production Environment

1. Go to Settings → Environments → New environment
2. Name: `production`
3. Configure:
   - Required reviewers: `quantum-chain/release-managers` (2 reviewers)
   - Wait timer: 5 minutes (optional)
   - Deployment branches: `main` only

### Secrets Configuration

| Secret Name | Scope | Description |
|-------------|-------|-------------|
| `GITHUB_TOKEN` | Repository | Default, read/write |
| `GHCR_TOKEN` | Production env | Container registry push |
| `SLACK_WEBHOOK` | Repository | CI notifications |
| `METRICS_ENDPOINT` | Repository | Observability metrics |

---

## Verification Checklist

After configuring branch protection, verify:

- [ ] PRs to `main` require 2 approvals
- [ ] PRs to `main` require all status checks to pass
- [ ] Code Owners are auto-requested for review
- [ ] Administrators cannot bypass checks
- [ ] Force pushes are blocked
- [ ] Production deployments require approval

---

## Monitoring

The CI/CD Fortress generates metrics in the GitHub Step Summary. For external monitoring:

1. Configure `METRICS_ENDPOINT` secret pointing to your observability platform
2. CI telemetry job exports:
   - Workflow name
   - Run ID
   - Status (success/failure)
   - Duration
   - Branch and SHA
   - Actor

---

## Rollback Procedure

If CI/CD Fortress is too strict during initial rollout:

1. Temporarily disable `ZERO_TOLERANCE` in workflow_dispatch inputs
2. Review failed checks and adjust thresholds
3. Re-enable after tuning

**Never disable branch protection rules as a workaround.**
