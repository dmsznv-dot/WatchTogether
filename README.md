# Watch Together

Совместный просмотр фильма на расстоянии. У каждого участника своя копия файла,
приложение синхронизирует запуск, паузу, перемотку и скорость. Видео по сети не
передаётся, поэтому качество не зависит от интернета.

- Mac и Windows, видео до 1080p и 60 кадров/с. На Windows с HDR-монитором — вывод
  в HDR, если в Windows включён HDR; иначе HDR-фильмы правильно показываются в обычном режиме.
- Подключение через интернет без настройки роутера, проброса портов и VPN.
- До 5 участников в комнате.
- У каждого свои громкость, аудиодорожка и субтитры. Озвучку или субтитры отдельным
  файлом можно добавить к фильму (`.mp3`, `.m4a`, `.ac3`, `.flac`, `.mka` и др.; `.srt`, `.ass`).

## Скачать

| Система | Файл |
|:---|:---|
| **macOS 15+**, Apple Silicon (M1 и новее) | [Watch_Together_macOS.dmg](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-b3e37ba/Watch_Together_macOS.dmg) |
| **Windows 10 / 11**, x64 | [WatchTogether-Windows-x64-Setup.exe](https://github.com/dmsznv-dot/WatchTogether/releases/download/preview-b3e37ba/WatchTogether-Windows-x64-Setup.exe) |

Все версии — на странице [Releases](https://github.com/dmsznv-dot/WatchTogether/releases).
Кнопка **Code → Download ZIP** скачивает только исходный код, без приложения.

## Установка

**macOS.** Откройте DMG и перетащите Watch Together в «Программы». Приложение не
подписано сертификатом Apple, поэтому при первом запуске macOS его заблокирует.
Откройте Системные настройки → Конфиденциальность и безопасность → «Всё равно открыть»
или выполните в Терминале:

```bash
xattr -dr com.apple.quarantine "/Applications/Watch Together.app"
```

**Windows.** Запустите установщик. Если появится SmartScreen, нажмите
«Подробнее» → «Выполнить в любом случае».

Обоим участникам нужна одна и та же версия приложения. Когда выходит новая, приложение
показывает плашку **Update available** — обновление ставится одной кнопкой.

## Как смотреть вместе

1. Каждый открывает в приложении свою копию **одного и того же** фильма: кнопка
   **Open**, **Cmd/Ctrl + O** или перетаскивание файла в окно (`.mp4`, `.mkv`, `.mov`).
   Длительность копий должна совпадать с точностью до 1,5 секунды.
2. **Ведущий** нажимает **Watch Together**, выбирает режим управления и нажимает
   **Create Room**:
   - **Free Control** — управлять воспроизведением могут все;
   - **Host Only** — только ведущий.
3. Ведущий нажимает **Copy Connection Code** и отправляет код `WT2-…` (или ссылку
   `watchtogether://…`) гостям. Адрес и порт вводить не нужно — они уже в коде.
4. **Гость** нажимает **Join Room**, вставляет код и ещё раз нажимает **Join Room**.

Озвучку отдельным файлом добавляют через меню аудиодорожек (значок наушников) →
**Add External Audio…** или перетаскиванием файла в окно; субтитры — так же, через меню
субтитров. Это видно только вам и на синхронизацию не влияет.

Комната существует, пока она открыта у ведущего. Если связь пропадает, воспроизведение
у гостя ставится на паузу. Подробнее — в [инструкции](docs/SELF_HOSTED_SYNC.md).

## Горячие клавиши

| Клавиша | Действие |
|:---|:---|
| **Пробел** | Пуск / пауза |
| **← / →** | Назад / вперёд на 5 с |
| **Shift + ← / →** | Назад / вперёд на 30 с |
| **↑ / ↓** | Громкость ±5 % (только у себя) |
| **M** | Выключить звук (только у себя) |
| **F** или двойной клик | Полный экран |
| **Esc** | Выйти из полного экрана |
| **Cmd / Ctrl + O** | Открыть файл |
| **I** | Статистика: кадры/с, потерянные кадры, разрешение, декодер |

## Если не подключается

Утилита [`wt-nettest`](tools/nettest/README.md) проверяет связь между двумя
компьютерами без приложения и сохраняет лог. Скачать:
[macOS](https://github.com/dmsznv-dot/WatchTogether/releases/download/nettest/wt-nettest-macos.zip) ·
[Windows](https://github.com/dmsznv-dot/WatchTogether/releases/download/nettest/wt-nettest-windows.exe).

## Как это устроено

- **Сеть:** [iroh](https://docs.rs/iroh) — зашифрованный QUIC с пробивкой NAT.
  Если прямое соединение невозможно, трафик идёт через relay по HTTPS. Состояние
  комнаты хранится у ведущего. По умолчанию используются публичные relay n0;
  свои задаются переменной окружения `WT_RELAY_URL`.
- **Синхронизация:** часы гостя подстраиваются под ведущего по измеренной задержке,
  расхождение позиции исправляется автоматически.
- **Видео:** [libmpv](https://mpv.io) с аппаратным декодированием. На macOS кадр
  выводится через OpenGL, на Windows — через общую память WebView2 и WebGL.

## Сборка из исходников

Нужны [Rust](https://rustup.rs) 1.91+, [Node.js](https://nodejs.org) 20+ и Tauri CLI:

```bash
cargo install tauri-cli --version "^2.0.0" --locked
```

**macOS**

```bash
brew install mpv
cd apps/desktop && npm install
cargo tauri dev                               # запуск в режиме разработки
cargo tauri build --bundles app --no-sign     # сборка .app
cd ../.. && python3 scripts/bundle_mpv_macos.py "target/release/bundle/macos/Watch Together.app"
```

`bundle_mpv_macos.py` кладёт libmpv и все её библиотеки внутрь приложения, чтобы
оно работало без Homebrew. DMG создаётся из готового `.app` командой `hdiutil create`.

**Windows** (нужны Visual Studio C++ Build Tools)

```powershell
python scripts/setup_mpv_windows.py
cd apps\desktop; npm install
cargo tauri dev
cargo tauri build --bundles msi,nsis --config src-tauri/tauri.windows.conf.json
```

**Тесты**

```bash
cargo test --workspace          # Rust
cd apps/desktop && npm test     # интерфейс
```

Подпись сборок для Apple и Windows и ключ обновлений — в [docs/SIGNING.md](docs/SIGNING.md).

Тесты с настоящим libmpv и сетью помечены `#[ignore]` и запускаются с `--ignored`
(см. [build.yml](.github/workflows/build.yml)). Сборки для обеих систем и релизы
собирает GitHub Actions при каждом пуше в `main`.

## Структура

```
apps/desktop/          приложение: интерфейс (React, TypeScript) и Tauri (Rust)
crates/network/        соединение через iroh и логика комнаты на стороне участников
crates/room-server/    комната: участники, права, состояние воспроизведения
crates/sync-engine/    синхронизация часов и позиции
crates/media-engine/   плеер на libmpv, дорожки, субтитры, вывод видео
crates/shared-protocol/  формат сообщений между ведущим и гостями
tools/nettest/         утилита проверки соединения
scripts/               упаковка libmpv, тестовые видео, проверки сборок
docs/                  инструкция и примечания к релизу
```

## Лицензия

MIT или Apache-2.0, на выбор.
