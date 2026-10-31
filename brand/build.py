#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "resvg-py==0.2.6",
#   "pillow==11.3.0",
#   "fonttools[woff]==4.59.0",
#   "uharfbuzz==0.51.7",
# ]
# ///
"""Build the Streams Reader brand from brand/tokens.toml.

    uv run --script brand/build.py            # everything
    uv run --script brand/build.py tokens     # token layer only (fast)
    uv run --script brand/build.py art        # tokens + SVG sources, no PNG exports
    uv run --script brand/build.py check      # contrast check only
"""

from __future__ import annotations

import sys

sys.dont_write_bytecode = True
from pathlib import Path  # noqa: E402

BRAND = Path(__file__).resolve().parent
sys.path.insert(0, str(BRAND))

from brandkit import emit, tokens as tok  # noqa: E402


def build_tokens(t: tok.Tokens) -> None:
    out = BRAND / "dist" / "tokens"
    out.mkdir(parents=True, exist_ok=True)
    emit.write_scss(t, out / "_tokens.scss")
    emit.write_css(t, out / "tokens.css")
    emit.write_json(t, out / "tokens.json")
    (out / "contrast.md").write_text(
        "# Contrast (WCAG 2.1), generated from tokens.toml\n\n" + emit.contrast_markdown(t), encoding="utf-8"
    )
    readme = BRAND / "README.md"
    if readme.exists():
        text = readme.read_text(encoding="utf-8")
        start, end = "<!-- contrast:start -->", "<!-- contrast:end -->"
        if start in text and end in text:
            head, rest = text.split(start, 1)
            tail = rest.split(end, 1)[1]
            readme.write_text(f"{head}{start}\n{emit.contrast_summary(t)}{end}{tail}", encoding="utf-8")
    print(f"tokens -> {out.relative_to(BRAND.parent)}")


def check_contrast(t: tok.Tokens) -> bool:
    failed = [r for r in tok.contrast_report(t) if not r.ok]
    for r in failed:
        print(f"CONTRAST FAIL {r.theme}: {r.fg} on {r.bg} = {r.ratio:.2f} < {r.minimum}", file=sys.stderr)
    return not failed


def main(argv: list[str]) -> int:
    step = argv[1] if len(argv) > 1 else "all"
    t = tok.load(BRAND / "tokens.toml")
    ok = check_contrast(t)
    if step == "check":
        return 0 if ok else 1
    build_tokens(t)
    if step in ("art", "all"):
        from brandkit import art

        art.build_all(t, BRAND)
    if step == "all":
        from brandkit import export

        export.build_all(t, BRAND)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
