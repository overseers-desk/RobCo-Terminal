#!/usr/bin/env python3
"""Slice the shell paint out of the two reference mocks.

usage: slice.py <dir holding RoboCo-Amber.png and metalic-blue.png>

Every asset the textured shells composite at runtime is cut here, from the
mock PNGs, at mock scale (1448x1086), into app/qml/shells/<shell>/assets/:

  robco-amber: bank.png (bank column, furniture cleaned off), window*.png
  (row bezels, interiors carved to alpha), key.png (ridged pager key cap),
  frame.png (CRT bezel + chassis margins, glass carved).

  robco-blue: bank.png (chassis column with rail + bracket baked),
  window*.png, knob.png (a slotted screw head, the carriage's knob),
  frame.png (deep bezel + margins, barrel glass carved).

The judge's carve-outs (CRT glass interior, LED window inner panels) are
never baked: those interiors are alpha 0 in every slice that crosses them.
Cleaning uses full-width donor bands from the same mock, mirror-tiled
vertically, so columns stay aligned and no horizontal seam is introduced.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

MOCKS = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
SHELLS = Path(__file__).resolve().parents[2] / "app" / "qml" / "shells"


def rounded_mask(size, box, radius, feather):
    """White inside the rounded rect, feathered edge."""
    m = Image.new("L", size, 0)
    d = ImageDraw.Draw(m)
    d.rounded_rectangle(box, radius=radius, fill=255)
    if feather > 0:
        m = m.filter(ImageFilter.GaussianBlur(feather))
    return m


def tiled_field(donor, height):
    """Mirror-tile a full-width band vertically to the given height."""
    bands = []
    flip = False
    while sum(b.height for b in bands) < height:
        bands.append(donor.transpose(Image.FLIP_TOP_BOTTOM) if flip else donor)
        flip = not flip
    field = Image.new("RGB", (donor.width, sum(b.height for b in bands)))
    y = 0
    for b in bands:
        field.paste(b, (0, y))
        y += b.height
    return field.crop((0, 0, donor.width, height))


def clean(img, donor_box, rects):
    """Replace the given rects with donor-band texture, columns aligned."""
    donor = img.crop(donor_box)
    field = tiled_field(donor, img.height)
    for (x0, y0, x1, y1) in rects:
        img.paste(field.crop((x0, y0, x1, y1)), (x0, y0))
    return img


def clean_banded(img, rects, band_at, band_h, pitch, count, first_cell_y):
    """Replace rects with the mock's own clean inter-row bands, mirror-tiled
    within each row's pitch cell. The visible gaps between windows keep their
    original pixels (they are the bands), and the synthesized rows carry the
    band nearest to them, so the column keeps its own shading and grime."""
    a = np.asarray(img, dtype=np.float32).copy()
    period = 2 * band_h - 2
    for (x0, y0, x1, y1) in rects:
        for y in range(y0, y1):
            i = min(count - 1, max(0, (y - first_cell_y) // pitch))
            b0 = band_at + i * pitch
            r = (y - b0) % period
            row = b0 + (r if r < band_h else period - r)
            row = min(img.height - 1, max(0, row))
            a[y, x0:x1, :] = a[row, x0:x1, :]
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))


def carve(rgba, box, radius, feather):
    """Set alpha 0 inside the rounded rect (the judge's carve-outs)."""
    hole = rounded_mask(rgba.size, box, radius, feather)
    a = np.array(rgba.getchannel("A"), dtype=np.int16)
    a = np.clip(a - np.array(hole, dtype=np.int16), 0, 255).astype(np.uint8)
    rgba.putalpha(Image.fromarray(a))
    return rgba


def with_outer_mask(rgb_crop, radius, feather):
    rgba = rgb_crop.convert("RGBA")
    w, h = rgba.size
    rgba.putalpha(rounded_mask((w, h), (0, 0, w - 1, h - 1), radius, feather))
    return rgba


def amber():
    src = Image.open(MOCKS / "RoboCo-Amber.png").convert("RGB")
    out = SHELLS / "robco-amber" / "assets"
    out.mkdir(parents=True, exist_ok=True)

    # Bank column: plate, screws, moulding and patina stay; windows,
    # numerals, pager labels and key caps are cleaned back to plate.
    bank = src.crop((0, 0, 344, 1086))
    clean(bank, (0, 917, 344, 952), [
        (42, 50, 310, 922),     # numerals + window column
        (40, 946, 300, 980),    # PREV/NEXT labels and arrows
        (62, 978, 140, 1036),   # PREV key cap
        (210, 978, 286, 1036),  # NEXT key cap
    ])
    bank.save(out / "bank.png")

    # Window bezels from three different rows, for per-window irregularity.
    # Outer rect x 92..303, h 44, pitch 45.06 from y 61; 3px margin baked,
    # outer edge feathered into the plate, inner panel carved to alpha.
    for n in (1, 2, 3):
        y0 = 61 + round(45.06 * n)
        w = src.crop((89, y0 - 3, 307, y0 + 47))
        # Inner panel 98..298, y +5..+40 within the window, radius 5.
        w = with_outer_mask(w, radius=11, feather=2.0)
        w = carve(w, (9, 7, 209, 44), 5, 1.0)
        w.save(out / f"window{n}.png")

    # One ridged key cap (PREV's), 3px of its shadow margin kept.
    key = src.crop((69, 983, 132, 1029))
    key = with_outer_mask(key, radius=5, feather=1.5)
    key.save(out / "key.png")

    # The CRT frame: everything right of the bank, glass carved out.
    frame = src.crop((344, 0, 1448, 1086)).convert("RGBA")
    frame = carve(frame, (368 - 344, 22, 1430 - 344, 1068), 55, 1.5)
    frame.save(out / "frame.png")


def blue():
    src = Image.open(MOCKS / "metalic-blue.png").convert("RGB")
    out = SHELLS / "robco-blue" / "assets"
    out.mkdir(parents=True, exist_ok=True)

    # Chassis column: rail, bracket, hinge eyelet and grime stay baked;
    # numerals and windows are cleaned back to chassis. Donor band is the
    # clean strip under row 16.
    bank = src.crop((0, 0, 404, 1086))
    # Bands of bare chassis between the windows (9px each, one per pitch
    # cell) rebuild the column; the bracket's ear keeps its head clear.
    bank = clean_banded(bank,
                        rects=[(119, 86, 394, 160), (106, 160, 394, 1058)],
                        band_at=145, band_h=9, pitch=61, count=16,
                        first_cell_y=95)
    bank.save(out / "bank.png")

    # Window bezels from rows 2 and 3. Outer x 167..383, h 47, pitch 61
    # from y 95; 4px margin, inner panel carved.
    for n in (1, 2):
        y0 = 95 + 61 * n
        w = src.crop((163, y0 - 4, 388, y0 + 51))
        # Inner panel 173..378, y +2..+41 within the window, radius 2.
        w = with_outer_mask(w, radius=6, feather=2.0)
        w = carve(w, (10, 5, 215, 46), 2, 1.0)
        w.save(out / f"window{n}.png")

    # The carriage's knob: the bracket's right screw head, cut round.
    knob = src.crop((89, 102, 116, 129))
    knob = with_outer_mask(knob, radius=13, feather=1.5)
    knob.save(out / "knob.png")

    # The CRT frame: deep bezel and chassis margins, barrel glass carved
    # along the tuned rounded rect (its overshoot lands on the dark wall).
    frame = src.crop((404, 0, 1448, 1086)).convert("RGBA")
    frame = carve(frame, (430 - 404, 18, 1394 - 404, 1046), 110, 1.5)
    frame.save(out / "frame.png")


if __name__ == "__main__":
    amber()
    blue()
    print("sliced into " + str(SHELLS), file=sys.stderr)
