"""Colour parsing, compositing and WCAG 2.1 contrast."""

from __future__ import annotations

import re
from dataclasses import dataclass

_HEX = re.compile(r"^#([0-9a-fA-F]{6})([0-9a-fA-F]{2})?$")


@dataclass(frozen=True)
class Rgba:
    r: int
    g: int
    b: int
    a: float = 1.0

    @classmethod
    def parse(cls, value: str) -> Rgba:
        m = _HEX.match(value.strip())
        if not m:
            raise ValueError(f"not a #RRGGBB or #RRGGBBAA colour: {value!r}")
        h = m.group(1)
        alpha = int(m.group(2), 16) / 255 if m.group(2) else 1.0
        return cls(int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), round(alpha, 4))

    def with_alpha(self, alpha: float) -> Rgba:
        return Rgba(self.r, self.g, self.b, alpha)

    @property
    def hex(self) -> str:
        """#RRGGBB, ignoring alpha."""
        return f"#{self.r:02X}{self.g:02X}{self.b:02X}"

    @property
    def css(self) -> str:
        if self.a >= 1.0:
            return self.hex
        return f"rgba({self.r}, {self.g}, {self.b}, {_fmt_alpha(self.a)})"

    def over(self, base: Rgba) -> Rgba:
        """Composite this colour on an opaque base."""
        a = self.a
        mix = lambda c, d: round(c * a + d * (1 - a))  # noqa: E731
        return Rgba(mix(self.r, base.r), mix(self.g, base.g), mix(self.b, base.b), 1.0)

    def luminance(self) -> float:
        def chan(c: int) -> float:
            s = c / 255
            return s / 12.92 if s <= 0.03928 else ((s + 0.055) / 1.055) ** 2.4

        return 0.2126 * chan(self.r) + 0.7152 * chan(self.g) + 0.0722 * chan(self.b)


def _fmt_alpha(a: float) -> str:
    return f"{a:.3f}".rstrip("0").rstrip(".")


def contrast(fg: Rgba, bg: Rgba) -> float:
    """WCAG 2.1 contrast ratio; translucent colours are composited first."""
    if bg.a < 1.0:
        raise ValueError("background must be opaque; composite it first")
    if fg.a < 1.0:
        fg = fg.over(bg)
    hi, lo = sorted((fg.luminance(), bg.luminance()), reverse=True)
    return (hi + 0.05) / (lo + 0.05)
