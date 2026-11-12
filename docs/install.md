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

Running it again replaces the binary and the init script and keeps `rivet.toml`.

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

Stops Rivet, deletes `/etc/init.d/S80rivet` and `/opt/rivet`, renames `_S80guppyscreen` back
and starts GuppyScreen. `rivet.toml` and the logs stay; delete them by hand if not wanted.

## Try without installing

`tools/swap-screen.sh rivet PRINTER_IP` runs Rivet from `/tmp` until the next reboot
(it stops an installed Rivet too); `tools/swap-screen.sh guppy PRINTER_IP` goes back.
