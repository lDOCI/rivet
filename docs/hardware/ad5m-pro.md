# AD5M Pro on KlipperMod — network check

Read-only check of a real printer from the Mac, 2026-11-08. No commands were sent, nothing was
installed. Raw responses are in `crates/model/tests/fixtures/ad5m-pro/` and are used by the model
tests. Probe: `cargo run -p moonraker --example probe -- ws://<printer>:7125/websocket`.

## Versions

| | |
|---|---|
| Klipper | v0.13.0-285-gfc2aaae69-ffadm5-Buildroot |
| Moonraker | v0.9.3-Buildroot, API 1.5.0 |
| Host | 2-core ARMv7, hostname `AD5M` |
| MCU | stm32f103xe, 144 MHz |

## Rivet's client against it

- Connects, identifies as a display, subscribes: works first time.
- About 4 `notify_status_update` per second while idle, plus `notify_proc_stat_update` every second
  (ignored today; it carries CPU and memory for the System screen).
- Model snapshot matches the printer: temperatures, `standby`, `Printer is ready`.
- Every object in `model::subscription()` exists on the printer (test).

## Memory (the budget that matters)

| | |
|---|---|
| Total RAM visible to Linux | 110 MB |
| Available with Klipper, Moonraker and the stock screen running | 38 MB |
| Moonraker RSS | 43 MB |

Rivet's target of ≤ 20 MB RSS fits, but with little room: no in-memory caches without a byte limit,
and calibration data is read from disk, not held.

## What the printer has (and what the prototype assumed)

| Thing | On this printer | Prototype before | Now |
|---|---|---|---|
| Axis limits X/Y | −112 … 112 | −110 … 110 | fixed |
| Light | `led chamber_led`, white only, 80 % at start | 70 % | fixed |
| Input shaper | X: MZV 56.4 Hz, Y: ZV 46.0 Hz | MZV 54.2 / EI 41.8 | fixed |
| Pressure advance | 0.035 | 0.040 | fixed |
| Bed mesh | profile `auto` saved, none loaded | «default», 0.18 mm | fixed |
| Screws tilt | `screws_tilt_adjust`, 4 screws named, CW-M4 | assumed absent | screen applies |
| Air quality | `flashforge_tvoc` (tvoc, co2, hcho; status `unknown`) | `TVOC_Level` | fixed |
| Load cell | `flashforge_loadcell.force_g` | temperature sensor | fixed |
| MCU / host temperature | not configured | shown | marked as absent |
| update_manager | present: Mainsail, Fluidd; Klipper and Moonraker have no version info | "not present" | fixed |
| Webcams | none | — | camera empty state applies |
| Power devices | component not loaded | example rows | section hidden |
| Spoolman | not configured | — | hidden |
| Print history | 770 jobs, 999 h, 8.1 km filament | 142 jobs | fixed |
| Layer info | `print_stats.info` is null unless the slicer calls `SET_PRINT_STATS_INFO` | always shown | M1: hide when null |
| Homed axes | empty after boot | — | Motion must show «not homed» |
| `skew_correction` | present | not in the feature map | added as a candidate for M3 |
| Extra macros | `BELTS_TEST`, `GUPPY_EXCITATE_AXIS_AT_FREQ` (from GuppyScreen) | — | belt check can use core `TEST_RESONANCES` |

## On the device (2026-11-12)

| What | Found | For Rivet |
|---|---|---|
| Display | `/dev/fb0`, 800×480, `Bgrx8888`, double-buffered by panning | `fbdev-platform` draws into RAM and flips pages |
| Touch | `ts_uinput` (event3), axes 0–799 × 0–479, no rotation | no calibration needed |
| Accelerometer | `adxl345` on the eboard, `resonance_tester` configured; not listed among Klipper's objects | detected from the config |
| Wi-Fi | `wpa_supplicant` on `wlan0`, control socket `/var/run/wpa_supplicant`, `update_config=1`; `udhcpc` per interface | scan and join through the socket, renew with SIGUSR1 |
| Backlight | `/usr/bin/backlight <percent>` (Guppy's init sets 0 % then 75 %) | brightness setting can use it |
| Moonraker `[machine]` | `provider: none` | `machine.shutdown` as KlipperScreen and HelixScreen do |
| Power | rear switch only; `gcode_button btn_power` runs `_SHUTDOWN_BUTTON_TRIGGER` | not verified what the switch does before the power drops |

### How KlipperMod picks the screen

It does not pick at run time: `/etc/init.d/rcS` starts every executable
`/etc/init.d/S??*` in order, and the screen is just `S80guppyscreen`
(`start-stop-daemon`, niceness 10, backlight set first). `/etc` is writable.
So installing Rivet means an `S80rivet` of the same shape and keeping
`S80guppyscreen` from starting (rename it, e.g. `_S80guppyscreen`, which the
`S??*` pattern skips); uninstalling restores it. `tools/swap-screen.sh` does the
same for one boot without touching `/etc`.
