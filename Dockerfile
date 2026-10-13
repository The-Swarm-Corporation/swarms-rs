# syntax=docker/dockerfile:1

# The swarms-rs examples, built and ready to run.
#
#   docker build -t swarms-rs .
#   docker run --rm swarms-rs                                          # list the examples
#   docker run --rm -e ANTHROPIC_API_KEY swarms-rs anthropic_claude_agent
#
# Both stages use Debian trixie, so the examples find the glibc and OpenSSL they were linked
# against.

FROM rust:1.99-slim-trixie AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

# RUSTUP_TOOLCHAIN overrides rust-toolchain.toml, which asks for "stable": build with the
# compiler this image pins instead of downloading another one. Cargo also leaves a hashed
# copy of each example (name-1a2b3c); only the named ones are kept.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    RUSTUP_TOOLCHAIN="$RUST_VERSION" cargo build --release -p swarms-rs --examples \
    && mkdir /examples \
    && find target/release/examples -maxdepth 1 -type f -perm -u+x ! -name '*-*' \
        -exec cp {} /examples/ \;

FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3t64 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --home-dir /app swarms

COPY --from=builder /examples /app/examples

USER swarms
WORKDIR /app
ENV PATH="/app/examples:$PATH"
CMD ["ls", "/app/examples"]
