# moq-keycast

A terminal chat client where every keystroke is streamed live over [Media over QUIC (MoQ)](https://github.com/moq-dev/moq) — no send button, no message history.

## How it works

```
┌──────────────────── Terminal A (alice) ──────────────────────┐
│  moq-keycast :: lobby | 1 peer                                  │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  bob: hey, how are                                   │    │
│  └──────────────────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  alice: doing great, thanks█                         │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

Each character you type is immediately published as a MoQ group on your `typing` track.  
Remote peers subscribe to all users in the room and display their current buffer in real time.

### MoQ path layout

```
moq-keycast/{room}/{username}  ──► broadcast
                                  └── track: "typing"
                                        └── groups: one per keystroke
                                              └── frame: full current input text
```

## Quick start

### 1 — Start the relay

```bash
podman run --rm -p 4443:4443/udp -p 4443:4443/tcp \
  $(podman build -q ./relay) \
  --server-bind [::]:4443 \
  --web-http-listen [::]:4443 \
  --tls-generate localhost \
  --auth-public ""
```

Or use the relay from the companion `moq-keycast` project:

```bash
cd ../moq-keycast && podman compose up relay
```

### 2 — Run the chat client

```bash
cargo run -- \
  --relay https://localhost:4443 \
  --room lobby \
  --username alice \
  --tls-disable-verify
```

Open additional terminals with different usernames to chat.

## Keybindings

| Key        | Action                                       |
|------------|----------------------------------------------|
| Any char   | Append to input, publish immediately         |
| Backspace  | Delete last char, publish immediately        |
| Enter      | Clear your input (resets peers' view of you) |
| Esc / ^C   | Quit                                         |

## Building

```bash
cargo build --release
```

The binary will be at `target/release/moq-keycast`.

## Project layout

```
src/
  main.rs       — CLI args, channel setup, task orchestration
  publish.rs    — MoQ publisher task (announces our typing)
  subscribe.rs  — MoQ subscriber task (watches room for peers)
  tui.rs        — ratatui terminal UI + PeerEvent type
```

## Dependencies

| Crate       | Role                               |
|-------------|------------------------------------|
| moq-lite    | MoQ protocol (tracks, groups, etc) |
| moq-native  | QUIC / WebSocket transport         |
| ratatui     | Terminal UI rendering              |
| crossterm   | Cross-platform terminal control    |
| tokio       | Async runtime                      |
| clap        | CLI argument parsing               |
