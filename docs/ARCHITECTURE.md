# Rivet — архитектура

Техническое устройство проекта. Что и когда делаем — в [ROADMAP.md](../ROADMAP.md), почему так — в [DECISIONS.md](DECISIONS.md).

## Платформы

| Приоритет | Платформа | Железо | Примечание |
|---|---|---|---|
| dev | macOS / Linux desktop | — | окно 800×480 через winit, Moonraker по сети или мок |
| P1 | AD5M / AD5M Pro + KlipperMod | Allwinner T113-S3, 2×Cortex-A7, 128 МБ, 4.3″ 800×480 | Buildroot, init.d (нет systemd), chroot, root SSH |
| P2 | AD5M / Pro + Forge-X | то же | другой rootfs/init, своя обвязка подсветки и звука |
| P3 | Remote-экран на Pi / мини-ПК | aarch64 / x86_64 | тот же бинарь, другие флаги |
| P4 | AD5X | MIPS32, IFS 4 цвета | Rust для MIPS — tier 3: nightly + `build-std`. KlipperMod AD5X не поддерживает |

Rust target для P1/P2: `armv7-unknown-linux-musleabihf`, статический бинарь без зависимостей от glibc прошивки (D-010). Сборка: `cargo printer`.

## Стек

- **Rust stable** (MSRV фиксируем в `rust-toolchain.toml`).
- **Slint 1.18**:
  - на устройстве: свой платформенный слой `crates/fbdev-platform` (D-009): `MinimalSoftwareWindow` рисует только изменённые области прямо в mmap `/dev/fb0` (XRGB8888, XBGR8888, RGB565), тач читается из evdev (по умолчанию tslib `ts_uinput`), цикл событий на `poll` + pipe для `invoke_from_event_loop`. Настройка через `RIVET_FB`, `RIVET_TOUCH`, `RIVET_ROTATION`, `RIVET_TOUCH_SWAP_XY/INVERT_X/INVERT_Y`;
  - на десктопе: backend `winit` + тот же software renderer;
  - шрифты Inter вшиты в бинарь (`import` в `.slint` + `EmbedFiles`), fontconfig грузится через dlopen и на устройстве не нужен.
  - Лицензия Slint: GPLv3 (подходит, проект GPLv3).
- **tokio** (`current_thread`) + **tokio-tungstenite** — WebSocket JSON-RPC Moonraker.
- **serde / serde_json** — модели.
- HTTP для файлов/миниатюр: **ureq** или минимальный hyper-клиент (без reqwest, он тяжёлый).
- **i18n**: `@tr()` в `.slint` + gettext `.po`, вшитые в бинарь (`slint-build` bundled translations). Языки: RU, EN.
- Логи: `tracing` с ротацией, по умолчанию уровень `info` (флешку не убиваем).
- Конфиг: `rivet.toml` в `/root/printer_data/config/`, чтобы его можно было править из Mainsail/Fluidd.

## Архитектура

```
rivet/
├─ crates/
│  ├─ moonraker/   # JSON-RPC клиент: connect, reconnect, request/response, notifications
│  ├─ model/       # PrinterState: слияние status-дельт, klippy state, типы объектов
│  ├─ platform/    # подсветка, звук, WiFi (iwd D-Bus), выключение — за трейтом, с моками для десктопа
│  └─ app/         # бинарь: Slint UI, мост UI ⇄ ядро, экраны
├─ ui/             # .slint файлы: тема, компоненты, экраны
├─ i18n/           # ru.po, en.po
├─ tools/mock-moonraker/   # фейковый Moonraker для разработки без принтера
└─ packaging/      # init.d скрипт, пакет Buildroot, инсталлятор для KlipperMod
```

**Потоки:**

1. **UI-поток** — event loop Slint. Только рисует и шлёт команды.
2. **Сетевой поток** — tokio runtime: WebSocket, подписки, HTTP.

**Связь:**

- UI → ядро: `mpsc::Sender<Command>` (`StartPrint(path)`, `SetTemp{heater,t}`, `Gcode(String)`, …).
- Ядро → UI: батчинг дельт состояния не чаще 10 Гц → `slint::invoke_from_event_loop` → обновление глобальных свойств/моделей Slint.

**Моделирование:** Moonraker шлёт частичные апдейты `notify_status_update`. `model` держит полное дерево (`serde_json::Value` на сыром уровне + типизированные срезы для UI) и мержит дельты.

**Состояния соединения:** `Disconnected → Connecting → KlippyStartup → Ready / Shutdown / Error`. Для каждого свой экран-оверлей с кнопками `FIRMWARE_RESTART` / `RESTART` и текстом ошибки.

## Используемое API Moonraker

| Задача | Метод |
|---|---|
| Идентификация | `server.connection.identify`, `server.info`, `printer.info` |
| Объекты | `printer.objects.list`, `printer.objects.subscribe`, `notify_status_update` |
| Klippy события | `notify_klippy_ready`, `notify_klippy_shutdown`, `notify_klippy_disconnected` |
| G-code | `printer.gcode.script`, `notify_gcode_response` (консоль, ответы калибровок) |
| Печать | `printer.print.start / pause / resume / cancel` |
| Файлы | `server.files.list`, `server.files.metadata`, миниатюры по HTTP `/server/files/gcodes/...` |
| История | `server.history.list` |
| Exclude object | объект `exclude_object` в подписке, `EXCLUDE_OBJECT NAME=...` |
| Spoolman | `server.spoolman.get_spool_id`, `post_spool_id`, `server.spoolman.proxy` |
| Система | `machine.system_info`, `machine.services.restart`, `machine.shutdown/reboot` |
| Power devices | `machine.device_power.*` (если есть) |
| Камера | MJPEG ustreamer `:8080` (позже, если хватит CPU) |

## Экраны (черновик, уточняется в D1)

Навигация: левая вертикальная панель с 4–5 крупными иконками + верхняя строка статуса (температуры, WiFi, время).

1. **Главный / Печать**
   - простой: миниатюра последнего файла, температуры сопла и стола, кнопка «Печать»;
   - во время печати: миниатюра, прогресс-кольцо, слой X/Y, ETA, пауза/стоп, кнопка «Тюнинг».
2. **Файлы**: сетка карточек с миниатюрами → карточка файла (время, филамент, слои) → «Печать».
3. **Управление**: движение XYZ (шаги 0.1/1/10/50), Home, температуры с пресетами PLA/PETG/ABS/TPU, экструзия/ретракт, вентиляторы, свет.
4. **Тюнинг в печати** (оверлей): Z-offset (babystep), скорость %, поток %, вентиляторы, Pressure Advance, макросы.
5. **Калибровки**:
   - Bed mesh: запуск + 2D heatmap (3D позже, по бюджету);
   - PID: сопло/стол, живой график;
   - Input shaper: запуск `SHAPER_CALIBRATE`, результат текстом. Графики PSD требуют CSV из `/tmp` на устройстве, это фаза 2 и только локально;
   - Z-offset wizard.
6. **Spoolman**: активная катушка в статус-строке, список, выбор, остаток.
7. **Exclude object**: карта стола с контурами объектов (`exclude_object.objects[].polygon`), тап → подтверждение.
8. **Настройки**: язык, яркость (KlipperMod: `backlight N`), screensaver, звук, WiFi (iwd), сеть/IP, о системе.
9. **Система**: restart Klipper / Firmware restart / Moonraker, перезагрузка, выключение, логи.
10. **Оверлеи**: «Klipper не готов», «Нет соединения», модальные подтверждения, тосты ответов G-code.

## Разработка

- **На Mac:** `cargo run -p rivet -- --moonraker ws://<ip>:7125/websocket`, окно 800×480 (флаг `--scale` для HiDPI). Можно подключаться к реальному принтеру по сети: на него ничего не ставится.
- Без принтера: `cargo run -p mock-moonraker` + проигрывание записанных сессий.
- Тесты: unit на мерж состояний и парсинг; интеграционные против мока; снапшоты экранов (рендер Slint software renderer в PNG) для визуальной регрессии.
- CI (GitHub Actions): fmt, clippy, тесты, кросс-сборка armv7, артефакт релиза.
- Лицензия: GPL-3.0-or-later.

