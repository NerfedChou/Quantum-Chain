# QC-Compute: Portable GPU/CPU Compute Abstraction

A vendor-agnostic compute layer for Quantum-Chain that automatically selects the best available backend.

## Philosophy: No Vendor Lock-in

We explicitly **avoid CUDA-only code**. While CUDA is fast, it locks you into NVIDIA hardware. Our abstraction works on:

| Hardware | Backend |
|----------|---------|
| NVIDIA GPUs | Vulkan, OpenCL |
| AMD GPUs | Vulkan, OpenCL |
| Intel GPUs | Vulkan, OpenCL |
| Apple Silicon | MoltenVK (Vulkan), OpenCL |
| Raspberry Pi 4+ | V3DV (Vulkan) |
| Any CPU | Rayon (always available) |

## Subsystem Compute Requirements

| Subsystem | Workload | Best Backend | Reason |
|-----------|----------|--------------|--------|
| **QC-17** (Mining) | SHA256 hashing | 🎮 GPU | Embarrassingly parallel |
| **QC-10** (Signatures) | ECDSA/BLS verify | 🎮 GPU | Batch verification |
| **QC-03** (Merkle) | SHA256 tree | 🎮 GPU | Parallel hashing |
| **QC-04** (State) | Trie operations | 💻 CPU | Memory-bound, branching |
| **QC-07** (Bloom) | Bit operations | 💻 CPU | Simple, memory-bound |
| **QC-08** (Consensus) | Validation | 💻 CPU | Logic-heavy |
| **QC-06** (Mempool) | Sorting/filtering | 💻 CPU | Memory-bound |
| **QC-02** (Storage) | I/O operations | 💻 CPU | Disk-bound |

## Usage

```rust
use qc_compute::{auto_detect, ComputeEngine};

// Auto-detect best backend
let engine = auto_detect()?;
println!("Using: {}", engine.device_info().name);

// PoW Mining
let result = engine.pow_mine(
    &header_template,
    difficulty_target,
    0,           // nonce_start
    1_000_000,   // nonce_count
).await?;

if let Some((nonce, hash)) = result {
    println!("Found nonce: {}", nonce);
}

// Batch SHA256
let hashes = engine.batch_sha256(&[
    b"tx1".to_vec(),
    b"tx2".to_vec(),
    b"tx3".to_vec(),
]).await?;

// Batch signature verification
let results = engine.batch_verify_ecdsa(
    &messages,
    &signatures,
    &public_keys,
).await?;
```

## Features

```toml
[dependencies]
qc-compute = { path = "../qc-compute", features = ["cpu"] }

# Enable GPU backends
qc-compute = { path = "../qc-compute", features = ["cpu", "opencl"] }
qc-compute = { path = "../qc-compute", features = ["cpu", "vulkan"] }
```

| Feature | Description |
|---------|-------------|
| `cpu` (default) | Rayon-based CPU parallelism |
| `opencl` | OpenCL GPU compute |
| `vulkan` | Vulkan compute shaders |

## Docker Integration

```yaml
# docker-compose.yml
services:
  quantum-chain:
    # For GPU support, add:
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
    # Or for AMD:
    devices:
      - /dev/dri:/dev/dri
```

## Performance Expectations

| Operation | CPU (8 cores) | GPU (RTX 3080) |
|-----------|---------------|----------------|
| SHA256 (1M hashes) | ~800ms | ~50ms |
| ECDSA verify (10K) | ~2000ms | ~200ms |
| Mining (1M nonces) | ~1500ms | ~100ms |

## Backend Selection Logic

```
1. Try Vulkan (most portable)
   └─ Works on: NVIDIA, AMD, Intel, Apple (MoltenVK)
   
2. Try OpenCL
   └─ Works on: NVIDIA, AMD, Intel, Apple
   
3. Fall back to CPU/Rayon
   └─ Always works
```

## Why Not CUDA?

| CUDA | Our Approach |
|------|--------------|
| NVIDIA only | Works everywhere |
| Proprietary | Open standards |
| Requires CUDA toolkit | No special tooling |
| Complex setup | Simple feature flags |

We use **Vulkan** and **OpenCL** because:
- They're **open standards**
- They work on **all major GPUs**
- They don't require **proprietary drivers**
- They can be **cross-compiled**

## License

Unlicense - Public Domain
