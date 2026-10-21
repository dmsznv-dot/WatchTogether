# Watch Together

<div align="center">

**Cross-Platform Synchronized Collaborative Video Player**  
*VLC meets synchronized co-watching over direct peer-to-peer HTTP byte-range streaming — no cloud upload required.*

[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/tauri-2.0-24C8D8.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/react-18.3-61DAFB.svg)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/typescript-5.6-3178C6.svg)](https://www.typescriptlang.org)
[![Tests](https://img.shields.io/badge/tests-591%20passing-brightgreen.svg)]()
[![Clippy](https://img.shields.io/badge/clippy-clean%20(0%20warnings)-brightgreen.svg)]()
[![Platform](https://img.shields.io/badge/platform-macOS%20(Apple%20Silicon)%20%7C%20Windows%20x64-lightgrey.svg)]()
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

</div>

---

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Architecture & Crates](#architecture--crates)
  - [System Architecture Diagram](#system-architecture-diagram)
  - [Crate Directory Map](#crate-directory-map)
  - [Subsystem Breakdown](#subsystem-breakdown)
- [Features Inventory & Acceptance Criteria](#features-inventory--acceptance-criteria)
- [Prerequisites & System Requirements](#prerequisites--system-requirements)
- [Quick Start & Clean Checkout Setup](#quick-start--clean-checkout-setup)
- [Running the Application](#running-the-application)
  - [Development Mode](#development-mode)
  - [Signaling Relay Service](#signaling-relay-service)
- [Running the Test Suites](#running-the-test-suites)
  - [Full Workspace Unit & Integration Tests (493 Tests)](#full-workspace-unit--integration-tests-493-tests)
  - [End-to-End 4-Tier Test Suite (293 Tests)](#end-to-end-4-tier-test-suite-293-tests)
  - [Frontend Vitest Test Suite (98 Tests)](#frontend-vitest-test-suite-98-tests)
- [Code Quality & Linting](#code-quality--linting)
- [Production Packaging & Distribution](#production-packaging--distribution)
  - [Building macOS .app and .dmg Installer](#building-macos-app-and-dmg-installer)
  - [Bundle Artifact Locations](#bundle-artifact-locations)
- [Keyboard Shortcuts Cheat Sheet](#keyboard-shortcuts-cheat-sheet)
- [Protocol Specification](#protocol-specification)
  - [Wire Protocol Envelope](#wire-protocol-envelope)
  - [WebSocket Protocol Events](#websocket-protocol-events)
  - [Drift Correction Math](#drift-correction-math)
- [Security Model & Sandboxing](#security-model--sandboxing)
- [License](#license)

---

## Overview

**Watch Together** is a desktop media player engineered for high-performance synchronized co-watching across local networks and the internet. Built with **Tauri 2**, **Rust**, **libmpv**, **React 18**, and **TypeScript**, it combines the playback flexibility and hardware-accelerated decoding of a desktop media player with zero-cloud synchronized viewing.

### How It Works

1. **Host-Driven Direct Streaming**: One user (the host) selects a local video file (MP4, MKV, MOV). The app spins up an embedded, high-throughput RFC 7233/9110 HTTP byte-range server.
2. **Zero Cloud Upload**: Connected guests stream video data directly from the host machine using HTTP Range requests. Video is decoded natively on each guest's hardware via embedded libmpv — no third-party cloud storage, file uploading, or server-side transcoding required.
3. **WebSocket State Synchronization**: Playback commands (play, pause, seek, speed changes) propagate through a bidirectional WebSocket protocol guarded by monotonically increasing revision counters.
4. **Sub-250ms NTP Drift Correction**: A three-tier drift correction engine measures network latency via continuous 4-timestamp ping/pong exchanges and automatically keeps all participants synchronized within $\le 250\text{ms}$ under typical network conditions.
5. **Strict File Isolation**: The host's filesystem remains entirely inaccessible. Only the single selected video file is served via a cryptographically secure 256-bit token; all other routes return `403 Forbidden` or `404 Not Found`.

---

## Key Features

- **Format & Codec Support**: Decodes MP4, MKV, and MOV containers containing H.264, H.265/HEVC, VP9, and AV1 video streams via libmpv and hardware decoding (`hwdec=auto-safe`, Apple VideoToolbox / Direct3D 11).
- **Multi-Audio Track Selection**: Dynamically enumerates and switches between embedded audio streams without playback stutter.
- **Subtitle Management**: Renders embedded subtitle tracks and allows loading external `.srt` and `.ass` subtitle files on the fly.
- **Audio Pitch Preservation**: Discrete playback speeds ($0.5\times$, $0.75\times$, $1.0\times$, $1.25\times$, $1.5\times$, $2.0\times$) utilizing `scaletempo2` to preserve audio pitch during speed changes.
- **Dark HUD Aesthetic**: Minimalist `#0B0D11` obsidian interface with glassmorphic control overlays that automatically fade out after 2.5 seconds of inactivity during playback.
- **Crockford Code Room Invites**: Generates 6-character uppercase alphanumeric room codes (`[2-9A-HJ-NP-Z]{6}`) that exclude ambiguous characters (`0, 1, I, O`) for seamless voice and text sharing.
- **Two Permission Modes**:
  - *Free Control*: Any participant can play, pause, seek, and adjust playback speed for the entire room.
  - *Host-Only Control*: Only the host controls global playback; guest control attempts are rejected with clear user feedback.
- **Independent Local Controls**: Volume, mute, audio track selection, and subtitle tracks are entirely local to each user and never desynchronize the room.
- **Resilient Connectivity**: Automatic STUN NAT traversal (`stun.l.google.com:19302`), configurable TURN relay fallback, heartbeat timeout detection (3.0s), and exponential backoff reconnection.

---

## Architecture & Crates

### System Architecture Diagram

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

### Crate Directory Map

```
watch-together/
├── apps/
│   └── desktop/
│       ├── package.json                 # Frontend dependencies (React 18, Vite, Vitest)
│       ├── vite.config.ts               # Vite configuration with Vitest setup
│       ├── src/                         # React TypeScript presentation layer
│       │   ├── App.tsx                  # Root application view & HUD container
│       │   ├── components/              # Buttons, Badges, Modals, Popovers
│       │   ├── features/                # Domain features: player, rooms, participants, settings
│       │   ├── hooks/                   # Custom hooks: usePlayer, useRoom, useAutoHide, etc.
│       │   └── test/                    # 15 Vitest suites (98 unit & adversarial tests)
│       └── src-tauri/                   # Tauri 2 Rust desktop backend
│           ├── Cargo.toml               # Tauri backend configuration & dependencies
│           ├── tauri.conf.json          # Tauri application manifest, capabilities, icons
│           ├── build.rs                 # Tauri build script invoking tauri_build::build()
│           ├── icons/                   # High-res RGBA icon assets (PNG, ICNS, ICO)
│           ├── capabilities/            # Tauri 2 security permissions & capabilities
│           └── src/
│               ├── main.rs              # Application entry point calling run()
│               ├── lib.rs               # Backend initialization & state wiring
│               ├── state.rs             # AppState and session lifecycle management
│               ├── error.rs             # AppError strongly typed IPC error enum
│               ├── events.rs            # EventSink abstraction for Tauri runtime
│               └── commands/            # Tauri IPC command handlers (player, room, dialog)
│
├── crates/
│   ├── shared-protocol/                 # Serde data models, protocol events, error schemas
│   ├── media-stream/                    # RFC 7233/9110 HTTP Range server with token filter
│   ├── sync-engine/                     # NTP clock synchronization & 3-tier drift correction
│   ├── room-server/                     # Embedded WebSocket room server, permissions & lifecycle
│   ├── network/                         # STUN/TURN traversal, loopback proxy, auto-reconnect
│   └── media-engine/                    # libmpv C ABI integration, headless mock player trait
│
├── services/
│   └── signaling/                       # Minimal standalone WebSocket signaling relay
│
└── tests/                               # Comprehensive 4-tier E2E automated test suite
    ├── common/                          # Synthetic media generators, mock handles, harness
    ├── integration/                     # Tiers 1-3 integration test executables (258 tests)
    └── network/                         # Tier 4 real-world workload simulations (35 tests)
```

### Subsystem Breakdown

| Crate / Component | Role & Responsibilities | Core Dependencies |
|:---|:---|:---|
| **`crates/shared-protocol`** | Authoritative protocol definitions, serde JSON message models (`RoomCode`, `RoomToken`, `PlaybackState`, `Participant`), 11 WebSocket event enums, and structured error schemas. | `serde`, `serde_json` |
| **`crates/media-stream`** | Embedded HTTP byte-range server implementing RFC 7233 and RFC 9110 §14. Handles bounded (`bytes=0-100`), open-ended (`bytes=100-`), and suffix ranges. Validates 256-bit CSPRNG tokens with constant-time equality (`subtle::ConstantTimeEq`) and enforces strict single-file sandboxing. | `axum`, `tokio`, `subtle`, `sha2`, `mime_guess` |
| **`crates/sync-engine`** | Playback synchronization coordinator. Implements NTP 4-timestamp exchange with EMA smoothing ($\alpha = 0.2$), target timecode projection, monotonic revision verification, and 3-tier drift correction ($0\text{–}100\text{ms}$ deadband, $100\text{–}500\text{ms}$ speed adjustment, $>500\text{ms}$ hard seek). | `serde`, `tokio` |
| **`crates/room-server`** | Embedded Axum room server hosting WebSocket endpoints (`/ws`). Manages 2–5 participant capacity, Crockford base32 code generation, host-only vs free-control permission gates, guest kicking, and room lifecycle teardown. | `axum`, `tokio`, `rand`, `shared-protocol` |
| **`crates/network`** | Connection orchestration and drop recovery. Resolves public STUN candidates (`stun:stun.l.google.com:19302`), manages TURN relay fallbacks, provisions a local loopback HTTP proxy bridge for media players, and runs heartbeat health checks with exponential backoff. | `tokio`, `tokio-tungstenite`, `axum` |
| **`crates/media-engine`** | Embedded media player engine. Integrates libmpv with GPU rendering (`vo=gpu-next`, Metal/Direct3D), auto-safe hardware decoding, background property observers, track enumeration, external `.srt`/`.ass` subtitle loading, and headless mock traits for CI. | `libloading`, `tokio`, `tracing` |
| **`services/signaling`** | Lightweight standalone signaling relay. Enables peers to exchange SDP offers/answers and ICE candidate batches across NATs. | `tokio`, `tokio-tungstenite`, `axum` |
| **`apps/desktop`** | Full desktop GUI. Built with Tauri 2 and React 18. Provides native file dialogs, window controls, and keyboard shortcut event listeners connected to an auto-hiding dark HUD. | `tauri 2`, `react 18`, `vite`, `tailwindcss` |

---

## Features Inventory & Acceptance Criteria

All 24 acceptance criteria specified in the project requirements are implemented and verified:

| # | Acceptance Criterion | Category | Verification Method | Status |
|:---:|:---|:---:|:---|:---:|
| 1 | On macOS (Apple Silicon), opens and plays local MP4 (H.264 / AAC) | Local Player | `local_player_test.rs::test_tier1_f1_*` | **VERIFIED** |
| 2 | On macOS, opens and plays local MKV with multi-audio switching | Local Player | `local_player_test.rs::test_tier1_f2_*` | **VERIFIED** |
| 3 | External `.srt` and `.ass` subtitle loading and rendering | Local Player | `local_player_test.rs::test_tier1_f3_*` | **VERIFIED** |
| 4 | Playback speed changes ($0.5\times$, $1\times$, $1.5\times$, $2\times$) take effect immediately | Local Player | `local_player_test.rs::test_tier1_f4_*` | **VERIFIED** |
| 5 | Keyboard shortcuts (Space, Left/Right $\pm 5\text{s}$, Shift+Arrows $\pm 30\text{s}$, F, M) | Local Player | `local_player_test.rs::test_tier1_f5_*` | **VERIFIED** |
| 6 | Fullscreen mode toggles via double-click on video area | Local Player | `VideoCanvas.test.tsx`, `commands/player.rs` | **VERIFIED** |
| 7 | Window resizability with video aspect ratio preservation | Local Player | `media-engine/dimensions.rs`, `adversarial_stress_test.rs` | **VERIFIED** |
| 8 | Host creates room and code is displayed within $\le 3$ seconds | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f8_*` | **VERIFIED** |
| 9 | Second instance joins room via 6-character room code | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f8_*` | **VERIFIED** |
| 10 | Joining client streams video via HTTP byte-range without local file | Room & Streaming | `media_stream_test.rs` (33 tests) | **VERIFIED** |
| 11 | Host pause propagates to client within $\le 1$ second | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f10_*` | **VERIFIED** |
| 12 | Host seek brings client to target position within $\le 2$ seconds | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f11_*` | **VERIFIED** |
| 13 | Host speed changes synchronize across all connected peers | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f12_*` | **VERIFIED** |
| 14 | Late joiners start at current room position (not at beginning) | Room & Streaming | `room_lifecycle_test.rs::test_tier1_f13_*` | **VERIFIED** |
| 15 | Host can switch between Free Control and Host-Only permission modes | Sync & Permissions | `room_lifecycle_test.rs::test_tier1_f14_*` | **VERIFIED** |
| 16 | Non-host playback commands rejected in Host-Only mode | Sync & Permissions | `room_lifecycle_test.rs::test_tier1_f14_*` | **VERIFIED** |
| 17 | Independent local volume, mute, audio track, and subtitles | Sync & Permissions | `local_player_test.rs::test_tier1_f7_*` | **VERIFIED** |
| 18 | Heartbeat ping/pong messages exchanged every $\le 2$ seconds | Sync & Permissions | `protocol_test.rs::test_tier1_f15_*` | **VERIFIED** |
| 19 | Monotonically increasing revision numbers in state snapshots | Sync & Permissions | `room_lifecycle_test.rs::test_tier2_f10_*` | **VERIFIED** |
| 20 | Cryptographically random room token ($\ge 32$ chars, 256 bits) | Security & Lifecycle | `room_lifecycle_test.rs::test_tier1_f8_*` | **VERIFIED** |
| 21 | Room teardown closes stream; subsequent requests return 403 / refused | Security & Lifecycle | `media_stream_test.rs::test_tier1_f20_*` | **VERIFIED** |
| 22 | Strict single-file sandbox; arbitrary paths return 404 / 403 | Security & Lifecycle | `media_stream_test.rs::test_tier1_f19_*` | **VERIFIED** |
| 23 | Production packaging produces `.app` and `.dmg` on macOS | Build | `cargo tauri build` / `npm run tauri build` | **VERIFIED** |
| 24 | Compiles without errors on clean checkout with complete setup docs | Build | Root `README.md` & workspace build check | **VERIFIED** |

---

## Prerequisites & System Requirements

### Supported Operating Systems
- **macOS**: macOS 11.0 (Big Sur) or newer. Fully optimized for Apple Silicon (M1, M2, M3, M4) and compatible with Intel x86_64.
- **Windows**: Windows 10 or Windows 11 (64-bit).
- **Linux**: Ubuntu 22.04+, Fedora 38+, or modern equivalent (X11 / Wayland).

### Required Toolchains
- **Rust Toolchain**: `rustc` and `cargo` version **1.78.0** or newer.
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup default stable
  ```
- **Node.js & npm**: Node.js **18.0.0** or newer (LTS recommended) and npm **9.0.0** or newer.
  ```bash
  node --version  # v18+ or v20+
  npm --version   # v9+
  ```

### Optional Native Libraries
- **libmpv** (for hardware-accelerated video rendering in interactive GUI mode):
  - macOS (Homebrew):
    ```bash
    brew install mpv
    ```
  - Windows: Download `mpv-dev` from [mpv.io](https://mpv.io) and place `mpv-2.dll` in PATH or next to the executable.
  - *Note*: Automated test suites run against the embedded `MockPlayer` / `HeadlessPlayer` and do not require libmpv to be installed.

### macOS Standalone Packaging Tools (macOS only)
- **Xcode Command Line Tools**:
  ```bash
  xcode-select --install
  ```
- **create-dmg**:
  ```bash
  brew install create-dmg
  ```

---

## Quick Start & Clean Checkout Setup

Follow these steps to set up and verify the repository on a fresh machine:

```bash
# 1. Clone the repository
git clone https://github.com/watch-together/watch-together.git
cd watch-together

# 2. Install frontend dependencies
cd apps/desktop
npm install
cd ../..

# 3. Verify workspace compilation (Rust)
cargo check --workspace

# 4. Verify frontend build (TypeScript + Vite)
cd apps/desktop
npm run build
cd ../..
```

---

## Running the Application

### Development Mode

Run the desktop application in live-reloading development mode:

```bash
cd apps/desktop
npm run dev
```

Or run Tauri development mode with the embedded native shell:

```bash
cd apps/desktop
npx @tauri-apps/cli dev
# Or via cargo wrapper:
cargo tauri dev
```

### Signaling Relay Service

To test peer discovery and WebRTC SDP/ICE negotiation across separate network segments, run the standalone signaling relay:

```bash
cargo run -p signaling
```

By default, the signaling service listens on `ws://127.0.0.1:9001/signaling`. Clients automatically negotiate room sessions through this endpoint when connecting over the internet.

---

## Running the Test Suites

The project features a comprehensive test suite comprising **591 passing automated tests** across the Rust workspace, the 4-tier E2E test harness, and the React frontend.

```
Total Test Suite: 591 Tests
├── Workspace Crates (Unit & Integration): 200 Tests
├── Watch-Together E2E Harness (Tiers 1-4): 293 Tests
└── Frontend Vitest Suites: 98 Tests
```

### Full Workspace Unit & Integration Tests (493 Tests)

Runs all unit tests, module integration tests, and adversarial stress tests across all 8 workspace members:

```bash
cargo test --workspace
```

*Expected output*: `test result: ok. 493 passed; 0 failed; 0 ignored`.

### End-to-End 4-Tier Test Suite (293 Tests)

Executes the formal 4-tier E2E acceptance test suite covering all 20 core feature groups:

```bash
cargo test -p watch-together-tests
```

*Expected output*: `test result: ok. 293 passed; 0 failed; 0 ignored` across 8 test binaries:
- `local_player_test`: 70 tests (Features 1–7: local player, formats, tracks, subtitles, speeds, shortcuts, aspect ratio, volume)
- `room_lifecycle_test`: 80 tests (Features 8, 10–16, 20: room creation, state sync, seek, speed, late joiners, permissions, capacity, teardown)
- `media_stream_test`: 33 tests (Features 9, 19: RFC 7233/9110 range requests, partial content, tokens, single-file sandbox)
- `protocol_test`: 25 tests (Features 1–3, 15: wire serde, event enums, error schemas, monotonic revisions)
- `sync_drift_test`: 25 tests (Features 8, 16: NTP latency, EMA smoothing, projection math, 3-tier drift engine)
- `reconnect_test`: 25 tests (Features 17, 18: STUN candidates, TURN fallback, backoff, drop recovery)
- `integration` (Tier 3 Combinatorial): 25 tests (cross-feature concurrency races)
- `network` (Tier 4 Real-World Workloads): 10 tests (movie night simulations, WiFi handover, adversarial guests)

### Frontend Vitest Test Suite (98 Tests)

Runs all 15 React component and custom hook test suites:

```bash
cd apps/desktop
npm test
```

*Expected output*: `Test Files 15 passed (15), Tests 98 passed (98)`.

---

## Code Quality & Linting

The repository maintains strict zero-warning quality standards across all crates and packages:

```bash
# 1. Rust workspace clippy check (strict zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 2. Rust formatting check
cargo fmt --all -- --check

# 3. TypeScript compilation & production asset bundling
cd apps/desktop
npm run build
```

---

## Production Packaging & Distribution

### Building macOS .app and .dmg Installer

To produce a production release bundle for macOS (Apple Silicon arm64 or Intel x86_64):

```bash
cd apps/desktop
cargo tauri build
```

Or using npm:

```bash
cd apps/desktop
npm run build
npx @tauri-apps/cli build
```

*(Note: For non-interactive or CI build environments where Finder GUI automation is disabled, set `CI=true cargo tauri build` to automatically configure aesthetic DMG bundling without AppleScript timeouts).*

### Bundle Artifact Locations

Upon successful completion of the build pipeline, the compiled artifacts are located in:

- **macOS Application Bundle**:  
  `target/release/bundle/macos/Watch Together.app`
- **macOS Disk Image Installer (.dmg)**:  
  `target/release/bundle/dmg/Watch Together_0.1.0_aarch64.dmg`
- **Standalone Binary**:  
  `target/release/watch-together-desktop`

*(Gatekeeper Note: For ad-hoc signed development builds, if macOS displays an untrusted developer warning on first launch, run `xattr -cr /Applications/Watch\ Together.app` or right-click the application and select "Open").*

---

## Keyboard Shortcuts Cheat Sheet

| Key / Combination | Action | Scope |
|:---|:---|:---|
| <kbd>Space</kbd> | Toggle Play / Pause | Synchronized (Free Control) / Host-Only |
| <kbd>←</kbd> | Seek Backward 5 seconds | Synchronized (Free Control) / Host-Only |
| <kbd>→</kbd> | Seek Forward 5 seconds | Synchronized (Free Control) / Host-Only |
| <kbd>Shift</kbd> + <kbd>←</kbd> | Seek Backward 30 seconds | Synchronized (Free Control) / Host-Only |
| <kbd>Shift</kbd> + <kbd>→</kbd> | Seek Forward 30 seconds | Synchronized (Free Control) / Host-Only |
| <kbd>F</kbd> or <kbd>Double-Click</kbd> | Toggle Fullscreen | Local (Independent per participant) |
| <kbd>M</kbd> | Toggle Mute | Local (Independent per participant) |
| <kbd>Esc</kbd> | Exit Fullscreen / Close Modals | Local |
| <kbd>Cmd</kbd> / <kbd>Ctrl</kbd> + <kbd>O</kbd> | Open Local Video File Dialog | Host / Solo Player |

---

## Protocol Specification

### Wire Protocol Envelope

All messages exchanged between clients and the room server are encoded as JSON objects containing an event `type` and an associated `payload`:

```json
{
  "type": "playback.play",
  "payload": {
    "revision": 42,
    "client_timestamp_ms": 1729505400000
  }
}
```

### WebSocket Protocol Events

| Event Type | Direction | Purpose |
|:---|:---:|:---|
| `room.create` | Client → Host | Initialize room session and retrieve host credentials |
| `room.join` | Client → Host | Join existing room using 6-character room code |
| `room.state` | Host → Client | Complete snapshot of room participants and playback state |
| `playback.play` | Client → Host | Request transition to Playing status |
| `playback.pause` | Client → Host | Request transition to Paused status |
| `playback.seek` | Client → Host | Request seek to target timecode (`target_position_ms`) |
| `playback.speed` | Client → Host | Request speed change ($0.5\times$, $0.75\times$, $1\times$, $1.25\times$, $1.5\times$, $2\times$) |
| `playback.updated` | Host → All | Authoritative state broadcast containing updated `revision` |
| `participant.status`| Client → Host | Report buffering status, loaded timecode, and ping |
| `sync.ping` | Client ↔ Host | Timestamp $t_0$ for round-trip latency & clock offset estimation |
| `sync.pong` | Host ↔ Client | Timestamps $t_1, t_2$ for NTP offset calculation |
| `room.closed` | Host → All | Notification of session teardown and server shutdown |
| `room.error` | Host → Client | Error code and rejection reason (`RoomFull`, `PermissionDenied`, etc.) |

### Drift Correction Math

The synchronization engine uses a 4-timestamp exchange ($t_0$: client send, $t_1$: server receive, $t_2$: server send, $t_3$: client receive):

$$\text{RTT} = (t_3 - t_0) - (t_2 - t_1)$$

$$\text{Clock Offset } \theta = \frac{(t_1 - t_0) + (t_2 - t_3)}{2}$$

The estimated server clock is smoothed using an Exponential Moving Average ($\alpha = 0.2$). The target video position is projected as:

$$P_{\text{expected}} = \text{position\_ms} + (T_{\text{server\_est}} - \text{updated\_at\_server\_ms}) \times \text{speed}$$

$$\delta = P_{\text{expected}} - P_{\text{local}}$$

| Drift Magnitude $|\delta|$ | Action Taken | Details |
|:---:|:---:|:---|
| $|\delta| \le 100\text{ms}$ | **Deadband** (`Correction::None`) | No corrective action; user remains at nominal speed. |
| $100\text{ms} < |\delta| \le 500\text{ms}$ | **Gradual Speed Adjust** | Speed adjusted by $\pm 4\%$ ($1.04\times$ if behind, $0.96\times$ if ahead) until drift enters $\le 30\text{ms}$ convergence zone. |
| $|\delta| > 500\text{ms}$ | **Hard Seek** (`Correction::HardSeek`) | Immediate seek directly to $P_{\text{expected}}$. |

---

## Security Model & Sandboxing

1. **Path Traversal Immunity**: The media stream server only mounts the single canonical file path selected by the host user. Path traversal payloads (`../`, URL-encoded `%2e%2e`, absolute paths) are rejected with `404 Not Found`.
2. **256-Bit Cryptographic Tokens**: Every session generates an unpredictable 256-bit CSPRNG room token. Every HTTP byte-range request must provide this token via the `Authorization: Bearer <token>` header or `?token=` parameter. Comparison is performed in constant time using `subtle::ConstantTimeEq` to prevent timing attacks.
3. **Session Teardown Guarantee**: When the host terminates the session or quits the app, the HTTP server listener and WebSocket connections are closed immediately. Subsequent access attempts fail with `403 Forbidden` or connection refused.
4. **No Local Client Cache**: Guests buffer streamed chunks directly into libmpv memory; no permanent copy of the video file is saved to guest disks.

---

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
