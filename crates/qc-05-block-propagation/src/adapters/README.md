# Adapters

Per **Architecture.md v2.3** (Hexagonal Architecture), adapters are the outer layer
that implements the outbound port traits defined in `../ports/outbound.rs`.

## Design Decision

Adapters for qc-05-block-propagation are implemented in **node-runtime** rather than
in this crate. This is because:

1. Adapters depend on external infrastructure (P2P networking, other subsystems)
2. The subsystem library should remain a pure domain library with no I/O
3. node-runtime is the "wiring" layer that connects all subsystems

## Port Implementations

The following ports are implemented in `node-runtime/src/adapters/ports/block_propagation.rs`:

| Port Trait | Adapter Implementation |
|------------|------------------------|
| `PeerNetwork` | `BlockPropNetworkAdapter` |
| `ConsensusGateway` | `BlockPropConsensusAdapter` |
| `MempoolGateway` | `BlockPropMempoolAdapter` |
| `SignatureVerifier` | `BlockPropSignatureAdapter` |

## Testing

For unit/integration testing, mock implementations are provided in `../service.rs`
under the `#[cfg(test)]` module.

