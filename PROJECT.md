# Project: Watch Together

## Architecture
Cross-platform collaborative video player desktop application for macOS (Apple Silicon) and Windows (x64). Combines a full-featured local media player (Tauri 2 + React + TypeScript + libmpv) with synchronized co-watching over direct HTTP byte-range streaming and WebSocket state synchronization.

```
                    ┌───────────────────────────────────────────────┐
                    │               Desktop Frontend                │
                    │   React 18 + TypeScript + Vite + Dark HUD     │
                    │   (apps/desktop/src: player, rooms, settings) │
                    └───────────────────────┬───────────────────────┘
                                            │ Tauri 2 IPC
                    ┌───────────────────────▼───────────────────────┐
                    │                Tauri 2 Backend                │
                    │   (apps/desktop/src-tauri: commands, events)  │
                    └───────┬───────────────┬───────────────┬───────┘
                            │               │               │
        ┌───────────────────┴───┐   ┌───────┴───────┐   ┌───┴───────────────────┐
        │  crates/media-engine  │   │  crates/      │   │  crates/              │
        │  libmpv C ABI, Metal/ │   │  room-server  │   │  network              │
        │  Direct3D surface,    │   │  Lifecycle,   │   │  STUN/TURN, ICE, P2P  │
        │  events, audio/subs   │   │  Permissions  │   │  DataChannel proxy    │
        └───────────────────────┘   └───┬───────┬───┘   └───────────┬───────────┘
                                        │       │                   │
                    ┌───────────────────┘       └──────────┐        │
                    │                                      │        │
        ┌───────────▼───────────┐              ┌───────────▼────────▼───────────┐
        │  crates/media-stream  │              │      crates/sync-engine        │
        │  RFC 7233/9110 HTTP   │              │  Clock offset, NTP ping,       │
        │  Byte-Range Server    │              │  3-tier drift correction       │
        └───────────┬───────────┘              └───────────┬────────────────────┘
                    │                                      │
                    └───────────────────┬──────────────────┘
                                        │
                            ┌───────────▼───────────┐
                            │ crates/shared-protocol│
                            │ Serde models, events, │
                            │ errors, revision tags │
                            └───────────────────────┘
                                        │
                            ┌───────────▼───────────┐
                            │   services/signaling  │
                            │ Standalone WS relay   │
                            │ SDP / ICE exchange    │
                            └───────────────────────┘
```

## Feature Inventory
Every feature extracted during Phase 0 survey is inventoried below with its assigned milestone.

| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Shared Data Models | Serde JSON data models: `RoomId`, `RoomCode`, `RoomToken`, `Participant`, `PlaybackState`, `VideoMetadata` | M1 | Survey 2 |
| 2 | Protocol Event Enums | Strongly-typed WebSocket message enums (11 events) with externally tagged JSON payloads | M1 | Survey 2 |
| 3 | Protocol Error Schemas | Standard error types (`RoomFull`, `InvalidToken`, `PermissionDenied`, `StaleRevision`, etc.) | M1 | Survey 2 |
| 4 | RFC 7233/9110 Range Parser | Robust parser for bounded (`bytes=0-100`), open-ended (`bytes=100-`), and suffix ranges | M1 | Survey 2 |
| 5 | 206 Partial Content Streaming | Chunked async file streaming via Tokio emitting `Content-Range`, `Accept-Ranges`, `Content-Length` | M1 | Survey 2 |
| 6 | Token Authorization Filter | Validates 32+ char CSPRNG token on every HTTP Range request via query param or Bearer header | M1 | Survey 2 |
| 7 | Strict Path Isolation & Anti-Traversal | Exposes only the single host-selected video file; all other paths return 404/403 | M1 | Survey 2 |
| 8 | Clock Offset & Latency Estimation | NTP-style 4-timestamp exchange (`sync.ping`/`sync.pong`) with EMA smoothing | M1 | Survey 2 |
| 9 | Monotonic Revision Enforcement | Tracks command `revision: u64` and discards out-of-order or stale playback commands | M1 | Survey 2 |
| 10 | Target Position Prediction | Projects expected playback timecode from server time, elapsed delta, and speed | M1 | Survey 2 |
| 11 | Three-Tier Drift Correction | 0–100ms deadband, 100–500ms gradual speed adjustment (±4%), >500ms hard seek | M1 | Survey 2 |
| 12 | Late Joiner State Synchronization | Injects current room playback position and status into joining client | M1 | Survey 2 |
| 13 | Room Creation & Code Generation | Generates 6-character uppercase alphanumeric code and 32+ character CSPRNG token | M2 | Survey 2 |
| 14 | Participant Registry & Capacity Cap | Enforces 2–5 participant limit; tracks join, leave, connection, and ping status | M2 | Survey 2 |
| 15 | Permission Mode Enforcement | Toggles `FreeControl` vs `HostOnly`; rejects non-host commands in `HostOnly` | M2 | Survey 2 |
| 16 | Host Participant Kicking | Host can kick any guest; invalidates their session and revokes access | M2 | Survey 2 |
| 17 | Room Lifecycle & Teardown | Shuts down server, broadcasts `room.closed`, closes open sockets and file handles | M2 | Survey 2 |
| 18 | Standalone Signaling Service | Standalone Node.js/Rust WebSocket relay service in `services/signaling` | M2 | Survey 3 |
| 19 | STUN NAT Traversal | Resolves public server-reflexive candidates using `stun.l.google.com:19302` | M2 | Survey 3 |
| 20 | Configurable TURN Relay | Relays traffic via configured TURN server when direct peer connection fails | M2 | Survey 3 |
| 21 | Encrypted P2P Transport | DTLS/TLS encrypted WebRTC DataChannels for command and media transport | M2 | Survey 3 |
| 22 | Loopback HTTP Bridge | Local proxy (`127.0.0.1:<port>`) bridging libmpv HTTP Range requests across P2P | M2 | Survey 3 |
| 23 | Auto-Reconnection & Drop Recovery | Heartbeat timeout detection (3s) with exponential backoff re-connect and state recovery | M2 | Survey 3 |
| 24 | Embedded libmpv Core Engine | Initializes libmpv context with GPU acceleration (`vo=gpu-next`, Metal/D3D11) | M3 | Survey 1 |
| 25 | Hardware Acceleration Fallback | `hwdec=auto-safe` utilizing Apple VideoToolbox / Direct3D 11 with CPU fallback | M3 | Survey 1 |
| 26 | Background Property Observer Loop | Dedicated thread blocking on `mpv_wait_event` forwarding events with zero CPU polling | M3 | Survey 1 |
| 27 | Multi-Format Decoding | Decodes MP4, MOV, MKV containing H.264, HEVC, VP9, AV1 | M3 | Survey 1 |
| 28 | Multi-Audio Track Switching | Enumerates audio tracks and switches active audio stream (`aid`) without stutter | M3 | Survey 1 |
| 29 | Embedded & External Subtitles | Renders embedded subtitle tracks and loads external `.srt` and `.ass` files | M3 | Survey 1 |
| 30 | Audio Pitch Preservation | Discrete playback speeds (`0.5x`, `0.75x`, `1x`, `1.25x`, `1.5x`, `2x`) with scaletempo2 pitch correction | M3 | Survey 1 |
| 31 | Aspect Ratio Preservation | Locks video proportion with letterboxing/pillarboxing and window aspect ratio | M3 | Survey 1 |
| 32 | Headless/Mock Engine Trait | Decoupled `MediaPlayer` trait enabling CI headless testing (`vo=null, ao=null`) | M3 | Survey 3 |
| 33 | Tauri 2 IPC Commands & State | Exposes strongly-typed Rust Tauri commands for playback, tracks, and room state | M3 | Survey 1 |
| 34 | macOS Entitlements & Packaging | Network client/server entitlements, libmpv linking, and `.dmg` bundling configuration | M3 | Survey 3 |
| 35 | Dark Theme Media Player UI | Minimalist dark aesthetic (`#0B0D11`) with glassmorphic control overlays | M4 | Survey 1 |
| 36 | Auto-Hiding Controls HUD | Overlays fade out after 2.5s of inactivity during playback; cursor hides | M4 | Survey 1 |
| 37 | File Ingestion (Dialog & Drag/Drop) | Open files via native OS dialog (`Cmd/Ctrl+O`) and window drag-and-drop | M4 | Survey 1 |
| 38 | Keyboard Shortcuts | Space (pause/play), Left/Right (±5s), Shift+arrows (±30s), F (fullscreen), M (mute) | M4 | Survey 1 |
| 39 | Timeline Scrubber & Timecode | Interactive seekbar with hover timestamp preview, buffer bar, and elapsed/total time | M4 | Survey 1 |
| 40 | Volume Control & Local Mute | Independent local volume slider and mute toggle (NOT synchronized) | M4 | Survey 1 |
| 41 | Speed & Track Selector Menus | Discrete speed pill menu, audio track dropdown, and subtitle selector | M4 | Survey 1 |
| 42 | Fullscreen Modes | Double-click video canvas, `F` key, or button; exit via `Esc` | M4 | Survey 1 |
| 43 | Room UI (Create, Join, Badges) | "Watch Together" button, room code copy modal, join modal, header status badge | M4 | Survey 1 |
| 44 | Participant Roster & Buffer Status | Popover showing connected participants, ping latency, and buffering indicator | M4 | Survey 1 |
| 45 | Local Settings Persistence | Remembers volume and audio/subtitle preferences across launches | M4 | Survey 1 |
| 46 | Full End-to-End Co-Watching | 2–5 instances streaming HTTP byte-range video with ≤250ms sync drift | M5 | Survey 2 & 3 |
| 47 | 100% E2E Test Suite Pass | Complete execution and 100% pass rate across test Tiers 1–4 | M5 | Survey 3 |
| 48 | Adversarial Coverage Hardening | White-box Tier 5 edge case testing, stress tests, and fuzzing | M5 | Project Pattern |
| 49 | Production macOS Apple Silicon Bundle | `cargo tauri build` producing functional standalone `.dmg` installer | M5 | Survey 3 |

## Milestones

| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Foundation Crates & Core Protocol | `crates/shared-protocol`, `crates/media-stream`, `crates/sync-engine` | none | DONE |
| M2 | Room Server, Network & Signaling | `crates/room-server`, `crates/network`, `services/signaling` | M1 | DONE |
| M3 | Media Engine & Desktop Backend | `crates/media-engine`, `apps/desktop/src-tauri` | M1, M2 | DONE |
| M4 | Desktop Frontend UI & Player HUD | `apps/desktop/src` (React, TypeScript, components, hooks) | M3 | DONE |
| M5 | End-to-End Integration & Final Acceptance | Full E2E testing, Tier 5 hardening, macOS bundle `.dmg`, acceptance audit | M1, M2, M3, M4 | IN_PROGRESS |

## Interface Contracts

### `shared-protocol` ↔ All Components
- **Data Models**:
  - `RoomCode`: 6-character uppercase alphanumeric string (`[2-9A-HJ-NP-Z]{6}`)
  - `RoomToken`: $\ge 32$ character CSPRNG hex string (256 bits entropy)
  - `PlaybackState`: `{ revision: u64, status: PlaybackStatus, position_ms: u64, speed: f32, updated_at_server_ms: u64, duration_ms: u64 }`
  - `PermissionMode`: `FreeControl` | `HostOnly`
- **Protocol Envelope**:
  - Client -> Host: `{"type": "<event>", "payload": { ... }}`
  - Host -> Client: `{"type": "<event>", "payload": { ... }}`
  - Events: `room.create`, `room.join`, `room.state`, `playback.play`, `playback.pause`, `playback.seek`, `playback.speed`, `playback.updated`, `participant.status`, `sync.ping`, `sync.pong`, `room.closed`, `room.error`

### `media-stream` ↔ Media Engine & Remote Peers
- **Endpoint**: `GET /video/stream`
- **Headers**:
  - Request: `Range: bytes=start-end` or `bytes=start-`, `Authorization: Bearer <token>` or query `?token=<token>`
  - Response: `HTTP/1.1 206 Partial Content`, `Content-Range: bytes <start>-<end>/<total>`, `Content-Length: <length>`, `Accept-Ranges: bytes`
- **Security Guard**:
  - Token constant-time validation (`subtle::ConstantTimeEq`). Invalid -> `403 Forbidden`.
  - Path traversal / non-stream routes -> `404 Not Found` or `403 Forbidden`.
  - Room closed -> `403 Forbidden` / Connection Refused.

### `sync-engine` ↔ Media Engine & Room Server
- **Ping / Latency**:
  - $\text{RTT} = (t_3 - t_0) - (t_2 - t_1)$
  - $\text{Clock Offset } \theta = \frac{(t_1 - t_0) + (t_2 - t_3)}{2}$ (EMA smoothed, $\alpha = 0.2$)
- **Position Prediction**:
  - $P_{\text{expected}} = \text{position\_ms} + (T_{\text{server\_est}} - \text{updated\_at\_server\_ms}) \times \text{speed}$
- **Drift Threshold Actions**:
  - $|\delta| \le 100\text{ms}$: `Correction::None` (nominal speed)
  - $100\text{ms} < |\delta| \le 500\text{ms}$: `Correction::AdjustSpeed(nominal_speed * 1.04 or 0.96)`
  - $|\delta| > 500\text{ms}$: `Correction::HardSeek(P_expected)`

### `media-engine` ↔ Desktop Frontend IPC
- **Commands**:
  - `cmd_player_open_file(path: String) -> Result<(), String>`
  - `cmd_player_play() -> Result<(), String>`
  - `cmd_player_pause() -> Result<(), String>`
  - `cmd_player_seek(position_seconds: f64, exact: bool) -> Result<(), String>`
  - `cmd_player_set_speed(speed: f64) -> Result<(), String>`
  - `cmd_player_set_volume(volume: f64) -> Result<(), String>`
  - `cmd_player_set_mute(muted: bool) -> Result<(), String>`
  - `cmd_player_select_audio(track_id: i64) -> Result<(), String>`
  - `cmd_player_select_subtitle(track_id: i64) -> Result<(), String>`
  - `cmd_player_add_subtitle(path: String) -> Result<(), String>`
  - `cmd_player_get_state() -> Result<PlaybackState, String>`
  - `cmd_player_get_tracks() -> Result<Vec<MediaTrack>, String>`
  - `cmd_window_toggle_fullscreen() -> Result<bool, String>`
- **Events**:
  - `player://time-pos`: `{ position: f64, duration: f64 }`
  - `player://state-change`: `{ paused: bool, speed: f64, volume: f64, muted: bool }`
  - `player://tracks-change`: `{ tracks: Vec<MediaTrack> }`

## Code Layout
```
watch-together/
├── apps/
│   └── desktop/
│       ├── package.json
│       ├── vite.config.ts
│       ├── src/
│       │   ├── App.tsx
│       │   ├── main.tsx
│       │   ├── components/
│       │   ├── features/
│       │   │   ├── player/
│       │   │   ├── rooms/
│       │   │   ├── participants/
│       │   │   └── settings/
│       │   ├── hooks/
│       │   ├── types/
│       │   └── styles/
│       └── src-tauri/
│           ├── Cargo.toml
│           ├── tauri.conf.json
│           ├── Entitlements.plist
│           └── src/
│               ├── main.rs
│               ├── state.rs
│               ├── commands/
│               └── events.rs
├── crates/
│   ├── shared-protocol/
│   │   ├── Cargo.toml
│   │   └── src/ (models.rs, messages.rs, errors.rs, lib.rs)
│   ├── media-stream/
│   │   ├── Cargo.toml
│   │   └── src/ (server.rs, range.rs, auth.rs, lib.rs)
│   ├── sync-engine/
│   │   ├── Cargo.toml
│   │   └── src/ (clock.rs, drift.rs, revision.rs, lib.rs)
│   ├── room-server/
│   │   ├── Cargo.toml
│   │   └── src/ (server.rs, room.rs, participant.rs, permissions.rs, lib.rs)
│   ├── network/
│   │   ├── Cargo.toml
│   │   └── src/ (p2p.rs, stun.rs, turn.rs, proxy.rs, reconnect.rs, lib.rs)
│   └── media-engine/
│       ├── Cargo.toml
│       ├── build.rs
│       └── src/ (engine.rs, properties.rs, commands.rs, tracks.rs, mock.rs, lib.rs)
├── services/
│   └── signaling/
│       ├── package.json (or Cargo.toml)
│       └── index.js (or main.rs)
└── tests/
    ├── integration/
    │   ├── local_player_test.rs
    │   ├── streaming_test.rs
    │   └── room_lifecycle_test.rs
    └── network/
        ├── sync_drift_test.rs
        └── reconnect_test.rs
```
