#!/usr/bin/env bash
# Blacken the compared-out regions of a terminal image, from a mock-metrics
# JSON (authored by hand from measurements of the mock).
#
# Comparison keys on furniture, not content, so the same carve-outs are cut
# from mock and screenshot alike: the CRT glass rounded rect (widened by a
# shine band, because the bezel edge beside the well reflects the live glass
# and differs run to run), and every LED window's inner panel (row 1's rect
# stepped down by the measured pitch).
#
# usage: mask.sh <mock-metrics.json> <image.png> <out.png>
set -euo pipefail

JSON=$(realpath "${1:?mock metrics json}")
IMG=$(realpath "${2:?image}")
OUT=$(realpath -m "${3:?output}")

SHINE_BAND=12

python3 - "$JSON" "$IMG" "$OUT" "$SHINE_BAND" <<'PY'
import json, sys
from PIL import Image, ImageDraw

json_path, img_path, out_path, band = sys.argv[1:5]
band = int(band)

with open(json_path) as f:
    m = json.load(f)

im = Image.open(img_path).convert("RGB")
draw = ImageDraw.Draw(im)

carve = m["carveouts"]

g = carve["crt_glass"]
draw.rounded_rectangle(
    [g["x0"] - band, g["y0"] - band, g["x1"] + band, g["y1"] + band],
    radius=g["radius"] + band, fill=(0, 0, 0))

led = carve["led_panels"]
r = led["row1"]
for i in range(led["count"]):
    dy = round(i * led["pitch"])
    draw.rounded_rectangle(
        [r["x0"], r["y0"] + dy, r["x1"], r["y1"] + dy],
        radius=r["radius"], fill=(0, 0, 0))

im.save(out_path)
print(out_path)
PY
