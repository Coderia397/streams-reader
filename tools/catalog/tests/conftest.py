"""Shared fixtures. Each `fixtures/<repo>.zip` holds a Standard Ebooks repo's LICENSE.md, COMMIT and the `src/`
files the pipeline reads, plus the boilerplate pages it must drop (imprint, colophon, uncopyright), at the commit
named in COMMIT."""

from __future__ import annotations

import shutil
import sys
import zipfile
from pathlib import Path

import pytest

CATALOG = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(CATALOG))

FIXTURES = CATALOG / "tests" / "fixtures"
BOOKS = ("kahlil-gibran_the-prophet", "t-s-eliot_poetry")


def load_fixture(repo: str) -> dict[str, bytes]:
    with zipfile.ZipFile(FIXTURES / f"{repo}.zip") as z:
        return {n: z.read(n) for n in z.namelist() if not n.endswith("/")}


def src_files(repo: str) -> dict[str, bytes]:
    return {k[4:]: v for k, v in load_fixture(repo).items() if k.startswith("src/")}


@pytest.fixture(scope="session")
def converter() -> Path:
    if shutil.which("cargo") is None:
        pytest.skip("cargo is not installed")
    from secat import convert

    return convert.build()
