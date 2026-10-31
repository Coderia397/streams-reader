"""Generate every SVG source from brand/src and tokens.toml."""

from __future__ import annotations

from pathlib import Path

from . import icons, og, pixel, wordmark
from .icons import SMALL_GRID
from .svgkit import colour, inner, load, svg
from .tokens import Tokens

def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def look(t: Tokens, brand: Path) -> icons.Look:
    c = t.art_colors
    return icons.Look(
        grid=pixel.load(brand / "src" / "mark.txt"),
        small=pixel.load(brand / "src" / "mark-small.txt", SMALL_GRID),
        white=c["white"].hex, blue=c["blue"].hex, ink=c["ink"].hex, ember=c["ember"].hex,
        tile=c["tile"].hex, tile_top=c["tile-top"].hex, tile_bottom=c["tile-bottom"].hex,
        shadow=c["tile-shadow"], rim=c["tile-rim"],
        ratio=float(t.art["mark-on-tile"]), shadow_offset=float(t.art["shadow-offset"]),
        blue_dark=c["blue-soft"].hex,
    )


def marks(t: Tokens, brand: Path, lk: icons.Look) -> dict[str, str]:
    """Mark files. Every variant is one flat colour; only -ember shows the spoken-word pixel."""
    out = brand / "mark"
    files = {
        "streams-reader-mark.svg": icons.mark_svg(lk, lk.blue),
        "streams-reader-mark-white.svg": icons.mark_svg(lk, "#FFFFFF"),
        "streams-reader-mark-black.svg": icons.mark_svg(lk, "#000000"),
        "streams-reader-mark-currentcolor.svg": icons.mark_svg(lk, "currentColor"),
        "streams-reader-mark-ember.svg": icons.mark_svg(lk, lk.blue, ember=True),
        "safari-pinned-tab.svg": icons.mark_svg(lk, "#000000"),
    }
    for name, text in files.items():
        _write(out / name, text)
    # 16-unit markup for embedding in lockups and the OG image.
    return {
        "blue": pixel.markup(lk.grid, lk.blue),
        "white": pixel.markup(lk.grid, "#FFFFFF"),
        "black": pixel.markup(lk.grid, "#000000"),
        "faint": pixel.markup(lk.grid, "#FFFFFF", extra=' fill-opacity="0.05"'),
    }


def app_icons(t: Tokens, brand: Path, lk: icons.Look) -> None:
    """1024 masters (108 dp for the Android layers, 288 dp splash icon, 2732 splash).
    The raster exports call the same functions at each target size instead of scaling these."""
    out = brand / "app-icon"
    files = {
        "icon.svg": icons.tile(lk),
        "icon-square.svg": icons.tile(lk, rounded=False),
        "icon-macos.svg": icons.macos(lk),
        "icon-maskable.svg": icons.maskable(lk),
        "android-adaptive-foreground.svg": icons.adaptive_foreground(lk),
        "android-adaptive-background.svg": icons.adaptive_background(lk),
        "android-adaptive-monochrome.svg": icons.adaptive_monochrome(lk),
        "favicon.svg": icons.favicon(lk),
        "splash.svg": icons.splash(lk),
        "splash-icon.svg": icons.splash_icon(lk),
    }
    for name, text in files.items():
        _write(out / name, text)


def lockups(t: Tokens, brand: Path, m: dict[str, str]) -> None:
    out = brand / "wordmark"
    variants = {
        "on-dark": (m["blue"], "#FFFFFF"),
        "on-light": (m["blue"], t.palette["void"].hex),
        "mono-white": (m["white"], "#FFFFFF"),
        "mono-black": (m["black"], "#000000"),
    }
    for name, (mark, fill) in variants.items():
        _write(out / f"streams-reader-horizontal-{name}.svg", wordmark.horizontal(mark, fill))
        _write(out / f"streams-reader-stacked-{name}.svg", wordmark.stacked(mark, fill))
        _write(out / f"streams-reader-wordmark-{name}.svg", wordmark.wordmark_svg(fill))


def og_image(t: Tokens, brand: Path, m: dict[str, str]) -> None:
    ink = t.themes["ink"].colors
    c = {
        "ink": t.art_colors["ink"].hex, "panel": t.art_colors["panel"].hex, "blue": t.art_colors["blue"].hex,
        "surface": ink["surface-1"].hex, "border": ink["border"].hex, "text": ink["text"].hex,
        "text2": ink["text-2"].hex, "wash": t.art_colors["blue"].hex, "pill": t.art_colors["ember"].hex,
        "ember": t.art_colors["ember"].hex,
    }
    # Lockup geometry from the horizontal SVG's viewBox.
    lock = wordmark.horizontal(m["blue"], "#FFFFFF")
    vb = lock.split('viewBox="', 1)[1].split('"', 1)[0].split()
    body = lock.split("</title>", 1)[1].rsplit("</svg>", 1)[0]
    text = og.og_image(c, m["faint"], body, float(vb[2]), float(vb[3]), float(vb[1]))
    _write(brand / "app-icon" / "og-image.svg", text)


def build_all(t: Tokens, brand: Path) -> None:
    lk = look(t, brand)
    m = marks(t, brand, lk)
    app_icons(t, brand, lk)
    lockups(t, brand, m)
    og_image(t, brand, m)
    print("art -> brand/mark, brand/app-icon, brand/wordmark")


__all__ = ["build_all", "look"]
