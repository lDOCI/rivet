#!/usr/bin/env bash
# Remove Rivet from the printer without a trace and give the screen back to GuppyScreen.
#
#   tools/uninstall.sh [--keep-config] PRINTER_IP
#
# Stops Rivet and deletes everything it put on the printer, printing each item:
#   /etc/init.d/S80rivet, /opt/rivet
#   /root/printer_data/logs/rivet.log* and rivet-console.log*
#   /run/rivet*.pid; in /tmp what tools/swap-screen.sh and Rivet left
#     (rivet, rivet.log, rivet.pid, rivet-wpa-* sockets, rivet-measure.sh)
#   /root/printer_data/config/rivet.toml and rivet-touch.json
#   the "rivet" namespace in Moonraker's database (Rivet's settings)
# then renames /etc/init.d/_S80guppyscreen back to S80guppyscreen and starts it.
#
# --keep-config keeps rivet.toml, rivet-touch.json and the database settings,
# for reinstalling later. Nothing else is touched: Wi-Fi networks, Klipper and
# Moonraker config, other database namespaces and GuppyScreen's files stay.
#
# Environment: PRINTER_USER (default root).

set -euo pipefail

keep=0
host="${PRINTER_HOST:-}"
for arg in "$@"; do
  case "$arg" in
    --keep-config) keep=1 ;;
    -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown option $arg" >&2; exit 1 ;;
    *) host="$arg" ;;
  esac
done
user="${PRINTER_USER:-root}"
[ -n "$host" ] || { sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 1; }

ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new "$user@$host" "KEEP=$keep sh -s" <<'EOF'
set -e
status=0
gone() {
  for f in "$@"; do
    if [ -e "$f" ] || [ -L "$f" ]; then
      rm -rf "$f"
      echo "removed $f"
    fi
  done
}

# Stop every Rivet: the installed one (its supervisor first, or it restarts it)
# and one started from /tmp by tools/swap-screen.sh.
if [ -x /etc/init.d/S80rivet ]; then
  /etc/init.d/S80rivet stop
  echo "stopped /etc/init.d/S80rivet"
fi
if [ -f /tmp/rivet.pid ]; then kill "$(cat /tmp/rivet.pid)" 2>/dev/null || true; fi
for p in $(pidof rivet); do
  kill "$p" 2>/dev/null && echo "stopped rivet (pid $p)"
done
sleep 1
for p in $(pidof rivet); do kill -9 "$p" 2>/dev/null || true; done

gone /etc/init.d/S80rivet /etc/init.d/.S80rivet.new /opt/rivet
gone /run/rivet.pid /run/rivet.child.pid
gone /tmp/rivet /tmp/rivet.new /tmp/rivet.log /tmp/rivet.pid /tmp/rivet-measure.sh /tmp/rivet-wpa-*
gone /root/printer_data/logs/rivet.log /root/printer_data/logs/rivet.log.* \
     /root/printer_data/logs/rivet-console.log /root/printer_data/logs/rivet-console.log.*

if [ "$KEEP" = 1 ]; then
  echo "kept (--keep-config): /root/printer_data/config/rivet.toml, rivet-touch.json, Moonraker database namespace \"rivet\""
else
  gone /root/printer_data/config/rivet.toml /root/printer_data/config/rivet-touch.json
  # Rivet's settings live in Moonraker's database, namespace "rivet": delete every key.
  python3 - <<'PY' || status=1
import json, sys, urllib.error, urllib.parse, urllib.request
base = "http://127.0.0.1:7125/server/database"
def call(path, method="GET"):
    req = urllib.request.Request(base + path, method=method)
    return json.load(urllib.request.urlopen(req, timeout=10))["result"]
try:
    if "rivet" not in call("/list")["namespaces"]:
        sys.exit(0)
    keys = list(call("/item?namespace=rivet")["value"].keys())
except urllib.error.HTTPError as e:
    if e.code == 404:
        sys.exit(0)
    print("cannot read Moonraker's database: %s; Rivet's settings are left in namespace \"rivet\"" % e, file=sys.stderr)
    sys.exit(1)
except Exception as e:
    print("cannot reach Moonraker (%s); Rivet's settings are left in namespace \"rivet\"" % e, file=sys.stderr)
    sys.exit(1)
for k in keys:
    call("/item?namespace=rivet&key=" + urllib.parse.quote(k, safe=""), "DELETE")
    print("removed Moonraker database rivet.%s" % k)
if "rivet" in call("/list")["namespaces"]:
    print("Moonraker still lists the empty namespace \"rivet\" (it has no call to drop one)")
PY
fi

if [ -f /etc/init.d/_S80guppyscreen ]; then
  if [ -f /etc/init.d/S80guppyscreen ]; then
    echo "both S80guppyscreen and _S80guppyscreen exist; keeping S80guppyscreen" >&2
  else
    mv /etc/init.d/_S80guppyscreen /etc/init.d/S80guppyscreen
    echo "renamed /etc/init.d/_S80guppyscreen back to S80guppyscreen"
  fi
fi
if [ -x /etc/init.d/S80guppyscreen ]; then
  pidof guppyscreen >/dev/null || /etc/init.d/S80guppyscreen start
  sleep 2
  pidof guppyscreen >/dev/null && echo "GuppyScreen is running" || { echo "GuppyScreen did not start" >&2; status=1; }
else
  echo "no /etc/init.d/S80guppyscreen to start" >&2
  status=1
fi
exit $status
EOF
