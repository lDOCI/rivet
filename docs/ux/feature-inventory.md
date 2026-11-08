# Feature inventory and screen map

Every user-facing function found in the source of KlipperScreen (`panels/*.py`, menu configs),
GuppyScreen (`src/*_panel.cpp`) and HelixScreen (`ui_xml/*.xml`, `src/ui/*`), mapped to exactly one
home in Rivet. Collected Nov 2026 from the repos' default branches.

Rules for this map:

- **Everything the three have is in.** Rivet targets generic Klipper, not only the AD5M.
- **One home per function.** A function may have a shortcut elsewhere (e.g. print tuning reuses
  the Z-offset widget), but it is the same widget, not a second copy with different behavior.
- **Capability-gated.** A screen or row appears only if Klipper/Moonraker reports the hardware or
  macro (Helix does this, e.g. chamber air only when `AIR_CIRCULATION_*` macros exist).
- **Every action takes its parameters.** Extrude length and speed, jog step and speed, PID target,
  mesh profile name, etc. are adjustable on the screen, with sensible defaults remembered.
- **Live tiles.** A tile that shows a value (temperature, fan %, speed %) is live from Moonraker;
  a tile that opens a tool shows a static icon.

Legend: KS = KlipperScreen, G = GuppyScreen, H = HelixScreen. ◐ = only when the hardware/macro/plugin exists.

## Navigation

Rail, 5 items: **Главная · Управление · Инструменты · Настройки · Сообщения**.
Always available: E-stop, back, connection state.

## 1. Главная (idle) and printing

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Live temps on home (nozzle, bed, chamber) | ✓ | ✓ | ✓ | Главная, live tiles |
| Last / recent file, reprint | | | ✓ | Главная |
| Active spool / loaded material | ✓ | ✓ | ✓ | Главная tile → Экструдер |
| Light / power shortcut on home | | | ✓ | Главная (◐) |
| Job status: progress, elapsed, left, ETA, layer, Z, filament used/total, speed, flow, accel, PA | ✓ | ✓ | ✓ | Печать |
| Pause / resume / cancel, cancel escalation to E-stop | ✓ | ✓ | ✓ | Печать |
| Tune in print: speed %, flow %, Z babystep + save, PA, fans, temps | ✓ | ✓ | ✓ | Печать › Подстройка |
| Exclude object (map + list) | ✓ | | ✓ | Печать › Пропуск объектов |
| Pause at layer (next / chosen layer) | | | ✓ | Печать › меню |
| Camera, timelapse, light in print | ✓ | | ✓ | Печать › меню (◐) |
| Completion: result, stats, reprint | ✓ | | ✓ | Печать завершена |
| Filament runout: load / unload / purge / resume / cancel | | | ✓ | Закончился филамент |
| Screensaver with progress | | | ✓ | Заставка |
| Lock screen with PIN | ✓ | | ✓ | Блокировка |

## 2. Файлы

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Printer / USB storage, folders | ✓ | ✓ | ✓ | Файлы |
| Sort (name, date, size, time), search, recently printed | ✓ | | ✓ | Файлы |
| File detail: thumbnail, time, filament, slicer, layer height, nozzle Ø, last duration | ✓ | ✓ | ✓ | Файл |
| Pre-print checks (bed clear, temps, material) and start options | | | ✓ | Файл |
| Delete, rename, move, copy from USB | ✓ | | ✓ | Файл › меню |
| Job queue: add, pause, reorder, remove | | ✓ | ✓ | Файлы › Очередь (◐) |
| Print history list + statistics (success rate, time, filament by type) | | | ✓ | Инструменты › История |

## 3. Управление

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Every heater: target via arc, keypad, ±; cooldown all | ✓ | ✓ | ✓ | Нагрев (one screen, heater tabs) |
| Preheat presets per material (edit temps, preheat macro) | ✓ | | ✓ | Нагрев › Пресеты |
| Temperature graph | | | ✓ | Нагрев |
| PID / MPC from a heater | ✓ | | ✓ | Инструменты › Калибровки › Нагрев (shortcut from Нагрев) |
| Jog XY/Z with step selector, home all/XY/Z, motors off, park | ✓ | ✓ | ✓ | Движение |
| Go to coordinates, query endstops, invert axes, jog speeds | ✓ | | ✓ | Движение › Дополнительно |
| QGL / Z-tilt | | | ✓ | Движение (◐) |
| Extrude / retract with length + speed, temp | ✓ | ✓ | ✓ | Экструдер |
| Load / unload / purge wizard | ✓ | ✓ | ✓ | Экструдер › Заправка |
| Spoolman: spools, search, set active, edit weight | ✓ | ✓ | ✓ | Экструдер › Катушки (◐) |
| Fans: every controllable fan %, auto fans read-only | ✓ | ✓ | ✓ | Вентиляторы |
| Chamber air modes (AD5M Pro `AIR_CIRCULATION_*`, Snapmaker purifier) | | | ✓ | Вентиляторы (◐) |
| LEDs: on/off, brightness, presets, RGBW colour, white tone | ✓ | ✓ | ✓ | Свет |
| LED auto-state (colour follows printer state), on at startup | | | ✓ | Свет › Настройка |
| Speed % / flow % (outside print) | ✓ | ✓ | ✓ | Скорость и поток |
| Z-offset babystep + save | ✓ | ✓ | ✓ | Z-offset (also opened from Печать) |
| Power devices (Moonraker) | ✓ | ✓ | ✓ | Устройства (◐) |
| Output pins | ✓ | | | Устройства (◐) |
| Firmware retraction G10/G11 | ✓ | | ✓ | Экструдер › Ретракт (◐) |

## 4. Инструменты

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Bed mesh: calibrate, profiles (load/save/rename/delete/clear), view | ✓ | ✓ | ✓ | Калибровки › Карта стола |
| Screws adjust / tilt | ✓ | | ✓ | Калибровки › Винты стола (◐) |
| Z-offset / probe calibration (paper test, probe accuracy, offsets) | ✓ | | ✓ | Калибровки › Щуп и Z (◐) |
| Input shaper: X / Y / both, graphs, save | ✓ | ✓ | ✓ | Калибровки › Вибрации (◐ accelerometer) |
| Belt comparison | | ✓ | ✓ | Калибровки › Ремни (◐) |
| PID / MPC | ✓ | | ✓ | Калибровки › Нагрев |
| Pressure advance (auto / tower) | ✓ | ✓ | ✓ | Калибровки › Pressure advance |
| Load-cell tare | | | | Калибровки › Тензодатчик (◐, AD5M) |
| TMC autotune / metrics | | ✓ | | Калибровки › Драйверы (◐) |
| Machine limits (velocity, accel, SCV, cruise ratio) | ✓ | ✓ | ✓ | Лимиты |
| Macros (params, favourites, hidden) | ✓ | ✓ | ✓ | Макросы |
| Console | ✓ | ✓ | ✓ | Консоль |
| Camera | ✓ | | ✓ | Камера (◐) |
| Timelapse videos + settings | | | ✓ | Таймлапс (◐) |
| Maintenance intervals (filter, nozzle, rails) | | | ✓ | Обслуживание |
| Dry filament on the bed | | | ✓ | Сушка (◐ bed) |
| Sensors (runout on/off, temp sensors, humidity, load cell) | | | ✓ | Датчики |
| Save config pending | ✓ | | ✓ | banner on every screen |

## 5. Настройки

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Wi-Fi scan/connect/forget/hidden, Ethernet, IP, MAC, hostname | ✓ | ✓ | ✓ | Сеть |
| Brightness (`LCD_ON BACKLIGHT` on AD5M), sleep, sleep while printing, screensaver, rotation | ✓ | ✓ | ✓ | Экран |
| Touch calibration | | | ✓ | Экран |
| PIN, auto-lock | ✓ | | ✓ | Блокировка |
| Sound, volume, end-of-print melody (`M300`, `PLAY_MIDI` ◐) | | | ✓ | Звук |
| Language, timezone, 12/24 h | ✓ | | ✓ | Язык и время |
| Confirm E-stop, cancel escalation, confirm macros, alerts | ✓ | ✓ | ✓ | Безопасность |
| Multiple printers | ✓ | ✓ | ✓ | Принтеры |
| Updates (per component, recover) | ✓ | ✓ | ✓ | Обновления |
| Restart Klipper / firmware / host / Rivet, shutdown, reboot | ✓ | ✓ | ✓ | Система |
| Off after print, boot stock firmware (KlipperMod) | | | | Система (◐) |
| System info (CPU, memory, disk), log level, storage cleanup, factory reset | ✓ | ✓ | ✓ | Система |

## 6. Сообщения

| Function | KS | G | H | Rivet screen |
|---|---|---|---|---|
| Notification log, clear | ✓ | | ✓ | Сообщения |
| Klipper action prompts (`action:prompt_*`) | ✓ | ✓ | ✓ | overlay on any screen |
| Klipper error / shutdown, firmware restart | ✓ | ✓ | ✓ | Принтер остановлен |
| Connecting / offline, retry, restart host | ✓ | ✓ | ✓ | Нет связи |

## Not taken

- Appearance / themes (H): Rivet ships one theme.
- Plugins (H): Helix-specific runtime, not a Klipper feature.
- Hardware health (H): Helix's own config validation; Rivet shows missing hardware by hiding ◐ screens.

## Answers to review questions

- **Live or static tiles.** Control tiles for nozzle, bed, fan, speed, light show live values and a
  live mini-gauge; tiles that open tools (Движение, Экструдер) have static icons.
- **«Пластики» presets.** Not a settings item any more: they are the preheat presets KS and Helix
  have, and live inside Нагрев (one tap: PLA 210/60, …; editable there).
- **Z-offset.** One screen in Управление (live babystep + save). The calibration wizard (paper test
  or probe) is a button on that screen and in Калибровки › Щуп и Z, opening the same wizard.
  Values in the prototype are sample data; the app reads `gcode_move.homing_origin`.
