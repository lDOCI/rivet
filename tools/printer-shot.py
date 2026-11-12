#!/usr/bin/env python3
"""Take proof shots of Rivet screens on the printer, straight from /dev/fb0.

    tools/printer-shot.py PRINTER_IP OUT_DIR screen [screen ...]

Rivet must already be on the printer (tools/swap-screen.sh rivet PRINTER_IP). For
each screen this restarts it with `--screen <name>` (Russian), waits for live data,
and saves the page that is on the glass: Rivet flips between two pages of video
memory, so the visible one is read from /sys/class/graphics/fb0/pan.

Needs: ssh access as root, Pillow.
"""
import os
import pathlib, subprocess, sys, time
from PIL import Image

W, H, BPP = 800, 480, 4
PAGE = W * H * BPP


def ssh(host, cmd, binary=False):
    out = subprocess.run(["ssh", "-o", "ConnectTimeout=5", f"root@{host}", cmd],
                         check=True, capture_output=True)
    return out.stdout if binary else out.stdout.decode()


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    host, out, screens = sys.argv[1], pathlib.Path(sys.argv[2]), sys.argv[3:]
    out.mkdir(parents=True, exist_ok=True)
    for screen in screens:
        ssh(host, "kill $(cat /tmp/rivet.pid) 2>/dev/null; sleep 1; "
                  f"nohup env RIVET_LANG=ru /tmp/rivet --screen {screen} >/tmp/rivet.log 2>&1 </dev/null & "
                  "echo $! >/tmp/rivet.pid")
        # SHOT_WAIT: seconds for live data (a Wi-Fi scan needs ~12).
        time.sleep(float(os.environ.get("SHOT_WAIT", "7")))
        raw = ssh(host, "cat /sys/class/graphics/fb0/pan; head -c %d /dev/fb0" % (2 * PAGE), binary=True)
        pan, frame = raw.split(b"\n", 1)
        y = int(pan.decode().split(",")[1])
        page = frame[(y // H) * PAGE:(y // H + 1) * PAGE]
        Image.frombuffer("RGBA", (W, H), page, "raw", "BGRA", 0, 1).convert("RGB").save(out / f"{screen}.png")
        print(f"{screen}: page {y // H}")


if __name__ == "__main__":
    main()
