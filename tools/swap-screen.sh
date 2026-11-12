#!/usr/bin/env bash
# Switch the printer's touchscreen between Rivet and the stock screen (GuppyScreen)
# without installing or removing anything on the printer.
#
#   tools/swap-screen.sh rivet  [PRINTER_IP]   upload the armv7 binary to /tmp, stop Guppy, start Rivet
#   tools/swap-screen.sh guppy  [PRINTER_IP]   stop Rivet, start Guppy again
#   tools/swap-screen.sh status [PRINTER_IP]   show what is running
#   tools/swap-screen.sh log    [PRINTER_IP]   tail the Rivet log
#
# Rivet lives in /tmp (RAM): a reboot brings the printer back to Guppy, or to the
# installed Rivet if tools/install.sh was used (this script stops it until then).
#
# Environment:
#   PRINTER_HOST   printer address (or pass it as the 2nd argument)
#   PRINTER_USER   ssh user, default root
#   GUPPY_INIT     init.d script of the screen to restore; autodetected when empty
#   RIVET_BIN      binary to upload, default target/armv7-unknown-linux-musleabihf/release/rivet
#   RIVET_ARGS     extra arguments for rivet
#   RIVET_ENV      environment for rivet, e.g. "RUST_LOG=debug RIVET_LANG=ru"

set -euo pipefail

cmd="${1:-}"
host="${2:-${PRINTER_HOST:-}}"
user="${PRINTER_USER:-root}"
bin="${RIVET_BIN:-$(dirname "$0")/../target/armv7-unknown-linux-musleabihf/release/rivet}"
remote_bin=/tmp/rivet
pidfile=/tmp/rivet.pid
logfile=/tmp/rivet.log

usage() { sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-1}"; }

[ -n "$cmd" ] || usage
[ "$cmd" = "-h" ] || [ "$cmd" = "--help" ] && usage 0
[ -n "$host" ] || { echo "printer address missing: pass it as the 2nd argument or set PRINTER_HOST" >&2; exit 1; }

ssh_opts=(-o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new)
run() { ssh "${ssh_opts[@]}" "$user@$host" "$@"; }

# Find the init.d script of GuppyScreen on the printer.
find_guppy() {
  if [ -n "${GUPPY_INIT:-}" ]; then echo "$GUPPY_INIT"; return; fi
  local found
  found="$(run 'ls /etc/init.d/ 2>/dev/null | grep -i guppy | head -n 1' || true)"
  if [ -z "$found" ]; then
    echo "cannot find a Guppy init script in /etc/init.d; set GUPPY_INIT=/path/to/script" >&2
    exit 1
  fi
  echo "/etc/init.d/$found"
}

stop_rivet() {
  # Rivet installed by tools/install.sh: stop its supervisor too, or it would restart it.
  run "if [ -x /etc/init.d/S80rivet ]; then /etc/init.d/S80rivet stop; fi"
  run "if [ -f $pidfile ] && kill -0 \$(cat $pidfile) 2>/dev/null; then kill \$(cat $pidfile); sleep 1; fi; rm -f $pidfile"
  # Back to the first page of video memory, or the next screen draws off screen.
  run "echo 0,0 > /sys/class/graphics/fb0/pan 2>/dev/null || true"
}

case "$cmd" in
  rivet)
    [ -f "$bin" ] || { echo "no binary at $bin; run 'cargo printer' first" >&2; exit 1; }
    guppy="$(find_guppy)"
    echo "uploading $(basename "$bin") to $host:$remote_bin"
    ssh "${ssh_opts[@]}" "$user@$host" "cat > $remote_bin.new && chmod +x $remote_bin.new && mv $remote_bin.new $remote_bin" < "$bin"
    stop_rivet
    echo "stopping Guppy ($guppy)"
    run "$guppy stop" || true
    # The init script kills by pid file; make sure no Guppy process still owns the screen.
    # (pidof, not ps|grep: the grep would match this very command line)
    run "sleep 1; killall guppyscreen 2>/dev/null; sleep 1; if pidof guppyscreen >/dev/null; then echo 'Guppy still running' >&2; exit 1; fi" \
      || { echo "could not stop Guppy, not starting Rivet" >&2; exit 1; }
    echo "starting Rivet"
    # shellcheck disable=SC2029
    run "nohup env ${RIVET_ENV:-} $remote_bin ${RIVET_ARGS:-} >$logfile 2>&1 </dev/null & echo \$! >$pidfile"
    sleep 2
    run "kill -0 \$(cat $pidfile) 2>/dev/null" \
      && echo "Rivet is running. Back to Guppy: $0 guppy $host" \
      || { echo "Rivet exited right away, log:" >&2; run "tail -n 30 $logfile" >&2; echo "restoring Guppy" >&2; run "$guppy start" || true; exit 1; }
    ;;
  guppy)
    guppy="$(find_guppy)"
    stop_rivet
    echo "starting Guppy ($guppy)"
    run "$guppy start"
    ;;
  status)
    run "if [ -f $pidfile ] && kill -0 \$(cat $pidfile) 2>/dev/null; then echo 'Rivet: running (pid '\$(cat $pidfile)')'; else echo 'Rivet: not running'; fi"
    run "ps | grep '[g]uppyscreen' || echo 'Guppy: not running'"
    ;;
  log)
    run "tail -n 100 $logfile"
    ;;
  *)
    usage
    ;;
esac
