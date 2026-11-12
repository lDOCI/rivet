#!/usr/bin/env bash
# Remove Rivet from the printer without a trace and give the screen back to the one it replaced.
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
# then renames back exactly the screen scripts install.sh disabled, as recorded in
# /opt/rivet/replaced (e.g. _S80guppyscreen -> S80guppyscreen), and starts those
# that were running. No record: nothing is re-enabled.
#
# The [update_manager rivet] section install.sh added to moonraker.conf is removed
# too (Moonraker is restarted to forget it).
#
# --keep-config keeps rivet.toml, rivet-touch.json and the database settings,
# for reinstalling later. Nothing else is touched: Wi-Fi networks, Klipper and
# Moonraker config, other database namespaces and the replaced screen's files stay.
#
# Environment: PRINTER_USER (default root).

set -euo pipefail

keep=0
host="${PRINTER_HOST:-}"
for arg in "$@"; do
  case "$arg" in
    --keep-config) keep=1 ;;
    -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown option $arg" >&2; exit 1 ;;
    *) host="$arg" ;;
  esac
done
user="${PRINTER_USER:-root}"
[ -n "$host" ] || { sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 1; }

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
# What tools/install.sh disabled, read before /opt/rivet goes.
REPLACED="$(cat /opt/rivet/replaced 2>/dev/null || true)"

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
# Rivet may have stopped on its second page of video memory; the screen that
# comes back takes that offset for good and would draw off screen: reset it.
[ -w /sys/class/graphics/fb0/pan ] && echo 0,0 > /sys/class/graphics/fb0/pan || true

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

# The update manager section: from its header to the next section or the end.
conf=/root/printer_data/config/moonraker.conf
if [ -f $conf ] && grep -q '^\[update_manager rivet\]' $conf; then
  awk '/^\[update_manager rivet\]/{skip=1; next} /^\[/{skip=0} !skip' $conf > $conf.tmp
  # Drop the blank line install.sh put before it, when it is the last line now.
  sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' $conf.tmp
  mv -f $conf.tmp $conf
  echo "removed [update_manager rivet] from $conf"
  setsid /etc/init.d/S65moonraker restart </dev/null >/dev/null 2>&1 && echo "restarted Moonraker"
  rm -f $conf.before-rivet
fi

# A screen started while Moonraker is still coming up can hang on its first
# frame (GuppyScreen does): wait for Moonraker to answer, up to a minute.
i=0
while [ $i -lt 30 ] && ! python3 -c 'import urllib.request; urllib.request.urlopen("http://127.0.0.1:7125/server/info", timeout=2)' 2>/dev/null; do
  sleep 2
  i=$((i + 1))
done

# Bring back exactly the screens tools/install.sh disabled, nothing else.
if [ -n "$REPLACED" ]; then
  echo "$REPLACED" | while read -r name st; do
    [ -n "$name" ] || continue
    if [ -f "/etc/init.d/$name" ]; then
      echo "/etc/init.d/$name exists again (reinstalled?); leaving it and _$name as they are" >&2
    elif [ -f "/etc/init.d/_$name" ]; then
      mv "/etc/init.d/_$name" "/etc/init.d/$name"
      echo "renamed /etc/init.d/_$name back to $name"
    else
      echo "/etc/init.d/_$name is gone; cannot restore $name" >&2
      continue
    fi
    if [ "$st" = running ]; then
      setsid "/etc/init.d/$name" start </dev/null >/dev/null 2>&1 && echo "started /etc/init.d/$name" || echo "/etc/init.d/$name did not start" >&2
    else
      echo "left /etc/init.d/$name stopped (it was not running before Rivet); it starts at the next boot"
    fi
  done
else
  echo "nothing to restore: /opt/rivet/replaced records no screen, the printer had no other screen"
fi
exit $status
EOF
