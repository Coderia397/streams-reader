"""Builds catalogue entries, R2 keys and the staged files that make up the upload plan.

R2 layout (bucket nolabs-media), all under `catalog/v1/`:
  catalog.json                                   the index the app reads (short cache)
  books/{id}/body-{sha12}.json                    DocumentBody v1, exact bytes the app would upload (immutable)
  books/{id}/about-{sha8}.json                    detail-sheet text: long description and subjects (immutable)
  covers/{id}-{w}-{sha8}.{avif|webp}             2:3 thumbnails at 200 and 400 px wide (immutable)
"""

from __future__ import annotations

import hashlib
import json
import math
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:  # keeps the upload path free of the image dependencies
    from .convert import BodyMeta
    from .covers import Thumb
    from .curation import Pick, Shelf
    from .opf import BookMeta

PREFIX = "catalog/v1/"
INDEX_KEY = PREFIX + "catalog.json"
CHARS_PER_SECOND = 15  # Kokoro at 1x, rounded from note 08 measurements (assumed; recalibrate on real renders)
IMMUTABLE = "public, max-age=31536000, immutable"
INDEX_CACHE = "public, max-age=300"
LICENCE = {
    "id": "CC0-1.0",
    "name": "U.S. public domain; Standard Ebooks contributions under CC0 1.0",
    "url": "https://creativecommons.org/publicdomain/zero/1.0/",
    "notice": "Public domain in the U.S. Outside the U.S., check your local laws.",
}


@dataclass(frozen=True)
class Upload:
    key: str
    path: Path
    content_type: str
    cache_control: str
    size: int
    sha256: str

    def as_json(self) -> dict:
        return {
            "key": self.key,
            "path": str(self.path),
            "content_type": self.content_type,
            "cache_control": self.cache_control,
            "bytes": self.size,
            "sha256": self.sha256,
        }


def listening_minutes(chars: int, cps: float = CHARS_PER_SECOND) -> int:
    return max(1, math.ceil(chars / cps / 60))


def body_key(book_id: str, sha256: str) -> str:
    return f"{PREFIX}books/{book_id}/body-{sha256[:12]}.json"


def about_key(book_id: str, data: bytes) -> str:
    return f"{PREFIX}books/{book_id}/about-{hashlib.sha256(data).hexdigest()[:8]}.json"


def about_bytes(meta: BookMeta) -> bytes:
    """The detail sheet's longer text, kept out of the index so catalog.json stays small (about 22 KB gzip)."""
    doc = {"v": 1, "description": meta.description or meta.abstract, "subjects": meta.subjects}
    return json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def cover_key(book_id: str, thumb: Thumb) -> str:
    sha = hashlib.sha256(thumb.data).hexdigest()[:8]
    return f"{PREFIX}covers/{book_id}-{thumb.width}-{sha}.{thumb.fmt}"


def licence_ok(license_md: str) -> bool:
    """SE's LICENSE.md: U.S. public domain text and art, contributions dedicated under CC0 1.0."""
    t = " ".join(license_md.split())
    return "CC0 1.0" in t and "United States public domain" in t


def stage(out: Path, key: str, data: bytes, content_type: str, cache_control: str) -> Upload:
    path = out / key
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return Upload(key, path, content_type, cache_control, len(data), hashlib.sha256(data).hexdigest())


def book_entry(
    pick: Pick,
    meta: BookMeta,
    body: BodyMeta,
    body_upload: Upload,
    about_upload: Upload,
    covers: list[tuple[Thumb, str]],
    commit: str,
    year: int | None = None,
) -> dict:
    """One book of the index. `region` is "world" or "us" (U.S. public domain only: the Worker must serve the entry,
    its body, about file and covers to U.S. requests only); `year` is the build year it is computed for."""
    sizes: dict[int, dict] = {}
    for thumb, key in covers:
        sizes.setdefault(thumb.width, {"w": thumb.width, "h": thumb.height})[thumb.fmt] = key
    author = ", ".join(meta.authors)
    return {
        "id": pick.repo,
        "title": meta.title,
        "author": author,
        "translators": meta.translators,
        "year": pick.year,
        "year_approx": pick.year_approx,
        "language": meta.language.split("-")[0],
        "abstract": meta.abstract,
        "genres": meta.genres,
        "chars": body.char_count,
        "words": body.word_count,
        "blocks": body.block_count,
        "listening_minutes": listening_minutes(body.char_count),
        "body": {"key": body_upload.key, "bytes": body.body_bytes, "sha256": body.sha256},
        "about": about_upload.key,
        "cover": {"alt": f"{meta.title} by {author}", "sizes": [sizes[w] for w in sorted(sizes)]},
        "licence": LICENCE["id"],
        "region": pick.region(year),
        "world_from": pick.world_from,
        "last_death": pick.died,
        "source": {
            "name": "Standard Ebooks",
            "url": meta.url,
            "repo": f"https://github.com/standardebooks/{pick.repo}",
            "commit": commit,
        },
    }


def index(shelves: list[Shelf], entries: dict[str, dict], withdrawn: list[dict], now: datetime | None = None) -> dict:
    """The catalogue index. `us_only` lists the ids whose entry, body, about file and covers go to U.S. requests only
    (CF-IPCountry = US); every other country gets the index without them."""
    now = now or datetime.now(UTC)
    return {
        "v": 1,
        "generated_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "chars_per_second": CHARS_PER_SECOND,
        "licence": LICENCE,
        "credit": "Books and cover art from Standard Ebooks (standardebooks.org).",
        "shelves": [
            {
                "id": s.id,
                "title": s.title,
                "blurb": s.blurb,
                "books": [entries[p.repo] for p in s.picks if p.repo in entries],
            }
            for s in shelves
        ],
        "us_only": sorted(e["id"] for e in entries.values() if e["region"] == "us"),
        "withdrawn": withdrawn,
    }


def index_bytes(doc: dict) -> bytes:
    return json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
