# Install on the printer

For a Flashforge Adventurer 5M / 5M Pro running [Klipper Mod](https://github.com/xblax/flashforge_ad5m_klipper_mod).
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
| `/etc/init.d/_S80guppyscreen` | GuppyScreen's init script, renamed so it no longer starts (`rcS` runs only `/etc/init.d/S??*`) |
| `/root/printer_data/config/rivet.toml` | settings, written from `docs/rivet.toml.example` only if missing |
| `/root/printer_data/logs/rivet.log` | Rivet's log, rotated (512 KB × 3) |
| `/root/printer_data/logs/rivet-console.log` | crashes (panics) and restarts, one old copy kept |

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

Then it renames `_S80guppyscreen` back and starts GuppyScreen.

`--keep-config` keeps `rivet.toml`, `rivet-touch.json` and the database settings, for installing
again later.

Nothing that is not Rivet's is touched: Wi-Fi networks joined from Rivet, Klipper's config and
`SAVE_CONFIG` results, Moonraker's config and other database namespaces, GuppyScreen's files.

## Try without installing

`tools/swap-screen.sh rivet PRINTER_IP` runs Rivet from `/tmp` until the next reboot
(it stops an installed Rivet too); `tools/swap-screen.sh guppy PRINTER_IP` goes back.
