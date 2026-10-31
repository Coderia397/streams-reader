"""Subset the site's fonts from crates/web/assets/fonts into crates/site/assets/fonts (note 15: fonts <= 90 KB).

Inter and Literata (upright): Latin-1 plus the punctuation and arrows the pages use, wght axis kept.
Literata italic: only the characters the site sets in italic (lockup word, footer tagline, accent words, the demo
poem, the reader mock's chapter label), in the five Latin-script locales. The list is written next to the fonts as
literata-italic.chars.txt; `cargo test` checks that it still covers those strings.

uv run --no-project --python 3.12 --with fonttools==4.55.0 --with brotli==1.1.0 python crates/site/tools/subset_fonts.py
"""
import json, pathlib, re, sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

SITE = pathlib.Path(__file__).resolve().parent.parent
SRC = SITE.parent / "web/assets/fonts"
OUT = SITE / "assets/fonts"
LATIN_LOCALES = ["en", "es", "fr", "de", "pt"]
ITALIC_KEYS = ["brand-product", "footer-tagline", "hero-title-accent", "ch3-title-accent"]
UPRIGHT_W = (400, 700)
ITALIC_W = (450, 550)
UPRIGHT = "U+0000-00FF,U+2010-2027,U+2190-2193,U+00D7"


def italic_text() -> str:
    parts = ["Chapter 1"]
    for lang in LATIN_LOCALES:
        ftl = (SITE / "locales" / lang / "site.ftl").read_text()
        for key in ITALIC_KEYS:
            m = re.search(rf"^{re.escape(key)} = (.*)$", ftl, re.M)
            if not m:
                sys.exit(f"{lang}: {key} missing")
            parts.append(m.group(1))
    demo = json.loads((SITE / "assets/demo/demo.json").read_text())
    parts += [t["text"] for t in demo["texts"] if "\n" in t["text"]]
    return "".join(parts)


def run(src: str, dst: str, wght: tuple, unicodes: str = "", text: str = "") -> int:
    """Limit the weight axis to the weights the site sets (tokens: 450, 550, 620), then subset to WOFF2."""
    tmp = OUT / f".{dst}.ttf"
    font = TTFont(SRC / src)
    lo, hi = wght
    instancer.instantiateVariableFont(font, {"wght": lo if lo == hi else (lo, hi)}, inplace=True, updateFontNames=False)
    font.flavor = None
    font.save(tmp)
    args = [str(tmp), f"--output-file={OUT / dst}", "--flavor=woff2", "--layout-features+=cv11,ss01,tnum", "--no-hinting",
            "--desubroutinize", "--drop-tables+=vhea,vmtx,STAT", "--notdef-outline"]
    args.append(f"--unicodes={unicodes}" if unicodes else f"--text={text}")
    subset.main(args)
    tmp.unlink()
    return (OUT / dst).stat().st_size


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    chars = "".join(sorted(set(italic_text()) - {"\n"} | set(" .,’'-")))
    (OUT / "literata-italic.chars.txt").write_text(chars + "\n")
    sizes = {
        "inter-latin-wght-normal.woff2": run("inter-latin-wght-normal.woff2", "inter-latin-wght-normal.woff2", UPRIGHT_W, UPRIGHT),
        "literata-latin-wght-normal.woff2": run("literata-latin-wght-normal.woff2", "literata-latin-wght-normal.woff2", UPRIGHT_W, UPRIGHT),
        "literata-latin-wght-italic.woff2": run("literata-latin-wght-italic.woff2", "literata-latin-wght-italic.woff2", ITALIC_W, text=chars),
    }
    for name in ["OFL-Inter.txt", "OFL-Literata.txt"]:
        (OUT / name).write_bytes((SRC / name).read_bytes())
    for k, v in sizes.items():
        print(f"{k}: {v} B")
    print(f"total: {sum(sizes.values())} B; italic chars: {len(chars)}")


if __name__ == "__main__":
    main()
