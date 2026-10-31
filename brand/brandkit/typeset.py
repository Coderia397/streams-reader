"""Set text as outlined SVG paths (no font dependency in the output).

Fonts are the variable Inter and Literata files the web app already ships
(crates/web/assets/fonts); they are read, never modified.
"""

from __future__ import annotations

import io
import re
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

import uharfbuzz as hb
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

FONT_DIR = Path(__file__).resolve().parents[2] / "crates" / "web" / "assets" / "fonts"
FONTS = {
    "inter": "inter-latin-wght-normal.woff2",
    "literata": "literata-latin-wght-normal.woff2",
    "literata-italic": "literata-latin-wght-italic.woff2",
}


@lru_cache(maxsize=None)
def _face(name: str) -> tuple[hb.Face, int, int, int]:
    font = TTFont(FONT_DIR / FONTS[name])
    font.flavor = None
    buf = io.BytesIO()
    font.save(buf)
    os2 = font["OS/2"]
    return hb.Face(buf.getvalue()), font["head"].unitsPerEm, os2.sCapHeight, os2.sxHeight


@dataclass(frozen=True)
class Run:
    path: str      # SVG path data, baseline at y=0, origin at x=0
    width: float   # advance width in px
    cap: float     # cap height in px
    xheight: float


def run(text: str, family: str, size: float, weight: float, tracking: float = 0.0) -> Run:
    """Shape `text` (kerning on) and return its outline. tracking is in em."""
    face, upem, cap, xh = _face(family)
    font = hb.Font(face)
    font.set_variations({"wght": weight})
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(font, buf, {"kern": True, "liga": True, "ss01": False})
    scale = size / upem
    pen = SVGPathPen(None)
    x = 0.0
    track = tracking * upem
    infos, positions = buf.glyph_infos, buf.glyph_positions
    for i, (info, pos) in enumerate(zip(infos, positions)):
        tpen = TransformPen(pen, (scale, 0, 0, -scale, (x + pos.x_offset) * scale, -pos.y_offset * scale))
        font.draw_glyph_with_pen(info.codepoint, tpen)
        x += pos.x_advance + (track if i < len(infos) - 1 else 0)
    return Run(_round(pen.getCommands()), x * scale, cap * scale, xh * scale)


_NUM = re.compile(r"-?\d+\.\d+")


def _round(d: str) -> str:
    def fmt(m: re.Match[str]) -> str:
        v = f"{float(m.group(0)):.2f}".rstrip("0").rstrip(".")
        return "0" if v == "-0" else v

    return _NUM.sub(fmt, d)


def text_path(r: Run, x: float, y: float, fill: str, extra: str = "") -> str:
    return f'<path transform="translate({x:g} {y:g})" fill="{fill}" d="{r.path}"{extra}/>'
