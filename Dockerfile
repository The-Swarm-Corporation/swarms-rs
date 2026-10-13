# syntax=docker/dockerfile:1

# Swarms Code, the coding agent built on swarms-rs, in a container.
#
#   docker build -t swarms-code .
#   docker run --rm -it -e ANTHROPIC_API_KEY -v "$PWD":/workspace swarms-code
#
# Both stages use Debian trixie, so the binary finds the glibc and OpenSSL it was linked against.

FROM rust:1.99-slim-trixie AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

# RUSTUP_TOOLCHAIN overrides rust-toolchain.toml, which asks for "stable": build with the
# compiler this image pins instead of downloading another one.
ARG TARGETPLATFORM
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target,id=swarms-target-$TARGETPLATFORM \
    RUSTUP_TOOLCHAIN="$RUST_VERSION" cargo build --release -p swarms-code \
    && cp target/release/swarms-code /usr/local/bin/swarms-code

FROM debian:trixie-slim

# The agent works through bash, and uses git to see and review what it changed.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3t64 git \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash swarms \
    && mkdir /workspace \
    && chown swarms:swarms /workspace

COPY --from=builder /usr/local/bin/swarms-code /usr/local/bin/swarms-code

USER swarms
WORKDIR /workspace
ENTRYPOINT ["swarms-code"]
