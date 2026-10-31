"""Colour SVG templates from tokens: class f-<name>/s-<name> -> fill/stroke."""

from __future__ import annotations

import copy
import re
import xml.etree.ElementTree as ET
from pathlib import Path

from .colors import Rgba

NS = "http://www.w3.org/2000/svg"
ET.register_namespace("", NS)

# Mono (currentColor) mapping: art colour -> (paint, opacity). Light fills become the
# host surface so shapes still occlude; ember stays the spoken-word colour.
MONO = {
    "ink": ("currentColor", 1.0),
    "panel": ("currentColor", 1.0),
    "blue": ("currentColor", 1.0),
    "blue-deep": ("currentColor", 1.0),
    "blue-night": ("currentColor", 1.0),
    "blue-soft": ("currentColor", 0.55),
    "blue-pale": ("currentColor", 0.28),
    "blue-mist": ("currentColor", 0.14),
    "plate": ("currentColor", 0.07),
    "ember": ("var(--color-ember, #FFB45C)", 1.0),
    "ember-pale": ("var(--color-ember, #FFB45C)", 0.5),
    "ember-deep": ("var(--color-ember, #FFB45C)", 1.0),
    "paper": ("var(--color-surface-1, Canvas)", 1.0),
    "white": ("var(--color-surface-1, Canvas)", 1.0),
}

_CLASS = re.compile(r"^([fs])-([a-z0-9-]+)$")


def load(path: Path) -> ET.Element:
    return ET.parse(path).getroot()


def _paint(el: ET.Element, kind: str, value: str, opacity: float, use_style: bool) -> None:
    attr = "fill" if kind == "f" else "stroke"
    if use_style:
        style = el.get("style", "")
        style += f"{attr}:{value};"
        if opacity < 1:
            style += f"{attr}-opacity:{opacity:g};"
        el.set("style", style)
    else:
        el.set(attr, value)
        if opacity < 1:
            el.set(f"{attr}-opacity", f"{opacity:g}")


def colour(root: ET.Element, palette: dict[str, Rgba], strokes: dict[str, float], mono: bool = False) -> ET.Element:
    """Return a copy with every class token turned into paint attributes."""
    out = copy.deepcopy(root)
    for el in out.iter():
        sw = el.attrib.pop("data-stroke", None)
        if sw is not None:
            el.set("stroke-width", f"{strokes[sw]:g}")
        cls = el.attrib.pop("class", None)
        if not cls:
            continue
        keep = []
        for token in cls.split():
            m = _CLASS.match(token)
            if not m:
                keep.append(token)
                continue
            kind, name = m.groups()
            if name not in palette:
                raise KeyError(f"unknown art colour {name!r} in class {token!r}")
            if mono:
                paint, alpha = MONO[name]
                if paint.startswith("var("):
                    _paint(el, kind, paint, alpha, use_style=True)
                else:
                    _paint(el, kind, paint, alpha, use_style=False)
            else:
                c = palette[name]
                _paint(el, kind, c.hex, c.a, use_style=False)
        if keep:
            el.set("class", " ".join(keep))
    _strip_comments(out)
    return out


def _strip_comments(root: ET.Element) -> None:
    # ElementTree drops comments on parse by default; kept for clarity if a parser keeps them.
    for parent in root.iter():
        for child in list(parent):
            if child.tag is ET.Comment:
                parent.remove(child)


def tostring(root: ET.Element) -> str:
    return ET.tostring(root, encoding="unicode", short_empty_elements=True)


def inner(root: ET.Element) -> str:
    """Children of the root <svg> as markup, for embedding in another drawing."""
    return "".join(tostring(child) for child in root if child.tag != f"{{{NS}}}title")


def svg(width: float, height: float, body: str, title: str | None = None, view: str | None = None) -> str:
    vb = view or f"0 0 {width:g} {height:g}"
    t = f"<title>{title}</title>" if title else ""
    return (
        f'<svg xmlns="{NS}" width="{width:g}" height="{height:g}" viewBox="{vb}">{t}{body}</svg>\n'
    )


def place(inner_markup: str, x: float, y: float, size: float, grid: float = 24) -> str:
    s = size / grid
    return f'<g transform="translate({x:g} {y:g}) scale({s:g})">{inner_markup}</g>'
