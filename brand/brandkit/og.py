"""Open Graph / social image, 1200x630: the product in one sentence."""

from __future__ import annotations

from . import typeset
from .svgkit import svg

W, H = 1200, 630
PAD = 80


def og_image(c: dict[str, str], mark: str, lockup_body: str, lockup_w: float, lockup_h: float, lockup_top: float) -> str:
    """c: hex colours ink, panel, surface, text, text2, wash, pill, ember, blue."""
    head = typeset.run("Hear anything you’d read.", "inter", 66, 640, -0.03)
    sub = typeset.run("Free, open-source read-aloud with natural voices.", "inter", 26, 450, -0.005)
    url = typeset.run("listen.streams.watch", "inter", 22, 550, 0.0)

    # Reading card with a washed sentence and the spoken word.
    size, weight = 30, 420
    before = "The page that "
    word = "speaks"
    after = ", word by word."
    r_before = typeset.run(before, "literata", size, weight)
    r_word = typeset.run(word, "literata", size, weight)
    r_after = typeset.run(after, "literata", size, weight)
    line_w = r_before.width + r_word.width + r_after.width
    card_x, card_y, card_h = PAD, 352, 132
    card_w = round(line_w + 72)
    tx, ty = card_x + 36, card_y + 80
    wash = f'<rect x="{tx - 8:g}" y="{ty - 32:g}" width="{line_w + 16:g}" height="46" rx="8" fill="{c["wash"]}" fill-opacity="0.16"/>'
    wx = tx + r_before.width
    pill = (
        f'<rect x="{wx - 5:g}" y="{ty - 29:g}" width="{r_word.width + 10:g}" height="40" rx="6" fill="{c["pill"]}" fill-opacity="0.28"/>'
        f'<rect x="{wx - 5:g}" y="{ty + 8:g}" width="{r_word.width + 10:g}" height="3" rx="1.5" fill="{c["ember"]}"/>'
    )
    text = (
        typeset.text_path(r_before, tx, ty, c["text"])
        + typeset.text_path(r_word, wx, ty, c["text"])
        + typeset.text_path(r_after, wx + r_word.width, ty, c["text"])
    )
    scale = 48 / lockup_h  # the lockup is as tall as its mark: 48 px = 3 px cells, crisp
    body = (
        "<defs>"
        '<linearGradient id="ogbg" x1="0" y1="0" x2="1" y2="1">'
        f'<stop offset="0" stop-color="{c["panel"]}"/><stop offset="0.55" stop-color="{c["ink"]}"/></linearGradient>'
        '<radialGradient id="ogglow" cx="0.5" cy="0.5" r="0.5">'
        f'<stop offset="0" stop-color="{c["blue"]}" stop-opacity="0.20"/><stop offset="1" stop-color="{c["blue"]}" stop-opacity="0"/></radialGradient>'
        "</defs>"
        f'<rect width="{W}" height="{H}" fill="url(#ogbg)"/>'
        f'<circle cx="1032" cy="300" r="420" fill="url(#ogglow)"/>'
        + big_mark(mark)
        +         f'<g transform="translate({PAD} {PAD}) scale({scale:g}) translate(0 {-lockup_top:g})">{lockup_body}</g>'
        + typeset.text_path(head, PAD, 236, c["text"])
        + typeset.text_path(sub, PAD, 286, c["text2"])
        + f'<rect x="{card_x}" y="{card_y}" width="{card_w}" height="{card_h}" rx="24" fill="{c["surface"]}"/>'
        + f'<rect x="{card_x + 0.5}" y="{card_y + 0.5}" width="{card_w - 1}" height="{card_h - 1}" rx="23.5" fill="none" stroke="{c["border"]}"/>'
        + wash + pill + text
        + typeset.text_path(url, PAD, H - PAD + 8, c["text2"])
    )
    return svg(W, H, body, "Streams Reader")


def big_mark(mark: str, size: float = 288, x: float = 888, y: float = 156) -> str:
    """The large faint pixel mark on the right of the OG image (16-unit markup, 18 px cells):
    x 888-1176, 35 px clear of the headline, 24 px right margin, centred on the glow row."""
    return f'<g transform="translate({x:g} {y:g}) scale({size / 16:g})">{mark}</g>'
