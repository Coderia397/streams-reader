#!/usr/bin/env -S uv run --no-project --python 3.12 --script
"""Fail when SCSS outside the token layer hard-codes colours or radii.

    uv run --no-project --python 3.12 --script brand/check_tokens.py crates/web/style site/style

Flags, per line (comments ignored): hex colours (#abc, #aabbcc, #aabbccdd), colour functions (rgb, hsl, hwb,
oklch, oklab, lab, lch) with a literal number among their arguments (`oklch(var(--orb-l) var(--orb-c) var(--hue-a))`
is fine, `hsl(var(--hue) 48% 31%)` is not), and border-radius values in px/rem/em. Colours and radii come from
brand/dist/tokens/_tokens.scss (t.color(...), t.radius(...), var(--…)). A line ending in
`// tokens-ok: <reason>` is allowed. Exit 1 on any finding.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HEX = re.compile(r"(?<![\w&-])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b")
# Any colour function; its arguments may only be var(--…) references (the token layer), never a literal number.
COLOR_FN = re.compile(r"\b(?:rgba?|hsla?|hwb|oklch|oklab|lab|lch)\(")
VAR_REF = re.compile(r"var\([^()]*\)")
NUMBER = re.compile(r"(?<![\w-])\d*\.?\d+")
RADIUS = re.compile(r"border(?:-[a-z]+)*-radius\s*:\s*[^;]*?\b\d*\.?\d+(?:px|rem|em)\b")
ALLOW = "tokens-ok"


def strip_comments(text: str) -> list[str]:
    text = re.sub(r"/\*.*?\*/", lambda m: "\n" * m.group(0).count("\n"), text, flags=re.S)
    return text.splitlines()


def colour_literal(code: str) -> str | None:
    """The first colour function call on the line whose arguments hold a number outside var(...), if any."""
    for m in COLOR_FN.finditer(code):
        depth, end = 1, m.end()
        while end < len(code) and depth:
            depth += {"(": 1, ")": -1}.get(code[end], 0)
            end += 1
        args = code[m.end() : end - 1 if depth == 0 else end]
        while VAR_REF.search(args):
            args = VAR_REF.sub(" ", args)
        if NUMBER.search(args):
            return code[m.start() : end].strip()
    return None


def scan(path: Path) -> list[str]:
    findings = []
    raw_lines = path.read_text(encoding="utf-8").splitlines()
    for no, line in enumerate(strip_comments(path.read_text(encoding="utf-8")), 1):
        if ALLOW in raw_lines[no - 1]:
            continue
        code = re.sub(r"//.*$", "", line) if "://" not in line else line
        if code.rstrip().endswith("{") and ":" not in code.split("{")[0].split(" ")[-1]:
            code = code.split("{", 1)[1]  # a selector line: ids like #add are not colours
        for rx, what in ((HEX, "hex colour"), (RADIUS, "raw radius")):
            m = rx.search(code)
            if m:
                findings.append(f"{path}:{no}: {what} '{m.group(0).strip()}' (use the token layer)")
        literal = colour_literal(code)
        if literal:
            findings.append(f"{path}:{no}: colour literal '{literal}' (use the token layer)")
    return findings


def main(args: list[str]) -> int:
    roots = [Path(a) for a in args] or [Path(".")]
    findings: list[str] = []
    for root in roots:
        files = [root] if root.is_file() else sorted(root.rglob("*.scss"))
        for f in files:
            if "brand/dist/tokens" in f.as_posix():
                continue
            findings += scan(f)
    for line in findings:
        print(line, file=sys.stderr)
    print(f"check_tokens: {len(findings)} finding(s) in {', '.join(map(str, roots))}")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
