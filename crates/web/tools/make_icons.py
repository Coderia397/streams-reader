# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10"]
# ///
"""Generates the PWA icons in crates/web/assets/icons from the same drawing as components/logo.rs:
a violet rounded square with an open book whose pages carry sound-wave bars.

Usage (from the repo root): uv run --script crates/web/tools/make_icons.py
Writes icon-192.png, icon-512.png, maskable-192.png, maskable-512.png, apple-touch-icon.png, favicon.svg.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent.parent / "assets" / "icons"
SS = 4  # supersampling factor for smooth edges
TOP = (138, 124, 255)  # gradient from a lighter violet …
BOTTOM = (102, 85, 240)  # … to the button violet (#6655F0)
WHITE = (255, 255, 255, 255)


def cubic(p0, p1, p2, p3, steps=24):
    out = []
    for i in range(steps + 1):
        t = i / steps
        mt = 1 - t
        x = mt**3 * p0[0] + 3 * mt**2 * t * p1[0] + 3 * mt * t**2 * p2[0] + t**3 * p3[0]
        y = mt**3 * p0[1] + 3 * mt**2 * t * p1[1] + 3 * mt * t**2 * p2[1] + t**3 * p3[1]
        out.append((x, y))
    return out


# The book outline on a 64-unit grid (same path as the SVG mark).
BOOK = (
    cubic((14, 22), (20, 19.4), (26, 19.4), (32, 23.2))
    + cubic((32, 23.2), (38, 19.4), (44, 19.4), (50, 22))[1:]
    + [(50, 43)]
    + cubic((50, 43), (44, 40.4), (38, 40.4), (32, 44.2))[1:]
    + cubic((32, 44.2), (26, 40.4), (20, 40.4), (14, 43))[1:]
    + [(14, 22)]
)
SPINE = [((32, 23.2), (32, 44.2))]
BARS = [((20, 31), (20, 37)), ((25, 28.5), (25, 39.5)), ((39, 28.5), (39, 39.5)), ((44, 31), (44, 37))]


def gradient(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size))
    px = img.load()
    for y in range(size):
        t = y / max(1, size - 1)
        c = tuple(round(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3)) + (255,)
        for x in range(size):
            px[x, y] = c
    return img


def round_line(draw: ImageDraw.ImageDraw, pts, width: float) -> None:
    draw.line(pts, fill=WHITE, width=round(width), joint="curve")
    r = width / 2
    for x, y in (pts[0], pts[-1]):
        draw.ellipse((x - r, y - r, x + r, y + r), fill=WHITE)


def draw_glyph(img: Image.Image, scale: float, offset: float) -> None:
    draw = ImageDraw.Draw(img)

    def tx(p):
        return (offset + p[0] * scale, offset + p[1] * scale)

    pts = [tx(p) for p in BOOK]
    draw.line(pts, fill=WHITE, width=round(3.2 * scale), joint="curve")
    # Round the four outer corners (the polyline's own joints are only rounded between segments).
    r = 3.2 * scale / 2
    for corner in ((14, 22), (50, 22), (50, 43), (14, 43)):
        x, y = tx(corner)
        draw.ellipse((x - r, y - r, x + r, y + r), fill=WHITE)
    for a, b in SPINE:
        round_line(draw, [tx(a), tx(b)], 3.2 * scale)
    for a, b in BARS:
        round_line(draw, [tx(a), tx(b)], 2.6 * scale)


def icon(px: int, maskable: bool) -> Image.Image:
    size = px * SS
    base = gradient(size)
    if maskable:
        # Full bleed; the glyph stays inside the 80 % safe circle.
        scale = size / 64 * 0.82
        draw_glyph(base, scale, (size - 64 * scale) / 2)
        return base.resize((px, px), Image.LANCZOS)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), radius=size * 16 / 64, fill=255)
    draw_glyph(base, size / 64, 0)
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(base, (0, 0), mask)
    return out.resize((px, px), Image.LANCZOS)


FAVICON = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8A7CFF"/><stop offset="1" stop-color="#6655F0"/></linearGradient></defs>
<rect width="64" height="64" rx="16" fill="url(#g)"/>
<path d="M14 22c6-2.6 12-2.6 18 1.2c6-3.8 12-3.8 18-1.2v21c-6-2.6-12-2.6-18 1.2c-6-3.8-12-3.8-18-1.2z" fill="none" stroke="#fff" stroke-width="3.2" stroke-linejoin="round"/>
<path d="M32 23.2v21" stroke="#fff" stroke-width="3.2" stroke-linecap="round"/>
<path d="M20 31v6M25 28.5v11M39 28.5v11M44 31v6" stroke="#fff" stroke-width="2.6" stroke-linecap="round"/>
</svg>
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    icon(192, False).save(OUT / "icon-192.png", optimize=True)
    icon(512, False).save(OUT / "icon-512.png", optimize=True)
    icon(192, True).save(OUT / "maskable-192.png", optimize=True)
    icon(512, True).save(OUT / "maskable-512.png", optimize=True)
    icon(180, True).convert("RGB").save(OUT / "apple-touch-icon.png", optimize=True)
    (OUT / "favicon.svg").write_text(FAVICON, encoding="utf-8")
    print(f"icons written to {OUT}")


if __name__ == "__main__":
    main()
