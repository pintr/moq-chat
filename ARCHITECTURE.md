# moq-keycast — Architecture & Interoperability

## Overview

`moq-keycast` is a live-typing chat demo built on the **Media over QUIC (MoQ)** protocol. It has two independent client implementations that share a single relay server and a common wire format, so a Rust terminal user and a browser user can see each other's keystrokes in real time.

---

## The Relay (`relay/`)

The relay is the central hub — a `moq-relay` binary compiled from the [moq-dev/moq](https://github.com/moq-dev/moq) project. Every client connects to it; it never stores state, it just fans out data from publishers to subscribers.

It listens on a single port (4443) over two transports:

| Transport | Protocol | Purpose |
|---|---|---|
| UDP 4443 | QUIC / WebTransport | Real-time MoQ data delivery (preferred) |
| TCP 4443 | HTTP + WebSocket | TLS fingerprint endpoint + WebSocket fallback |

For local development the relay auto-generates a self-signed TLS certificate (`--tls-generate localhost`). Browsers fetch its fingerprint from `http://localhost:4443/certificate.sha256` and pass it as `serverCertificateHashes` to the WebTransport API, so Chrome trusts it without a CA. Firefox and Safari automatically fall back to WebSocket.

---

## Rust TUI Client (`rs/`)

### Architecture

Four async tasks wired together with Tokio channels:

```
 keyboard events
      │
      ▼
  tui.rs  ──── typing_tx ────▶  publish.rs ──▶ relay
      ▲
      │ peer_rx
  subscribe.rs ◀───────────────────────────── relay
```

**`src/main.rs`** parses CLI args (`--relay`, `--room`, `--username`), creates two unbounded channels (`typing_tx/rx`, `peer_tx/rx`), and spawns three concurrent tasks.

**`src/tui.rs`** is the event loop. On every keypress it updates the local input buffer and sends the full current string over `typing_tx`. It receives `PeerEvent` messages from `peer_rx` to update remote users' display boxes. Uses `ratatui` for rendering and `crossterm` for terminal input.

| Key | Action |
|---|---|
| Any char | Append to input, publish immediately |
| Backspace | Delete last char, publish immediately |
| Enter | Clear input (resets others' view of you) |
| Esc / ^C | Quit |

**`src/publish.rs`** connects to the relay, announces a broadcast at `moq-keycast/{room}/{username}`, and creates two tracks:
- `typing` — the primary track; every string received from `typing_rx` is written as a JSON frame.
- `messages` — a no-op stub created for TypeScript SPA compatibility (the browser always subscribes to it and needs a clean response).

Each frame is serialised with `serde_json`:

```json
{ "text": "hello", "timestamp": 1709730000000 }
```

**`src/subscribe.rs`** connects to the relay, watches the `moq-keycast/{room}/` prefix for announced broadcasts, and for each discovered peer spawns a task to read their `typing` track. Parsed text is forwarded as `PeerEvent::Update` to the TUI. It also handles `Joined`/`Offline` lifecycle events.

`parse_typing_frame()` tries JSON first and falls back to raw UTF-8, making the Rust client tolerant of legacy or non-conforming clients.

---

## TypeScript Browser SPA (`ts/`)

### Architecture

The browser SPA is a Vite-built static site served by nginx. It uses `@moq/lite` to speak MoQ directly from the browser — no server-side component other than the shared relay.

```
 input events
      │
      ▼
  main.ts ──▶ publisher.ts ──▶ relay
      │
      └──▶ subscriber.ts ◀───── relay
                │
                ▼
          chat-room.ts (DOM)
```

**`src/moq/connection.ts`** manages a singleton `Moq.Connection`. On connect, `@moq/lite` races WebTransport (QUIC) vs WebSocket, then fetches the relay's self-signed TLS fingerprint from `/certificate.sha256` and passes it as `serverCertificateHashes` so Chrome trusts it without a CA.

**`src/moq/publisher.ts`** announces a `Moq.Broadcast` at `moq-keycast/{roomId}/{username}`. It then enters a `serveTrackRequests()` loop — for every subscriber that connects, it opens a live track object and appends a new group (= one frame) for each keystroke. It handles both `typing` and `messages` tracks. An initial empty frame is sent immediately on `typing` so the subscriber's `nextGroup()` does not block.

**`src/moq/subscriber.ts`** calls `connection.announced(roomPrefix)` to watch the `moq-keycast/{roomId}/` namespace. For each new user it `consume()`s their broadcast and subscribes to both their `typing` and `messages` tracks concurrently in background loops.

**`src/main.ts`** is the app entry point: it renders the lobby, then on join connects to the relay, starts publishing, starts subscribing, and wires DOM events (`moq:typing` custom event) to `publishTypingUpdate()`.

### TypeScript-only features

The browser client adds a `messages` track on top of the shared `typing` track. When a user presses Enter or clicks Send, a `MessagePayload` is published:

```json
{ "text": "hello", "username": "alice", "timestamp": 1709730000000 }
```

Messages accumulate in a conversation history (unlike `typing` frames, which overwrite). The Rust client never reads or writes this track.

---

## How They Interoperate

Both clients are designed around three shared contracts.

### 1. Shared MoQ path layout

```
moq-keycast/{room}/{username}   ← broadcast path
  └── track: "typing"           ← one group per keystroke; each group = full text snapshot
  └── track: "messages"         ← TypeScript only; Rust creates a no-op stub
```

The subscribe side of both clients watches the `moq-keycast/{room}/` prefix. When the relay announces a new broadcast, both strip the prefix to extract the username. This works identically whether the publisher is Rust or TypeScript.

### 2. Shared wire format

Every frame on the `typing` track is the same JSON structure, defined in TypeScript as `TypingPayload` and replicated in Rust via `serde_json`:

```json
{ "text": "hello world", "timestamp": 1709730000000 }
```

| Side | Serialisation |
|---|---|
| Rust publisher | `serde_json::json!({"text": text, "timestamp": timestamp})` |
| TS publisher | `group.writeJson(payload)` |
| Rust subscriber | `serde_json::from_slice` → `.get("text")` with raw-bytes fallback |
| TS subscriber | `group.readJson()` cast to `TypingPayload` |

### 3. Shared MoQ semantics (latest-group-wins)

Both the `moq-lite` Rust crate and the `@moq/lite` npm package automatically discard stale groups. The subscriber always reads the most recent group, so no debounce, buffering, or sequencing logic is needed on either side. Each group is a complete snapshot of the current input, not a delta.

### `messages` track compatibility shim

The TypeScript client always subscribes to a peer's `messages` track. Without it, it would receive an error. The Rust publisher creates an empty no-op `messages` track so the browser gets a clean `SUBSCRIBE_OK` and simply waits — it will never receive a message frame from the Rust side, but it handles the empty stream gracefully.

---

## Cross-client data flow: one keystroke end-to-end

```
Rust user types "h"
  → tui.rs appends to input buffer, sends "h" over typing_tx
  → publish.rs serialises {"text":"h","timestamp":…} as a MoQ group on the typing track
  → relay fans out the group to all subscribers of moq-keycast/lobby/alice

Browser subscriber sees new group on moq-keycast/lobby/alice/typing
  → readTypingUpdates() awaits nextGroup(), reads JSON, extracts text "h"
  → calls onTypingUpdate("alice", "h")
  → chat-room.ts updates alice's live card in the DOM to show "h"
```

The reverse path (browser → Rust TUI) is identical with roles switched.

---

## System diagram

```
┌──────────────────────────────────────────────────────────┐
│                  moq-relay  (port 4443)                  │
│   UDP 4443 — QUIC / WebTransport                         │
│   TCP 4443 — HTTP (cert fingerprint) + WebSocket         │
└──────────┬───────────────────────┬───────────────────────┘
           │  Podman network       │
           │                       │
 ┌─────────┴──────────┐   ┌────────┴──────────┐
 │  ts/  (Browser SPA)│   │  rs/  (Rust TUI)  │
 │  nginx → port 8080 │   │  terminal process │
 │  @moq/lite         │   │  moq-lite crate   │
 │  WebTransport / WS │   │  QUIC / WebSocket │
 └────────────────────┘   └───────────────────┘
```

---

## Dependency summary

| Component | Key dependencies |
|---|---|
| Relay | `moq-relay` (moq-dev/moq, compiled from source) |
| Rust client | `moq-lite`, `moq-native`, `ratatui`, `crossterm`, `tokio`, `clap`, `serde_json` |
| TS client | `@moq/lite`, Vite, nginx |
