#!/usr/bin/env python3
"""Draw the RhemaFlow app icons (our own simple mark: a gold serif R on the app's dark colour).
Run once; the PNGs are committed in web/src/icons/.   python3 web/make_icons.py
Font: Liberation Serif Bold (SIL Open Font License 1.1), from crates/rf-app/assets/."""
import pathlib
from PIL import Image, ImageDraw, ImageFont

here = pathlib.Path(__file__).parent
font_path = here.parent / "crates" / "rf-app" / "assets" / "LiberationSerif-Bold.ttf"
BG, GOLD = (13, 16, 23), (224, 178, 90)
out = here / "src" / "icons"
out.mkdir(exist_ok=True)


def icon(size, letter_frac, ring=True):
    img = Image.new("RGB", (size, size), BG)
    d = ImageDraw.Draw(img)
    if ring:
        pad = size * 0.075
        d.ellipse([pad, pad, size - pad, size - pad], outline=GOLD, width=max(2, round(size * 0.02)))
    f = ImageFont.truetype(str(font_path), round(size * letter_frac))
    d.text((size / 2, size / 2), "R", font=f, fill=GOLD, anchor="mm")
    return img


icon(192, 0.60).save(out / "icon-192.png")
icon(512, 0.60).save(out / "icon-512.png")
# maskable: the phone may crop to a circle or squircle, so everything stays inside the middle 60 %
icon(512, 0.40, ring=False).save(out / "icon-maskable-512.png")
icon(180, 0.60).save(out / "apple-touch-icon.png")
print("icons written to", out)
