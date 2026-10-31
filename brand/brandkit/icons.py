"""App icon compositions for the pixel mark.

Every composition takes the output size in pixels and snaps the mark to whole
pixels: the cell size is an integer and the mark's origin is an integer, so the
16x16 grid stays crisp at every exported size (no resampling of a master).
Geometry numbers are platform template values (noted where used).
"""

from __future__ import annotations

import math
from dataclasses import dataclass

from . import pixel
from .colors import Rgba
from .svgkit import svg

RADIUS = 0.22  # corner radius / side, the Streams icon value (225.28 / 1024)
TITLE = "Streams Reader"
SMALL_GRID = 10  # brand/src/mark-small.txt
SMALL_TILE = {16: 1, 32: 2}  # tile side -> cell of the 10x10 reduction (3 and 6 px margins)


@dataclass(frozen=True)
class Look:
    """Everything a composition needs, read from tokens.toml."""

    grid: pixel.Grid
    white: str
    blue: str          # the mark's own colour on light/dark grounds (blue-500)
    ink: str
    ember: str
    tile: str
    tile_top: str
    tile_bottom: str
    shadow: Rgba
    rim: Rgba
    ratio: float          # mark side / tile side
    shadow_offset: float  # in cells
    small: pixel.Grid | None = None  # 10x10 reduction for the 16 and 32 px tiles
    blue_dark: str = "#6E9BFF"       # favicon.svg colour under prefers-color-scheme: dark (blue-400)


def cell_for(side: float, ratio: float, grid: int = pixel.GRID) -> int:
    """Whole-pixel cell nearest to ratio*side/grid (ties go up: 64 -> 3, 192 -> 8), at least 1."""
    return max(1, math.floor(side * ratio / grid + 0.5))


def _alpha(c: Rgba) -> tuple[str, str]:
    return c.hex, f"{c.a:g}"


def mark_at(look: Look, x: float, y: float, box: float, cell: int, fill: str | None = None,
            shadow: bool = False, ember: bool = False, grid: pixel.Grid | None = None) -> str:
    """The mark (or `grid`) centred in the square box at (x, y), snapped to whole pixels."""
    g = grid or look.grid
    side = cell * g.size
    ox = math.floor(x + (box - side) / 2)
    oy = math.floor(y + (box - side) / 2)
    body = ""
    if shadow and cell >= 4:
        d = max(1, round(cell * look.shadow_offset))
        colour, a = _alpha(look.shadow)
        body += pixel.markup(g, colour, None, cell, ox + d, oy + d, f' fill-opacity="{a}"')
    body += pixel.markup(g, fill or look.white, look.ember if ember else None, cell, ox, oy)
    return body


def squircle_d(cx: float, cy: float, a: float, n: float = 5, samples: int = 256) -> str:
    """Superellipse |x/a|^n + |y/a|^n = 1 as a closed path (Apple's continuous-curvature
    icon body is close to n = 5)."""
    pts = []
    for i in range(samples):
        th = 2 * math.pi * i / samples
        c, s = math.cos(th), math.sin(th)
        px = cx + a * math.copysign(abs(c) ** (2 / n), c)
        py = cy + a * math.copysign(abs(s) ** (2 / n), s)
        pts.append(f"{px:.2f} {py:.2f}")
    return "M" + "L".join(pts) + "Z"


def _ground(look: Look, uid: str, x: float, y: float, side: float, radius: float, rim: bool,
            squircle: bool = False) -> str:
    """Tile body with the faint gradient and the top-light rim. squircle: a superellipse body
    (macOS) instead of the rounded rect."""
    r = f' rx="{radius:g}"' if radius else ""
    defs = (
        f'<defs><linearGradient id="t{uid}" x1="0" y1="0" x2="0" y2="1">'
        f'<stop offset="0" stop-color="{look.tile_top}"/><stop offset="1" stop-color="{look.tile_bottom}"/>'
        "</linearGradient>"
    )
    rim_markup = ""
    if rim and side >= 128:
        colour, a = _alpha(look.rim)
        w = side / 512  # 2 px at 1024
        defs += (
            f'<linearGradient id="r{uid}" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{colour}" stop-opacity="{a}"/>'
            f'<stop offset="0.45" stop-color="{colour}" stop-opacity="0"/></linearGradient>'
        )
        stroke = f' fill="none" stroke="url(#r{uid})" stroke-width="{w:g}"/>'
        if squircle:
            rim_markup = f'<path d="{squircle_d(x + side / 2, y + side / 2, side / 2 - w / 2)}"' + stroke
        else:
            ri = f' rx="{max(radius - w / 2, 0):g}"' if radius else ""
            rim_markup = f'<rect x="{x + w / 2:g}" y="{y + w / 2:g}" width="{side - w:g}" height="{side - w:g}"{ri}' + stroke
    defs += "</defs>"
    if squircle:
        body = f'<path d="{squircle_d(x + side / 2, y + side / 2, side / 2)}" fill="url(#t{uid})"/>'
    else:
        body = f'<rect x="{x:g}" y="{y:g}" width="{side:g}" height="{side:g}"{r} fill="url(#t{uid})"/>'
    return defs + body + rim_markup


def tile(look: Look, px: int = 1024, rounded: bool = True) -> str:
    """Blue tile with the white mark. rounded: 22 % radius (Linux, PWA, Android legacy, desktop);
    square: Windows tiles, apple-touch-icon, Play Store (the platform masks it).
    At 16 and 32 px the 10x10 hand reduction is drawn (1 and 2 px cells), as Streams redraws
    its cube for its small desktop icons; at 24 px the 16 grid on 1 px cells."""
    grid, c = look.grid, cell_for(px, look.ratio)
    if look.small is not None and px in SMALL_TILE:
        grid, c = look.small, SMALL_TILE[px]
    body = _ground(look, "a", 0, 0, px, px * RADIUS if rounded else 0, rim=True) + mark_at(
        look, 0, 0, px, c, shadow=True, grid=grid)
    return svg(px, px, body, TITLE)


def round_tile(look: Look, px: int) -> str:
    """Legacy Android ic_launcher_round: square ground, the circle mask is applied on export.
    Cell = px/24 so the mark's square corners (8*sqrt2 cells from the centre) stay inside the circle."""
    c = max(1, px // 24)
    body = _ground(look, "o", 0, 0, px, 0, rim=False) + mark_at(look, 0, 0, px, c, shadow=True)
    return svg(px, px, body, TITLE)


def macos(look: Look, px: int = 1024) -> str:
    """macOS 1024 template: 824 body at 100 with continuous-curvature corners (superellipse,
    n = 5, a = 412), drop shadow (y 10, blur 10, 30 %)."""
    k = px / 1024
    x, side = 100 * k, 824 * k
    c = cell_for(side, look.ratio)
    flt = (
        '<defs><filter id="sh" x="-20%" y="-20%" width="140%" height="140%">'
        f'<feDropShadow dx="0" dy="{10 * k:g}" stdDeviation="{10 * k:g}" flood-color="#000" flood-opacity="0.3"/>'
        "</filter></defs>"
    )
    body = (
        flt
        + f'<g filter="url(#sh)"><path d="{squircle_d(512 * k, 512 * k, 412 * k)}" fill="{look.tile}"/></g>'
        + _ground(look, "m", x, x, side, 0, rim=True, squircle=True)
        + mark_at(look, x, x, side, c, shadow=True)
    )
    return svg(px, px, body, TITLE)


def maskable(look: Look, px: int = 1024) -> str:
    """PWA maskable: full-bleed ground; the mark's corners (8*sqrt2 cells from the centre)
    inside the central 80 % circle, so cell <= 0.4*px / 11.31."""
    c = max(1, math.floor(px * 0.4 / (8 * math.sqrt(2)) * 0.9))
    body = _ground(look, "k", 0, 0, px, 0, rim=False) + mark_at(look, 0, 0, px, c, shadow=True)
    return svg(px, px, body, TITLE)


def _adaptive_cell(px: int) -> int:
    """108 dp layer: the largest whole-pixel cell that keeps the mark's square corners
    (8*sqrt2 cells from the centre) inside the 66 dp safe-zone circle (r 33 dp):
    mdpi 2, hdpi 4, xhdpi 5, xxhdpi 8, xxxhdpi 11 px (32 to 44 dp marks)."""
    return max(1, math.floor(px * 33 / 108 / (8 * math.sqrt(2))))


def adaptive_foreground(look: Look, px: int = 108) -> str:
    """Android adaptive foreground: white mark plus the flat offset shadow (no filters)."""
    return svg(px, px, mark_at(look, 0, 0, px, _adaptive_cell(px), shadow=True))


def adaptive_background(look: Look, px: int = 108) -> str:
    return svg(px, px, _ground(look, "b", 0, 0, px, 0, rim=False))


def adaptive_monochrome(look: Look, px: int = 108) -> str:
    """Android 13+ themed icon layer: the bare mark in one colour, no shadow; the system tints it."""
    return svg(px, px, mark_at(look, 0, 0, px, _adaptive_cell(px), fill="#000000"))


def bare(look: Look, px: int = 16, fill: str | None = None, ember: bool = False) -> str:
    """The mark alone on transparent, cell = px // 16 (favicons and the 16-32 px icon sizes)."""
    c = max(1, px // pixel.GRID)
    return svg(px, px, mark_at(look, 0, 0, px, c, fill=fill or look.blue, ember=ember), TITLE)


def favicon(look: Look) -> str:
    """SVG favicon: the bare blue mark on the 16 grid, as Streams ships its cube. On dark
    browser chrome it switches to blue-400 (4.5:1 on #35363A; blue-500 is 2.7:1)."""
    body = (
        f"<style>path{{fill:{look.blue}}}@media (prefers-color-scheme:dark){{path{{fill:{look.blue_dark}}}}}</style>"
        f'<path shape-rendering="crispEdges" d="{pixel.path_d(look.grid)}"/>'
    )
    return svg(16, 16, body, TITLE).replace('width="16" height="16"', 'width="64" height="64"', 1)


def splash(look: Look, px: int = 2732) -> str:
    """Square splash (crop-safe centre): the rounded tile at 20 % of the side on flat void."""
    side = round(px * 0.2)
    x = (px - side) // 2
    c = cell_for(side, look.ratio)
    body = (
        f'<rect width="{px}" height="{px}" fill="{look.ink}"/>'
        + _ground(look, "p", x, x, side, side * RADIUS, rim=True)
        + mark_at(look, x, x, side, c, shadow=True)
    )
    return svg(px, px, body, TITLE)


def splash_icon(look: Look, px: int = 288) -> str:
    """Android 12+ splash icon: 288 dp canvas, a 192 dp blue disc (the visible circle) with the
    white mark; cells of 7 dp keep the mark's corners inside the disc."""
    k = px / 288
    c = max(1, math.floor(7 * k))
    body = (
        f'<defs><linearGradient id="ts" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{look.tile_top}"/>'
        f'<stop offset="1" stop-color="{look.tile_bottom}"/></linearGradient></defs>'
        f'<circle cx="{px / 2:g}" cy="{px / 2:g}" r="{96 * k:g}" fill="url(#ts)"/>'
        + mark_at(look, 0, 0, px, c, shadow=True)
    )
    return svg(px, px, body)


def mark_svg(look: Look, fill: str, ember: bool = False, title: str = TITLE) -> str:
    """Standalone mark file: 16-unit viewBox, 256 px default size (as Streams' mark files)."""
    body = pixel.markup(look.grid, fill, look.ember if ember else None)
    return svg(16, 16, body, title).replace('width="16" height="16"', 'width="256" height="256"', 1)
