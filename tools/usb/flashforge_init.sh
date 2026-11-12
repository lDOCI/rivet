#!/bin/sh
# Rivet's USB installer for the Flashforge AD5M / AD5M Pro with Klipper Mod.
#
# The release's rivet-<version>-usb.zip, unpacked onto a FAT32 USB stick:
#   flashforge_init.sh     this script
#   rivet/                 rivet, rivet.sha256, release_info.json, S80rivet,
#                          rivet.toml.example, *.fb.gz (the screen pictures)
# Insert the stick and switch the printer on. Klipper Mod sees flashforge_init.sh
# on the stick and leaves the boot to the stock firmware's auto_run.sh, which runs
# this script (as for any Flashforge or Klipper Mod update). It installs Rivet
# into Klipper Mod (or removes it, when rivet/remove is on the stick), shows the
# result on the screen, writes rivet-install.log to the stick, and waits for the
# stick to be removed; then the printer restarts into Klipper Mod.
#
# What it changes is what tools/install.sh changes over ssh: /opt/rivet,
# /etc/init.d/S80rivet, rivet.toml (only if missing), the [update_manager rivet]
# section of moonraker.conf; other screens' init scripts are renamed with a "_"
# and recorded in /opt/rivet/replaced. Removal puts all of that back.
#
# For tests: MOD (Klipper Mod's root, default /data/.klipper_mod/chroot), USB (the
# stick, default /mnt), FB (default /dev/fb0), NO_REBOOT=1 (return instead of
# waiting and restarting).

MOD="${MOD-/data/.klipper_mod/chroot}"
USB="${USB:-/mnt}"
FB="${FB:-/dev/fb0}"
WORK=/tmp/rivet-usb
LOG="$USB/rivet-install.log"

# Run from /tmp, so the stick can be unmounted while this runs.
if [ "$(dirname "$0")" != "$WORK" ]; then
    rm -rf $WORK && mkdir -p $WORK && cp "$0" $WORK/ && cp -r "$USB/rivet" $WORK/ || {
        echo "rivet: cannot copy the installer to $WORK" >> "$LOG"
        exit 1
    }
    exec /bin/sh $WORK/flashforge_init.sh "$@"
fi
SRC=$WORK/rivet

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') $*" >> "$LOG"; echo "rivet: $*"; }
picture() { [ -f $SRC/$1.fb.gz ] && gzip -dc $SRC/$1.fb.gz > "$FB" 2>/dev/null; }

# Show the result, let the stick go, restart once it is out.
finish() {
    picture "$1"
    sync
    dev=$(awk -v m="$USB" '$2 == m { print $1 }' /proc/mounts | head -n1)
    [ "$NO_REBOOT" = 1 ] && return 0
    umount "$USB" 2>/dev/null
    disk=$(basename "$dev" | sed 's/[0-9]*$//')
    while [ -n "$disk" ] && [ -e /sys/block/$disk ]; do sleep 1; done
    sleep 1
    sync
    reboot -f
    sleep 100000
}

CHANGED=
fail() {
    if [ -n "$CHANGED" ]; then
        log "ERROR: $*. Rivet is only partly installed: run this again, or remove it (rivet/remove)."
    else
        log "ERROR: $*. Nothing was changed."
    fi
    finish error
    exit 1
}

# A file written next to its target, then renamed over it: never half a file,
# and a running binary is replaced without "text file busy".
put() { # put SRC DEST MODE
    cp -f "$1" "$2.new" && chmod "$3" "$2.new" && mv -f "$2.new" "$2"
}

INIT=$MOD/etc/init.d
OPT=$MOD/opt/rivet
CONF=$MOD/root/printer_data/config

install_rivet() {
    for f in rivet rivet.sha256 release_info.json S80rivet rivet.toml.example; do
        [ -f $SRC/$f ] || fail "rivet/$f is missing on the USB stick"
    done
    (cd $SRC && sha256sum -c rivet.sha256 >/dev/null 2>&1) || fail "rivet/rivet does not match rivet.sha256 (copy the files to the stick again)"
    free=$(df -k "$MOD/" | awk 'NR == 2 { print $4 }')
    [ "${free:-0}" -gt 40000 ] || fail "only ${free} KB free on the data partition, 40 MB needed"
    version=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' $SRC/release_info.json)
    log "installing Rivet $version into $MOD"

    CHANGED=1
    mkdir -p $OPT || fail "cannot create $OPT"
    put $SRC/rivet $OPT/rivet 755 || fail "cannot write $OPT/rivet"
    cp -f $SRC/rivet.sha256 $SRC/release_info.json $OPT/
    rm -f $OPT/rolled-back
    put $SRC/S80rivet $INIT/S80rivet 755 || fail "cannot write $INIT/S80rivet"
    if [ -f $CONF/rivet.toml ]; then
        log "kept existing rivet.toml"
    else
        cp -f $SRC/rivet.toml.example $CONF/rivet.toml && log "created rivet.toml"
    fi

    # Other screens: off, by renaming their init scripts (rcS runs only S??*).
    touch $OPT/replaced
    for s in $INIT/S??*; do
        [ -f "$s" ] || continue
        name=$(basename "$s")
        case "$(echo "$name" | tr 'A-Z' 'a-z')" in
            *guppyscreen*|*klipperscreen*|*helixscreen*)
                mv -f "$s" "$INIT/_$name"
                grep -q "^$name " $OPT/replaced || echo "$name running" >> $OPT/replaced
                log "disabled $name (now _$name)" ;;
        esac
    done

    if [ -f $CONF/moonraker.conf ] && ! grep -q '^\[update_manager rivet\]' $CONF/moonraker.conf; then
        cp -f $CONF/moonraker.conf $CONF/moonraker.conf.before-rivet
        cat >> $CONF/moonraker.conf <<'CONF_END'

[update_manager rivet]
# Rivet, the touch screen: releases from GitHub (added by Rivet's installer)
type: web
channel: stable
repo: lDOCI/rivet
path: /opt/rivet
persistent_files:
    rivet.good
    replaced
    rolled-back
CONF_END
        log "added [update_manager rivet] to moonraker.conf"
    fi
    sync
    log "done: Rivet $version starts with Klipper Mod"
}

remove_rivet() {
    log "removing Rivet from $MOD"
    replaced=$(cat $OPT/replaced 2>/dev/null)
    rm -rf $OPT $INIT/S80rivet $INIT/.S80rivet.new
    rm -f $CONF/rivet.toml $CONF/rivet-touch.json
    rm -f $MOD/root/printer_data/logs/rivet.log* $MOD/root/printer_data/logs/rivet-console.log*
    if [ -f $CONF/moonraker.conf ] && grep -q '^\[update_manager rivet\]' $CONF/moonraker.conf; then
        awk '/^\[update_manager rivet\]/{skip=1; next} /^\[/{skip=0} !skip' $CONF/moonraker.conf > $CONF/moonraker.conf.tmp
        sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' $CONF/moonraker.conf.tmp
        mv -f $CONF/moonraker.conf.tmp $CONF/moonraker.conf
        rm -f $CONF/moonraker.conf.before-rivet
        log "removed [update_manager rivet] from moonraker.conf"
    fi
    echo "$replaced" | while read -r name st; do
        [ -n "$name" ] || continue
        if [ -f "$INIT/_$name" ] && [ ! -f "$INIT/$name" ]; then
            mv "$INIT/_$name" "$INIT/$name" && log "enabled $name again"
        fi
    done
    log "Rivet's settings in Moonraker's database (namespace \"rivet\") are left; they do nothing without Rivet"
    sync
    log "done: Rivet removed"
}

[ -d "$MOD/etc/init.d" ] && grep -q '^PRETTY_NAME="Klipper Mod' "$MOD/etc/os-release" 2>/dev/null \
    || fail "Klipper Mod is not installed on this printer (Rivet needs it; ZMOD and the stock firmware are not supported)"

picture installing
if [ -f $SRC/remove ]; then
    remove_rivet
    finish removed
else
    install_rivet
    finish done
fi
exit 0
