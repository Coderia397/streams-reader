"""Runs the Rust converter (tools/catalog/epub2body), which wraps the web app's own EPUB importer."""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

CATALOG_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = CATALOG_DIR.parent.parent
MANIFEST = CATALOG_DIR / "epub2body" / "Cargo.toml"
TARGET_DIR = REPO_ROOT / "target"
BINARY = TARGET_DIR / "release" / "epub2body"


class ConvertError(RuntimeError):
    pass


@dataclass(frozen=True)
class BodyMeta:
    title: str
    author: str | None
    char_count: int
    word_count: int
    block_count: int
    heading_count: int
    body_bytes: int
    sha256: str


def build() -> Path:
    """Builds the converter (release) into the root target dir, reusing the workspace's compiled dependencies."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(TARGET_DIR)}
    cmd = ["cargo", "build", "--release", "--quiet", "--manifest-path", str(MANIFEST)]
    out = subprocess.run(cmd, env=env, capture_output=True, text=True, check=False)
    if out.returncode != 0:
        raise ConvertError(f"cargo build failed:\n{out.stderr[-4000:]}")
    return BINARY


def convert(pairs: list[tuple[Path, Path]], binary: Path = BINARY) -> dict[Path, BodyMeta | str]:
    """Converts each (epub, body.json) pair. Returns body path → metadata, or the error text for a failed book."""
    if not pairs:
        return {}
    args = [str(binary)] + [str(p) for pair in pairs for p in pair]
    out = subprocess.run(args, capture_output=True, text=True, check=False)
    if out.returncode not in (0, 1):
        raise ConvertError(f"epub2body exited {out.returncode}: {out.stderr[-2000:]}")
    results: dict[Path, BodyMeta | str] = {}
    for line in out.stdout.splitlines():
        row = json.loads(line)
        key = Path(row["output"])
        results[key] = BodyMeta(**row["meta"]) if "meta" in row else row.get("error", "unknown error")
    missing = [o for _, o in pairs if o not in results]
    if missing:
        raise ConvertError(f"epub2body gave no result for {missing}")
    return results
