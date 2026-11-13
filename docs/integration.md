# Rivet as one of your firmware's screens

For people who build printer firmware — Z-Mod, Forge-X, a custom image — and want to offer
Rivet next to GuppyScreen or HelixScreen. [Русская версия](integration.ru.md).

Rivet's own installers (USB stick, `tools/install.sh`) are for Klipper Mod only and refuse to
run anywhere else. On your firmware, your firmware installs, starts and stops Rivet; this page is
what it needs to know. Questions: [open an issue](https://github.com/lDOCI/rivet/issues).

## Short version

1. Let Moonraker's update manager fetch `rivet-ad5m.zip` from the latest GitHub release of
   `lDOCI/rivet` into a folder of your choice (`type: web`).
2. Start and stop the screen with the `S80rivet` script that comes inside the zip.
3. Stop your other screens before starting it, and never run two at once.

## The release

Every release of [lDOCI/rivet](https://github.com/lDOCI/rivet/releases) has, **first**, an asset
named exactly `rivet-ad5m.zip`. The name does not change between releases. Releases are never
marked as pre-releases, so `channel: stable` (GitHub's "latest") sees every one of them.

| File in the zip | What |
|---|---|
| `rivet` | the program: static, `armv7-unknown-linux-musleabihf`, no shared libraries, fonts and translations inside |
| `rivet.sha256` | its checksum, as `sha256sum` prints it |
| `release_info.json` | `{"project_name": "rivet", "project_owner": "lDOCI", "version": "v0.1.0-alpha", "asset_name": "rivet-ad5m.zip"}` |
| `S80rivet` | start/stop script with a watchdog, works from whatever folder it is in |

The other assets of a release (`rivet-<version>-usb.zip`, `…-usb-remove.zip`,
`…-installer.tar.gz`) are Klipper Mod installers; ignore them.

## Update manager

```ini
[update_manager rivet]
type: web
channel: stable
repo: lDOCI/rivet
path: /srv/rivet
persistent_files:
    rivet.good
    rolled-back
```

`path` is any folder you like. Moonraker needs a `release_info.json` there before the first
install; seed it with an old version so that the first refresh offers the real one:

```json
{"project_name":"rivet","project_owner":"lDOCI","version":"v0.0.1","asset_name":"rivet-ad5m.zip"}
```

`persistent_files` keeps the watchdog's state across updates: `rivet.good` is the last version
that worked, `rolled-back` marks that a new one did not start.

After Moonraker installs an update, the running Rivet sees `notify_update_response` and exits with
code 0; `S80rivet` then starts the new one. Nothing for you to restart.

## Starting and stopping

```sh
/srv/rivet/S80rivet start     # returns at once; the watchdog runs in the background
/srv/rivet/S80rivet stop      # returns when the screen is free
/srv/rivet/S80rivet restart
```

Link it into `/etc/init.d` if your firmware starts screens from there, or call it from your own
screen switcher.

What the watchdog does:

- starts Rivet, and starts it again if it exits (1 s, then up to 60 s after repeated quick crashes;
  an exit with code 0 is a restart on purpose and is not counted);
- before every start checks the binary against `rivet.sha256`;
- after 30 s of running keeps a copy as `rivet.good`; a new binary that exits twice within 30 s,
  or does not match its checksum, is replaced by `rivet.good` and `rolled-back` is written;
- on stop, points the framebuffer back at its first page (see below).

`S80rivet` picks its folders:

| | Z-Mod (`/opt/config/mod_data` exists) | Klipper Mod (`/root/printer_data/config`) | otherwise |
|---|---|---|---|
| program | the folder `S80rivet` is in, if it holds `rivet`; else `/opt/rivet`; `RIVET_DIR` overrides | same | same |
| `rivet.toml`, `rivet-touch.json` | `/opt/config/mod_data` | `/root/printer_data/config` | the program's folder |
| logs | `/opt/config/mod_data/log/rivet.log`, `rivet-console.log` | `/root/printer_data/logs/…` | the program's folder |
| language | Z-Mod's `mod_data/lang.cfg`, unless `rivet.toml` sets one | system `LANG` | system `LANG` |

The PID files are `/run/rivet.pid` (watchdog) and `/run/rivet.child.pid` (Rivet).

## What Rivet needs from the system

| | Default | Override |
|---|---|---|
| Framebuffer | `/dev/fb0`, 32 bpp (XRGB/XBGR) or RGB565, 800×480; two pages are used when the driver can pan | `RIVET_FB=/dev/fbN` |
| Touch | first evdev device with absolute X/Y, `ts_uinput` (tslib) preferred | `RIVET_TOUCH=/dev/input/eventN` |
| Raw touch panel | when the device is not tslib's, its values go through tslib's linear calibration from `$TSLIB_CALIBFILE`, `/etc/pointercal` or `/tmp/pointercal` (Z-Mod's copy) | `RIVET_TOUCH_SWAP_XY`, `…_INVERT_X`, `…_INVERT_Y` = `1` |
| Rotation | none | `RIVET_ROTATION=90` (`0`, `90`, `180`, `270`) |
| Backlight | `/sys/class/backlight/*`, else Klipper Mod's `/usr/bin/backlight`, else the Allwinner `/dev/disp` ioctls (Z-Mod) | — |
| Moonraker | `ws://127.0.0.1:7125/websocket` | `--moonraker URL`, `RIVET_MOONRAKER`, `moonraker` in rivet.toml |
| Config folder | see the table above | `RIVET_CONFIG_DIR`, `--config FILE` |
| Wi-Fi (optional) | `wpa_supplicant`'s control socket in `/var/run/wpa_supplicant` or `/run/wpa_supplicant`; needs root | — |

Rivet runs as root on the AD5M (framebuffer, input, backlight and Wi-Fi need it).

**Framebuffer pages.** Rivet draws into one page and shows it by panning, alternately. A screen
started after it that keeps the offset it finds at start (GuppyScreen's LVGL driver does) would
draw on the page that is not shown. `S80rivet stop` resets it; if you stop Rivet another way,
do it yourself: `echo 0,0 > /sys/class/graphics/fb0/pan`.

## What Rivet writes

| Where | What |
|---|---|
| config folder | `rivet-touch.json` after a touch calibration (`rivet.toml` is only read) |
| log folder | `rivet.log` (rotated 512 KB × 3), `rivet-console.log` (cut at 256 KB) |
| program folder | `rivet.good`, `rolled-back` (watchdog) |
| Moonraker's database, namespace `rivet` | settings changed on the screen |
| `/tmp` | a `rivet-wpa-<pid>` socket while talking to `wpa_supplicant` |

To remove Rivet completely: delete the program folder, those files, the update manager section,
and the keys of the `rivet` database namespace.

## Klipper macros it uses when present

Rivet works with any Klipper config and shows what the printer has. On an AD5M it also uses these
macros when they exist, and hides the rows when they do not: `LOAD_FILAMENT`, `UNLOAD_FILAMENT`,
`SET_PAUSE_AT_LAYER`, `SET_PAUSE_NEXT_LAYER`, `LOAD_CELL_TARE`, `AUTO_SHUTDOWN`,
`REBOOT_STOCK_SYSTEM`, `M300`.

## Z-Mod

What Z-Mod does for HelixScreen, done for Rivet:

- `.shell/plugman/plugins.json`:

  ```json
  "rivet": {"type": "web", "repo": "lDOCI/rivet", "path": "/srv/rivet",
            "persistent_files": ["rivet.good", "rolled-back"],
            "var": "rivet", "var_swap": "guppy", "var_off": ["helix"],
            "check_files": ["rivet", "S80rivet"], "printers": ["FF5M"]}
  ```

- `start.sh`: seed `/srv/rivet/release_info.json` as above when it is missing;
- the screen switcher (`S80guppyscreen`, `zdisplay.sh`, `stop.sh`): when `rivet = 1`, start and
  stop `/srv/rivet/S80rivet` where it does `S80helixscreen`, and stop Rivet (`S80rivet stop`)
  where it kills the other screens.

Proposed to Z-Mod in [ghzserg/z_ff5m#16](https://github.com/ghzserg/z_ff5m/pull/16) (plugins, `start.sh`,
`S80rivet`, `S80guppyscreen`, `zdisplay.sh`), [ghzserg/base#19](https://github.com/ghzserg/base/pull/19)
(`DISPLAY_OFF RIVET=1`) and [ghzserg/zmod#799](https://github.com/ghzserg/zmod/pull/799) (documentation).
`var_off` is a key those pull requests add to plugman: variables set to 0 when the plugin is turned on.

## Forge-X

Not supported yet. Forge-X has its own chroot and screen handling; it would need the same three
things (update manager entry, `S80rivet`, stopping the other screen). Untested.
