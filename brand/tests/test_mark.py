"""Mark and icon tests (D17 pixel mark).

Run with the render dependencies:
    uv run --no-project --python 3.12 --with resvg-py==0.2.6 --with pillow==11.3.0 --with "fonttools[woff]==4.59.0" --with uharfbuzz==0.51.7 -m unittest discover -s brand/tests
Without them the render and export checks are skipped and the grid checks still run.
"""

from __future__ import annotations

import colorsys
import re
import sys

sys.dont_write_bytecode = True
import unittest  # noqa: E402
from pathlib import Path  # noqa: E402

BRAND = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BRAND))

from brandkit import icons, pixel  # noqa: E402
from brandkit.tokens import load  # noqa: E402

try:  # render checks need resvg + Pillow
    from PIL import Image

    from brandkit import art
    from brandkit.raster import render

    HAVE_RENDER = True
except ImportError:  # pragma: no cover
    HAVE_RENDER = False

GRID = pixel.load(BRAND / "src" / "mark.txt")
SMALL = pixel.load(BRAND / "src" / "mark-small.txt", icons.SMALL_GRID)
TOKENS = load(BRAND / "tokens.toml")
EMBER = TOKENS.art_colors["ember"].hex.upper()

# The Streams cube's outline cells shared by the Reader mark (decoded from
# streams-frontend-rs/brand/logo/streams-mark-white.svg on the 16 grid).
STREAMS_SHARED = (
    [(x, 0) for x in range(3, 13)]               # back line on row 0
    + [(2, 1), (1, 2)]                           # two-step top-left diagonal
    + [(0, y) for y in range(3, 16)]             # 13-row front face, left edge
    + [(x, 15) for x in range(0, 13)]            # bottom edge to column 12
    + [(15, y) for y in range(3, 13)]            # right depth line
    + [(14, 13), (13, 14)]                       # three-step bottom-right diagonal into (12, 15)
)


def _components(grid: pixel.Grid) -> list[set[tuple[int, int]]]:
    """8-connected groups of lit pixels."""
    left = set(grid.cells())
    groups = []
    while left:
        stack = [left.pop()]
        group = set(stack)
        while stack:
            x, y = stack.pop()
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (x + dx, y + dy)
                    if n in left:
                        left.remove(n)
                        group.add(n)
                        stack.append(n)
        groups.append(group)
    return groups


class GridTests(unittest.TestCase):
    def test_sixteen_square_with_one_ember_pixel(self) -> None:
        self.assertEqual(GRID.size, 16)
        self.assertEqual(len(GRID.cells("o")), 1)

    def test_shares_the_streams_cube_outline(self) -> None:
        missing = [c for c in STREAMS_SHARED if not GRID.lit(*c)]
        self.assertEqual(missing, [])
        # The chamfered corners stay empty, as on the cube.
        for c in [(0, 0), (1, 1), (0, 2), (15, 15), (14, 14), (15, 13)]:
            self.assertFalse(GRID.lit(*c), c)

    def test_parts_never_touch(self) -> None:
        """Body, three text lines and the inner arc are separate 8-connected parts, so the
        voice arc keeps an empty pixel between itself, the page edge and the outer arc."""
        groups = sorted(_components(GRID), key=len, reverse=True)
        self.assertEqual(len(groups), 5)
        arc = next(g for g in groups if (13, 8) in g)
        self.assertEqual(sorted(y for _, y in arc), list(range(4, 12)))
        self.assertTrue(all(x in (12, 13) for x, _ in arc))
        # Concentric with the outer arc: both symmetric about row 7.5.
        outer = [y for y in range(16) if GRID.lit(15, y)]
        self.assertEqual(min(outer) + max(outer), 15)
        self.assertEqual(min(y for _, y in arc) + max(y for _, y in arc), 15)
        self.assertEqual(sorted(y for x, y in arc if x == 12), [4, 11])

    def test_text_lines_differ_in_length(self) -> None:
        lines = [len(g) for g in _components(GRID) if len({y for _, y in g}) == 1]
        self.assertEqual(len(lines), 3)
        self.assertEqual(len(set(lines)), 3)

    def test_small_reduction(self) -> None:
        """10x10 for the 16/32 px desktop tiles: chamfered corners, spine, two text lines of
        different length, outer arc symmetric about row 4.5, no voice arc."""
        self.assertEqual(SMALL.size, 10)
        for c in [(0, 0), (1, 0), (0, 1), (9, 0), (9, 1), (9, 8), (9, 9), (8, 9)]:
            self.assertFalse(SMALL.lit(*c), c)
        self.assertTrue(all(SMALL.lit(2, y) for y in range(2, 10)))
        arc = [y for y in range(10) if SMALL.lit(9, y)]
        self.assertEqual(min(arc) + max(arc), 9)
        groups = _components(SMALL)
        self.assertEqual(len(groups), 3)
        lines = sorted(len(g) for g in groups if len({y for _, y in g}) == 1)
        self.assertEqual(lines, [1, 2])

    def test_parse_rejects_bad_grids(self) -> None:
        with self.assertRaises(ValueError):
            pixel.parse("#" * 16)
        with self.assertRaises(ValueError):
            pixel.parse("\n".join(["x" * 16] * 16))


class GeometryTests(unittest.TestCase):
    def test_whole_pixel_cells_on_the_master_sizes(self) -> None:
        ratio = float(TOKENS.art["mark-on-tile"])
        self.assertEqual([icons.cell_for(n, ratio) for n in (1024, 512, 256, 128)], [40, 20, 10, 5])

    def test_ties_round_up_so_the_ratio_stays_steady(self) -> None:
        ratio = float(TOKENS.art["mark-on-tile"])
        self.assertEqual([icons.cell_for(n, ratio) for n in (24, 48, 64, 96, 192)], [1, 2, 3, 4, 8])
        for n in (48, 64, 96, 128, 192, 256, 512, 1024):
            share = 16 * icons.cell_for(n, ratio) / n
            self.assertGreaterEqual(share, 0.6, n)
            self.assertLessEqual(share, 0.75, n)

    def test_adaptive_mark_inside_the_safe_zone(self) -> None:
        cells = [icons._adaptive_cell(round(108 * k)) for k in (1, 1.5, 2, 3, 4)]
        self.assertEqual(cells, [2, 4, 5, 8, 11])
        for k, c in zip((1, 1.5, 2, 3, 4), cells):
            corner_dp = 8 * 2 ** 0.5 * c / k
            self.assertLessEqual(corner_dp, 33, k)

    def test_maskable_and_round_marks_stay_inside_their_circles(self) -> None:
        corner = 8 * 2 ** 0.5  # the mark's square corners, in cells from the centre
        for px in (192, 512, 1024):
            svg = icons.maskable(art.look(TOKENS, BRAND) if HAVE_RENDER else _look(), px)
            cell = _cell_of(svg)
            self.assertLessEqual(corner * cell, 0.4 * px, px)
        for px in (48, 72, 96, 144, 192):
            self.assertLessEqual(corner * max(1, px // 24), px / 2, px)

    def test_tile_gradient_is_faint(self) -> None:
        def light(name: str) -> float:
            c = TOKENS.art_colors[name]
            return colorsys.rgb_to_hls(c.r / 255, c.g / 255, c.b / 255)[1]

        self.assertLessEqual(abs(light("tile-top") - light("tile-bottom")), 0.03)


def _look() -> icons.Look:
    c = TOKENS.art_colors
    return icons.Look(GRID, "#FFFFFF", c["blue"].hex, c["ink"].hex, c["ember"].hex, c["tile"].hex,
                      c["tile-top"].hex, c["tile-bottom"].hex, c["tile-shadow"], c["tile-rim"],
                      float(TOKENS.art["mark-on-tile"]), float(TOKENS.art["shadow-offset"]))


def _cell_of(svg: str) -> int:
    """Cell size of the white mark path: the height of its first rectangle (row 0 run)."""
    d = re.search(r'fill="#FFFFFF" shape-rendering="crispEdges" d="M[\d.]+ [\d.]+h[\d.]+v([\d.]+)', svg)
    assert d, "no white mark path"
    return int(float(d.group(1)))


class SourceFileTests(unittest.TestCase):
    """The generated SVG sources (run brand/build.py first)."""

    def test_one_colour_variants_have_one_colour(self) -> None:
        expect = {
            "streams-reader-mark.svg": {TOKENS.art_colors["blue"].hex.upper()},
            "streams-reader-mark-white.svg": {"#FFFFFF"},
            "streams-reader-mark-black.svg": {"#000000"},
            "streams-reader-mark-currentcolor.svg": {"CURRENTCOLOR"},
            "safari-pinned-tab.svg": {"#000000"},
        }
        for name, colours in expect.items():
            text = (BRAND / "mark" / name).read_text()
            self.assertEqual({f.upper() for f in re.findall(r'fill="([^"]+)"', text)}, colours, name)

    def test_ember_only_in_the_ember_variant(self) -> None:
        self.assertIn(EMBER, (BRAND / "mark" / "streams-reader-mark-ember.svg").read_text().upper())
        for folder in ("mark", "app-icon"):
            for f in (BRAND / folder).glob("*.svg"):
                if f.name in ("streams-reader-mark-ember.svg", "og-image.svg"):
                    continue
                self.assertNotIn(EMBER, f.read_text().upper(), f.name)

    def test_android_layers_have_no_filters_and_monochrome_no_shadow(self) -> None:
        for name in ("android-adaptive-foreground.svg", "android-adaptive-monochrome.svg"):
            text = (BRAND / "app-icon" / name).read_text()
            self.assertNotIn("filter", text, name)
        mono = (BRAND / "app-icon" / "android-adaptive-monochrome.svg").read_text()
        self.assertEqual(mono.count("<path"), 1)
        self.assertNotIn("opacity", mono)
        self.assertEqual(_cell_of((BRAND / "app-icon" / "android-adaptive-foreground.svg").read_text()), 2)

    def test_favicon_svg_switches_to_blue_400_on_dark(self) -> None:
        text = (BRAND / "app-icon" / "favicon.svg").read_text()
        self.assertIn("prefers-color-scheme:dark", text)
        self.assertIn(TOKENS.palette["blue-400"].hex.upper(), text.upper())
        self.assertNotIn('fill="', text)

    def test_og_big_mark_clears_the_headline_and_the_edge(self) -> None:
        text = (BRAND / "app-icon" / "og-image.svg").read_text()
        x, y, s = map(float, re.search(r'translate\(([\d.]+) ([\d.]+)\) scale\(([\d.]+)\)">' + '<path fill="#FFFFFF"', text).groups())
        self.assertEqual((x, y, 16 * s), (888, 156, 288))
        self.assertLessEqual(x + 16 * s, 1200 - 24)
        self.assertEqual(y + 8 * s, 300)

    def test_macos_body_is_a_superellipse(self) -> None:
        text = (BRAND / "app-icon" / "icon-macos.svg").read_text()
        self.assertNotIn("rx=", text)
        self.assertGreaterEqual(text.count("<path d=\"M"), 3)  # shadow, body, rim


@unittest.skipUnless(HAVE_RENDER, "resvg-py and pillow not installed")
class RenderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.look = art.look(TOKENS, BRAND)

    def test_bare_mark_is_exact_at_16_32_48_on_void_and_paper(self) -> None:
        blue = TOKENS.art_colors["blue"]
        for px in (16, 32, 48):
            img = render(icons.bare(self.look, px), px)
            alphas = set(img.getchannel("A").getdata())
            self.assertLessEqual(alphas, {0, 255}, f"anti-aliased pixels at {px}")
            k = px // 16
            down = img.resize((16, 16), Image.NEAREST) if k > 1 else img
            self.assertEqual(pixel.ascii_render(down), [r.replace("o", "#") for r in GRID.rows], px)
            for bg in (TOKENS.palette["void"], TOKENS.palette["paper"]):
                ground = Image.new("RGBA", img.size, (bg.r, bg.g, bg.b, 255))
                ground.alpha_composite(img)
                colours = {c[:3] for c in ground.getdata()}
                self.assertEqual(colours, {(bg.r, bg.g, bg.b), (blue.r, blue.g, blue.b)})

    def test_small_tiles_use_the_reduction_with_margins(self) -> None:
        for px, cell in ((16, 1), (32, 2)):
            img = render(icons.tile(self.look, px), px)
            whites = [(x, y) for y in range(px) for x in range(px) if img.getpixel((x, y))[:3] == (255, 255, 255)]
            xs, ys = [x for x, _ in whites], [y for _, y in whites]
            m = (px - 10 * cell) // 2
            self.assertEqual((min(xs), min(ys), max(xs), max(ys)), (m, m, px - m - 1, px - m - 1), px)
            self.assertEqual(img.getpixel((px // 2, 1))[3], 255)  # solid tile, not a bare glyph
        img = render(icons.tile(self.look, 24), 24)
        whites = [(x, y) for y in range(24) for x in range(24) if img.getpixel((x, y))[:3] == (255, 255, 255)]
        self.assertEqual(min(x for x, _ in whites), 4)

    def test_tile_pixels_are_crisp_white_at_128_and_1024(self) -> None:
        for px in (128, 1024):
            img = render(icons.tile(self.look, px), px)
            cell = icons.cell_for(px, self.look.ratio)
            o = (px - 16 * cell) // 2
            for x, y in GRID.cells():
                for dx, dy in ((0, 0), (cell - 1, cell - 1)):
                    self.assertEqual(img.getpixel((o + x * cell + dx, o + y * cell + dy))[:3], (255, 255, 255))


@unittest.skipUnless(HAVE_RENDER and (BRAND / "dist" / "web" / "favicon.ico").exists(), "run brand/build.py first")
class ExportTests(unittest.TestCase):
    def size(self, rel: str) -> tuple[int, int]:
        with Image.open(BRAND / "dist" / rel) as im:
            return im.size

    def test_platform_files_exist_with_their_sizes(self) -> None:
        expect = {
            "web/apple-touch-icon.png": (180, 180),
            "web/icon-192.png": (192, 192),
            "web/icon-512.png": (512, 512),
            "web/icon-maskable-512.png": (512, 512),
            "web/og-image.png": (1200, 630),
            "android/mipmap-xxxhdpi/ic_launcher.png": (192, 192),
            "android/mipmap-xxxhdpi/ic_launcher_foreground.png": (432, 432),
            "android/mipmap-mdpi/ic_launcher_round.png": (48, 48),
            "android/playstore-icon.png": (512, 512),
            "tauri/32x32.png": (32, 32),
            "tauri/128x128@2x.png": (256, 256),
            "tauri/icon.png": (512, 512),
            "windows/Square150x150Logo.png": (150, 150),
            "linux/hicolor/256x256/apps/streams-reader.png": (256, 256),
            "email/streams-reader-mark@2x.png": (96, 96),
        }
        for rel, size in expect.items():
            self.assertEqual(self.size(rel), size, rel)
        for rel in ("macos/icon.icns", "tauri/icon.icns", "tauri/icon.ico", "web/favicon.svg",
                    "email/streams-reader-logo-on-light.png", "email/streams-reader-logo-on-light@2x.png"):
            self.assertTrue((BRAND / "dist" / rel).exists(), rel)

    def test_desktop_small_icons_are_tiles(self) -> None:
        blue = TOKENS.art_colors["blue"]
        rels = ["linux/hicolor/16x16/apps/streams-reader.png", "linux/hicolor/24x24/apps/streams-reader.png",
                "linux/hicolor/32x32/apps/streams-reader.png", "tauri/32x32.png", "png/icon-16.png",
                "macos/icon.iconset/icon_16x16.png", "macos/icon.iconset/icon_16x16@2x.png"]
        for rel in rels:
            with Image.open(BRAND / "dist" / rel) as im:
                im = im.convert("RGBA")
                self.assertEqual(im.getpixel((im.width // 2, 1))[3], 255, rel)
                r, g, b, _ = im.getpixel((im.width // 2, 1))
                self.assertLess(abs(b - blue.b) + abs(r - blue.r), 40, rel)
        with Image.open(BRAND / "dist" / "windows" / "icon.ico") as ico:
            for n in (16, 24, 32):
                ico.size = (n, n)
                ico.load()
                frame = ico.convert("RGBA")
                self.assertEqual(frame.getpixel((n // 2, 1))[3], 255, n)

    def test_icos_carry_every_size(self) -> None:
        with Image.open(BRAND / "dist" / "web" / "favicon.ico") as ico:
            self.assertEqual(sorted(ico.info["sizes"]), [(16, 16), (32, 32), (48, 48)])
        with Image.open(BRAND / "dist" / "windows" / "icon.ico") as ico:
            self.assertEqual(sorted(s[0] for s in ico.info["sizes"]), [16, 24, 32, 48, 64, 128, 256])

    def test_email_logo_mark_lands_on_whole_cells(self) -> None:
        self.assertEqual(self.size("email/streams-reader-logo-on-light.png")[1], 48)
        self.assertEqual(self.size("email/streams-reader-logo-on-light@2x.png")[1], 96)


if __name__ == "__main__":
    unittest.main()
