# Подпись сборок и обновления

CI (`.github/workflows/build.yml`) подписывает сборки сам, как только в репозитории
появляются нужные секреты: **Settings → Secrets and variables → Actions → New repository
secret**. Пока секретов нет, шаги подписи пропускаются и сборка выходит как раньше.

## Обновления в приложении — уже настроено

Приложение раз в несколько часов проверяет
`https://github.com/dmsznv-dot/WatchTogether/releases/download/updates/latest.json`
и показывает плашку «Update available». Каждая сборка из `main` получает версию
`0.2.<номер запуска CI>` и публикуется в релиз `updates`.

Файлы обновления подписываются ключом (это не подпись Apple/Microsoft, а защита от
подмены файла). Секреты уже добавлены:

| Секрет | Что это |
|:---|:---|
| `TAURI_SIGNING_PRIVATE_KEY` | закрытый ключ подписи обновлений |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | его пароль |

Копия ключа: `~/.tauri/watch-together-updater.key` и `.password` на Mac, где ключ был
создан. **Сохраните их в надёжном месте.** Если ключ потерять, установленные версии
перестанут принимать обновления — придётся один раз переустановить приложение вручную
с новым открытым ключом в `tauri.conf.json` (`plugins.updater.pubkey`).

## macOS: Developer ID и нотаризация

Убирает предупреждение «не удаётся проверить разработчика».

1. Вступить в [Apple Developer Program](https://developer.apple.com/programs/) ($99 в год).
2. Создать сертификат **Developer ID Application** (Xcode → Settings → Accounts →
   Manage Certificates, или developer.apple.com → Certificates), экспортировать его из
   «Связки ключей» в `.p12` с паролем.
3. Создать ключ App Store Connect API (App Store Connect → Users and Access →
   Integrations → Keys, роль Developer) и скачать `AuthKey_XXXX.p8`.
4. Добавить секреты:

| Секрет | Значение |
|:---|:---|
| `APPLE_CERTIFICATE` | `base64 -i DeveloperID.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | пароль от `.p12` |
| `APPLE_API_KEY` | `base64 -i AuthKey_XXXX.p8 \| pbcopy` |
| `APPLE_API_KEY_ID` | Key ID ключа (10 символов) |
| `APPLE_API_ISSUER` | Issuer ID со страницы ключей |

CI подпишет каждую библиотеку и приложение (hardened runtime, права —
`apps/desktop/src-tauri/Entitlements.developer-id.plist`), отправит его на нотаризацию
и прикрепит ответ Apple к приложению. Без `APPLE_API_*` будет только подпись, без
нотаризации — Gatekeeper всё равно предупредит.

## Windows: подпись кода

Убирает «Неизвестный издатель». SmartScreen перестаёт предупреждать, когда у
сертификата накопится репутация (для EV-сертификатов — сразу).

1. Купить сертификат подписи кода (OV или EV) у центра сертификации — например,
   Sectigo, DigiCert, SSL.com. Нужен файл `.pfx` с закрытым ключом.
   EV-сертификаты на токене в CI так не подключаются — для них нужен облачный
   вариант подписи у продавца.
2. Добавить секреты:

| Секрет | Значение |
|:---|:---|
| `WINDOWS_CERTIFICATE` | `base64 -i codesign.pfx \| pbcopy` |
| `WINDOWS_CERTIFICATE_PASSWORD` | пароль от `.pfx` |

CI импортирует сертификат и подпишет приложение и установщики (SHA-256, метка
времени DigiCert).
