# ==============================================================================
# Quantum-Chain Node Dockerfile
# ==============================================================================
# Multi-stage build for the Quantum-Chain blockchain node.
# Architecture Reference: Documentation/Architecture.md V2.3
#
# Design Principles:
#   - Single-binary architecture (all 15 subsystems compiled into one binary)
#   - Minimal final image (~50MB)
#   - Non-root user for security (UID 1000)
#   - Read-only root filesystem support
#   - No new privileges
#   - Reproducible builds
#
# Security Features:
#   - Multi-stage build (no build tools in final image)
#   - Non-root user (quantum:1000)
#   - Minimal base image (debian:bookworm-slim)
#   - No shell in production (can be removed)
#   - Explicit file permissions
#   - Health checks enabled
#
# Usage:
#   # Basic run
#   docker build -t quantum-chain:latest .
#   docker run -p 30303:30303 -p 8545:8545 quantum-chain:latest
#
#   # Secure run (recommended for production)
#   docker run -p 30303:30303 -p 8545:8545 \
#     --security-opt=no-new-privileges:true \
#     --cap-drop=ALL \
#     --read-only \
#     --tmpfs /tmp:rw,noexec,nosuid,size=64m \
#     quantum-chain:latest
# ==============================================================================

# ==============================================================================
# STAGE 1: Build Stage
# ==============================================================================
# Use stable Rust for edition2024 support (required by base64ct 1.8.0+)
ARG RUST_VERSION=stable
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

# Build arguments for reproducibility
ARG BUILD_DATE
ARG VCS_REF

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create workspace directory
WORKDIR /usr/src/quantum-chain

# Copy dependency manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy ALL crate directories to preserve structure
# This ensures Cargo.toml files are in correct locations
COPY crates/ crates/

# Create dummy source files for dependency caching
RUN echo "fn main() {}" > crates/node-runtime/src/main.rs && \
    echo "" > crates/shared-types/src/lib.rs && \
    for dir in crates/qc-*/; do \
        echo "" > "${dir}src/lib.rs"; \
    done

# Build dependencies (cached layer)
RUN cargo fetch

# Copy actual source code
COPY . .

# Build the release binary with security flags
# All 15 subsystems are compiled into a single binary
RUN RUSTFLAGS="-D warnings" \
    cargo build --release --bin node-runtime

# Strip debug symbols for smaller binary
RUN strip --strip-all /usr/src/quantum-chain/target/release/node-runtime 2>/dev/null || true

# ==============================================================================
# STAGE 2: Runtime Stage
# ==============================================================================
FROM debian:bookworm-slim AS runtime

# Build arguments
ARG BUILD_DATE
ARG VCS_REF

# Install runtime dependencies (minimal)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean \
    && rm -rf /var/cache/apt/archives/*

# Create non-root user for security
# Using explicit UID/GID for Kubernetes compatibility
RUN groupadd -g 1000 quantum \
    && useradd -u 1000 -g quantum -s /sbin/nologin -M quantum

# Create data directories with proper permissions
RUN mkdir -p /var/quantum-chain/data \
    && mkdir -p /var/quantum-chain/config \
    && mkdir -p /var/quantum-chain/logs \
    && chown -R quantum:quantum /var/quantum-chain \
    && chmod -R 750 /var/quantum-chain

# Copy the compiled binary from builder
COPY --from=builder --chown=quantum:quantum \
    /usr/src/quantum-chain/target/release/node-runtime \
    /usr/local/bin/quantum-chain

# Set proper permissions on binary
RUN chmod 550 /usr/local/bin/quantum-chain

# Switch to non-root user
USER quantum

# Set working directory
WORKDIR /var/quantum-chain

# Expose ports
# P2P port for peer discovery and block propagation (TCP + UDP)
EXPOSE 30303/tcp
EXPOSE 30303/udp
# RPC port for API access (JSON-RPC)
EXPOSE 8545/tcp
# WebSocket port for subscriptions
EXPOSE 8546/tcp
# Metrics port for Prometheus
EXPOSE 9090/tcp

# Health check
# Checks if the node is responsive
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/usr/local/bin/quantum-chain", "health"]

# Environment variables
ENV QC_DATA_DIR=/var/quantum-chain/data
ENV QC_CONFIG_DIR=/var/quantum-chain/config
ENV QC_LOG_DIR=/var/quantum-chain/logs
ENV QC_LOG_LEVEL=info
ENV RUST_BACKTRACE=1

# Security: Ensure no new privileges can be gained
# This is enforced at runtime with --security-opt=no-new-privileges:true

# Volume for persistent data
# These directories can be mounted read-write
VOLUME ["/var/quantum-chain/data", "/var/quantum-chain/config"]

# Labels (OCI spec compliant)
LABEL org.opencontainers.image.title="Quantum-Chain"
LABEL org.opencontainers.image.description="Modular Blockchain with Quantum-Inspired Architecture"
LABEL org.opencontainers.image.vendor="Quantum-Chain Contributors"
LABEL org.opencontainers.image.source="https://github.com/NerfedChou/Quantum-Chain"
LABEL org.opencontainers.image.documentation="https://github.com/NerfedChou/Quantum-Chain#readme"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.revision="${VCS_REF}"
# Custom labels
LABEL quantum-chain.architecture.version="2.3"
LABEL quantum-chain.subsystem.count="15"
LABEL quantum-chain.deployment.mode="monolithic"
LABEL quantum-chain.security.non-root="true"
LABEL quantum-chain.security.read-only-rootfs="supported"

# Entrypoint
ENTRYPOINT ["/usr/local/bin/quantum-chain"]

# Default command (can be overridden)
CMD ["--data-dir", "/var/quantum-chain/data", "--config-dir", "/var/quantum-chain/config"]
