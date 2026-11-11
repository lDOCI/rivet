#!/usr/bin/env python3
"""Compare every built screen with its prototype page, pixel by pixel.

    tools/compare-prototype.py --proto http://localhost:8765 --out /tmp/cmp [scene ...]

For each scene it renders our screen (ui-snapshot, Russian) and the prototype
page (headless Chrome, 800x480), writes <out>/<scene>.png as
"ours | prototype | difference" and prints the share of pixels that differ.
The prototype pages need their runtime (support.js) next to them and a
static server, e.g. `python3 -m http.server 8765` in that folder.

Needs: Pillow, numpy, a headless Chrome (CHROME=/path, else Playwright's).
"""
import argparse, glob, os, pathlib, subprocess, sys
import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent

# our snapshot scene -> prototype page
PAIRS = {
    "home": "Main", "printing": "Printing", "done": "PrintDone",
    "tune": "PrintTune", "more": "PrintMore",
    "files": "Files", "file": "FileDetail",
    "control": "Control", "heating": "Heating", "motion": "Motion",
    "extruder": "Extruder", "fans": "Fan", "light": "Light",
    "speed": "Speed", "zoffset": "ZOffset",
    "tools": "Tools", "console": "Console", "macros": "Macros",
    "settings": "Settings", "offline": "Offline", "error": "KlipperError",
}

def chrome():
    if os.environ.get("CHROME"):
        return os.environ["CHROME"]
    found = glob.glob(os.path.expanduser(
        "~/Library/Caches/ms-playwright/chromium_headless_shell-*/*/chrome-headless-shell"))
    if not found:
        sys.exit("no headless Chrome: set CHROME=/path/to/chrome-headless-shell")
    return sorted(found)[-1]

def shot_proto(base, page, dest):
    subprocess.run([chrome(), "--no-sandbox", "--hide-scrollbars", "--window-size=800,480",
                    "--virtual-time-budget=8000", f"--screenshot={dest}", f"{base}/{page}.dc.html"],
                   check=True, capture_output=True)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--proto", default="http://localhost:8765")
    ap.add_argument("--out", default="/tmp/rivet-compare")
    ap.add_argument("scenes", nargs="*")
    args = ap.parse_args()
    scenes = args.scenes or list(PAIRS)
    out = pathlib.Path(args.out); out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, RIVET_LANG="ru")
    subprocess.run(["cargo", "run", "-q", "-p", "ui-snapshot", "--", str(out / "ours")] + scenes,
                   cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
    rows = []
    for s in scenes:
        proto_png = out / f"proto-{s}.png"
        shot_proto(args.proto, PAIRS[s], proto_png)
        a = Image.open(out / "ours" / f"{s}.png").convert("RGB")
        b = Image.open(proto_png).convert("RGB").resize(a.size)
        d = np.abs(np.asarray(a, int) - np.asarray(b, int)).max(axis=2)
        share = float((d > 48).mean())
        heat = Image.fromarray(np.clip(d * 3, 0, 255).astype("uint8")).convert("RGB")
        sheet = Image.new("RGB", (a.width * 3 + 20, a.height), (60, 60, 60))
        for i, im in enumerate((a, b, heat)):
            sheet.paste(im, (i * (a.width + 10), 0))
        sheet.save(out / f"{s}.png")
        rows.append((share, s, PAIRS[s]))
    print(f"{'differ':>7}  scene -> prototype")
    for share, s, p in sorted(rows, reverse=True):
        print(f"{share * 100:6.2f}%  {s} -> {p}")

if __name__ == "__main__":
    main()
