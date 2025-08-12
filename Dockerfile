# Multi-stage build for swarms-rs
FROM rust:1.89-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy source code
COPY swarms-macro/ ./swarms-macro/
COPY swarms-rs/ ./swarms-rs/
COPY examples/ ./examples/

# Build the project
RUN cargo build --release --workspace

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false -m -d /app swarms

# Set working directory
WORKDIR /app

# Copy binary from builder stage (if it exists)
COPY --from=builder /app/target/release/swarms-rs* ./bin/ 2>/dev/null || true

# Copy examples
COPY --from=builder /app/target/release/examples/ ./examples/ 2>/dev/null || true

# Change ownership
RUN chown -R swarms:swarms /app

# Switch to app user
USER swarms

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Default command
CMD ["./bin/swarms-rs"]
