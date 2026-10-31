# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Trunk post-build hook for the Streams Reader web app (served under /app/).

0. Runs tools/check_style.py (which runs brand/check_tokens.py over crates/web/style): the build fails when a
   stylesheet hard-codes a colour, radius or font size, or Rust markup a hex colour, instead of using the tokens.
1. Moves trunk's inline `<script type="module">` loader into a content-hashed file, because the Worker serves the
   app with `script-src 'self' 'wasm-unsafe-eval'` (no inline scripts). Fails the build if any inline script remains.
2. Fills the theme colours (index.html, manifest, theme-boot.js) from brand/dist/tokens/tokens.json, the single
   source of truth, so no colour is copied by hand.
3. Gives the import worker (crates/web-import, built by trunk with fixed names) content-hashed names under
   `import/`, writes its loader (`import/loader-<hash>.js`, no inline script) and puts the loader URL into the
   `import-worker` meta of index.html. The service worker caches these on first use, not at install.
4. Writes the service worker's precache list (under the public URL) and version into `sw.js`.

Runs inside trunk's staging directory (TRUNK_STAGING_DIR) before the files are moved to dist/.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

INLINE_MODULE = re.compile(r"<script\b(?![^>]*\bsrc=)([^>]*\btype=[\"']module[\"'][^>]*)>(.*?)</script>", re.S | re.I)
ANY_INLINE_SCRIPT = re.compile(r"<script\b(?![^>]*\bsrc=)[^>]*>", re.I)
# Large, rarely used files are cached on first use by the service worker instead.
# Remix icon sources are compiled into the wasm; they are copied only for reference and licensing.
SKIP_PREFIXES = ("assets/vendor/", "assets/icons/remix/", "import/")
# trunk's output names for the import worker (`data-bin` in index.html).
WORKER_BIN = "nolabs-import-worker"
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


def check_tokens(source: Path) -> None:
    checker = source / "tools" / "check_style.py"
    env = {**os.environ, "TRUNK_SOURCE_DIR": str(source)}
    if subprocess.run([sys.executable, str(checker)], env=env).returncode != 0:
        fail("raw colour, radius or font size found (see above); use brand/dist/tokens")


def theme_colours(source: Path) -> dict[str, str]:
    tokens_path = source.parent.parent / "brand" / "dist" / "tokens" / "tokens.json"
    if not tokens_path.exists():
        fail(f"brand tokens missing: {tokens_path} (run brand/build.py)")
    tokens = json.loads(tokens_path.read_text(encoding="utf-8"))
    themes = tokens["themes"]
    colours = {name: themes[name]["color"]["bg"]["on_bg"] for name in ("ink", "paper", "night")}
    # Safari's pinned-tab colour: the Streams primary, as the artwork uses it.
    colours["brand"] = tokens["palette"]["blue-500"]
    return colours


def fill_colours(staging: Path, colours: dict[str, str]) -> None:
    fills = {
        "index.html": {"__THEME_BG__": colours["ink"], "__BRAND__": colours["brand"]},
        "manifest.webmanifest": {"__THEME_BG__": colours["ink"]},
        "assets/theme-boot.js": {f"__BG_{k.upper()}__": colours[k] for k in ("ink", "paper", "night")},
    }
    for rel, pairs in fills.items():
        path = staging / rel
        if not path.exists():
            fail(f"{rel} missing from the build")
        text = path.read_text(encoding="utf-8")
        for placeholder, value in pairs.items():
            if placeholder not in text:
                fail(f"{rel}: placeholder {placeholder} not found")
            text = text.replace(placeholder, value)
        path.write_text(text, encoding="utf-8")


def short_hash(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:16]


def hash_import_worker(staging: Path, public_url: str) -> str:
    """Moves the worker's JS and wasm to hashed names under import/ and writes their loader. Returns its URL."""
    js = staging / f"{WORKER_BIN}.js"
    wasm = staging / f"{WORKER_BIN}_bg.wasm"
    if not js.exists() or not wasm.exists():
        fail(f"import worker missing from the build ({js.name}, {wasm.name})")
    out = staging / "import"
    out.mkdir(exist_ok=True)
    names = {}
    for path, stem, ext in ((js, WORKER_BIN, ".js"), (wasm, f"{WORKER_BIN}_bg", ".wasm")):
        data = path.read_bytes()
        name = f"{stem}-{short_hash(data)}{ext}"
        (out / name).write_bytes(data)
        path.unlink()
        names[ext] = f"{public_url}import/{name}"
    loader = (
        f"// Starts the import worker (generated by tools/postbuild.py).\n"
        f"importScripts({json.dumps(names['.js'])});\n"
        f"wasm_bindgen({{ module_or_path: {json.dumps(names['.wasm'])} }});\n"
    )
    loader_name = f"loader-{short_hash(loader.encode())}.js"
    (out / loader_name).write_text(loader, encoding="utf-8")
    loader_url = f"{public_url}import/{loader_name}"
    index = staging / "index.html"
    html = index.read_text(encoding="utf-8")
    if "__IMPORT_WORKER__" not in html:
        fail("index.html: placeholder __IMPORT_WORKER__ not found")
    index.write_text(html.replace("__IMPORT_WORKER__", loader_url), encoding="utf-8")
    return loader_url


def precache(staging: Path, public_url: str) -> tuple[list[str], str]:
    urls: list[str] = []
    digest = hashlib.sha256()
    for path in sorted(p for p in staging.rglob("*") if p.is_file()):
        rel = path.relative_to(staging).as_posix()
        if path.name in SKIP_FILES or rel.startswith(SKIP_PREFIXES) or rel.endswith(SKIP_SUFFIXES):
            continue
        digest.update(rel.encode())
        digest.update(hashlib.sha256(path.read_bytes()).digest())
        urls.append(public_url if rel == "index.html" else f"{public_url}{rel}")
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
    source = Path(os.environ.get("TRUNK_SOURCE_DIR", Path(__file__).resolve().parent.parent))
    check_tokens(source)
    public_url = os.environ.get("TRUNK_PUBLIC_URL", "/") or "/"
    if not public_url.endswith("/"):
        public_url += "/"
    externalise_loader(staging, public_url)
    fill_colours(staging, theme_colours(source))
    worker = hash_import_worker(staging, public_url)
    urls, version = precache(staging, public_url)
    write_service_worker(staging, urls, version)
    print(f"postbuild: tokens clean, loader externalised, import worker {worker}, {len(urls)} files precached, "
          f"shell version {version}")


if __name__ == "__main__":
    main()
