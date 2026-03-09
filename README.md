# moq-keycast

A live-typing chat demo built on [Media over QUIC (MoQ)](https://datatracker.ietf.org/wg/moq/about/) — every keystroke is published to the relay and appears on every other connected user's screen in real time. There is no Send button and no message history, just raw live text delivered over QUIC.

This repo contains two client implementations that **share the same relay and wire format**, so they can talk to each other in real time:

| Directory | Language | UI |
|---|---|---|
| [`ts/`](#typescript-browser-spa-ts) | TypeScript | Browser (Vite SPA, served by nginx) |
| [`rs/`](#rust-tui-client-rs) | Rust | Terminal (TUI via ratatui) |
| [`relay/`](#the-relay) | — | Shared `moq-relay` server |

Everything runs in Podman. All commands below are run from the **project root**.

---

## How It Works

### The relay

`moq-relay` (from [moq-dev/moq](https://github.com/moq-dev/moq)) is the central server. **Every client — browser and terminal — connects to it.** It fans out data from publishers to all subscribers and serves a self-signed TLS certificate fingerprint at `/certificate.sha256`.

The relay listens on a single port (4443) over two transport protocols:

| Protocol | Transport | Purpose |
|---|---|---|
| QUIC / WebTransport | UDP 4443 | Real-time MoQ data delivery |
| HTTP / WebSocket | TCP 4443 | TLS fingerprint endpoint + WebSocket fallback |

For local development the relay auto-generates a self-signed TLS certificate (`--tls-generate localhost`). Browsers fetch its fingerprint from `http://localhost:4443/certificate.sha256` and pass it as `serverCertificateHashes` to the WebTransport API, so Chrome trusts it without a CA. Firefox and Safari fall back to WebSocket automatically.

### MoQ protocol layout

Each client publishes a personal broadcast at `moq-keycast/{room}/{username}` with a `typing` track. Every keystroke writes a new MoQ group to that track — a complete snapshot of the current input buffer. Subscribers watch the `moq-keycast/{room}/` prefix, read the latest group from each peer's track, and discard stale ones. The result is zero-latency, keystroke-by-keystroke delivery.

```
moq-keycast/{room}/{username}
  └── track "typing"    ← one group per keypress (full text snapshot)
  └── track "messages"  ← (TypeScript only) confirmed sent messages
```

### Wire format

All frames on the `typing` track use the same JSON encoding so that the Rust TUI and the browser SPA can read each other's output:

```json
{ "text": "hello", "timestamp": 1709730000000 }
```

The Rust client serializes this with `serde_json` and deserializes it transparently. If a frame cannot be parsed as JSON (e.g. from a legacy client) the raw bytes are displayed as-is.

### System diagram

```
┌──────────────────────────────────────────────────────────┐
│                  moq-relay  (port 4443)                  │
│   UDP 4443 — QUIC / WebTransport                        │
│   TCP 4443 — HTTP (cert fingerprint) + WebSocket        │
└──────────┬───────────────────────────┬───────────────────┘
           │  Podman internal network  │
           │                           │
 ┌─────────┴──────────┐   ┌────────────┴──────────┐
 │  ts/ (Browser SPA) │   │  rs/ (Rust TUI)        │
 │  nginx → port 8080 │   │  podman compose run    │
 │  browser connects  │   │  terminal              │
 │  to relay directly │   │  connects to relay     │
 └────────────────────┘   └───────────────────────┘
           ▲
           │  browser loads SPA from http://localhost:8080,
           │  then connects DIRECTLY to relay at http://localhost:4443
```

---

## Quick Start

> **First build:** The relay Dockerfile compiles `moq-relay` from Rust source, which takes ~10–20 minutes. All subsequent builds are served from the image layer cache.

### Browser client + relay (recommended)

```bash
podman compose up --build
```

Then open **<http://localhost:8080>** in Chrome (recommended) or Firefox. Open two tabs, join the same room with different usernames, and start typing.

Ports exposed on the host:

| Port | Protocol | Service |
|---|---|---|
| `4443` | TCP | Relay — HTTP fingerprint endpoint + WebSocket |
| `4443` | UDP | Relay — QUIC / WebTransport |
| `8080` | TCP | Frontend SPA (nginx) |

### Terminal (TUI) client

The TUI client connects to the relay over the internal Podman network. Start the relay first if it is not already running:

```bash
podman compose up relay -d
```

Then run the TUI client, passing your username

```bash
podman compose run --rm tui --username alice
```

Open a second terminal with a different username to chat:

```bash
podman compose run --rm tui --username bob
```

> The `tui` service is defined with the `tui` [profile](https://docs.podman.io/en/latest/markdown/podman-compose.1.html) so it never starts automatically with `podman compose up`.

### Running both clients together (cross-client chat)

The Rust TUI and the browser SPA share the same relay and wire format, so a terminal user and a browser user in the same room see each other's keystrokes in real time.

```bash
# Terminal 1 — start the relay and browser frontend
podman compose up --build -d

# Terminal 2 — join as a TUI user
podman compose run --rm tui --username alice

# Then open http://localhost:8080 in a browser, join room "general" as "bob"
# alice and bob will see each other type, keystroke by keystroke
```

---

## Rust TUI Client (`rs/`)

A terminal chat client built with [ratatui](https://ratatui.rs/) and [crossterm](https://github.com/crossterm-rs/crossterm), communicating over QUIC via [quinn](https://github.com/quinn-rs/quinn) through the `moq-native` crate.

### Keybindings

| Key | Action |
|---|---|
| Any character | Append to input and publish immediately |
| Backspace | Delete last character and publish immediately |
| Enter | Clear input (resets peers' view of you) |
| Esc / Ctrl+C | Quit |

### CLI flags

| Flag | Default (in container) | Description |
|---|---|---|
| `--relay` | `https://relay:4443` | Relay URL — uses internal Podman service name |
| `--room` | `general` | Chat room name |
| `--username` | *(required)* | Your display name |
| `--tls-disable-verify` | on (in container) | Skip TLS hostname verification (cert is issued for `localhost`, not `relay`) |

Logs are written to `moq-keycast.log` (never to stderr, which would corrupt the TUI alternate screen).

### Build image only

```bash
podman compose build tui
```

### Dependencies

| Crate | Role |
|---|---|
| `moq-lite` (git) | MoQ tracks, groups, frames, broadcasts |
| `moq-native` (git) | QUIC/WebSocket transport, TLS, CLI helpers |
| `ratatui` | Terminal UI rendering |
| `crossterm` | Cross-platform terminal control and async event stream |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `anyhow` | Error handling |
| `tracing` / `tracing-subscriber` | Structured logging |

---

## TypeScript Browser SPA (`ts/`)

A browser-based client using [`@moq/lite`](https://www.npmjs.com/package/@moq/lite) that supports QUIC/WebTransport (Chrome) with an automatic WebSocket fallback (Firefox, Safari). Served by nginx.

### Frontend-only dev server (hot-reload)

To iterate quickly on the TypeScript frontend without rebuilding the container image, run the relay in Podman and the Vite dev server natively:

```bash
# Start the relay
podman compose up relay -d

# Run Vite with hot-reload (requires Node.js installed locally)
cd ts/frontend
npm install
npm run dev
# → open http://localhost:5173
```

### Configuration

| Build arg | Default | Description |
|---|---|---|
| `VITE_RELAY_URL` | `http://localhost:4443` | Relay URL baked into the JS bundle at build time |

Override for a custom relay host:

```bash
podman compose build frontend --build-arg VITE_RELAY_URL=https://relay.example.com:4443
```

### Dependencies

| Package | Role |
|---|---|
| `@moq/lite` | MoQ protocol + QUIC/WebSocket transport in the browser |
| `vite` | Build tool and dev server |
| `typescript` | Type checking |

---

## The Relay

`moq-relay` is sourced from [moq-dev/moq](https://github.com/moq-dev/moq) and compiled during the container build (`relay/Dockerfile`). It is the single point all clients connect to: it fans out each publisher's tracks to all matching subscribers.

### Relay flags (configured in `compose.yml`)

| Flag | Value | Description |
|---|---|---|
| `--server-bind` | `[::]:4443` | QUIC/WebTransport on UDP 4443, all interfaces |
| `--web-http-listen` | `[::]:4443` | HTTP + WebSocket on TCP 4443, all interfaces |
| `--tls-generate` | `localhost` | Auto-generate a short-lived self-signed TLS cert |
| `--auth-public` | `""` | All MoQ paths publicly readable/writable (dev only) |

For production: replace `--tls-generate` with a CA-signed certificate and remove `--auth-public ""`, replacing it with `--auth-key` to restrict access.

---

## Architecture Decisions

| Decision | Reason |
|---|---|
| One broadcast per user | Each user owns their track lifecycle; relay fans out to all subscribers |
| One group per keypress | MoQ groups are the unit of delivery; old groups are automatically skipped, so subscribers always get the latest snapshot with no debounce needed |
| No Send / no history | Showcases raw MoQ delivery latency; text is deliberately ephemeral |
| JSON wire format for `typing` frames | `{"text":"...","timestamp":...}` is the shared contract between the Rust and TypeScript clients; `serde_json` on the Rust side, `group.writeJson()` / `group.readJson()` on the TypeScript side |
| Rust falls back to raw bytes | If a `typing` frame is not valid JSON, the Rust TUI displays the raw bytes — forward-compatible with hypothetical non-JSON clients |
| `http://` relay URL in browser | `@moq/lite` auto-fetches `/certificate.sha256` and uses it as `serverCertificateHashes`, enabling WebTransport with a self-signed cert without a CA |
| WebSocket fallback | `@moq/lite` races QUIC vs WebSocket; Firefox and Safari fall back transparently |
| `tui` Compose profile | Prevents the interactive TUI container from starting automatically with `podman compose up` |
| Internal Podman network for TUI | The TUI container reaches the relay by service name (`relay:4443`) without extra port mapping; `--tls-disable-verify` handles the hostname mismatch |
