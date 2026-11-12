# Running Rivet from another firmware

For people who build printer firmware (Z-Mod, Forge-X, a custom image) and want
Rivet as one of the screens their users can pick, the way they offer GuppyScreen
or HelixScreen. Rivet's own installer (`tools/install.sh`) is for Klipper Mod
only and refuses anywhere else; on another system the firmware starts Rivet
itself, with what is below.

## The program

One static binary, `rivet`, for `armv7-unknown-linux-musleabihf` (built with
`cargo printer`). No shared libraries, no fonts or files of its own: fonts,
icons and translations are inside. About 14 MB on disk; on an AD5M Pro it uses
10–12 MB of RAM idle, about 17 MB at most (the Files grid with previews), and
under 1 % CPU idle.

## What it needs

| What | Default | Change it with |
|---|---|---|
| Framebuffer | `/dev/fb0`, 32 bpp (XRGB/XBGR) or RGB565; panning for two pages is used when there | `RIVET_FB=/dev/fbN` |
| Touch | the first evdev device with absolute X/Y, preferring tslib's `ts_uinput` | `RIVET_TOUCH=/dev/input/eventN` |
| Touch axes | as the device reports them, scaled to the screen | `RIVET_TOUCH_SWAP_XY=1`, `RIVET_TOUCH_INVERT_X=1`, `RIVET_TOUCH_INVERT_Y=1` |
| Rotation | none | `RIVET_ROTATION=90` (`0`, `90`, `180`, `270`) |
| Moonraker | `ws://127.0.0.1:7125/websocket` | `--moonraker URL`, `RIVET_MOONRAKER`, or `moonraker` in rivet.toml |
| Language | `language` in rivet.toml, else `LANG`, else English (`en`, `ru`) | `--lang ru`, `RIVET_LANG=ru` |

Only one program may draw on the framebuffer: stop the screen Rivet replaces
before starting it, and start that screen again when switching back.

Optional, used when present:

- **Backlight**: `/sys/class/backlight/*`, else the AD5M's `/usr/bin/backlight`
  (screen sleep and the brightness setting). Without either the screen never sleeps.
- **Wi-Fi**: `wpa_supplicant`'s control socket in `/var/run/wpa_supplicant` or
  `/run/wpa_supplicant` (scan and join from Settings › Network); `update_config=1`
  lets a joined network be saved. Rivet must run as root for it.
- **Resonance CSVs** in `/tmp` (Klipper writes them there) for the vibration graphs.

## Starting it

```sh
rivet --config /path/to/rivet.toml --log-file /path/to/rivet.log
```

- `--config`: the settings file; without it Rivet looks for `rivet.toml` in
  `/root/printer_data/config`, then `/home/pi/printer_data/config`, then
  `/data/printer_data/config`, then the working directory. A missing file is fine.
- `--log-file`: write the log there (rotated, 512 KB × 3) instead of stderr;
  `log_level` in rivet.toml or `RUST_LOG` sets the level.
- Keep it running: restart it if it exits (Klipper Mod's `S80rivet` runs a small
  supervisor loop with a back-off).

## What it writes

| Where | What |
|---|---|
| The config folder above | `rivet-touch.json` after a touch calibration (rivet.toml itself only ever read) |
| The log file | as given |
| Moonraker's database, namespace `rivet` | Rivet's settings (theme, brightness, sleep, lock, heat presets, …) |
| `/tmp` | a `rivet-wpa-<pid>` socket while talking to wpa_supplicant |

Removing Rivet completely means deleting the binary, those files, the log, and
the keys of the `rivet` namespace (Moonraker has no call to drop a namespace).

## Macros it uses when the printer has them

Rivet works with any Klipper; it shows what the printer's config has. On an AD5M
it also uses Klipper Mod's macros where they exist (`LOAD_FILAMENT`,
`UNLOAD_FILAMENT`, `SET_PAUSE_AT_LAYER`, `SET_PAUSE_NEXT_LAYER`,
`LOAD_CELL_TARE`, `AUTO_SHUTDOWN`, `REBOOT_STOCK_SYSTEM`, `M300`), and hides
the rows when they are not there.
