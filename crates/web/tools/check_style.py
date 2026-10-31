# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Trunk pre-build check: the web app styles only through the central brand tokens.

1. Runs brand/check_tokens.py (owned by the brand team) over crates/web/style: no hex colours, colour literals or
   raw radii outside the token layer.
2. Also fails on raw font sizes (px/rem), raw durations (`160ms`) and raw blurs (`blur(20px)`) in the app's SCSS,
   and on hex colours inside Rust markup: durations come from t.dur(...), blurs from t.glass or t.size(...).

3. After a build (TRUNK_STAGING_DIR, or `--css <file>`), fails on a class written in Rust markup (`class="a b"`,
   `class:name=`) that no selector in the built stylesheet uses: a class nobody styles is either a missing rule or
   dead markup. Hooks used only by tests or scripts are listed in UNSTYLED_OK.

A line ending in `// tokens-ok: <reason>` is allowed.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

HEX_IN_RUST = re.compile(r"\"[^\"\n]*#[0-9a-fA-F]{6}\b[^\"\n]*\"")
RAW_FONT_SIZE = re.compile(r"font-size\s*:\s*[^;]*?\b\d*\.?\d+(?:px|rem|em)\b")
RAW_DURATION = re.compile(r"(?<![\w-])\d*\.?\d+ms\b")
RAW_BLUR = re.compile(r"blur\(\s*\d*\.?\d+(?:px|rem|em)\s*\)")

CLASS_ATTR = re.compile(r'\bclass="([^"]*)"')
CLASS_DIRECTIVE = re.compile(r"\bclass:([A-Za-z_][\w-]*)\s*=")
CSS_CLASS = re.compile(r"\.(-?[A-Za-z_][\w-]*)")
CSS_SKIP = re.compile(r"url\([^)]*\)|\"[^\"]*\"|'[^']*'|/\*.*?\*/", re.S)
# Classes in markup on purpose without a rule: hooks for tests and scripts, not for looks.
UNSTYLED_OK = {
    "sheet__close",  # the e2e scripts close sheets through it; it is styled as .icon-btn
}

# (pattern, message) pairs checked on every SCSS line outside comments.
SCSS_RULES = (
    (RAW_FONT_SIZE, "raw font size (use t.text(...) or a type role)"),
    (RAW_DURATION, "raw duration (use t.dur(...) or t.transition)"),
    (RAW_BLUR, "raw blur (use t.glass or t.size(\"glass-blur\"))"),
)


def built_css() -> Path | None:
    """The stylesheet trunk just built: `--css <file>`, or the one in TRUNK_STAGING_DIR."""
    if "--css" in sys.argv:
        return Path(sys.argv[sys.argv.index("--css") + 1])
    staging = os.environ.get("TRUNK_STAGING_DIR")
    found = sorted(Path(staging).glob("*.css")) if staging else []
    return found[0] if found else None


def unstyled_classes(src: Path, css: str) -> list[str]:
    """`file:line: class` for each class in Rust markup that no selector in `css` mentions."""
    styled = set(CSS_CLASS.findall(CSS_SKIP.sub(" ", css)))
    out = []
    for path in sorted(src.rglob("*.rs")):
        for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if line.lstrip().startswith("//"):
                continue
            names = [n for m in CLASS_ATTR.finditer(line) for n in m.group(1).split()]
            names += CLASS_DIRECTIVE.findall(line)
            for name in names:
                if name not in styled and name not in UNSTYLED_OK and "{" not in name:
                    out.append(f"{path}:{no}: class '{name}' has no rule in the built CSS (add one or drop it)")
    return out


def main() -> int:
    web = Path(os.environ.get("TRUNK_SOURCE_DIR", Path(__file__).resolve().parent.parent))
    root = web.parent.parent
    checker = root / "brand" / "check_tokens.py"
    if not checker.exists():
        print(f"check_style: {checker} missing", file=sys.stderr)
        return 1
    status = subprocess.run([sys.executable, str(checker), str(web / "style")]).returncode
    findings: list[str] = []
    for path in sorted((web / "style").rglob("*.scss")):
        for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            code = re.sub(r"//.*$", "", line)
            if "tokens-ok" in line:
                continue
            for pattern, message in SCSS_RULES:
                if pattern.search(code):
                    findings.append(f"{path}:{no}: {message}")
    for path in sorted((web / "src").rglob("*.rs")):
        for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if "tokens-ok" in line or line.lstrip().startswith("//"):
                continue
            if HEX_IN_RUST.search(line):
                findings.append(f"{path}:{no}: hex colour in Rust (use a CSS class and the tokens)")
    css = built_css()
    if css is not None:
        findings += unstyled_classes(web / "src", css.read_text(encoding="utf-8"))
        print(f"check_style: markup classes checked against {css.name}")
    for f in findings:
        print(f, file=sys.stderr)
    print(f"check_style: {len(findings)} extra finding(s)")
    return 1 if status or findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
