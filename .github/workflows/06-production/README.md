# 06 • Production Manager

## Status: 🚨 HIGH PRIORITY (Docker replacement needed)

## Purpose
Docker builds, deployment, end-to-end testing.

## Implementation Plan

### Phase 1 (Week 2) - URGENT
- [ ] Replace docker-publish.yml functionality
- [ ] Build monolithic Docker image
- [ ] Push to registry

### Phase 2 (Week 3)
- [ ] Build per-subsystem images
- [ ] Multi-stage builds
- [ ] Image optimization

### Phase 3 (Week 4)
- [ ] End-to-end testing
- [ ] Deployment automation
- [ ] Rollback procedures

## Why High Priority?
We archived `docker-publish.yml` but need Docker builds for:
- Development environments
- Production deployment
- Testing infrastructure

## Will Implement When:
- ✅ ci-main.yml passes first time
- ✅ Core functionality validated
- **Target**: Week 2

## Dependencies
- Requires: ci-main.yml working, tests passing
- Blocks: Production deployment
- Priority: **HIGH**

## Estimated Time
- Design: 3 hours
- Implementation: 6 hours
- Testing: 3 hours
- **Total**: 1.5 days

## References
- Old: `.github/workflows/archive/docker-publish.yml`
- [Multi-stage Docker builds](https://docs.docker.com/build/building/multi-stage/)
- Dockerfile in repo root
