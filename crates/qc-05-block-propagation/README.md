# qc-05-block-propagation

**Block Propagation Subsystem - Network & Gossip**

[![Status](https://img.shields.io/badge/status-implemented-green.svg)]()
[![Security](https://img.shields.io/badge/security-shared_ipc-blue.svg)]()

## Overview

Distributes validated blocks across the P2P network using epidemic gossip protocol. Implements BIP152-style compact block relay for bandwidth efficiency.

## Architecture Role

```
[Consensus (8)] ──PropagateBlockRequest──→ [Block Propagation (5)]
                                                   │
                                                   ↓ gossip (fanout=8)
                                           ┌───────┴───────┐
                                           ↓               ↓
                                      [Peer A]        [Peer B] ...
```

## Features

- **Gossip Protocol**: Epidemic block propagation with configurable fanout
- **Compact Block Relay**: BIP152-style for bandwidth efficiency
- **Short Transaction IDs**: SipHash-based 6-byte identifiers
- **Rate Limiting**: Per-peer announcement limits
- **Deduplication**: LRU cache for seen blocks
- **Reputation-Based Routing**: Prioritize reliable peers

## Security (IPC-MATRIX.md)

- Only Consensus (8) can request block propagation
- HMAC signature validation on all IPC messages
- Invalid signatures → SILENT DROP (IP spoofing defense)

## Dependencies

| Subsystem | Purpose |
|-----------|---------|
| 1 (Peer Discovery) | Peer list for gossip |
| 6 (Mempool) | Transaction lookup for compact blocks |
| 8 (Consensus) | Validated blocks + submit received blocks |
| 10 (Sig Verify) | Block signature verification |

## Configuration

```toml
[block_propagation]
fanout = 8
max_announcements_per_second = 1
max_block_size_bytes = 10485760  # 10 MB
seen_cache_size = 10000
reconstruction_timeout_ms = 5000
request_timeout_ms = 10000
enable_compact_blocks = true
```

## Tests

```bash
cargo test -p qc-05-block-propagation
```

25 unit tests covering:
- Short ID calculation and collision resistance
- Compact block creation and reconstruction
- Rate limiting and deduplication
- Peer selection by reputation
- IPC security validation
