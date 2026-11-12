#!/usr/bin/env bash
# Remove Rivet from the printer and give the screen back to GuppyScreen.
#
#   tools/uninstall.sh PRINTER_IP
#
# Stops Rivet, deletes /etc/init.d/S80rivet and /opt/rivet, renames
# /etc/init.d/_S80guppyscreen back to S80guppyscreen and starts GuppyScreen.
# Keeps /root/printer_data/config/rivet.toml and the logs in
# /root/printer_data/logs (rivet*.log); delete them by hand if not wanted.
#
# Environment: PRINTER_USER (default root).

set -euo pipefail

host="${1:-${PRINTER_HOST:-}}"
user="${PRINTER_USER:-root}"

if [ -z "$host" ] || [ "$host" = "-h" ] || [ "$host" = "--help" ]; then
  sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
  [ -n "$host" ] && exit 0 || exit 1
fi

ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new "$user@$host" 'sh -s' <<'EOF'
set -e
if [ -x /etc/init.d/S80rivet ]; then
  /etc/init.d/S80rivet stop
  echo "Rivet stopped"
fi
if [ -f /tmp/rivet.pid ]; then kill "$(cat /tmp/rivet.pid)" 2>/dev/null || true; rm -f /tmp/rivet.pid; fi
rm -f /etc/init.d/S80rivet /etc/init.d/.S80rivet.new
rm -rf /opt/rivet
echo "removed /etc/init.d/S80rivet and /opt/rivet"

if [ -f /etc/init.d/_S80guppyscreen ]; then
  if [ -f /etc/init.d/S80guppyscreen ]; then
    echo "both S80guppyscreen and _S80guppyscreen exist; keeping S80guppyscreen" >&2
  else
    mv /etc/init.d/_S80guppyscreen /etc/init.d/S80guppyscreen
    echo "GuppyScreen enabled again: /etc/init.d/S80guppyscreen"
  fi
fi
if [ -x /etc/init.d/S80guppyscreen ]; then
  /etc/init.d/S80guppyscreen start
  sleep 2
  pidof guppyscreen >/dev/null && echo "GuppyScreen is running" || echo "GuppyScreen did not start" >&2
else
  echo "no /etc/init.d/S80guppyscreen to start" >&2
fi
EOF
