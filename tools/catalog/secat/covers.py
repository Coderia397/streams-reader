"""Cover thumbnails from SE's compiled cover (src/epub/images/cover.svg: the painting embedded as JPEG plus the
title box with its lettering already turned into paths, so no fonts are needed)."""

from __future__ import annotations

import io
from dataclasses import dataclass

import resvg_py
from PIL import Image

WIDTHS = (200, 400)
ASPECT = 1.5  # 2:3
AVIF_QUALITY = 50
WEBP_QUALITY = 70
MAX_SVG_BYTES = 6 * 1024 * 1024


@dataclass(frozen=True)
class Thumb:
    width: int
    height: int
    fmt: str  # "avif" | "webp"
    data: bytes

    @property
    def content_type(self) -> str:
        return f"image/{self.fmt}"


def render(svg: bytes, widths: tuple[int, ...] = WIDTHS) -> list[Thumb]:
    if len(svg) > MAX_SVG_BYTES:
        raise ValueError("cover SVG too large")
    big_w = max(widths)
    png = resvg_py.svg_to_bytes(
        svg_string=svg.decode("utf-8"), width=big_w, height=round(big_w * ASPECT), skip_system_fonts=True
    )
    base = Image.open(io.BytesIO(bytes(png))).convert("RGB")
    out: list[Thumb] = []
    for w in widths:
        h = round(w * ASPECT)
        img = base if base.size == (w, h) else base.resize((w, h), Image.Resampling.LANCZOS)
        for fmt, q in (("avif", AVIF_QUALITY), ("webp", WEBP_QUALITY)):
            buf = io.BytesIO()
            img.save(buf, fmt.upper(), quality=q)
            out.append(Thumb(w, h, fmt, buf.getvalue()))
    return out
