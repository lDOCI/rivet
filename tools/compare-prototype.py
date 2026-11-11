#!/usr/bin/env python3
"""Compare every built screen with its prototype page, pixel by pixel.

    tools/compare-prototype.py [--out DIR] [--max PCT] [scene ...]

For each scene it renders our screen (ui-snapshot, Russian) and takes the
reference picture of its prototype page (docs/ux/reference/<Page>.png, made by
tools/proto-reference.py), writes <out>/<scene>.png as
"ours | prototype | difference" and prints the share of pixels that differ.
With --max it exits non-zero when any scene differs more than PCT percent.

Needs: Pillow, numpy.
"""
import argparse, os, pathlib, subprocess, sys
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
    "motion-more": "MotionAdvanced", "file-actions": "FileActions",
    "layer-pause": "LayerPause", "retraction": "Retraction", "dryer": "Dryer",
    "spools": "Spools", "devices": "Devices", "ambient": "Ambient",
}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max", type=float, default=None, help="fail above this percent")
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
        proto_png = ROOT / "docs/ux/reference" / f"{PAIRS[s]}.png"
        a = Image.open(out / "ours" / f"{s}.png").convert("RGB")
        b = Image.open(proto_png).convert("RGB").resize(a.size)
        d = np.abs(np.asarray(a, int) - np.asarray(b, int)).max(axis=2)
        share = float((d > 12).mean())  # dark cards differ from the background by ~13
        heat = Image.fromarray(np.clip(d * 3, 0, 255).astype("uint8")).convert("RGB")
        sheet = Image.new("RGB", (a.width * 3 + 20, a.height), (60, 60, 60))
        for i, im in enumerate((a, b, heat)):
            sheet.paste(im, (i * (a.width + 10), 0))
        sheet.save(out / f"{s}.png")
        rows.append((share, s, PAIRS[s]))
    print(f"{'differ':>7}  scene -> prototype")
    for share, s, p in sorted(rows, reverse=True):
        print(f"{share * 100:6.2f}%  {s} -> {p}")
    if args.max is not None:
        bad = [s for share, s, _ in rows if share * 100 > args.max]
        if bad:
            sys.exit(f"over {args.max}%: {', '.join(bad)}")

if __name__ == "__main__":
    main()
