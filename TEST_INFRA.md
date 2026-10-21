# E2E Test Infra: Watch Together

## Test Philosophy
- Opaque-box, requirement-driven. Derived from user specifications in `ORIGINAL_REQUEST.md`, independent of internal implementation details.
- Comprehensive coverage across all inventoried features.
- Progressive testability: verification mechanisms do not rely on features more complex than what they verify.
- Systematic 4-tier methodology: Category-Partition, Boundary Value Analysis (BVA), Pairwise Combinatorial Testing, and Real-World Workload Testing.

## Test Architecture
- **Location**: `tests/integration/` and `tests/network/`
- **Harness & Runner**:
  - Rust integration test runner (`cargo test --test integration --test network`)
  - Standalone signaling test runner
  - Multi-process loopback harness (spawning 1 host + 2–4 clients over `127.0.0.1`)
  - Headless media engine support (`vo=null, ao=null` and `MockPlayer`)
- **Pass/Fail Semantics**:
  - Exit code 0 on all tests passing.
  - Strict assertions on timing, revision monotonicity, HTTP status codes, SHA-256 byte integrity, and drift thresholds ($\le 250\text{ms}$).

## Feature Inventory Coverage Map

| # | Feature | Requirement Source | Tier 1 (Min 5) | Tier 2 (Min 5) | Tier 3 (Pairwise) | Tier 4 (Workload) |
|---|---------|-------------------|:--------------:|:--------------:|:-----------------:|:-----------------:|
| 1 | Local MP4 H.264/AAC Playback | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 2 | Local MKV Multi-Audio Playback | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 3 | External Subtitle Loading (.srt, .ass) | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 4 | Playback Speed & Pitch (0.5x–2x) | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 5 | Keyboard Shortcuts & Navigation | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 6 | Fullscreen & Window Aspect Ratio | ORIGINAL_REQUEST § R1 | 5 | 5 | ✓ | ✓ |
| 7 | Local Volume & Mute | ORIGINAL_REQUEST § R1, R2 | 5 | 5 | ✓ | ✓ |
| 8 | Room Creation & Code/Token Generation | ORIGINAL_REQUEST § R2, R5 | 5 | 5 | ✓ | ✓ |
| 9 | HTTP Byte-Range Remote Streaming | ORIGINAL_REQUEST § R2, R5 | 5 | 5 | ✓ | ✓ |
| 10 | Playback State Synchronization (Play/Pause) | ORIGINAL_REQUEST § R2, R3 | 5 | 5 | ✓ | ✓ |
| 11 | Seek Synchronization | ORIGINAL_REQUEST § R2, R3 | 5 | 5 | ✓ | ✓ |
| 12 | Speed Synchronization | ORIGINAL_REQUEST § R2, R3 | 5 | 5 | ✓ | ✓ |
| 13 | Late Joiner Timecode Sync | ORIGINAL_REQUEST § R2, R3 | 5 | 5 | ✓ | ✓ |
| 14 | Permission Modes (Free vs Host-Only) | ORIGINAL_REQUEST § R2 | 5 | 5 | ✓ | ✓ |
| 15 | Heartbeat & Revision Monotonicity | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ | ✓ |
| 16 | Three-Tier Drift Correction (0-100, 100-500, >500ms) | ORIGINAL_REQUEST § R3 | 5 | 5 | ✓ | ✓ |
| 17 | STUN/TURN & Signaling Discovery | ORIGINAL_REQUEST § R4 | 5 | 5 | ✓ | ✓ |
| 18 | Connection Drop & Auto-Reconnection | ORIGINAL_REQUEST § R4 | 5 | 5 | ✓ | ✓ |
| 19 | Single-File Sandbox & Traversal Immunity | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ | ✓ |
| 20 | Room Teardown & Post-Close Invalidation | ORIGINAL_REQUEST § R5 | 5 | 5 | ✓ | ✓ |

## Test Tiers

### Tier 1: Feature Coverage (Equivalence Class Representatives)
- At least 5 test cases per feature in isolation.
- Covers happy paths: standard file playback, room creation, joining, streaming start, play, pause, seek, speed change, audio track switch, subtitle render.

### Tier 2: Boundary & Corner Cases (Boundary Value Analysis)
- At least 5 test cases per feature at domain boundaries:
  - Byte-range boundary tests (0-byte file, 1-byte range, range at exact EOF, `start > end`, `start >= file_size`, malformed headers).
  - Room capacity limits (2nd, 5th participant accepted; 6th rejected with `ROOM_FULL`).
  - Speed limits (0.5x, 2.0x, invalid 0.1x or 5.0x rejection/clamping).
  - Drift thresholds (99ms no action, 101ms speed adjust, 499ms speed adjust, 501ms hard seek).
  - Security boundaries (path traversal `..`, `%2f`, invalid 31-char token, expired token, revoked token).

### Tier 3: Cross-Feature Combinations (Pairwise Combinatorial Testing)
- Interaction pairs tested under concurrent execution:
  - Seek while Buffering
  - Speed change while in HostOnly mode from guest vs host
  - Audio track switch during active HTTP Range streaming
  - Late joiner connecting while host is actively seeking
  - Host changes permission mode while guest command in flight
  - Connection drop during high-speed (2x) playback

### Tier 4: Real-World Application Workloads
- Realistic end-to-end scenarios:
  1. **Full Movie Night Simulation**: Host opens 2-hour 4K MKV with dual audio and external subtitles; creates room; 3 guests join; 1 guest switches audio track locally; host pauses at 00:30:00 for break; host resumes; 1 guest seeks +30s in FreeControl mode; all 4 remain in sync $\le 250\text{ms}$.
  2. **Unstable Network & WiFi Handover**: Host streaming video to 2 guests; Guest 2 simulates 10% packet loss and 300ms jitter; verify drift engine applies gradual speed correction ($\pm 4\%$) without audio pitch distortion; disconnect interface for 4s; verify auto-reconnect and re-sync without reset to 0.
  3. **Adversarial Guest in Host-Only Room**: Host sets Host-Only mode; Guest attempts rapid-fire play/pause/seek commands; verify all commands rejected with `PermissionDenied` and host playback remains completely uninterrupted.
  4. **Host Graceful & Ungraceful Exit**: Host closes room during active streaming; verify all guest sockets receive `room.closed` and subsequent HTTP Range requests receive immediate 403 or connection refused.
  5. **Late Joiner Catch-Up**: Host plays video to 45:00 at 1.5x speed; new participant enters room code; client starts buffering and begins playback directly at 45:00 at 1.5x speed within 3 seconds.

## Coverage Thresholds
- **Tier 1**: $\ge 100$ test cases ($20 \text{ features} \times 5$)
- **Tier 2**: $\ge 100$ test cases ($20 \text{ features} \times 5$)
- **Tier 3**: $\ge 20$ pairwise interaction test cases
- **Tier 4**: $\ge 5$ end-to-end realistic application scenarios
- **Total Minimum**: $\ge 225$ automated test cases
