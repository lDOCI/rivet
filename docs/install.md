# Install on the printer

For a Flashforge Adventurer 5M / 5M Pro running [Klipper Mod](https://github.com/xblax/flashforge_ad5m_klipper_mod).
Klipper Mod only: on the stock firmware or ZMOD the installer refuses and changes nothing
(it checks `/etc/os-release` and Klipper Mod's chroot in `/mnt/data/.klipper_mod`).
Everything runs from your computer over ssh as `root`; nothing else is needed on the printer.

## Install or upgrade

```bash
cargo printer
tools/install.sh PRINTER_IP
```

What it does on the printer:

| Path | What |
|---|---|
| `/opt/rivet/rivet` | the binary |
| `/etc/init.d/S80rivet` | starts Rivet at boot, restarts it if it crashes |
| `/etc/init.d/_S80guppyscreen` | the replaced screen's init script, renamed so it no longer starts (`rcS` runs only `/etc/init.d/S??*`) |
| `/opt/rivet/replaced` | which screen scripts were disabled and whether they were running |
| `/root/printer_data/config/rivet.toml` | settings, written from `docs/rivet.toml.example` only if missing |
| `/root/printer_data/logs/rivet.log` | Rivet's log, rotated (512 KB × 3) |
| `/root/printer_data/logs/rivet-console.log` | crashes (panics) and restarts, one old copy kept |

The screen it replaces is whatever is there: the program holding `/dev/fb0` (not tslib's
`ts_uinput`) and the init script that starts it, or a GuppyScreen, KlipperScreen or HelixScreen
script that is installed but stopped. No screen (headless): nothing is disabled.
`tools/install.sh --dry-run PRINTER_IP` prints what it would disable and changes nothing.
If an unknown program holds the screen, it refuses unless `--force`.

Running it again replaces the binary and the init script; an existing `rivet.toml` is never
overwritten. Nothing else on the printer is changed, and the script prints each change.
It refuses while a print is running or paused (it asks Moonraker); `--force` installs anyway.

Files are copied with `ssh … 'cat > file'`: Klipper Mod has no `sftp-server`, so `scp` does not work.

## Crashes

`S80rivet` runs Rivet under a small supervisor loop. If Rivet exits, it is started again after 1 s;
each run shorter than 30 s doubles the wait (2, 4 … 60 s), so a crash loop does not load the CPU.
A run of 30 s or more resets the wait to 1 s.

On the printer:

```bash
/etc/init.d/S80rivet stop | start | restart
tail -f /root/printer_data/logs/rivet.log
```

Both logs are also visible in Fluidd/Mainsail next to `klipper.log`.

## Uninstall

```bash
tools/uninstall.sh PRINTER_IP
```

Leaves no trace of Rivet. It stops Rivet and deletes, printing each item:

- `/etc/init.d/S80rivet`, `/opt/rivet`;
- `rivet.toml` and `rivet-touch.json` (touch calibration) in `/root/printer_data/config`;
- `rivet.log*` and `rivet-console.log*` in `/root/printer_data/logs`;
- `/run/rivet*.pid`; in `/tmp` what `tools/swap-screen.sh` and Rivet left (`rivet`, `rivet.log`,
  `rivet.pid`, `rivet-wpa-*` sockets);
- every key of the `rivet` namespace in Moonraker's database (Rivet's settings).
  Moonraker drops a namespace once its last key is gone (checked on Moonraker v0.9.3), so
  `rivet` disappears from `/server/database/list`.

Then it renames back exactly the scripts listed in `/opt/rivet/replaced` and starts those that
were running. Without that record nothing is re-enabled.

`--keep-config` keeps `rivet.toml`, `rivet-touch.json` and the database settings, for installing
again later.

Nothing that is not Rivet's is touched: Wi-Fi networks joined from Rivet, Klipper's config and
`SAVE_CONFIG` results, Moonraker's config and other database namespaces, the replaced screen's files.
The only trace left is the modification time of `/opt` and `/etc/init.d`.

## Try without installing

`tools/swap-screen.sh rivet PRINTER_IP` runs Rivet from `/tmp` until the next reboot
(it stops an installed Rivet too); `tools/swap-screen.sh guppy PRINTER_IP` goes back.
