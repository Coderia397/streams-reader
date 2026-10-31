"""Contact sheet: icons, marks, lockups, OG image and spots on Ink and Paper, for review."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from PIL import Image, ImageDraw

from . import icons
from .raster import render
from .tokens import Tokens

if TYPE_CHECKING:
    from .export import Exporter

# The parent brand's tile, shown next to ours when the Streams repo is checked out beside this one.
STREAMS_TILE = Path.home() / "code/pstream/streams-frontend-rs/brand/logo/streams-icon-primary.svg"


def _rgb(hex_: str) -> tuple[int, int, int]:
    h = hex_.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def _circle_mask(img: Image.Image) -> Image.Image:
    s = 4
    mask = Image.new("L", (img.width * s, img.height * s), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, mask.width - 1, mask.height - 1), fill=255)
    mask = mask.resize(img.size, Image.LANCZOS)
    out = Image.new("RGBA", img.size, (0, 0, 0, 0))
    out.paste(img, (0, 0), mask)
    return out


def _adaptive(ex: Exporter, visible: int, themed: bool = False) -> Image.Image:
    """Launcher preview: 108 dp layers, the central 72 dp shown through a circle mask."""
    layer = visible * 108 // 72
    if themed:
        bg = Image.new("RGBA", (layer, layer), (214, 226, 255, 255))
        fg = ex.r(icons.adaptive_monochrome(ex.lk, layer), layer)
        tint = Image.new("RGBA", fg.size, (22, 44, 110, 255))
        tint.putalpha(fg.getchannel("A"))
        fg = tint
    else:
        bg = ex.r(icons.adaptive_background(ex.lk, layer), layer)
        fg = ex.r(icons.adaptive_foreground(ex.lk, layer), layer)
    bg.alpha_composite(fg)
    off = (layer - visible) // 2
    return _circle_mask(bg.crop((off, off, off + visible, off + visible)))


def _fit(path: Path, max_w: int, max_h: int) -> Image.Image:
    head = path.read_text(encoding="utf-8").split('viewBox="', 1)[1].split('"', 1)[0].split()
    w, h = float(head[2]), float(head[3])
    s = min(max_w / w, max_h / h)
    return render(path, round(w * s), round(h * s))


def _panel(t: Tokens, brand: Path, ex: Exporter, theme: str, spots: list[str], width: int) -> Image.Image:
    colors = t.themes[theme].colors
    bg = _rgb(colors["bg"].hex)
    label = _rgb(colors["text-3"].hex)
    pad, big, cell, cols = 32, 192, 120, 10
    rows = (len(spots) + cols - 1) // cols
    heights = [big + 24, 144 + 24, 120, 330 if theme == "ink" else 0, rows * (cell + 26)]
    h = pad + sum(x + pad for x in heights if x)
    img = Image.new("RGBA", (width, h), bg + (255,))
    draw = ImageDraw.Draw(img)

    # Row 1: the master compositions at one size, Streams' tile last for the family check.
    y, x = pad, pad
    row = [
        ("icon", ex.tile(big)),
        ("macOS", ex.r(icons.macos(ex.lk, big), big)),
        ("maskable (80 % circle)", _circle_mask(ex.r(icons.maskable(ex.lk, big), big))),
        ("Android adaptive", _adaptive(ex, big)),
        ("Android themed", _adaptive(ex, big, themed=True)),
    ]
    if STREAMS_TILE.exists():
        row.append(("Streams (parent)", render(STREAMS_TILE, big)))
    for name, im in row:
        img.alpha_composite(im, (x, y))
        draw.text((x, y + big + 6), name, fill=label)
        x += big + 20
    y += big + 24 + pad

    # Row 2: the size ladder as shipped (the tile at every size, the 10x10 reduction at 32
    # and 16), the bare favicon at 16, then 16 px x8.
    x = pad
    for n in (144, 128, 96, 64, 48, 32, 24, 16):
        img.alpha_composite(ex.tile(n), (x, y + 144 - n))
        draw.text((x, y + 150), f"{n}", fill=label)
        x += n + 18
    img.alpha_composite(ex.bare(16), (x, y + 144 - 16))
    draw.text((x, y + 150), "fav", fill=label)
    x += 16 + 18
    img.alpha_composite(ex.bare(16).resize((128, 128), Image.NEAREST), (x + 8, y + 16))
    draw.text((x + 8, y + 150), "16 px, x8", fill=label)
    x += 128 + 34
    img.alpha_composite(ex.r(icons.bare(ex.lk, 128, fill="#FFFFFF" if theme != "paper" else "#000000"), 128), (x, y + 16))
    draw.text((x, y + 150), "one colour", fill=label)
    x += 128 + 26
    img.alpha_composite(ex.r(icons.bare(ex.lk, 128, ember=True), 128), (x, y + 16))
    draw.text((x, y + 150), "-ember (128 px and up)", fill=label)
    y += 144 + 24 + pad

    # Row 3: lockups.
    variant = "on-dark" if theme != "paper" else "on-light"
    lock = _fit(brand / "wordmark" / f"streams-reader-horizontal-{variant}.svg", 560, 64)
    img.alpha_composite(lock, (pad, y + 28))
    stack = _fit(brand / "wordmark" / f"streams-reader-stacked-{variant}.svg", 320, 120)
    img.alpha_composite(stack, (pad + 640, y))
    y += 120 + pad

    # Row 4 (Ink only): the Open Graph image at half size.
    if theme == "ink":
        og = render(brand / "app-icon" / "og-image.svg", 600, 315)
        img.alpha_composite(og, (pad, y))
        draw.text((pad, y + 318), "og-image 1200x630 (half size)", fill=label)
        y += 330 + pad

    # Row 5: spots (unchanged v1 drawings; to be replaced by the voxel mascot renders).
    for i, name in enumerate(spots):
        cx = pad + (i % cols) * (cell + 8)
        cy = y + (i // cols) * (cell + 26)
        path = brand / "spot" / f"{name}.svg"
        if path.exists():
            img.alpha_composite(render(path, cell), (cx, cy))
        draw.text((cx + 4, cy + cell + 4), name, fill=label)
    return img


def build(t: Tokens, brand: Path, ex: Exporter, spots: list[str], out: Path) -> None:
    width = 32 * 2 + 10 * 128 + 120
    panels = [_panel(t, brand, ex, theme, spots, width) for theme in ("ink", "paper")]
    sheet = Image.new("RGBA", (width, sum(p.height for p in panels)))
    y = 0
    for p in panels:
        sheet.alpha_composite(p, (0, y))
        y += p.height
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out, optimize=True)
