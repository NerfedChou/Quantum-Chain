# ==============================================================================
# STAGE 1: Build Stage
#
# This stage uses the full Rust toolchain to compile the application with
# all necessary build dependencies. It is optimized for caching.
# ==============================================================================
FROM rust:1.72 AS builder

# Create workspace directory
WORKDIR /usr/src/quantum-chain

# Copy workspace manifest and lock file
COPY ./Cargo.toml ./Cargo.lock* ./

# Copy all crate manifests for workspace members
COPY ./crates/node-runtime/Cargo.toml ./crates/node-runtime/Cargo.toml
COPY ./crates/shared-types/Cargo.toml ./crates/shared-types/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p crates/node-runtime/src && echo "fn main() {}" > crates/node-runtime/src/main.rs
RUN mkdir -p crates/shared-types/src && echo "" > crates/shared-types/src/lib.rs

# Build dependencies only. This layer is cached until Cargo.toml changes.
RUN cargo build --release --verbose 2>&1 || true

# Now, copy the actual source code (overwrites dummy files)
COPY . .

# Build the final binary with real source code
RUN cargo build --release --verbose --bin node-runtime

# ==============================================================================
# STAGE 2: Final Stage
#
# This stage creates the final, minimal production image. It copies ONLY the
# compiled binary from the build stage, resulting in a small and secure image.
# ==============================================================================
FROM debian:bullseye-slim

# Set up a non-root user for security
RUN useradd -ms /bin/bash appuser

# Copy the compiled binary from the builder stage
# The binary is located in the target/release directory of the workspace
COPY --from=builder /usr/src/quantum-chain/target/release/node-runtime /usr/local/bin/quantum-chain-node

# Set the user and expose the default port
USER appuser
EXPOSE 30303

# Set the entrypoint for the container
ENTRYPOINT ["/usr/local/bin/quantum-chain-node"]
