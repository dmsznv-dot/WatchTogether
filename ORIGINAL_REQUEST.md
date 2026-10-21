# Original User Request

## 2026-10-20T20:48:04Z

Build a cross-platform collaborative video player desktop application ("Watch Together") for macOS (Apple Silicon) and Windows (x64). The app combines a full-featured local media player with synchronized co-watching: one user opens a local video file, creates a room, shares a link/code, and other participants join to watch the same video in sync. No cloud upload required — the host streams the original file directly via HTTP byte-range requests; playback commands synchronize through WebSocket.

Working directory: ~/teamwork_projects/watch_together
Integrity mode: development

### Reference Material

The full product requirements document is provided below as context. The team should use it as the authoritative specification.

---

## Requirements

### R1. Full-featured local video player

A Tauri 2 (Rust backend) + React + TypeScript (frontend) desktop application that plays local video files using libmpv (embedded). Must support:
- Opening files via file dialog and drag-and-drop
- Container formats: MP4, MOV, MKV
- Codec decoding: at minimum H.264, H.265/HEVC, VP9 (via libmpv/ffmpeg)
- Multiple audio track selection
- Subtitle support: embedded tracks + external .srt and .ass files
- Playback speed: 0.5×, 0.75×, 1×, 1.25×, 1.5×, 2×
- Seek: ±5s (arrow keys), ±30s (Shift+arrow)
- Pause/play (Space or button)
- Fullscreen toggle (double-click, F key, button)
- Local volume control with mute (M key)
- Timeline showing current position and total duration
- Resizable window maintaining video aspect ratio
- Keyboard shortcuts as specified in the PRD (Space, arrows, Shift+arrows, F, M, Esc, Cmd/Ctrl+O)

UI design: minimal, dark-themed desktop media player. Controls appear on mouse hover and hide during playback. See the mockup screenshots described in the PRD.

### R2. Room creation and co-watching with synchronized playback

When the host has a video open, they can click "Watch Together" to:
1. Start a local room server (embedded in the Tauri Rust backend)
2. Generate a unique room code and shareable invitation link
3. Serve the video file via HTTP byte-range requests to connected participants
4. Manage room state and broadcast playback commands via WebSocket

Participants join by entering a room code or link inside the app. Upon connecting:
- The client begins buffering the remote video stream (HTTP Range from host)
- libmpv on the client plays the streamed video using the HTTP URL
- The client synchronizes to the room's current playback position

Room capacity: 2–5 participants including the host.

Two permission modes (toggled by host):
- **Free control**: any participant can play/pause/seek/change speed, affecting everyone
- **Host-only control**: only the host controls shared playback

Synchronized globally: play/pause state, playback position (timecode), seek, playback speed.
Local per-participant (NOT synchronized): volume, fullscreen, audio track selection, subtitle selection.

### R3. Playback synchronization engine

The host maintains the authoritative room state (JSON with revision number, status, positionMs, speed, updatedAtServerMs). Updates are broadcast to all clients via WebSocket.

Synchronization algorithm:
- Heartbeat + position check every 1–2 seconds
- Latency estimation via timestamp exchange (sync.ping)
- Commands carry a monotonically increasing `revision` number; stale messages are ignored
- Drift correction thresholds:
  - 0–100ms: no correction
  - 100–500ms: gradual speed adjustment
  - >500ms: hard seek to target position
- Late joiners start at the current room position, not the beginning
- After reconnection, the client re-syncs to the server state
- Target: ≤250ms desynchronization between participants under stable network conditions

### R4. Network connectivity via public STUN/TURN and minimal signaling

For connections across the internet:
- Implement a minimal signaling server (can be a simple WebSocket relay for connection setup, deployed as a standalone Rust or Node.js service in the `services/signaling/` directory)
- Use public STUN servers for NAT traversal (e.g., Google's stun:stun.l.google.com:19302)
- Support TURN relay fallback using a configurable public TURN server URL (e.g., Cloudflare/Twilio/Metered — configured via app settings)
- The signaling server helps peers discover each other; actual video data flows directly between host and client when possible, or through the TURN relay when direct connection fails
- All connections must use TLS/encrypted transport
- Auto-reconnection on connection drop

### R5. Security and room lifecycle

- The app only exposes the specific video file selected by the host — no filesystem access
- Unique unpredictable room token (separate from the short display code)
- Every video data request is authorized with the room token
- Rate limiting on commands and connections
- Host can kick participants and revoke invitations
- Temporary tokens are deleted when the room closes
- No permanent video copy is saved on client machines
- Room server process terminates when the room is closed
- When the host quits, the room ends for everyone

## Project Structure

Follow this repository layout:

```
watch-together/
├── apps/
│   └── desktop/
│       ├── src/                    # React + TypeScript frontend
│       │   ├── components/
│       │   ├── screens/
│       │   ├── features/
│       │   │   ├── player/
│       │   │   ├── rooms/
│       │   │   ├── participants/
│       │   │   └── settings/
│       │   └── styles/
│       └── src-tauri/              # Rust backend
│
├── crates/
│   ├── media-engine/               # libmpv integration, playback control
│   ├── room-server/                # Room creation, state, permissions
│   ├── media-stream/               # HTTP byte-range file server
│   ├── sync-engine/                # Timecode sync, drift correction
│   ├── network/                    # Connection management, STUN/TURN
│   └── shared-protocol/           # Shared types, protocol events
│
├── services/
│   └── signaling/                  # Minimal signaling server
│
└── tests/
    ├── integration/
    └── network/
```

## Protocol Events

The WebSocket protocol must implement these events:

| Event | Direction | Purpose |
|-------|-----------|---------|
| `room.create` | Client → Host | Create room |
| `room.join` | Client → Host | Join room |
| `room.state` | Host → Client | Current state snapshot |
| `playback.play` | Client → Host | Request play |
| `playback.pause` | Client → Host | Request pause |
| `playback.seek` | Client → Host | Request seek |
| `playback.speed` | Client → Host | Change speed |
| `playback.updated` | Host → All | Apply new state |
| `participant.status` | Client → Host | Buffer status |
| `sync.ping` | Client ↔ Host | Latency measurement |
| `room.closed` | Host → All | End session |

## Acceptance Criteria

### Local Player
- [ ] On macOS (Apple Silicon), the app opens and plays a local MP4 file with H.264 video and AAC audio
- [ ] On macOS, the app opens and plays a local MKV file with multiple audio tracks; user can switch between audio tracks
- [ ] External .srt subtitle file can be loaded and displayed during playback
- [ ] Playback speed can be changed to 0.5×, 1×, 1.5×, 2× and the change takes effect immediately
- [ ] Keyboard shortcuts work: Space (pause/play), Left/Right arrows (±5s seek), F (fullscreen toggle), M (mute toggle)
- [ ] Fullscreen mode works via double-click on the video area
- [ ] The window is resizable and maintains the video's aspect ratio

### Room & Streaming
- [ ] Host clicks "Watch Together" and a room code is displayed within 3 seconds
- [ ] A second instance of the app (on the same machine or another machine on the same network) can join the room by entering the code
- [ ] The joining client receives the video stream via HTTP byte-range and plays it without having a local copy of the file
- [ ] When the host pauses, the client pauses within 1 second (on a local network)
- [ ] When the host seeks to a new position, the client follows to the same position within 2 seconds (on a local network)
- [ ] When the host changes playback speed, the client's speed changes accordingly
- [ ] A client joining mid-playback starts at the current room position, not at the beginning

### Sync & Permissions
- [ ] The host can switch between "free control" and "host-only" permission modes
- [ ] In "host-only" mode, a non-host participant's pause/seek commands are rejected
- [ ] Each participant can independently adjust volume and select subtitles without affecting others
- [ ] Heartbeat ping/pong messages are exchanged at least every 2 seconds (verifiable in logs)
- [ ] Room state includes a monotonically increasing `revision` number (verifiable in logs)

### Security & Lifecycle
- [ ] The room token is a cryptographically random string of at least 32 characters
- [ ] After the host closes the room, the client can no longer access the video stream (HTTP requests return 403 or connection refused)
- [ ] The app does not expose any file other than the selected video file (attempting to access a different path returns 404/403)

### Build
- [ ] `cargo tauri build` produces a working .dmg on macOS (Apple Silicon)
- [ ] The project compiles without errors on a clean checkout with documented setup steps (README with prerequisites and build instructions)

---

## Full PRD (Reference)

Below is the complete Product Requirements Document provided by the stakeholder. Use it as the authoritative source for any details not covered in the requirements above.

### Концепция
Десктопное приложение, объединяющее функциональность VLC с возможностью синхронного просмотра локального видео несколькими пользователями через интернет.

Главное отличие — пользователям не требуется загружать видео в облако, иметь одинаковые файлы или запускать сервер вручную.

### Архитектура
Гибридная модель: сервер комнаты работает на компьютере создателя, внешний сервис помогает с установлением соединений и резервной ретрансляцией. Сохраняет принцип локального хранения, решает проблему NAT.

### Передача видео
HTTP byte-range streaming исходного файла. Сохраняются оригинальные аудио/видео дорожки, субтитры. Без транскодирования. Декодирование локально у каждого участника.

### Синхронизация
Хост хранит авторитетное состояние комнаты с revision. Heartbeat каждые 1–2 секунды. Коррекция: 0–100мс без действий, 100–500мс плавная коррекция скорости, >500мс точная перемотка.

### Нефункциональные требования
- Участники: 2–5
- Создание комнаты: <3 сек
- Синхронизация: ≤250мс при стабильной сети
- Разрешение: до 4K
- Аппаратное декодирование когда доступно
- Автопереподключение при потере соединения
- Сохранение настроек между запусками

### Исключения из MVP
Нет: аккаунтов, облака, стриминга экрана, голоса, чата, мобильных приложений, транскодирования, медиатеки, плейлистов, торрентов.

## Follow-up — 2026-10-21T01:11:54Z

Quota limit has reset. Please resume work on the Watch Together project from where it stopped. All files in ~/teamwork_projects/watch_together are preserved.
