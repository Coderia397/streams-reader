"""Token layer tests. Run: uv run --no-project --python 3.12 -m unittest discover -s brand/tests"""

from __future__ import annotations

import re
import sys

sys.dont_write_bytecode = True
import tempfile  # noqa: E402
import unittest  # noqa: E402
from pathlib import Path  # noqa: E402

BRAND = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BRAND))

from brandkit import emit  # noqa: E402
from brandkit.colors import Rgba, contrast  # noqa: E402
from brandkit.tokens import TokenError, contrast_report, load, resolve  # noqa: E402

import check_tokens  # noqa: E402

# The custom-property names every consumer codes against (task contract).
CONTRACT = """
color-brand color-brand-hover color-brand-active color-brand-soft color-on-brand color-ember color-bg
color-surface-1 color-surface-2 color-surface-3 color-border color-text color-text-2 color-text-3
color-success color-warning color-danger color-focus color-sentence-wash color-word-pill
radius-xs radius-sm radius-md radius-lg radius-xl radius-pill font-ui font-read
text-xs text-sm text-md text-lg text-xl text-2xl text-3xl shadow-1 shadow-2 shadow-3
dur-fast dur-base dur-slow ease-out button-height button-height-sm button-radius button-padding-x
input-height card-radius sheet-radius chip-height tap-min
""".split() + [f"space-{n}" for n in range(1, 13)]


class ColourTests(unittest.TestCase):
    def test_contrast_extremes(self) -> None:
        self.assertAlmostEqual(contrast(Rgba.parse("#FFFFFF"), Rgba.parse("#000000")), 21.0, places=2)
        self.assertAlmostEqual(contrast(Rgba.parse("#777777"), Rgba.parse("#777777")), 1.0)

    def test_alpha_composites_before_contrast(self) -> None:
        half = Rgba.parse("#FFFFFF").with_alpha(0.5)
        self.assertEqual(half.over(Rgba.parse("#000000")).hex, "#808080")
        self.assertEqual(half.css, "rgba(255, 255, 255, 0.5)")

    def test_hex8(self) -> None:
        self.assertAlmostEqual(Rgba.parse("#16161AD9").a, 0.851, places=3)


class TokenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.t = load(BRAND / "tokens.toml")
        cls.tmp = Path(tempfile.mkdtemp())
        emit.write_css(cls.t, cls.tmp / "tokens.css")
        emit.write_scss(cls.t, cls.tmp / "_tokens.scss")
        cls.css = (cls.tmp / "tokens.css").read_text()

    def test_contract_names_present_in_every_theme(self) -> None:
        declared = set(re.findall(r"--([a-z0-9-]+):", self.css))
        missing = [n for n in CONTRACT if n not in declared]
        self.assertEqual(missing, [])
        for theme in self.t.themes:
            block = self.css.split(f':root[data-theme="{theme}"]', 1)[1].split("}", 1)[0]
            for name in (n for n in CONTRACT if n.startswith(("color-", "shadow-"))):
                self.assertIn(f"--{name}:", block, f"{name} missing in {theme}")

    def test_all_contrast_rules_pass(self) -> None:
        failed = [r for r in contrast_report(self.t) if not r.ok]
        self.assertEqual(failed, [], "\n".join(f"{r.theme} {r.fg}/{r.bg} {r.ratio:.2f}" for r in failed))

    def test_scalar_references_stay_live(self) -> None:
        self.assertIn("--button-radius: var(--radius-pill);", self.css)

    def test_unknown_reference_fails(self) -> None:
        raw = {
            "meta": {"themes": ["a"], "default_theme": "a", "light_theme": "a"},
            "palette": {"x": "#000000"},
            "theme": {"a": {"color": {"bg": "{palette.nope}"}}},
            "art": {"color": {}},
        }
        with self.assertRaises(TokenError):
            resolve(raw)

    def test_scss_has_no_selectors_besides_root(self) -> None:
        scss = (self.tmp / "_tokens.scss").read_text()
        selectors = set(re.findall(r"^(\S[^{\n]*)\{", scss, flags=re.M))
        for sel in selectors:
            self.assertTrue(sel.startswith((":root", "@", "$", "&")), sel)


class CheckScriptTests(unittest.TestCase):
    def test_flags_raw_values_and_allows_tokens(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "_x.scss"
            p.write_text(
                ".a { color: #fff; }\n"
                ".b { color: t.color(text); border-radius: t.radius(md); }\n"
                ".c { background: rgba(0, 0, 0, 0.4); }\n"
                ".d { border-radius: 6px; }\n"
                "// #123456 in a comment is fine\n"
                ".e { color: #abcdef; } // tokens-ok: third-party embed\n"
                "#main { display: block; }\n"
                "#add .x {\n"
                ".f { background: hsl(var(--hue-a) 48% 31%); }\n"
                ".g { background: oklch(t.gen(\"orb-l\") var(--orb-c) var(--hue-a) / var(--a)); }\n"
                ".h { background: oklch(var(--l) var(--c) var(--hue-b) / 0.34); }\n"
                ".i { color: rgb(var(--r), 0, 0); }\n"
            )
            findings = check_tokens.scan(p)
        lines = sorted(int(f.split(":")[1]) for f in findings)
        self.assertEqual(lines, [1, 3, 4, 9, 11, 12])


if __name__ == "__main__":
    unittest.main()
