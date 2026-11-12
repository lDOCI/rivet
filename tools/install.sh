#!/usr/bin/env bash
# Install Rivet as the printer's touch screen (Flashforge AD5M / AD5M Pro on Klipper Mod).
#
#   tools/install.sh [--force] PRINTER_IP
#
# Run it from the computer where Rivet was built with `cargo printer`. It copies
# over ssh (root, no scp: Klipper Mod has no sftp-server):
#   target/armv7-unknown-linux-musleabihf/release/rivet -> /opt/rivet/rivet
#   tools/install/S80rivet                              -> /etc/init.d/S80rivet
#   docs/rivet.toml.example -> /root/printer_data/config/rivet.toml (only if missing)
# then renames /etc/init.d/S80guppyscreen to _S80guppyscreen, so GuppyScreen no
# longer starts at boot (rcS only runs /etc/init.d/S??*), and starts Rivet.
#
# Running it again upgrades the binary and the init script; an existing rivet.toml
# is never overwritten. It refuses while a print is running or paused (asks
# Moonraker), unless --force. Nothing else on the printer is changed.
# Undo with tools/uninstall.sh PRINTER_IP (removes all of it, see there).
#
# Environment: PRINTER_USER (default root), RIVET_BIN (binary to upload).

set -euo pipefail

force=0
host="${PRINTER_HOST:-}"
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown option $arg" >&2; exit 1 ;;
    *) host="$arg" ;;
  esac
done
user="${PRINTER_USER:-root}"
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${RIVET_BIN:-$root/target/armv7-unknown-linux-musleabihf/release/rivet}"
init="$root/tools/install/S80rivet"
example="$root/docs/rivet.toml.example"

[ -n "$host" ] || { sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 1; }
[ -f "$bin" ] || { echo "no binary at $bin; run 'cargo printer' first" >&2; exit 1; }
file "$bin" 2>/dev/null | grep -q 'ARM' || { echo "$bin is not an ARM binary; build it with 'cargo printer'" >&2; exit 1; }

ssh_opts=(-o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new)
run() { ssh "${ssh_opts[@]}" "$user@$host" "$@"; }

run 'test -d /etc/init.d && test -x /usr/bin/backlight' \
  || { echo "$host does not look like Klipper Mod (no /etc/init.d or /usr/bin/backlight)" >&2; exit 1; }

# Restarting the screen during a print is harmless for Klipper, but do not surprise anyone.
if [ "$force" = 0 ]; then
  state="$(run "python3 -c '
import json, urllib.error, urllib.request
try:
    r = urllib.request.urlopen(\"http://127.0.0.1:7125/printer/objects/query?print_stats=state\", timeout=10)
    print(json.load(r)[\"result\"][\"status\"][\"print_stats\"][\"state\"])
except urllib.error.HTTPError:
    print(\"klippy-down\")
except Exception:
    print(\"unknown\")
'")"
  case "$state" in
    printing|paused) echo "a print is $state on $host; not installing (wait, or pass --force)" >&2; exit 1 ;;
    unknown) echo "cannot ask Moonraker on $host whether it is printing; not installing (pass --force)" >&2; exit 1 ;;
  esac
fi

echo "uploading $(basename "$bin") ($(du -h "$bin" | cut -f1 | tr -d ' ')) to $host:/opt/rivet/rivet"
run 'mkdir -p /opt/rivet && cat > /opt/rivet/rivet.new && chmod 755 /opt/rivet/rivet.new' < "$bin" \
  || { run 'rm -f /opt/rivet/rivet.new; rmdir /opt/rivet 2>/dev/null' || true; exit 1; }
run 'cat > /etc/init.d/.S80rivet.new && chmod 755 /etc/init.d/.S80rivet.new' < "$init"
run 'f=/root/printer_data/config/rivet.toml; if [ -e $f ]; then cat > /dev/null; echo "kept existing $f"; else cat > $f.new && mv $f.new $f && echo "created $f"; fi' < "$example"

run 'sh -s' <<'EOF'
set -e
# Free the screen: the installed Rivet, one started by tools/swap-screen.sh, GuppyScreen.
[ -x /etc/init.d/S80rivet ] && /etc/init.d/S80rivet stop || true
if [ -f /tmp/rivet.pid ]; then kill "$(cat /tmp/rivet.pid)" 2>/dev/null || true; rm -f /tmp/rivet.pid; fi
for g in /etc/init.d/S80guppyscreen /etc/init.d/_S80guppyscreen; do
  [ -x "$g" ] && "$g" stop >/dev/null 2>&1 || true
done
sleep 1
killall guppyscreen 2>/dev/null || true

if [ -f /opt/rivet/rivet ]; then what=upgraded; else what=installed; fi
mv -f /opt/rivet/rivet.new /opt/rivet/rivet
echo "$what /opt/rivet/rivet"
if [ -f /etc/init.d/S80rivet ]; then what=updated; else what=installed; fi
mv -f /etc/init.d/.S80rivet.new /etc/init.d/S80rivet
echo "$what /etc/init.d/S80rivet"
if [ -f /etc/init.d/S80guppyscreen ]; then
  mv -f /etc/init.d/S80guppyscreen /etc/init.d/_S80guppyscreen
  echo "renamed /etc/init.d/S80guppyscreen to _S80guppyscreen (GuppyScreen no longer starts at boot)"
fi

/etc/init.d/S80rivet start >/dev/null
sleep 3
pid="$(cat /run/rivet.child.pid 2>/dev/null || true)"
if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
  echo "Rivet is running (pid $pid, $(grep VmRSS /proc/$pid/status | tr -s ' \t' ' '))"
else
  echo "Rivet did not stay up, see /root/printer_data/logs/rivet-console.log:" >&2
  tail -n 20 /root/printer_data/logs/rivet-console.log >&2 || true
  exit 1
fi
EOF
echo "installed. Logs: /root/printer_data/logs/rivet.log. Undo: tools/uninstall.sh $host"
