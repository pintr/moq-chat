# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.88-slim-bookworm AS builder

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
# distroless/cc includes glibc + libgcc_s + libstdc++ — the minimal set needed
# by Rust release binaries. No shell, no package manager, no extra attack surface.
# The :nonroot variant pre-creates a non-root user (uid/gid 65532).
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder --chown=nonroot:nonroot /app/target/release/moq-keycast /home/nonroot/moq-keycast

WORKDIR /home/nonroot
USER nonroot

# moq-keycast is an interactive TUI — always run with:  podman run --rm -it ...
# Required flags: --username <name>
# Optional:       --room <name>  (default: general)
#
# When running via podman compose, extra args are APPENDED to the ENTRYPOINT,
# so baking the relay URL and TLS flag here ensures they are always present:
#   podman compose run --rm tui --username alice
#   → ./moq-keycast --relay https://relay:4443 --tls-disable-verify --username alice
ENTRYPOINT ["/home/nonroot/moq-keycast", "--relay", "https://relay:4443", "--tls-disable-verify"]
