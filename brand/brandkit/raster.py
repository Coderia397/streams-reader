"""SVG to PNG through resvg (no system fonts: every SVG here is outlined)."""

from __future__ import annotations

import io
from pathlib import Path

import resvg_py
from PIL import Image


def render(svg: Path | str, width: int, height: int | None = None) -> Image.Image:
    kwargs = {"svg_path": str(svg)} if isinstance(svg, Path) else {"svg_string": svg}
    data = resvg_py.svg_to_bytes(width=width, height=height or width, skip_system_fonts=True, **kwargs)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA")


def save_png(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)
