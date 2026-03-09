# MoQ Chat

A live-typing demo built with [Media over QUIC](https://github.com/moq-dev/moq) (`@moq/lite`).

Every keystroke is published instantly over QUIC and appears on all other users' screens in real time — no Send button, no Enter to confirm, just raw live text.

## How it works

Each user has two UI areas:

- **Input field (footer)** – what you are currently typing. Every `input` event publishes the full current text as a new MoQ group on the `typing` track.
- **Live feed cards (body)** – one card per remote peer, showing whatever they are typing right now, updated keystroke-by-keystroke as groups arrive.

```
Browser A                   moq-relay                   Browser B
   │                           │                           │
   │── publish broadcast ────▶│                           │
   │   moq-chat/room/alice     │◀─── announced prefix ────│
   │                           │   moq-chat/room/          │
   │                           │                           │
   │  [user types "h"]         │                           │
   │── typing group "h" ─────▶│── typing group "h" ─────▶│ card shows "h"
   │  [user types "hi"]        │                           │
   │── typing group "hi" ────▶│── typing group "hi" ────▶│ card shows "hi"
   │  [user types "hig"]       │                           │
   │── typing group "hig" ───▶│── typing group "hig" ───▶│ card shows "hig"
```

**MoQ path layout:**

- Each user publishes `moq-chat/{roomId}/{username}` (one broadcast per user)
- `typing` track – one group per keystroke; each group = the full current text snapshot

The relay and `@moq/lite` automatically discard stale groups, so subscribers always receive the latest snapshot — no debounce or client-side buffering needed.

**Transport:** `@moq/lite` races WebTransport (QUIC) vs WebSocket. For local development, it fetches the relay's self-signed TLS fingerprint from `http://localhost:4443/certificate.sha256` and uses `serverCertificateHashes` so the browser trusts it without a CA.

## Project structure

```
moq-chat/
├── compose.yml                 # Orchestrates relay + frontend
├── relay/
│   └── Dockerfile              # Builds moq-relay from moq-dev/moq source
└── frontend/
    ├── Dockerfile              # Builds Vite SPA, serves with nginx
    ├── src/
    │   ├── config.ts           # Relay URL and MoQ path constants
    │   ├── types.ts            # Shared TypeScript types (wire + UI)
    │   ├── main.ts             # App entry: lobby → room routing
    │   ├── moq/
    │   │   ├── connection.ts   # connectToRelay() singleton
    │   │   ├── publisher.ts    # Publish typing + messages via MoQ
    │   │   └── subscriber.ts   # Subscribe to room members via MoQ
    │   └── ui/
    │       ├── lobby.ts        # Lobby screen (username + room picker)
    │       ├── chat-room.ts    # Live feed (per-peer cards + local input)
    │       └── styles.css      # Dark-theme CSS
    └── index.html
```

## Quick start (Podman)

> **Requirements:** Podman with Compose support (`podman compose version`).  
> **First build:** The relay Dockerfile compiles Rust from source — takes ~15 min on the first run, cached after that.

```bash
# Build and start both containers
podman compose up --build

# Open the chat in your browser
open http://localhost:8080
```

Open two browser tabs (or two browsers) at `http://localhost:8080`. Join the same room with different usernames. Start typing in one tab and watch your text appear live in the other.

> **Chrome only for WebTransport:** Firefox and Safari don't yet support WebTransport. `@moq/lite` automatically falls back to WebSocket for unsupported browsers, so they still work — just without QUIC prioritisation.

## Local development (without Podman)

```bash
# 1. Build and run the relay container only
podman compose up relay -d

# 2. Install frontend dependencies and start Vite dev server
cd frontend
npm install
npm run dev

# 3. Open http://localhost:5173
```

The Vite dev server hot-reloads TypeScript changes instantly. The relay still runs in Podman.

## Configuration

| Variable | Default | Description |
|---|---|---|
| `VITE_RELAY_URL` | `http://localhost:4443` | MoQ relay URL seen by the **browser**. Override at build time: `podman compose build --build-arg VITE_RELAY_URL=http://myhost:4443` |

## Architecture decisions

| Decision | Reason |
|---|---|
| One broadcast per user | Each user publishes their own broadcast so they fully control the track lifecycle. Relay fans out to all subscribers. |
| One group per keypress | MoQ groups are the unit of delivery. The relay and `@moq/lite` skip old groups so subscribers always see the *latest* typing snapshot — no debounce needed. |
| No send / no history | The app showcases raw MoQ delivery latency. Text is ephemeral — each new group replaces the previous one in the feed card. |
| `http://` relay URL for local dev | `@moq/lite` auto-fetches `/certificate.sha256` and passes it as `serverCertificateHashes`, enabling WebTransport with a self-signed cert. |
| WebSocket fallback | `@moq/lite` races QUIC vs WebSocket. Firefox/Safari fall back to WebSocket transparently; the MoQ framing is identical. |
