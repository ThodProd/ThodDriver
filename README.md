# ThodDriver

Встроенная программа ThodHUB для поиска и установки обновлений драйверов из Windows Update.

Программа написана на Rust. Интерфейс работает внутри ThodHUB, а `thod-driver.exe` — отдельный JSON-RPC worker. Установка запускает одноразовый UAC-помощник только после явного действия пользователя.

Данные программы хранятся только в `THODHUB_DATA_DIR`: резервные копии — в `backup`, временные состояния задач — в `runtime`.

## Локальная сборка

Нужен Rust stable с MSVC target. Выполните `cargo build --release`. Для выпуска используется GitHub Actions: она собирает оба EXE, формирует ZIP с `manifest.json` в корне, проверяет descriptor и публикует `hub-v0.1.0`.
