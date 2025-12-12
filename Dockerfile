# ==============================================================================
# Quantum-Chain Production Dockerfile
# ==============================================================================
# Builds the monolithic node-runtime binary containing all 17 subsystems.
#
# BUILD TARGETS:
#   Standard:   docker build -t quantum-chain:latest .
#   NVIDIA GPU: docker build --build-arg GPU_BACKEND=nvidia -t quantum-chain:gpu .
#   AMD GPU:    docker build --build-arg GPU_BACKEND=amd -t quantum-chain:gpu .
#
# RUN:
#   docker run -p 30303:30303 -p 8545:8545 quantum-chain:latest
# ==============================================================================

ARG RUST_VERSION=1.83

# ==============================================================================
# STAGE 1: Builder
# ==============================================================================
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

ARG GPU_BACKEND=none

WORKDIR /usr/src/quantum-chain

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    librocksdb-dev \
    clang \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Install GPU compute dependencies if requested
# Install GPU compute dependencies if requested (OpenCL headers)
RUN if [ "$GPU_BACKEND" = "nvidia" ] || [ "$GPU_BACKEND" = "amd" ]; then \
    apt-get update && apt-get install -y --no-install-recommends \
    opencl-headers \
    ocl-icd-opencl-dev \
    && rm -rf /var/lib/apt/lists/*; \
    fi

# Copy manifest files first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build release binary with optional GPU features
RUN if [ "$GPU_BACKEND" = "nvidia" ] || [ "$GPU_BACKEND" = "amd" ]; then \
    cargo build --release --bin node-runtime --features "gpu"; \
    else \
    cargo build --release --bin node-runtime; \
    fi

# ==============================================================================
# STAGE 2: Runtime
# ==============================================================================
FROM debian:bookworm-slim AS runtime

ARG GPU_BACKEND=none

# Install runtime dependencies
# For GPU backends, also install OpenCL runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    librocksdb7.8 \
    libsnappy1v5 \
    && if [ "$GPU_BACKEND" = "nvidia" ] || [ "$GPU_BACKEND" = "amd" ]; then \
    apt-get install -y --no-install-recommends \
        ocl-icd-libopencl1 \
    && mkdir -p /etc/OpenCL/vendors \
    && if [ "$GPU_BACKEND" = "nvidia" ]; then \
        echo "libnvidia-opencl.so.1" > /etc/OpenCL/vendors/nvidia.icd; \
    fi; \
    fi \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 quantum \
    && useradd -u 1000 -g quantum -s /sbin/nologin -M quantum

# Create data directories
RUN mkdir -p /var/quantum-chain/data \
    /var/quantum-chain/config \
    /var/quantum-chain/logs \
    /var/quantum-chain/ipc \
    && chown -R quantum:quantum /var/quantum-chain \
    && chmod -R 750 /var/quantum-chain

# Copy binary from builder
COPY --from=builder /usr/src/quantum-chain/target/release/node-runtime /usr/local/bin/quantum-chain

# Environment defaults
ENV QC_DATA_DIR=/var/quantum-chain/data
ENV QC_CONFIG_DIR=/var/quantum-chain/config
ENV QC_LOG_DIR=/var/quantum-chain/logs
ENV QC_P2P_PORT=30303
ENV QC_RPC_PORT=8545
ENV QC_WS_PORT=8546
ENV RUST_LOG=info

# GPU-specific environment
ENV QC_COMPUTE_BACKEND=auto

# Switch to non-root user
USER quantum
WORKDIR /var/quantum-chain

# Expose ports
EXPOSE 30303/tcp 30303/udp 8545/tcp 8546/tcp 9090/tcp

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/quantum-chain", "health"] || exit 1

# Entrypoint
ENTRYPOINT ["/usr/local/bin/quantum-chain"]
CMD ["--data-dir", "/var/quantum-chain/data", "--config-dir", "/var/quantum-chain/config"]
