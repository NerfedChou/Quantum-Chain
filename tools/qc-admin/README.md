# QC-Admin: Subsystem Debug & Topology Control Panel

Administrative TUI for debugging, monitoring, and controlling Quantum-Chain subsystems.

## Purpose

While `qc-tui` is the **public read-only window** (what any wallet/dApp sees), `qc-admin` is the **back office** - a privileged debug and control panel for node operators.

## Features

- **Subsystem Dashboard** - Visual list of all 16 subsystems with live health status
- **Real-time System Metrics** - CPU/Memory usage from the local system
- **Drill-down Panels** - Select any subsystem to see detailed, subsystem-specific diagnostics
- **Component-based UI** - Each subsystem has its own dedicated panel renderer
- **Live Data Refresh** - Automatic polling with configurable interval
- **Demo Mode** - Run without a node for development/testing

## Installation

```bash
# Build from the tools directory
cd tools/qc-admin
cargo build --release

# Or run directly from repo root
cargo run --release -p qc-admin
```

## Usage

```bash
# Connect to default endpoint (http://127.0.0.1:8080)
qc-admin

# Connect to custom endpoint
qc-admin --endpoint http://localhost:8545

# Set refresh interval (default: 2 seconds)
qc-admin --refresh 5

# Run in demo mode (no API connection needed)
qc-admin --demo
```

## Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `-e, --endpoint` | `http://127.0.0.1:8080` | Admin API endpoint URL |
| `-r, --refresh` | `2` | Refresh interval in seconds |
| `--demo` | off | Run with fake data (no API needed) |

## Keyboard Controls

| Key | Action |
|-----|--------|
| `1-9` | Select subsystem qc-01 through qc-09 |
| `0` | Select qc-10 (Signature Verification) |
| `G` | Select qc-16 (API Gateway) |
| `↑/↓` | Navigate subsystem list |
| `Enter` | Drill down into subsystem |
| `B` | Back to previous view |
| `R` | Force refresh |
| `Q` | Quit |
| `?` | Show help overlay |

## Status Indicators

| Indicator | Meaning |
|-----------|---------|
| `● RUN` (green) | Subsystem running and healthy |
| `● WARN` (yellow) | Running but a dependency is down |
| `● STOP` (red) | Subsystem stopped or unreachable |
| `○ N/I` (gray) | Not implemented in codebase |

## Architecture

```
src/
├── main.rs           # Entry point, event loop, CLI args
├── lib.rs            # Library exports
├── domain/           # Domain models
│   ├── app.rs        # Application state
│   └── subsystem.rs  # SubsystemId, Status, Info
├── api/              # Admin API client
│   ├── client.rs     # HTTP/JSON-RPC client
│   └── types.rs      # API response types
└── ui/               # TUI components
    ├── layout.rs     # Main layout (header, body, footer)
    ├── left_panel.rs # Subsystem list + system health
    ├── right_panel.rs# Dispatch to subsystem renderers
    ├── widgets/      # Reusable UI components
    └── subsystems/   # Per-subsystem panel renderers
        ├── qc_01_peers.rs  # Peer Discovery panel
        └── ...             # One file per subsystem
```

## Subsystem Panels

Each implemented subsystem has its own dedicated panel showing subsystem-specific metrics:

| Subsystem | Panel Status | Key Metrics |
|-----------|--------------|-------------|
| qc-01 Peer Discovery | ✅ Implemented | Peers, buckets, banned, pending verification |
| qc-02 Block Storage | 🚧 Placeholder | - |
| qc-03 Transaction Indexing | 🚧 Placeholder | - |
| qc-04 State Management | 🚧 Placeholder | - |
| qc-05 Block Propagation | 🚧 Placeholder | - |
| qc-06 Mempool | 🚧 Placeholder | - |
| qc-07 Bloom Filters | ⬜ Not Implemented | - |
| qc-08 Consensus | 🚧 Placeholder | - |
| qc-09 Finality | 🚧 Placeholder | - |
| qc-10 Signature Verification | 🚧 Placeholder | - |
| qc-11 Smart Contracts | ⬜ Not Implemented | - |
| qc-12 Transaction Ordering | ⬜ Not Implemented | - |
| qc-13 Light Client Sync | ⬜ Not Implemented | - |
| qc-14 Sharding | ⬜ Not Implemented | - |
| qc-15 Cross-Chain | ⬜ Not Implemented | - |
| qc-16 API Gateway | 🚧 Placeholder | - |

## Security

- Connects to qc-16 **Tier 3 Admin endpoints** only
- **Localhost only** by default - Admin server binds to 127.0.0.1
- Does NOT modify blockchain state (read-only monitoring)
- System metrics read from `/proc` (Linux only)

## Data Sources

1. **System Metrics** (CPU/Memory): Read from local `/proc/stat` and `/proc/meminfo`
2. **Subsystem Health**: Fetched via `debug_subsystemHealth` JSON-RPC call
3. **Subsystem Metrics**: Fetched via `debug_subsystemStatus` for each subsystem

## Development

```bash
# Run in demo mode for UI development
cargo run -- --demo

# Build release binary
cargo build --release

# The binary will be at target/release/qc-admin
```
