# Architecture Exemption: node-runtime

## Status: ORCHESTRATOR (Not a Subsystem)

`node-runtime` is **not** a bounded context subsystem. It is the **orchestration layer** that wires subsystems together using dependency injection.

## Why This Is Not a LAW #1 Violation

The CI check in `.github/workflows/01-validate-architecture.yml` only scans:

```bash
for subsystem in crates/qc-*/; do  # Only matches qc-XX-* pattern
```

Since `node-runtime` is at `crates/node-runtime/` (no `qc-` prefix), it is **naturally excluded** from the subsystem isolation check.

This is intentional architectural design - the orchestration layer must import all subsystems to wire them together.

## Architectural Role

```
┌─────────────────────────────────────┐
│       node-runtime (Orchestrator)   │
│  - Wires up all subsystems          │
│  - Dependency injection container   │
│  - Event bus setup                  │
│  - NOT a bounded context            │
└─────────────────────────────────────┘
           │
           ▼ (dependency injection)
┌───────────────────────────────────┐
│     Subsystems (qc-01..qc-17)     │
│  - Bounded contexts               │
│  - Event-driven choreography      │
│  - No cross-subsystem imports     │
└───────────────────────────────────┘
```

## Compliance Strategy

### Allowed Imports in node-runtime:
- All `qc-XX` subsystem **ports** (traits)
- All `qc-XX` subsystem services for DI
- Event bus and shared types

### Forbidden in Subsystems (qc-XX):
- Imports from other `qc-XX` subsystems
- Only `shared-*` crate imports allowed

## Related Documentation

- `Documentation/Architecture.md` - V2.3 Bounded Contexts
- `.github/workflows/01-validate-architecture.yml` - Enforcement workflow
