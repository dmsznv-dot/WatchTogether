# Embedded player integration

The desktop app initializes libmpv after the Tauri window exists. macOS uses a
native NSOpenGLView and libmpv's OpenGL render API; Windows uses a native child
container underneath WebView2 and libmpv's D3D11 output. The transparent webview
keeps the React HUD, menus and keyboard focus above the native video surface.
Fullscreen changes the Tauri window, rather than a separate mpv window.

mpv's independent key bindings are disabled. The React handlers support physical
F/M/O keys on non-Latin layouts, skip editable controls and modified shortcuts,
and suppress repeated toggle actions. Native file drops use Tauri's absolute
paths; the subtitle picker filters SRT/ASS separately from video files.

The decoder observes `track-list`, `aid`, `sid` and file-loaded events. Audio and
subtitle menus receive the actual tracks, IDs, selection, language and codec.
Snapshot requests cannot overwrite newer track events or local selection. External
subtitle insertion is deduplicated by ID. The event observer is stopped and joined
before its mpv handle is destroyed; the AppKit renderer is freed first.

## Verification

- `npm --prefix apps/desktop test`: 101 tests pass.
- `npm --prefix apps/desktop run build`: TypeScript and Vite pass.
- `cargo build -p watch-together-desktop --offline`: compiles on Apple Silicon.
- `cargo test -p media-engine --offline`: 22 tests pass; the real-decoder test is
  opt-in and is run separately below.
- `python3 scripts/create_player_fixture.py` followed by
  `WT_TEST_MEDIA="$PWD/target/player-fixtures/multi.mkv" cargo test -p media-engine --test real_mpv_test -- --ignored`:
  passes against installed libmpv, with two AAC audio streams, embedded ASS and
  external Cyrillic SRT. It checks track notification, audio and subtitle
  selection, language metadata and subtitle disabling.

## Remaining verification before release

This change is a draft until actual playback with the HUD is checked on both
platforms. The current execution sandbox denies AppKit desktop services, cannot
create an accelerated OpenGL pixel format, and has no Windows runtime. A successful
build does not establish that video composition, focus and fullscreen work on a
real desktop.

`cargo test --workspace --offline --no-fail-fast` also encounters sandbox socket
permission failures. Failed targets: media-stream/adversarial_test and streaming_test;
network/adversarial_loopback_proxy_test, adversarial_stun_turn_test,
loopback_stress_test and proxy_test; room-server/adversarial_tests and server_test;
signaling/adversarial_signaling_test and signaling_test;
watch-together-desktop/adversarial_stress_test and desktop_commands_test;
watch-together-tests/integration, media_stream_test and network.
The CI matrix runs these outside the local socket sandbox and also runs the
real-decoder fixture on macOS and Windows.

On an interactive Mac, test native rendering and resizing with:

```sh
clang -Wno-deprecated-declarations -I"$(brew --prefix)/include" \
  -L"$(brew --prefix)/lib" -lmpv -framework AppKit -framework OpenGL \
  scripts/test_macos_surface.m crates/media-engine/src/mpv/macos_surface.m \
  -o /tmp/watchtogether-surface-test
/tmp/watchtogether-surface-test "$PWD/target/player-fixtures/multi.mkv"
```

Then verify in the complete app: video stays inside the HUD on open/reopen,
resize, Retina/DPI changes, fullscreen and exit; Space/arrows/Shift+arrows,
volume, mute, F, Escape and Cmd/Ctrl+O work while the HUD has focus; both audio
tracks sound different; embedded ASS and external SRT display correctly and can
be disabled; closing a window during playback exits cleanly.

## Packaging

The Windows helper now downloads a pinned libmpv release and verifies its SHA-256.
The Windows Tauri configuration packages the DLL beside the executable. The macOS
CI bundles libmpv's dependency chain before generating the DMG. Installer output
and launch on a machine without developer-installed libmpv remain release checks.
