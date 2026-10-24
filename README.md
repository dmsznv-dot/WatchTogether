# Watch Together 🎬🍿

## Совместный просмотр со своими копиями фильма

Каждый открывает локальную копию фильма на своём Mac или Windows. Ведущий создаёт
комнату внутри приложения и отправляет гостю один код `WT2-…`. Запуск, пауза,
перемотка и скорость передаются обоим участникам.

Соединение работает через интернет без настройки роутера и VPN: iroh пробивает NAT,
а если прямой путь невозможен, идёт через relay. Код уже содержит адрес компьютера
ведущего; одного шестизначного ID комнаты для подключения недостаточно.

[Инструкция подключения и проверки](docs/SELF_HOSTED_SYNC.md).

## Скачать актуальную сборку

Изменения находятся в ветке **main**:

- Видео встраивается в окно приложения: OpenGL Render API на macOS, canvas через libmpv Software Render API на Windows (до 1280×720, около 30 кадров/с).
- Исправлены хоткеи, включая русскую раскладку, и полноэкранный режим окна.
- Исправлены обновление списка аудиодорожек, встроенные ASS и загрузка внешних SRT.
- Добавлена новая бело-серебристая иконка с чёрным Play.

### Скачать приложение для Windows

**[Скачать установщик EXE (Windows x64)](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-3746a24/WatchTogether-Windows-x64-Setup.exe)** · **[Скачать ZIP](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-3746a24/WatchTogether-Windows-x64.zip)**

Windows CI проходит тесты, сборку установщика, тихую установку и загрузку установленной libmpv. EXE включает медиабиблиотеку.

### Скачать приложение для Mac

**[Скачать ZIP для Mac](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-3746a24/WatchTogether-macOS.zip)** · **[Скачать DMG напрямую](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-3746a24/Watch_Together_macOS.dmg)**

[Описание релиза](https://github.com/dmsznv-dot/WatchTogether/releases/tag/preview-3746a24). В ZIP из Releases установщик `Watch_Together_macOS.dmg` лежит прямо в корне архива.

**Code → Download ZIP скачивает только исходники. В этом ZIP приложения и DMG нет.**

В ZIP из Actions с названием `watch-together-macos-latest.zip` установщик лежит по пути `dmg/Watch_Together_macOS.dmg`.

Это тестовое обновление: вывод видео и управление требуют проверки на реальных macOS/Windows. Windows-инсталлятор опубликован в Releases; проверка интерактивного воспроизведения на реальном Windows остаётся необходимой.

## Проверка соединения

Отдельная консольная утилита `wt-nettest` проверяет, соединятся ли два компьютера в разных
сетях без настройки роутера (iroh: QUIC, пробивка NAT, запасной relay). Видео не передаётся,
только пинги в течение 60 секунд.

**[Скачать для Mac](https://github.com/dmsznv-dot/WatchTogether/releases/download/nettest/wt-nettest-macos.zip)** · **[Скачать для Windows](https://github.com/dmsznv-dot/WatchTogether/releases/download/nettest/wt-nettest-windows.exe)** · [Релиз nettest](https://github.com/dmsznv-dot/WatchTogether/releases/tag/nettest)

[Как проверить](tools/nettest/README.md).

---

<div align="center">

**Cross-Platform Synchronized Collaborative Media Player**  
*Synchronized playback of independent local video copies on macOS and Windows.*

[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/tauri-2.0-24C8D8.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/react-18.3-61DAFB.svg)](https://react.dev)
[![Tests](https://img.shields.io/badge/tests-591%20passing-brightgreen.svg)]()
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

</div>

---

## 💡 Overview

**Watch Together** is a lightweight desktop media player for macOS and Windows that lets friends watch local movies together in perfect sync over the internet.

- 📁 **Local Copies**: Each viewer opens their own copy of the same movie (`.mp4`, `.mkv`, `.mov`).
- ⚡ **Built-in Sync Server**: The host runs the room server inside the application. Only playback commands, timing and room metadata cross the network.
- 🔄 **NTP Sync Engine**: Play, pause, seek, and speed changes are synchronized with $\le 250\text{ms}$ accuracy.
- 🎧 **Independent Audio & Subtitles**: Each viewer chooses their own language, subtitles (`.srt`, `.ass`), and volume without affecting others.
- 🔒 **Room Invitations**: An invitation includes the host endpoint and a random authorization token. No video endpoint is exposed.

---

## 🚀 Quick Start Guide (For Users)

### 🍏 macOS

1. Download **`Watch_Together_macOS.dmg`** from [Releases](https://github.com/dmsznv-dot/WatchTogether/releases) or the project folder:
   ```bash
   open target/release/bundle/dmg/Watch_Together_macOS.dmg
   ```
2. Drag **Watch Together** into your **Applications** folder.
3. Open the app.  
   *(All 97 `libmpv` codecs and dynamic libraries are bundled inside — no additional tools or Homebrew required!)*

---

### 🪟 Windows (10 / 11)

1. Download the **[Windows x64 EXE installer](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-3746a24/WatchTogether-Windows-x64-Setup.exe)** from Releases.
2. Run the installer and launch **Watch Together** from your Start menu.  
   *(Embedded hardware-accelerated video decoding is included out of the box).*

---

## 👥 How to Watch Together

### As Host (Creating a Room)
1. Open a video file: click **«Open»** in the top right (or press **Cmd/Ctrl + O**, or drag & drop a file into the window).
2. Click the blue **«Watch Together»** button in the top left.
3. Pick your permissions:
   - **Free Control**: Any friend can play, pause, seek, or change speed.
   - **Host Only**: Only you control playback.
4. Click **«Create Room»** and copy your 6-character room code (e.g. `R7K2M9`) or invitation link.
5. Enter your reachable host address (TCP 8999), then send the complete invitation link or Connection Code. For internet access, configure TCP forwarding/firewall or use a shared VPN.

### As Guest (Joining a Room)
1. Open **Watch Together** and load your own copy of the movie.
2. Click **«Join Room»** in the top left.
3. Paste the complete invitation link or Connection Code and enter your display name.
4. Click **«Join»** — your local movie follows the room playback position after the host confirms the connection.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|:---|:---|
| **Space** | Play / Pause |
| **← / →** | Seek $-5\text{s}$ / $+5\text{s}$ |
| **Shift + ← / →** | Seek $-30\text{s}$ / $+30\text{s}$ |
| **↑ / ↓** | Volume $\pm 5\%$ (Local) |
| **M** | Toggle Mute (Local) |
| **F** / **Double-click** | Toggle Fullscreen |
| **Esc** | Exit Fullscreen |
| **Cmd / Ctrl + O** | Open Local Video File |

---

## 🛠️ Building from Source (For Developers)

The project is structured as a Cargo workspace with a Tauri 2 + React 18 desktop app.

### Prerequisites

| Tool | Minimum Version | Notes |
|:---|:---|:---|
| **Node.js** | `v20+` | Package manager: `npm` |
| **Rust** | `1.78+` | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Tauri CLI** | `v2.0+` | `cargo install tauri-cli --version "^2.0.0" --locked` |

---

### 🍏 macOS Build Instructions

1. **Install libmpv** (required for dynamic linking during build):
   ```bash
   brew install mpv
   ```

2. **Install frontend dependencies**:
   ```bash
   cd apps/desktop
   npm install
   ```

3. **Run in development mode (Live Reload)**:
   ```bash
   npx tauri dev
   ```

4. **Build self-contained `.app` and `.dmg`**:
   ```bash
   # Build the release binary
   cargo tauri build --bundles app --no-sign

   # Bundle libmpv and all transitive dylibs into the app package
   cd ../..
   python3 scripts/bundle_mpv_macos.py "target/release/bundle/macos/Watch Together.app"

   # Create standalone DMG
   hdiutil create -volname "Watch Together" \
     -srcfolder "target/release/bundle/macos/Watch Together.app" \
     -ov -format UDZO \
     "target/release/bundle/dmg/Watch_Together_macOS.dmg"
   ```

---

### 🪟 Windows Build Instructions

1. **Install Windows Build Tools**:
   - Install **Visual Studio C++ Build Tools** (Desktop development with C++).

2. **Configure libmpv**:
   ```powershell
   python scripts/setup_mpv_windows.py
   ```
   *(Or download `mpv-2.dll` from [shinchiro/mpv-winbuild-cmake](https://sourceforge.net/projects/mpv-player-windows/files/libmpv/) and place it in `target/release` and `apps/desktop/src-tauri`)*.

3. **Install frontend dependencies**:
   ```powershell
   cd apps\desktop
   npm install
   ```

4. **Run in development mode**:
   ```powershell
   npx tauri dev
   ```

5. **Build Windows Installer (`.msi` / `.exe`)**:
   ```powershell
   cargo tauri build --bundles msi,nsis
   ```

---

## 🧪 Running Automated Tests

The repository contains 590+ automated tests covering all components:

```bash
# 1. Run all Rust backend tests (493 unit, integration & adversarial tests)
cargo test --workspace

# 2. Run React frontend tests (98 Vitest component & HUD stress tests)
cd apps/desktop
npm test
```

---

## 📁 Repository Structure

```
watch-together/
├── apps/
│   └── desktop/                  # Tauri 2 frontend & desktop host
│       ├── src/                  # React 18 + TypeScript + Tailwind HUD
│       └── src-tauri/            # Tauri Rust backend & IPC commands
├── crates/
│   ├── shared-protocol/          # WebSocket schemas, events & Serde models
│   ├── media-stream/             # RFC 7233/9110 HTTP Range server
│   ├── sync-engine/              # NTP timecode sync & 3-tier drift correction
│   ├── room-server/              # Room lifecycle, Crockford codes & permissions
│   ├── network/                  # P2P tunneling, STUN/TURN & loopback proxy
│   └── media-engine/             # libmpv embedding, audio tracks & subtitles
├── services/
│   └── signaling/                # Standalone Axum WebSocket signaling relay
├── scripts/
│   ├── bundle_mpv_macos.py       # Automated macOS dylib bundler
│   └── setup_mpv_windows.py      # Windows mpv DLL helper
└── .github/workflows/
    └── build.yml                 # Automated macOS & Windows CI/CD builds
```

---

## 📄 License

Dual-licensed under [MIT](LICENSE) OR [Apache-2.0](LICENSE).
