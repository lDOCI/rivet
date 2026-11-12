#!/usr/bin/env bash
# Install Rivet as the printer's touch screen (Flashforge AD5M / AD5M Pro on Klipper Mod).
#
#   tools/install.sh [--dry-run] [--force] PRINTER_IP
#
# Klipper Mod only: on anything else (stock firmware, ZMOD) it refuses and changes nothing.
# Run it from the computer where Rivet was built with `cargo printer`. Over ssh
# (root, no scp: Klipper Mod has no sftp-server) it copies
#   target/armv7-unknown-linux-musleabihf/release/rivet -> /opt/rivet/rivet
#   tools/install/S80rivet                              -> /etc/init.d/S80rivet
#   docs/rivet.toml.example -> /root/printer_data/config/rivet.toml (only if missing)
# finds the screen Rivet replaces (the process holding /dev/fb0 and the init script
# that starts it, or a known GuppyScreen/KlipperScreen/HelixScreen script), stops
# it, disables its script by renaming it with a leading "_" (rcS runs only
# /etc/init.d/S??*), records that in /opt/rivet/replaced, and starts Rivet.
# No other screen: nothing is disabled.
#
# Running it again upgrades the binary and the init script; an existing rivet.toml
# is never overwritten. It refuses while a print is running or paused, or when an
# unknown program holds the screen, unless --force. --dry-run only prints what it
# would disable. Undo with tools/uninstall.sh PRINTER_IP.
#
# Environment: PRINTER_USER (default root), RIVET_BIN (binary to upload).

set -euo pipefail

force=0
dry=0
host="${PRINTER_HOST:-}"
usage() { sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; }
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    --dry-run) dry=1 ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "unknown option $arg" >&2; exit 1 ;;
    *) host="$arg" ;;
  esac
done
user="${PRINTER_USER:-root}"
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${RIVET_BIN:-$root/target/armv7-unknown-linux-musleabihf/release/rivet}"
init="$root/tools/install/S80rivet"
funcs="$root/tools/install/screen.sh"
example="$root/docs/rivet.toml.example"

[ -n "$host" ] || { usage; exit 1; }
if [ "$dry" = 0 ]; then
  [ -f "$bin" ] || { echo "no binary at $bin; run 'cargo printer' first" >&2; exit 1; }
  file "$bin" 2>/dev/null | grep -q 'ARM' || { echo "$bin is not an ARM binary; build it with 'cargo printer'" >&2; exit 1; }
fi

ssh_opts=(-o ConnectTimeout=8 -o StrictHostKeyChecking=accept-new)
run() { ssh "${ssh_opts[@]}" "$user@$host" "$@"; }
# Run a script on the printer with screen.sh's functions loaded.
remote() { { cat "$funcs"; cat; } | run "FORCE=$force sh -s"; }

# 1. Klipper Mod? Printing? Which screen? Nothing is changed yet.
plan="$(remote <<'EOF'
if ! is_klipper_mod; then
  echo "NOT_KLIPPER_MOD $(sed -n 's/^PRETTY_NAME=//p' /etc/os-release 2>/dev/null)"
  exit 0
fi
state=$(python3 -c '
import json, urllib.error, urllib.request
try:
    r = urllib.request.urlopen("http://127.0.0.1:7125/printer/objects/query?print_stats=state", timeout=10)
    print(json.load(r)["result"]["status"]["print_stats"]["state"])
except urllib.error.HTTPError:
    print("klippy-down")
except Exception:
    print("unknown")
' 2>/dev/null || echo unknown)
echo "PRINT $state"
detect_screens
EOF
)"

if first="$(grep '^NOT_KLIPPER_MOD' <<<"$plan")"; then
  echo "$host is not running Klipper Mod (${first#NOT_KLIPPER_MOD }); Rivet installs only there. Nothing changed." >&2
  exit 1
fi
state="$(sed -n 's/^PRINT //p' <<<"$plan")"
screens="$(grep -v -e '^PRINT ' -e '^UNKNOWN ' <<<"$plan" || true)"
unknown="$(grep '^UNKNOWN ' <<<"$plan" || true)"

echo "printer: Klipper Mod, print state: $state"
if [ -n "$screens" ]; then
  while read -r s st; do echo "screen to replace: /etc/init.d/$s ($st)"; done <<<"$screens"
else
  echo "no other screen found: nothing will be disabled"
fi
while read -r _ pid cmd; do
  [ -n "${pid:-}" ] && echo "unknown program holds /dev/fb0: pid $pid ($cmd); no init script starts it" >&2
done <<<"$unknown"

[ "$dry" = 1 ] && { echo "dry run: nothing changed"; exit 0; }

if [ "$force" = 0 ]; then
  case "$state" in
    printing|paused) echo "a print is $state on $host; not installing (wait, or pass --force). Nothing changed." >&2; exit 1 ;;
    unknown) echo "cannot ask Moonraker on $host whether it is printing; not installing (pass --force). Nothing changed." >&2; exit 1 ;;
  esac
  if [ -n "$unknown" ]; then
    echo "not installing while an unknown program owns the screen (stop it, or pass --force). Nothing changed." >&2
    exit 1
  fi
fi

# 2. Upload next to the targets; nothing is live until the renames below.
echo "uploading $(basename "$bin") ($(du -h "$bin" | cut -f1 | tr -d ' ')) to $host:/opt/rivet/rivet"
run 'mkdir -p /opt/rivet && cat > /opt/rivet/rivet.new && chmod 755 /opt/rivet/rivet.new' < "$bin" \
  || { run 'rm -f /opt/rivet/rivet.new; rmdir /opt/rivet 2>/dev/null' || true; exit 1; }
run 'cat > /etc/init.d/.S80rivet.new && chmod 755 /etc/init.d/.S80rivet.new' < "$init"
run 'f=/root/printer_data/config/rivet.toml; if [ -e $f ]; then cat > /dev/null; echo "kept existing $f"; else cat > $f.new && mv $f.new $f && echo "created $f"; fi' < "$example"

# 3. Switch: stop Rivet and the screens found, disable their scripts, start Rivet.
{ printf "SCREENS='%s'\n" "$screens"; cat <<'EOF'
set -e
[ -x /etc/init.d/S80rivet ] && /etc/init.d/S80rivet stop || true
if [ -f /tmp/rivet.pid ]; then kill "$(cat /tmp/rivet.pid)" 2>/dev/null || true; rm -f /tmp/rivet.pid; fi

touch /opt/rivet/replaced
echo "$SCREENS" | while read -r s st; do
  [ -n "$s" ] || continue
  /etc/init.d/$s stop >/dev/null 2>&1 || true
  mv -f /etc/init.d/$s /etc/init.d/_$s
  grep -q "^$s " /opt/rivet/replaced || echo "$s $st" >> /opt/rivet/replaced
  echo "stopped and disabled /etc/init.d/$s (now /etc/init.d/_$s; recorded in /opt/rivet/replaced)"
done
# A screen stopped by its script may still hold the framebuffer for a moment.
sleep 1
for p in /proc/[0-9]*; do
  ls -l "$p/fd" 2>/dev/null | grep -q ' /dev/fb0$' || continue
  case "$(basename "$(readlink "$p/exe")")" in ts_*|rivet) continue ;; esac
  [ "$FORCE" = 1 ] || [ -n "$SCREENS" ] || continue
  kill "${p#/proc/}" 2>/dev/null && echo "stopped pid ${p#/proc/}, it still held /dev/fb0"
done

if [ -f /opt/rivet/rivet ]; then what=upgraded; else what=installed; fi
mv -f /opt/rivet/rivet.new /opt/rivet/rivet
echo "$what /opt/rivet/rivet"
if [ -f /etc/init.d/S80rivet ]; then what=updated; else what=installed; fi
mv -f /etc/init.d/.S80rivet.new /etc/init.d/S80rivet
echo "$what /etc/init.d/S80rivet"

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
} | remote
echo "installed. Logs: /root/printer_data/logs/rivet.log. Undo: tools/uninstall.sh $host"
