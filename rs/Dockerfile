# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.88-bookworm AS builder

# Build dependencies required by aws-lc-rs (pulled in via moq-native/quinn):
#   cmake  — aws-lc build system
#   clang  — C/C++ compiler used by aws-lc-sys
#   golang-go — aws-lc build system (Go ≥ 1.13 required)
#   nasm   — assembly optimisations on x86_64 (skipped gracefully on other arches)
#   pkg-config / libssl-dev — fallback openssl probes in some crates
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake \
    clang \
    libclang-dev \
    golang-go \
    nasm \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the full source tree (Cargo.lock is committed, git deps resolved from it)
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release --locked

# ── Runtime stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ca-certificates is needed to validate TLS certificates for the relay server
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user
RUN useradd --create-home --shell /bin/sh appuser
WORKDIR /home/appuser

COPY --from=builder /app/target/release/moq-keycast ./moq-keycast
RUN chown appuser:appuser moq-keycast

USER appuser

# moq-keycast is an interactive TUI — always run with:  podman run --rm -it ...
# Required flags: --username <name>
# Optional:       --room <name>  (default: general)
#
# When running via podman compose, extra args are APPENDED to the ENTRYPOINT,
# so baking the relay URL and TLS flag here ensures they are always present:
#   podman compose run --rm tui --username alice
#   → ./moq-keycast --relay https://relay:4443 --tls-disable-verify --username alice
ENTRYPOINT ["./moq-keycast", "--relay", "https://relay:4443", "--tls-disable-verify"]
