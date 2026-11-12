#!/usr/bin/env bash
# Subset Inter to the scripts Rivet needs, to keep the printer binary small.
#
# Usage: tools/subset-fonts.sh <dir with original Inter-*.ttf>
#   Originals: Inter release zip, extras/ttf/ (https://github.com/rsms/inter/releases)
# Requires: pyftsubset (pip install fonttools)
set -euo pipefail

SRC=${1:?usage: $0 <dir with original Inter-*.ttf>}
OUT=$(cd "$(dirname "$0")/.." && pwd)/assets/fonts

# Basic Latin, Latin-1, Latin Extended-A, Cyrillic, general punctuation,
# superscripts (°C), currency, letterlike (№), fractions (screw turns: ⅓ ⅛),
# arrows, math operators (−, ±, ≤, ≥, ∞), keys (⌫ ⏎), ■ ● ★, ✓ ✗.
UNICODES="U+0020-007E,U+00A0-00FF,U+0100-017F,U+0400-045F,U+0490-0491,\
U+2000-206F,U+2070-209F,U+20A0-20BF,U+2116,U+2122,U+2153-215E,U+2190-21FF,\
U+2212-2215,U+221E,U+2248,U+2260-2265,U+232B,U+23CE,U+25A0,U+25CF,U+2605,\
U+2713,U+2717,U+FFFD"

# kern/liga/calt for normal text, tnum for digits that don't jump (temperatures).
FEATURES="kern,liga,calt,tnum,case,ccmp,locl,mark,mkmk"

for weight in Regular Medium SemiBold Bold ExtraBold; do
  pyftsubset "$SRC/Inter-$weight.ttf" \
    --unicodes="$UNICODES" \
    --layout-features="$FEATURES" \
    --no-hinting \
    --desubroutinize \
    --output-file="$OUT/Inter-$weight.ttf"
  # The subset keeps only the legacy names, where Medium/SemiBold/ExtraBold are
  # families of their own ("Inter SemiBold"): then `font-family: Inter` with weight
  # 600 never finds them. Make every file family "Inter"; the weight class picks it.
  python3 - "$OUT/Inter-$weight.ttf" <<'PY'
import sys
from fontTools.ttLib import TTFont
path = sys.argv[1]
f = TTFont(path)
name = f["name"]
style = "Bold" if f["OS/2"].usWeightClass == 700 else "Regular"
for rec in list(name.names):
    if rec.nameID in (16, 17):
        name.removeNames(nameID=rec.nameID)
name.setName("Inter", 1, 3, 1, 0x409)
name.setName(style, 2, 3, 1, 0x409)
f.save(path)
PY
  printf '%-10s %7d bytes\n' "$weight" "$(wc -c < "$OUT/Inter-$weight.ttf")"
done
