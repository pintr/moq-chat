# =============================================================================
# relay/Dockerfile – moq-relay
# =============================================================================
#
# Installs the official moq-relay binary directly from crates.io.
# The relay is the central server that all MoQ clients connect to. It handles:
#   - QUIC/WebTransport connections from browsers (for real-time pub/sub)
#   - WebSocket fallback (for browsers without WebTransport support)
#   - Broadcasting data from publishers to all subscribers
#   - Serving the TLS fingerprint endpoint (/certificate.sha256)
#
# PORT DESIGN (single port 4443, two transport protocols):
#   - UDP 4443: QUIC/WebTransport (used by modern browsers for MoQ data)
#   - TCP 4443: HTTP server (serves /certificate.sha256 for local TLS trust,
#               and WebSocket connections as fallback)
#
# SELF-SIGNED TLS:
#   The relay generates its own TLS certificate via --tls-generate.
#   @moq/lite automatically fetches the certificate fingerprint from
#   http://relay:4443/certificate.sha256 and passes it as serverCertificateHashes
#   to the WebTransport API -- this is how local dev bypasses CA verification.
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build moq-relay from crates.io
# -----------------------------------------------------------------------------
# Pin to bookworm-based Rust image so the compiled binary targets GLIBC 2.36,
# which matches the debian:bookworm-slim runtime stage below.
FROM rust:1.88-slim-bookworm AS builder

# System build dependencies:
#   pkg-config / libssl-dev - OpenSSL headers (for TLS/crypto compilation)
#   cmake / build-essential - required by crypto crate build scripts
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install moq-relay from crates.io.
# v0.10.7 is the stable release matching moq-lite v0.15.0 / moq-native v0.13.2.
# Disable iroh (P2P layer, not needed for basic relay) to speed up compilation.
RUN cargo install moq-relay \
    --version 0.10.7 \
    --no-default-features \
    --features quinn,websocket

# -----------------------------------------------------------------------------
# Stage 2: Minimal runtime image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies:
#   ca-certificates - needed for outbound TLS connections (cluster mode)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy only the compiled relay binary from the build stage
COPY --from=builder /usr/local/cargo/bin/moq-relay /usr/local/bin/moq-relay

# Expose both transport protocols on port 4443:
#   UDP - QUIC/WebTransport connections from browsers
#   TCP - HTTP server for /certificate.sha256 fingerprint and WebSocket fallback
EXPOSE 4443/udp
EXPOSE 4443/tcp

ENTRYPOINT ["moq-relay"]

# Default arguments (can be overridden in compose.yml):
#
#   --server-bind [::]:4443
#       Binds the QUIC/WebTransport server to port 4443 (UDP, all interfaces).
#       Browsers connect here for MoQ pub/sub over QUIC.
#
#   --web-http-listen [::]:4443
#       Binds the HTTP server to port 4443 (TCP, all interfaces).
#       Serves /certificate.sha256 so browsers can trust the self-signed cert.
#       Also serves WebSocket connections as a fallback transport.
#
#   --tls-generate localhost
#       Auto-generates a self-signed TLS certificate for the hostname "localhost".
#       The certificate hash is served at /certificate.sha256 and used by
#       @moq/lite's serverCertificateHashes for local development.
CMD [ \
    "--server-bind", "[::]:4443", \
    "--web-http-listen", "[::]:4443", \
    "--tls-generate", "localhost", \
    "--auth-public", "" \
    ]
