# Shell functions run on the printer by tools/install.sh (sourced, not installed).

# Klipper Mod only: its os-release names it, and the root is its chroot on the data
# partition. The stock Flashforge firmware (and ZMOD) has neither, and its screen is
# not an /etc/init.d script, so disabling "the screen" there could break the printer.
is_klipper_mod() {
    grep -q '^PRETTY_NAME="Klipper Mod' /etc/os-release 2>/dev/null \
        && [ -d /mnt/data/.klipper_mod/chroot ] \
        && [ -f /etc/init.d/rcS ]
}

# Init scripts that must never be taken for a screen, even if they mention its binary.
not_a_screen() {
    case "$(basename "$1")" in
        S80rivet|_*|S??klipper|S??moonraker|S??tslib|S??mdev|S??dbus|S??network|S??dropbear|S??iwd|S??wpa_supplicant) return 0 ;;
    esac
    return 1
}

# The /etc/init.d/S??* script whose text names $1, if exactly one screen-like one does.
script_for() {
    for s in /etc/init.d/S??*; do
        [ -f "$s" ] || continue
        not_a_screen "$s" && continue
        grep -q -F "$1" "$s" && { echo "$s"; return 0; }
    done
    return 1
}

# Prints one line per screen to disable: "<script name> running|stopped".
# A process holding /dev/fb0 that no init script starts is printed as
# "UNKNOWN <pid> <command>" so the caller can refuse.
detect_screens() {
    {
        for p in /proc/[0-9]*; do
            pid=${p#/proc/}
            [ "$pid" = "$$" ] && continue
            ls -l "$p/fd" 2>/dev/null | grep -q ' /dev/fb0$' || continue
            set -- $(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
            [ $# -gt 0 ] || continue
            # The thing to look for in init scripts: the program, or the script an
            # interpreter runs (KlipperScreen is python3 screen.py).
            key=$1
            case "$(basename "$1")" in
                ts_uinput|ts_*|rivet) continue ;;   # touch input of tslib, Rivet itself
                python*|sh|bash|ash|busybox) [ -n "${2:-}" ] && key=$2 ;;
            esac
            s=$(script_for "$key") || s=$(script_for "$(basename "$key")") || s=
            if [ -n "$s" ]; then
                echo "$(basename "$s") running"
            else
                echo "UNKNOWN $pid $*"
            fi
        done
        # Known screens that are installed but not running now: they would start at boot.
        for s in /etc/init.d/S??*; do
            [ -f "$s" ] || continue
            case "$(basename "$s" | tr 'A-Z' 'a-z')" in
                *guppyscreen*|*klipperscreen*|*helixscreen*) echo "$(basename "$s") stopped" ;;
            esac
        done
    } | awk '$1 == "UNKNOWN" { print; next } !($1 in s) || $2 == "running" { s[$1] = $2 } END { for (k in s) print k, s[k] }'
}
