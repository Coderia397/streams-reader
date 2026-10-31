# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Checks and packages the Chrome extension into a store-ready zip.

    uv run extensions/chrome/package.py                      # check + dist/streams-reader-chrome-<version>.zip
    uv run extensions/chrome/package.py --check              # check only (builds in a temp dir; dist/ is untouched)
    uv run extensions/chrome/package.py --app-origin https://staging.example --out /tmp/x   # staging build

The checks fail the build on: permissions beyond the allowed five, any host permission, a content script outside
the app origin, a missing or extra locale key, a store name or description over Chrome's limits, an icon whose PNG
size does not match its slot, remote or dynamic code in the scripts, or a zip over the size budget.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import struct
import sys
import tempfile
import zipfile
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parent
PROD_ORIGIN = "https://listen.streams.watch"
ALLOWED_PERMISSIONS = {"activeTab", "alarms", "contextMenus", "scripting", "storage"}
LOCALES = ("en", "ar", "es", "fr", "de", "pt_BR", "hi", "zh_CN")
SHIPPED = ("manifest.json", "src", "_locales", "icons")
ZIP_BUDGET_BYTES = 100 * 1024
NAME_MAX = 75
SHORT_NAME_MAX = 12
DESCRIPTION_MAX = 132
ORIGIN_LINE = re.compile(r'^export const APP_ORIGIN = "([^"]+)";$', re.M)
FORBIDDEN_JS = [
    (re.compile(r"\beval\s*\("), "eval()"),
    (re.compile(r"\bnew\s+Function\s*\("), "new Function()"),
    (re.compile(r"\bimportScripts\s*\("), "importScripts()"),
    (re.compile(r"\bimport\s*\("), "dynamic import()"),
    (re.compile(r"\bfetch\s*\("), "fetch() (the extension makes no network requests)"),
    (re.compile(r"\binnerHTML\s*="), "innerHTML assignment"),
]


class CheckError(Exception):
    pass


def normalise_origin(origin: str) -> str:
    """Writes `origin` the way the browser serialises it (`sender.origin`): no trailing slash, lower-case scheme and
    host, no default port. The extension compares the configured value with `sender.origin` and appends paths to it,
    so `https://staging.example/` would otherwise build an extension that never delivers. Anything with a path,
    query or fragment is returned as given for origin_pattern() to refuse."""
    o = origin.strip().rstrip("/")
    parts = urlsplit(o)
    if not parts.scheme or not parts.netloc or parts.path or parts.query or parts.fragment or "?" in o or "#" in o:
        return o
    scheme, netloc = parts.scheme.lower(), parts.netloc.lower()
    default = {"https": ":443", "http": ":80"}.get(scheme)
    if default and netloc.endswith(default):
        netloc = netloc[: -len(default)]
    return f"{scheme}://{netloc}"


def origin_pattern(origin: str) -> str:
    """Match pattern for the app's pages on `origin` (ports are not part of Chrome match patterns). `origin` must
    already be normalised: any path, query, fragment or user name is refused."""
    parts = urlsplit(origin)
    try:
        parts.port  # noqa: B018  (raises on a malformed port)
    except ValueError:
        raise CheckError(f"app origin has a malformed port: {origin!r}") from None
    if (
        parts.scheme not in ("https", "http")
        or not parts.hostname
        or parts.path
        or parts.query
        or parts.fragment
        or parts.username is not None
        or "?" in origin
        or "#" in origin
    ):
        raise CheckError(f"app origin must be scheme://host[:port], got {origin!r}")
    if parts.scheme == "http" and parts.hostname not in ("localhost", "127.0.0.1"):
        raise CheckError("plain http is allowed only for localhost test builds")
    return f"{parts.scheme}://{parts.hostname}/app/*"


def png_size(path: Path) -> tuple[int, int]:
    head = path.read_bytes()[:24]
    if head[:8] != b"\x89PNG\r\n\x1a\n" or head[12:16] != b"IHDR":
        raise CheckError(f"{path.name} is not a PNG")
    return struct.unpack(">II", head[16:24])


def load_messages(locales_dir: Path) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    for loc in LOCALES:
        f = locales_dir / loc / "messages.json"
        if not f.is_file():
            raise CheckError(f"missing locale {loc}")
        data = json.loads(f.read_text("utf-8"))
        out[loc] = {k: v["message"] for k, v in data.items()}
    extra = sorted(p.name for p in locales_dir.iterdir() if p.is_dir() and p.name not in LOCALES)
    if extra:
        raise CheckError(f"unexpected locales: {extra}")
    return out


def check_messages(manifest: dict, messages: dict[str, dict[str, str]], js_sources: str) -> None:
    keys = set(messages["en"])
    for loc, msgs in messages.items():
        if set(msgs) != keys:
            missing, extra = sorted(keys - set(msgs)), sorted(set(msgs) - keys)
            raise CheckError(f"{loc}: missing {missing}, extra {extra}")
        for k, v in msgs.items():
            if not v.strip():
                raise CheckError(f"{loc}: {k} is empty")
            if "$" in v:
                raise CheckError(f"{loc}: {k} has a placeholder; none are expected")
        if msgs["ext_short_name"] != "Streams Reader":
            raise CheckError(f"{loc}: the short name is the brand and is not translated")
        if len(msgs["ext_name"]) > NAME_MAX:
            raise CheckError(f"{loc}: name over {NAME_MAX} characters")
        if len(msgs["ext_description"]) > DESCRIPTION_MAX:
            raise CheckError(f"{loc}: description over {DESCRIPTION_MAX} characters")
        if "Streams Reader" not in msgs["ext_name"]:
            raise CheckError(f"{loc}: name must keep the brand")
    used = set(re.findall(r"__MSG_(\w+)__", json.dumps(manifest)))
    used |= set(re.findall(r'getMessage\("(\w+)"\)', js_sources))
    unknown = sorted(used - keys)
    if unknown:
        raise CheckError(f"messages used but not defined: {unknown}")
    errors = re.search(r"export const Errors = Object\.freeze\(\{(.*?)\}\);", js_sources, re.S)
    if not errors:
        raise CheckError("Errors table not found in payload.js")
    for code in re.findall(r':\s*"(\w+)"', errors.group(1)):
        if f"error_{code}" not in keys:
            raise CheckError(f"no message error_{code}")


def check_manifest(manifest: dict, app_origin: str) -> None:
    if manifest.get("manifest_version") != 3:
        raise CheckError("manifest_version must be 3")
    perms = set(manifest.get("permissions", []))
    if not perms <= ALLOWED_PERMISSIONS:
        raise CheckError(f"permissions beyond the allowed set: {sorted(perms - ALLOWED_PERMISSIONS)}")
    if manifest.get("host_permissions") or manifest.get("optional_host_permissions"):
        raise CheckError("host permissions are not allowed")
    want = [origin_pattern(app_origin)]
    for cs in manifest.get("content_scripts", []):
        if cs.get("matches") != want or cs.get("all_frames") or cs.get("match_about_blank"):
            raise CheckError(f"content script must match only {want} in the top frame")
    if "externally_connectable" in manifest or "web_accessible_resources" in manifest:
        raise CheckError("no externally_connectable or web_accessible_resources")
    csp = manifest.get("content_security_policy", {}).get("extension_pages", "")
    directives = {d.split()[0]: d.split()[1:] for d in (x.strip() for x in csp.split(";")) if d}
    if directives.get("script-src") != ["'self'"] or directives.get("default-src") != ["'none'"]:
        raise CheckError("CSP must be default-src 'none'; script-src 'self'")
    if not re.fullmatch(r"\d+\.\d+\.\d+", manifest.get("version", "")):
        raise CheckError("version must be MAJOR.MINOR.PATCH")


def check_icons(manifest: dict, base: Path) -> None:
    slots = dict(manifest.get("icons", {}))
    slots.update(manifest.get("action", {}).get("default_icon", {}))
    for size, rel in slots.items():
        w, h = png_size(base / rel)
        if (w, h) != (int(size), int(size)):
            raise CheckError(f"{rel} is {w}x{h}, slot {size}")


def check_js(src_dir: Path) -> str:
    combined = []
    for f in sorted(src_dir.glob("*.js")):
        text = f.read_text("utf-8")
        # Comments that start a line (or follow whitespace) are dropped; "http://*/*" inside a string is kept.
        code = re.sub(r"(?m)^\s*/\*[\s\S]*?\*/", "", text)
        code = re.sub(r"(?m)(^|\s)//.*$", r"\1", code)
        for pattern, what in FORBIDDEN_JS:
            if pattern.search(code):
                raise CheckError(f"{f.name}: {what} is not allowed")
        for url in re.findall(r"https?://[^\s\"'`]+", code):
            if not url.startswith(("http://*/", "https://*/")):
                if f.name != "config.js":
                    raise CheckError(f"{f.name}: remote address {url} (only config.js may name the app origin)")
        combined.append(text)
    return "\n".join(combined)


def build(out_dir: Path, app_origin: str = PROD_ORIGIN, src: Path = ROOT) -> dict:
    """Copies the shipped files to `out_dir` with the app origin applied, checks them, and returns the manifest."""
    app_origin = normalise_origin(app_origin)
    pattern = origin_pattern(app_origin)  # Before anything is deleted: a bad origin leaves out_dir as it was.
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    for name in SHIPPED:
        s = src / name
        if s.is_dir():
            shutil.copytree(s, out_dir / name, ignore=shutil.ignore_patterns(".*", "__pycache__"))
        else:
            shutil.copy2(s, out_dir / name)
    cfg = out_dir / "src" / "config.js"
    text = cfg.read_text("utf-8")
    if len(ORIGIN_LINE.findall(text)) != 1:
        raise CheckError("config.js must have exactly one APP_ORIGIN line")
    cfg.write_text(ORIGIN_LINE.sub(f'export const APP_ORIGIN = "{app_origin}";', text), "utf-8")
    mpath = out_dir / "manifest.json"
    manifest = json.loads(mpath.read_text("utf-8"))
    for cs in manifest.get("content_scripts", []):
        cs["matches"] = [pattern]
    if app_origin != PROD_ORIGIN:
        manifest["homepage_url"] = app_origin + "/"
    mpath.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", "utf-8")

    check_manifest(manifest, app_origin)
    js = check_js(out_dir / "src")
    check_messages(manifest, load_messages(out_dir / "_locales"), js)
    check_icons(manifest, out_dir)
    return manifest


def write_zip(tree: Path, zip_path: Path) -> tuple[int, str]:
    """Deterministic zip: sorted entries, fixed timestamps and modes."""
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for f in sorted(p for p in tree.rglob("*") if p.is_file()):
            info = zipfile.ZipInfo(f.relative_to(tree).as_posix(), date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, f.read_bytes())
    data = zip_path.read_bytes()
    if len(data) > ZIP_BUDGET_BYTES:
        raise CheckError(f"zip is {len(data)} bytes, budget {ZIP_BUDGET_BYTES}")
    return len(data), hashlib.sha256(data).hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--app-origin", default=PROD_ORIGIN)
    ap.add_argument("--out", type=Path, default=ROOT / "dist")
    ap.add_argument("--check", action="store_true", help="check only, write no zip")
    args = ap.parse_args(argv)
    app_origin = normalise_origin(args.app_origin)
    tree = args.out / "unpacked"
    try:
        if args.check:
            # A temp tree, so the dist/unpacked a developer loaded in chrome://extensions is not deleted under it.
            with tempfile.TemporaryDirectory(prefix="sr-check-") as tmp:
                build(Path(tmp) / "unpacked", app_origin)
            print("checks passed")
            return 0
        manifest = build(tree, app_origin)
        suffix = "" if app_origin == PROD_ORIGIN else "-" + (urlsplit(app_origin).hostname or "custom")
        zip_path = args.out / f"streams-reader-chrome-{manifest['version']}{suffix}.zip"
        size, digest = write_zip(tree, zip_path)
    except CheckError as e:
        print(f"package.py: {e}", file=sys.stderr)
        return 1
    print(f"{zip_path}  {size} bytes  sha256 {digest}")
    print(f"unpacked tree: {tree}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
