"""Packs a Standard Ebooks source tree (the repo's `src/` folder) into a minimal EPUB for the importer.

Only what the importer reads is kept: the container, the package document and the XHTML text. SE's own front and
back matter (imprint, colophon, licence page, lists of illustrations) is left out so the audio starts with the book;
the importer skips spine entries whose file is missing. Endnotes are already skipped by the importer's
`epub:type` rules, and their file is dropped here too."""

from __future__ import annotations

import io
import posixpath
import zipfile

CONTAINER = "META-INF/container.xml"
OPF = "epub/content.opf"
TEXT_DIR = "epub/text/"
# SE boilerplate and navigation, never read aloud.
EXCLUDED_TEXT = frozenset(
    {
        "imprint.xhtml",
        "colophon.xhtml",
        "uncopyright.xhtml",
        "halftitlepage.xhtml",
        "loi.xhtml",
        "endnotes.xhtml",
        "toc.xhtml",
    }
)


def wanted(src_path: str) -> bool:
    """Whether a path relative to `src/` goes into the EPUB."""
    if src_path in (CONTAINER, OPF):
        return True
    if src_path.startswith(TEXT_DIR) and src_path.endswith(".xhtml"):
        return posixpath.basename(src_path) not in EXCLUDED_TEXT
    return False


def pack(files: dict[str, bytes]) -> bytes:
    """`files` maps paths relative to `src/` to their bytes. Returns the EPUB bytes (mimetype stored first)."""
    for required in (CONTAINER, OPF):
        if required not in files:
            raise ValueError(f"source tree lacks {required}")
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr(zipfile.ZipInfo("mimetype"), b"application/epub+zip", compress_type=zipfile.ZIP_STORED)
        for path in sorted(p for p in files if wanted(p)):
            info = zipfile.ZipInfo(path, date_time=(2000, 1, 1, 0, 0, 0))
            z.writestr(info, files[path], compress_type=zipfile.ZIP_DEFLATED)
    return buf.getvalue()
