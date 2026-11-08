# Reference UIs: menus and value controls

Source-level look (Nov 2026) at KlipperScreen, GuppyScreen and HelixScreen, plus
the KlipperMod AD5M configs (`printer.base.cfg`, `macros*.cfg`, `guppyconfig.json`).
Goal: decide which menu items Rivet needs and how values are adjusted.

## How the others adjust values

| Value | KlipperScreen | GuppyScreen | HelixScreen | Rivet |
|---|---|---|---|---|
| Fan | horizontal slider 0–100, sent on release | slider per fan + Off / Max | 270° arc dial (Bambu-style) + on/off switch | vertical bar, drag in 5% steps, buttons 100 / +10 / −10 / Off |
| LED | RGBW pickers + named presets | slider | presets 10/25/50/75/100 + On/Off/Toggle, white tones | same bar as the fan |
| Speed / flow (in print) | ±buttons | ±buttons | −10 −5 +5 +10 (speed), −5 −1 +1 +5 (flow), "reset to 100%" | named speed modes (kept) |
| Z-offset | step selector + ± | step selector + ± | step chips 0.05/0.025/0.01/0.005 + ± | ± with the step built into the button |
| Screen brightness | — | slider | slider | horizontal bar |

Common rules taken:
- Drag for coarse, buttons for exact. Every slider has an Off and a Max shortcut.
- Send G-code on release, not on every drag tick (KlipperScreen and Guppy do this).
- Modes only where the hardware has real modes: AD5M Pro air circulation
  (`AIR_CIRCULATION_INTERNAL / _EXTERNAL / _STOP`).

## What the AD5M under KlipperMod actually has

- Fans: part fan `[fan]`, `fan_generic chamber_fan`, and on the Pro `internal_fan` / `external_fan`
  driven by the air-circulation macros.
- Light: `[led chamber_led]`, white channel only, software PWM, so 0–100% works.
- Built-in ADXL345 (`[resonance_tester]`): input shaper and belt comparison need no extra hardware.
- Load-cell probe: bed mesh 5×5 via `AUTO_BED_LEVEL` (heats, tares, wipes, probes);
  `LOAD_CELL_TARE`; Z-offset is applied to the probe (`Z_OFFSET_APPLY_PROBE`), not with paper.
- Screen backlight: `LCD_ON BACKLIGHT=<percent>` / `LCD_OFF`.
- Power: `SHUTDOWN`, `AUTO_SHUTDOWN` (off after cooldown), `REBOOT_STOCK_SYSTEM` (dual boot).
- Sound: `M300`, `PLAY_MIDI`.

## Menu: what the others have vs. what Rivet keeps

Others list 15–25 items (Move, Temperature, Extrude, Fan, LEDs, Macros, Pins, Power, Camera,
Console, Updater, Input Shaper, Limits, Network, Spoolman, System, Notifications, …).
Rivet keeps 4 rail items and groups the rest:

- Управление: nozzle, bed, speed, fan, motion, Z-offset, filament, light.
- Настройки (8): Wi-Fi · Экран (brightness, sleep, ambient while printing, lock) · Пластики
  (preset temperatures) · Калибровка · Звук · Язык · Макросы и консоль · Система
  (restart, shutdown, off after print, boot stock firmware, about).
- Калибровка (6): bed mesh, Z-offset, input shaper, belts, PID, load-cell tare.
- Сообщения.

Dropped on purpose: Pins, Limits (expert, rarely touched), Camera (later), Updater
(KlipperMod updates via USB image), multi-printer switching.
Spoolman shows up inside the filament flow only when Moonraker reports it.
