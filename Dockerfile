# ==============================================================================
# Quantum-Chain Node Dockerfile
# ==============================================================================
# Multi-stage build for the Quantum-Chain blockchain node.
# Architecture Reference: Documentation/Architecture.md V2.3
#
# Design Principles:
#   - Single-binary architecture (all 15 subsystems compiled into one binary)
#   - Minimal final image (~50MB)
#   - Non-root user for security
#   - Reproducible builds
#
# Usage:
#   docker build -t quantum-chain:latest .
#   docker run -p 30303:30303 -p 8545:8545 quantum-chain:latest
# ==============================================================================

# ==============================================================================
# STAGE 1: Build Stage
# ==============================================================================
ARG RUST_VERSION=1.75
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create workspace directory
WORKDIR /usr/src/quantum-chain

# Copy dependency manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy crates to cache dependencies
# This layer is cached and only re-runs if Cargo.toml changes
RUN mkdir -p crates/node-runtime/src && echo "fn main() {}" > crates/node-runtime/src/main.rs
RUN mkdir -p crates/shared-types/src && echo "" > crates/shared-types/src/lib.rs

# Create dummy subsystem crates
RUN for i in 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15; do \
    mkdir -p crates/qc-$i-*/src 2>/dev/null || true; \
    done

# Copy all Cargo.toml files for proper dependency resolution
COPY crates/node-runtime/Cargo.toml crates/node-runtime/
COPY crates/shared-types/Cargo.toml crates/shared-types/
COPY crates/qc-*/Cargo.toml crates/

# Build dependencies (cached layer)
RUN cargo fetch

# Copy actual source code
COPY . .

# Build the release binary
# All 15 subsystems are compiled into a single binary
RUN cargo build --release --bin node-runtime

# ==============================================================================
# STAGE 2: Runtime Stage
# ==============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -ms /bin/bash -u 1000 quantum

# Create data directories
RUN mkdir -p /var/quantum-chain/data \
    && mkdir -p /var/quantum-chain/config \
    && chown -R quantum:quantum /var/quantum-chain

# Copy the compiled binary from builder
COPY --from=builder /usr/src/quantum-chain/target/release/node-runtime /usr/local/bin/quantum-chain

# Set ownership
RUN chown quantum:quantum /usr/local/bin/quantum-chain

# Switch to non-root user
USER quantum

# Set working directory
WORKDIR /var/quantum-chain

# Expose ports
# P2P port for peer discovery and block propagation
EXPOSE 30303/tcp
EXPOSE 30303/udp
# RPC port for API access
EXPOSE 8545/tcp
# WebSocket port
EXPOSE 8546/tcp

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD /usr/local/bin/quantum-chain health || exit 1

# Environment variables
ENV QC_DATA_DIR=/var/quantum-chain/data
ENV QC_CONFIG_DIR=/var/quantum-chain/config
ENV QC_LOG_LEVEL=info
ENV RUST_BACKTRACE=1

# Volume for persistent data
VOLUME ["/var/quantum-chain/data", "/var/quantum-chain/config"]

# Labels
LABEL org.opencontainers.image.title="Quantum-Chain"
LABEL org.opencontainers.image.description="Modular Blockchain with Quantum-Inspired Architecture"
LABEL org.opencontainers.image.vendor="Quantum-Chain Contributors"
LABEL org.opencontainers.image.source="https://github.com/NerfedChou/Quantum-Chain"
LABEL org.opencontainers.image.documentation="https://github.com/NerfedChou/Quantum-Chain#readme"

# Entrypoint
ENTRYPOINT ["/usr/local/bin/quantum-chain"]

# Default command (can be overridden)
CMD ["--data-dir", "/var/quantum-chain/data"]
