#!/usr/bin/env -S uv run --no-project --python 3.12 --script
"""Fail when SCSS outside the token layer hard-codes colours or radii.

    uv run --no-project --python 3.12 --script brand/check_tokens.py crates/web/style site/style

Flags, per line (comments ignored): hex colours (#abc, #aabbcc, #aabbccdd), rgb()/rgba()/hsl()/hsla()
literals, and border-radius values in px/rem/em. Colours and radii come from
brand/dist/tokens/_tokens.scss (t.color(...), t.radius(...), var(--…)). A line ending in
`// tokens-ok: <reason>` is allowed. Exit 1 on any finding.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HEX = re.compile(r"(?<![\w&-])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b")
COLOR_FN = re.compile(r"\b(?:rgba?|hsla?|oklch|oklab|lab|lch)\(\s*[\d.]")
RADIUS = re.compile(r"border(?:-[a-z]+)*-radius\s*:\s*[^;]*?\b\d*\.?\d+(?:px|rem|em)\b")
ALLOW = "tokens-ok"


def strip_comments(text: str) -> list[str]:
    text = re.sub(r"/\*.*?\*/", lambda m: "\n" * m.group(0).count("\n"), text, flags=re.S)
    return text.splitlines()


def scan(path: Path) -> list[str]:
    findings = []
    raw_lines = path.read_text(encoding="utf-8").splitlines()
    for no, line in enumerate(strip_comments(path.read_text(encoding="utf-8")), 1):
        if ALLOW in raw_lines[no - 1]:
            continue
        code = re.sub(r"//.*$", "", line) if "://" not in line else line
        if code.rstrip().endswith("{") and ":" not in code.split("{")[0].split(" ")[-1]:
            code = code.split("{", 1)[1]  # a selector line: ids like #add are not colours
        for rx, what in ((HEX, "hex colour"), (COLOR_FN, "colour literal"), (RADIUS, "raw radius")):
            m = rx.search(code)
            if m:
                findings.append(f"{path}:{no}: {what} '{m.group(0).strip()}' (use the token layer)")
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
