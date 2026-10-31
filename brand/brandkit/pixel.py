"""The pixel mark: a 16x16 grid (brand/src/mark.txt) turned into crisp SVG geometry.

Grid characters: '#' a lit pixel, 'o' the ember pixel (the spoken word; lit in every
one-colour variant), '.' empty. Lines starting with ';' are comments.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

GRID = 16


@dataclass(frozen=True)
class Grid:
    rows: tuple[str, ...]

    @property
    def size(self) -> int:
        return len(self.rows)

    def cells(self, kinds: str = "#o") -> list[tuple[int, int]]:
        return [(x, y) for y, row in enumerate(self.rows) for x, ch in enumerate(row) if ch in kinds]

    def lit(self, x: int, y: int) -> bool:
        return 0 <= y < self.size and 0 <= x < self.size and self.rows[y][x] in "#o"


def parse(text: str, size: int = GRID) -> Grid:
    rows = tuple(line.strip() for line in text.splitlines() if line.strip() and not line.lstrip().startswith(";"))
    if len(rows) != size or any(len(r) != size for r in rows):
        raise ValueError(f"mark grid must be {size}x{size}, got {[len(r) for r in rows]}")
    bad = {ch for r in rows for ch in r} - set("#o.")
    if bad:
        raise ValueError(f"unknown grid characters {bad}")
    return Grid(rows)


def load(path: Path, size: int = GRID) -> Grid:
    return parse(path.read_text(encoding="utf-8"), size)


def _runs(grid: Grid, kinds: str) -> list[tuple[int, int, int, int]]:
    """Rectangles (x, y, w, h): horizontal runs, merged downwards while identical."""
    rows: list[list[tuple[int, int]]] = []
    for row in grid.rows:
        runs, x = [], 0
        while x < len(row):
            if row[x] in kinds:
                start = x
                while x < len(row) and row[x] in kinds:
                    x += 1
                runs.append((start, x - start))
            else:
                x += 1
        rows.append(runs)
    rects: list[tuple[int, int, int, int]] = []
    open_: dict[tuple[int, int], int] = {}  # (x, w) -> index in rects
    for y, runs in enumerate(rows):
        nxt: dict[tuple[int, int], int] = {}
        for run in runs:
            if run in open_:
                i = open_[run]
                x, yy, w, h = rects[i]
                rects[i] = (x, yy, w, h + 1)
                nxt[run] = i
            else:
                rects.append((run[0], y, run[1], 1))
                nxt[run] = len(rects) - 1
        open_ = nxt
    return rects


def path_d(grid: Grid, kinds: str = "#o", cell: float = 1.0, ox: float = 0.0, oy: float = 0.0) -> str:
    out = []
    for x, y, w, h in _runs(grid, kinds):
        out.append(f"M{ox + x * cell:g} {oy + y * cell:g}h{w * cell:g}v{h * cell:g}h{-w * cell:g}z")
    return "".join(out)


def markup(grid: Grid, fill: str, ember: str | None = None, cell: float = 1.0, ox: float = 0.0,
           oy: float = 0.0, extra: str = "") -> str:
    """Mark geometry as <path> elements. ember=None paints the ember pixel like the rest."""
    crisp = ' shape-rendering="crispEdges"'
    if ember is None:
        return f'<path fill="{fill}"{crisp}{extra} d="{path_d(grid, "#o", cell, ox, oy)}"/>'
    body = f'<path fill="{fill}"{crisp}{extra} d="{path_d(grid, "#", cell, ox, oy)}"/>'
    if grid.cells("o"):
        body += f'<path fill="{ember}"{crisp}{extra} d="{path_d(grid, "o", cell, ox, oy)}"/>'
    return body


def ascii_render(img, threshold: int = 128) -> list[str]:
    """Read an RGBA render back into grid rows by alpha (for tests and checks)."""
    w, h = img.size
    return ["".join("#" if img.getpixel((x, y))[3] >= threshold else "." for x in range(w)) for y in range(h)]
