"""Streams Reader wordmark and lockups, outlined.

"Streams" is set in Inter (the family's sans, as in the Streams app UI) and
"Reader" in Literata italic (the reading face of the product): the sans is the
parent, the serif is the page.
"""

from __future__ import annotations

from dataclasses import dataclass

from . import typeset
from .svgkit import place, svg

SIZE = 100.0              # design size of the wordmark in px (lockup units)
SANS = ("inter", 640, -0.022)
SERIF = ("literata-italic", 480, -0.01)
WORD_GAP = 0.24           # em between the two words
MARK_TO_CAP = 1.5         # mark height / cap height in the horizontal lockup
MARK_TO_CAP_STACKED = 2.6  # the same in the stacked lockup
GRID = 16                 # the pixel mark fills its 16-unit grid edge to edge
GAP = 0.5                 # mark-to-text gap in cap heights (horizontal)


@dataclass(frozen=True)
class Wordmark:
    body: str     # markup with baseline at y=0, x from 0
    width: float
    cap: float


def wordmark(fill: str) -> Wordmark:
    a = typeset.run("Streams", SANS[0], SIZE, SANS[1], SANS[2])
    b = typeset.run("Reader", SERIF[0], SIZE * 1.04, SERIF[1], SERIF[2])
    gap = WORD_GAP * SIZE
    body = typeset.text_path(a, 0, 0, fill) + typeset.text_path(b, a.width + gap, 0, fill)
    return Wordmark(body, a.width + gap + b.width, a.cap)


def _bounds(cap: float) -> tuple[float, float]:
    """Top and height of the text band (cap plus overshoot and italic descent room)."""
    return -cap * 1.06, cap * 1.12


def wordmark_svg(fill: str) -> str:
    w = wordmark(fill)
    top, h = _bounds(w.cap)
    return svg(w.width, h, w.body, "Streams Reader", f"0 {top:g} {w.width:g} {h:g}")


def horizontal(mark: str, fill: str) -> str:
    """Mark centred on the cap height, then the wordmark. mark: 16-unit pixel markup."""
    w = wordmark(fill)
    box = w.cap * MARK_TO_CAP
    gap = w.cap * GAP
    mark_y = -w.cap / 2 - box / 2
    body = place(mark, 0, mark_y, box, GRID) + f'<g transform="translate({box + gap:g} 0)">{w.body}</g>'
    width = box + gap + w.width
    top = min(mark_y, -w.cap * 1.06)
    height = max(mark_y + box, w.cap * 0.06) - top
    return svg(width, height, body, "Streams Reader", f"0 {top:g} {width:g} {height:g}")


def horizontal_mark_box(fill: str = "#000000") -> tuple[float, float]:
    """(mark side, viewBox height) of the horizontal lockup, to export it with whole-pixel cells."""
    w = wordmark(fill)
    box = w.cap * MARK_TO_CAP
    mark_y = -w.cap / 2 - box / 2
    top = min(mark_y, -w.cap * 1.06)
    return box, max(mark_y + box, w.cap * 0.06) - top


def stacked(mark: str, fill: str) -> str:
    w = wordmark(fill)
    box = w.cap * MARK_TO_CAP_STACKED
    gap = w.cap * 0.55
    width = max(w.width, box)
    mark_x = (width - box) / 2
    baseline = box + gap + w.cap
    body = place(mark, mark_x, 0, box, GRID) + f'<g transform="translate({(width - w.width) / 2:g} {baseline:g})">{w.body}</g>'
    height = baseline + w.cap * 0.06
    return svg(width, height, body, "Streams Reader", f"0 0 {width:g} {height:g}")
