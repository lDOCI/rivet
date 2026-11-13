# Install, update, remove

For the Flashforge Adventurer 5M / 5M Pro with [Klipper Mod](https://github.com/xblax/flashforge_ad5m_klipper_mod).
[Русская версия](install.ru.md).

On **Z-Mod**, Rivet is installed by Z-Mod itself once it lists it as one of its screens (the way it
offers HelixScreen); these installers refuse to run there. **Forge-X** and the stock firmware are
not supported.

## From a USB stick

This is the way Klipper Mod itself is installed and updated.

1. Download `rivet-<version>-usb.zip` from the [latest release](https://github.com/lDOCI/rivet/releases/latest).
2. Unpack it onto a USB stick formatted as FAT32. At the top of the stick there must be
   `flashforge_init.sh` and the `rivet` folder. Nothing else needs to be removed from the stick.
3. Switch the printer off, insert the stick, switch it on.
4. Klipper Mod sees `flashforge_init.sh` and hands it to the printer's own start-up script, which
   runs it. The screen shows *Installing Rivet…* (about a minute), then *Rivet is installed —
   remove the USB stick*.
5. Remove the stick. The printer restarts into Klipper Mod with Rivet.

If something stops the install, the screen says *Rivet was not installed* and the reason is in
`rivet-install.log` on the stick. Nothing on the printer was changed unless the log says
"partly installed" (then run the stick again, or remove Rivet with the remove stick). Remove the
stick and the printer starts as it did before.

If the screen never shows a result, remove the stick and switch the printer off and on: without
the stick Klipper Mod starts as usual.

**The printer starts as usual and ignores the stick.** Klipper Mod looks at the stick two seconds
into the start; a slow stick with a partition table is not ready by then (its log says
`mounting /dev/sda on /mnt failed`). Format the stick as FAT32 *without* a partition table, then
copy the files again. This erases the stick.

- macOS (find the stick's `diskN` in `diskutil list`):

  ```bash
  diskutil unmountDisk /dev/diskN
  sudo newfs_msdos -F 32 -v RIVET /dev/rdiskN
  diskutil mountDisk /dev/diskN
  ```

- Linux (the whole device, e.g. `/dev/sdX`, not `/dev/sdX1`): `sudo mkfs.vfat -F 32 -I /dev/sdX`
- Windows cannot do this by itself: try another stick, or use the ssh way.

**Remove Rivet** the same way with `rivet-<version>-usb-remove.zip`: the screen says *Rivet is
removed*, and GuppyScreen (or whatever screen Rivet replaced) is back after the restart.

## Over ssh, from a computer

For people who build Rivet themselves, or prefer a terminal. Needs ssh access as `root`
(Klipper Mod's default). From the unpacked `rivet-<version>-installer.tar.gz`, or from a source
checkout after `cargo printer`:

```bash
tools/install.sh PRINTER_IP           # install or upgrade
tools/install.sh --dry-run PRINTER_IP # only say what it would do
tools/uninstall.sh PRINTER_IP         # remove everything, give the old screen back
tools/uninstall.sh --keep-config PRINTER_IP
```

`install.sh` refuses while a print is running or paused, and when an unknown program holds the
screen; `--force` overrides both. `--keep-config` keeps the settings and the touch calibration for
a later install. Files go over `ssh … cat`: Klipper Mod has no sftp server, so `scp` does not work.

`tools/swap-screen.sh rivet PRINTER_IP` runs Rivet from `/tmp` until the next reboot without
installing anything; `tools/swap-screen.sh guppy PRINTER_IP` goes back.

## What changes on the printer

| Path | What |
|---|---|
| `/opt/rivet/rivet` | the program |
| `/opt/rivet/rivet-watchdog`, `/opt/rivet/S80rivet` | its watchdog and start script (updated with every release) |
| `/opt/rivet/rivet.sha256`, `release_info.json` | its checksum and version, for updates |
| `/opt/rivet/rivet.good` | a copy of the last version that ran for 30 seconds, to come back to |
| `/opt/rivet/replaced` | which screen was switched off and whether it was running |
| `/etc/init.d/S80rivet` | starts Rivet at boot (it calls `/opt/rivet/S80rivet`) |
| `/root/printer_data/config/rivet.cfg` | the `RIVET_SCREEN` macro, included from `printer.cfg` by an `[include rivet.cfg]` line |
| `/etc/init.d/_S80guppyscreen` | the old screen's start script, renamed so it no longer starts |
| `/root/printer_data/config/moonraker.conf` | gets an `[update_manager rivet]` section (a copy of the file before is kept as `moonraker.conf.before-rivet`) |
| `/root/printer_data/config/rivet.toml` | settings file, created only if missing ([example](rivet.toml.example)) |
| `/root/printer_data/config/rivet-touch.json` | the touch calibration, after you run it |
| `/root/printer_data/logs/rivet.log`, `rivet-console.log` | the log, and crashes and restarts |
| Moonraker's database, namespace `rivet` | settings changed on the screen: theme, brightness, sleep, lock, presets… |

Nothing else is touched. Removing Rivet takes all of this away and renames the old screen's script
back. The USB remover cannot reach Moonraker's database (Moonraker is not running at that point), so
it leaves the `rivet` namespace: harmless without Rivet, and it brings your settings back if you
install again. `tools/uninstall.sh` removes it too.

## Updates

Moonraker checks GitHub for new releases, like it does for Fluidd and Mainsail. To update:

- on the screen: **Settings › Updates**. The Rivet row says whether a newer version exists, and
  **Update the screen** installs it; the screen restarts by itself;
- or in Fluidd/Mainsail, in the list of updates (*rivet*).

Moonraker refuses updates during a print.

When the new version starts, it runs under a watchdog (`S80rivet`):

- a version that does not match its checksum is not run;
- a version that exits twice within 30 seconds of starting is replaced by the last version that ran
  for 30 seconds;
- either way Settings › Updates then says *the new version did not start; the previous one is back*.

If Rivet ever crashes, it is started again after 1 s; repeated quick crashes wait longer (up to a
minute), so a crash loop does not load the processor.

## From the console

In the console of Fluidd or Mainsail, or in your own macros:

```
RIVET_SCREEN ACTION=OFF       backlight off now (a touch wakes it, even during a print)
RIVET_SCREEN ACTION=ON        backlight on
RIVET_SCREEN ACTION=RESTART   start Rivet again
```

## Language and time zone

Rivet speaks English and Russian. Unless you pick one in Settings › Language and time, it follows the
language set in Fluidd or Mainsail (on Z-Mod, Z-Mod's own language).

The time zone is chosen in the same place. Until one is set, the printer's clock is UTC and Rivet
hides times of day rather than show wrong ones. On Klipper Mod Rivet writes the zone to
`printer_data/config/timezone`, where Klipper Mod reads it; on Z-Mod it runs Z-Mod's `SET_TIMEZONE`.

## Logs

In Fluidd or Mainsail, next to `klipper.log`: `rivet.log` (what Rivet did) and `rivet-console.log`
(crashes, restarts, roll-backs). On the printer:

```bash
/etc/init.d/S80rivet stop | start | restart
tail -f /root/printer_data/logs/rivet.log
```

More detail: `log_level = "debug"` in `rivet.toml`, then restart the screen.

## When touches miss

Settings › Screen › **Touch calibration**: touch nine crosses with the finger you normally use.
**Touch test** (same menu) shows where your taps land. The calibration is kept in
`rivet-touch.json`; delete the file to go back to the panel's own calibration.
