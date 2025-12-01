# ==============================================================================
# STAGE 1: Build Stage
#
# This stage uses the full Rust toolchain to compile the application with
# all necessary build dependencies. It is optimized for caching.
# ==============================================================================
# Use a more recent version of Rust to match modern lockfile formats
FROM rust:1.78 as builder

# Create a new, empty workspace so we can cache dependencies efficiently
WORKDIR /usr/src/quantum-chain
COPY ./Cargo.toml ./Cargo.lock* ./

# Create dummy crates to cache dependencies before copying source code
# This is more robust for workspace projects.
RUN mkdir -p crates/node-runtime/src && echo "fn main() {}" > crates/node-runtime/src/main.rs
RUN mkdir -p crates/shared-types/src && echo "" > crates/shared-types/src/lib.rs

# Build dependencies. This layer is cached and only re-runs if Cargo.toml changes.
# This will build all dependencies for all crates in the workspace.
# The "|| true" is a failsafe for the initial dummy build.
RUN cargo build --release --verbose 2>&1 || true

# Now, copy the actual source code
COPY . .

# Build the final binary, leveraging the cached dependencies
# This will be much faster as dependencies are already built.
# We specify the binary for our main node runtime.
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
