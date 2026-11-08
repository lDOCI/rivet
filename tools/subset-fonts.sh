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
# superscripts (°C), currency, letterlike (№), arrows, math operators (−, ×, ±, ≤, ≥).
UNICODES="U+0020-007E,U+00A0-00FF,U+0100-017F,U+0400-045F,U+0490-0491,\
U+2000-206F,U+2070-209F,U+20A0-20BF,U+2116,U+2122,U+2190-21FF,U+2212-2215,\
U+2248,U+2260-2265,U+FFFD"

# kern/liga/calt for normal text, tnum for digits that don't jump (temperatures).
FEATURES="kern,liga,calt,tnum,case,ccmp,locl,mark,mkmk"

for weight in Regular SemiBold Bold; do
  pyftsubset "$SRC/Inter-$weight.ttf" \
    --unicodes="$UNICODES" \
    --layout-features="$FEATURES" \
    --no-hinting \
    --desubroutinize \
    --output-file="$OUT/Inter-$weight.ttf"
  printf '%-10s %7d bytes\n' "$weight" "$(wc -c < "$OUT/Inter-$weight.ttf")"
done
