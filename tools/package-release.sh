#!/usr/bin/env bash
# Package a printer build of Rivet for release.
#
#   tools/package-release.sh VERSION [OUT_DIR]      (OUT_DIR defaults to dist)
#
# Needs the printer binary (`cargo printer`) and runs ui-snapshot for the USB
# installer's pictures. Writes to OUT_DIR:
#   rivet-ad5m.zip                   the update: Moonraker's update manager
#                                    (type web) unpacks it into /opt/rivet (Klipper
#                                    Mod) or /srv/rivet (Z-Mod, which finds it by
#                                    release_info.json's asset_name): rivet,
#                                    rivet.sha256, release_info.json, S80rivet,
#                                    rivet-watchdog
#   rivet-VERSION-usb.zip            unpack onto a FAT32 USB stick, switch the
#                                    printer on with it: installs Rivet
#   rivet-VERSION-usb-remove.zip     the same, but removes Rivet
#   rivet-VERSION-installer.tar.gz   install over ssh from a computer:
#                                    tools/install.sh, tools/uninstall.sh
#
# Environment: RIVET_OWNER (GitHub owner in release_info.json, default lDOCI).

set -euo pipefail
version="${1:?usage: tools/package-release.sh VERSION [OUT_DIR]}"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$(mkdir -p "${2:-$root/dist}" && cd "${2:-$root/dist}" && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
bin="$target/armv7-unknown-linux-musleabihf/release/rivet"
owner="${RIVET_OWNER:-lDOCI}"
[ -f "$bin" ] || { echo "no printer binary at $bin; run 'cargo printer' first" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
sha() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

# The update: what Moonraker installs.
mkdir -p "$work/zip"
watchdog="$(dirname "$bin")/rivet-watchdog"
[ -f "$watchdog" ] || { echo "no rivet-watchdog at $watchdog; run 'cargo printer-watchdog' first" >&2; exit 1; }
cp "$bin" "$work/zip/rivet" && chmod 755 "$work/zip/rivet"
cp "$watchdog" "$work/zip/rivet-watchdog" && chmod 755 "$work/zip/rivet-watchdog"
(cd "$work/zip" && sha rivet > rivet.sha256)
printf '{"project_name": "rivet", "project_owner": "%s", "version": "%s", "asset_name": "rivet-ad5m.zip"}\n' "$owner" "$version" > "$work/zip/release_info.json"
cp "$root/tools/install/S80rivet" "$work/zip/" && chmod 755 "$work/zip/S80rivet"
rm -f "$out/rivet-ad5m.zip"
(cd "$work/zip" && zip -qX "$out/rivet-ad5m.zip" rivet rivet.sha256 release_info.json S80rivet rivet-watchdog)

# The USB installer's pictures, as raw framebuffer images.
(cd "$root" && RIVET_FB_RAW=1 cargo run -q -p ui-snapshot -- "$work/pics" usb-installing usb-done usb-removed usb-error >/dev/null)
mkdir -p "$work/usb/rivet" "$work/usb-remove/rivet"
for n in installing done removed error; do
  gzip -9c "$work/pics/usb-$n.fb" > "$work/usb/rivet/$n.fb.gz"
  cp "$work/usb/rivet/$n.fb.gz" "$work/usb-remove/rivet/"
done
cp "$root/tools/usb/flashforge_init.sh" "$work/usb/" && chmod 755 "$work/usb/flashforge_init.sh"
cp "$root/tools/usb/flashforge_init.sh" "$work/usb-remove/"
cp "$work/zip/rivet" "$work/zip/rivet.sha256" "$work/zip/release_info.json" "$work/usb/rivet/"
cp "$root/tools/install/S80rivet" "$root/tools/install/S80rivet.initd" "$root/tools/install/rivet.cfg" "$work/zip/rivet-watchdog" "$work/usb/rivet/"
cp "$root/docs/rivet.toml.example" "$work/usb/rivet/"
echo "Remove Rivet: this stick's flashforge_init.sh takes Rivet off the printer." > "$work/usb-remove/rivet/remove"
rm -f "$out/rivet-$version-usb.zip" "$out/rivet-$version-usb-remove.zip"
(cd "$work/usb" && zip -qrX "$out/rivet-$version-usb.zip" .)
(cd "$work/usb-remove" && zip -qrX "$out/rivet-$version-usb-remove.zip" .)

# Over ssh from a computer.
inst="$work/rivet-$version"
mkdir -p "$inst/tools/install" "$inst/docs"
cp "$work/zip/rivet" "$work/zip/rivet-watchdog" "$work/zip/release_info.json" "$inst/"
cp "$root/tools/install.sh" "$root/tools/uninstall.sh" "$inst/tools/"
cp "$root"/tools/install/* "$inst/tools/install/"
cp "$root/docs/rivet.toml.example" "$inst/docs/"
tar -C "$work" -czf "$out/rivet-$version-installer.tar.gz" "rivet-$version"

ls -l "$out"
