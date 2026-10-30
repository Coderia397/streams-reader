# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Trunk post-build hook for the nolabs web app.

1. Moves trunk's inline `<script type="module">` loader into a content-hashed file, because the Worker serves the
   app with `script-src 'self' 'wasm-unsafe-eval'` (no inline scripts). Fails the build if any inline script remains.
2. Writes the service worker's precache list and version (a hash over every shell file) into `sw.js`.

Runs inside trunk's staging directory (TRUNK_STAGING_DIR) before the files are moved to dist/.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from pathlib import Path

INLINE_MODULE = re.compile(r"<script\b(?![^>]*\bsrc=)([^>]*\btype=[\"']module[\"'][^>]*)>(.*?)</script>", re.S | re.I)
ANY_INLINE_SCRIPT = re.compile(r"<script\b(?![^>]*\bsrc=)[^>]*>", re.I)
# Large, rarely used files are cached on first use by the service worker instead.
SKIP_PREFIXES = ("assets/vendor/",)
SKIP_FILES = {"sw.js", ".DS_Store"}
SKIP_SUFFIXES = (".map", ".txt", ".md")


def fail(message: str) -> None:
    print(f"postbuild: {message}", file=sys.stderr)
    sys.exit(1)


def externalise_loader(staging: Path, public_url: str) -> None:
    index = staging / "index.html"
    html = index.read_text(encoding="utf-8")
    match = INLINE_MODULE.search(html)
    if match:
        code = match.group(2).strip() + "\n"
        name = f"boot-{hashlib.sha256(code.encode()).hexdigest()[:16]}.js"
        (staging / name).write_text(code, encoding="utf-8")
        tag = f'<script type="module" src="{public_url}{name}"></script>'
        html = html[: match.start()] + tag + html[match.end() :]
    leftovers = ANY_INLINE_SCRIPT.findall(html)
    if leftovers:
        fail(f"inline scripts left in index.html (CSP would block them): {leftovers}")
    index.write_text(html, encoding="utf-8")


def precache(staging: Path) -> tuple[list[str], str]:
    urls: list[str] = []
    digest = hashlib.sha256()
    for path in sorted(p for p in staging.rglob("*") if p.is_file()):
        rel = path.relative_to(staging).as_posix()
        if path.name in SKIP_FILES or rel.startswith(SKIP_PREFIXES) or rel.endswith(SKIP_SUFFIXES):
            continue
        digest.update(rel.encode())
        digest.update(hashlib.sha256(path.read_bytes()).digest())
        urls.append("/" if rel == "index.html" else f"/{rel}")
    return urls, digest.hexdigest()[:12]


def write_service_worker(staging: Path, urls: list[str], version: str) -> None:
    sw = staging / "sw.js"
    if not sw.exists():
        fail("sw.js missing from the build (index.html must copy it)")
    text = sw.read_text(encoding="utf-8")
    if "__PRECACHE__" not in text or "__VERSION__" not in text:
        fail("sw.js placeholders not found")
    text = text.replace("__VERSION__", version).replace("__PRECACHE__", json.dumps(urls, indent=2))
    sw.write_text(text, encoding="utf-8")


def main() -> None:
    staging_env = os.environ.get("TRUNK_STAGING_DIR")
    if not staging_env:
        fail("TRUNK_STAGING_DIR is not set (run through trunk)")
    staging = Path(staging_env)
    public_url = os.environ.get("TRUNK_PUBLIC_URL", "/") or "/"
    if not public_url.endswith("/"):
        public_url += "/"
    externalise_loader(staging, public_url)
    urls, version = precache(staging)
    write_service_worker(staging, urls, version)
    print(f"postbuild: loader externalised, {len(urls)} files precached, shell version {version}")


if __name__ == "__main__":
    main()
