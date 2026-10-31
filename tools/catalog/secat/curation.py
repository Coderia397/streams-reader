"""Loads and validates the hand-curated shelves file (tools/catalog/shelves.toml).

Every pick carries `died`: the death year of the last-surviving person whose work is in the edition (author,
translator, illustrator, introduction writer). It decides where the title may be served (see `region`)."""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

REPO_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*(?:_[a-z0-9]+(?:-[a-z0-9]+)*)+$")
SHELF_ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
LIFE_PLUS = 70  # the longest common term after the author's death (UK, EU, and most Berne countries); assumed basis


class CurationError(ValueError):
    pass


@dataclass(frozen=True)
class Pick:
    repo: str
    year: int
    year_approx: bool
    shelf: str
    died: int  # death year of the last-surviving author, translator or illustrator in this edition
    died_approx: bool = False  # True when `died` is an upper bound because the exact year is unknown

    @property
    def world_from(self) -> int:
        """First calendar year the edition is out of copyright under life + 70 (terms run to the end of a year)."""
        return self.died + LIFE_PLUS + 1

    def region(self, year: int | None = None) -> str:
        """Returns "world" once life + 70 has run out, else "us": U.S. public domain only, so serve it to U.S. requests only."""
        year = year if year is not None else datetime.now(UTC).year
        return "world" if year >= self.world_from else "us"


@dataclass(frozen=True)
class Shelf:
    id: str
    title: str
    blurb: str
    picks: tuple[Pick, ...]


def load(path: Path) -> list[Shelf]:
    return parse(tomllib.loads(path.read_text(encoding="utf-8")))


def parse(doc: dict) -> list[Shelf]:
    if doc.get("version") != 1:
        raise CurationError("shelves file: expected version = 1")
    shelves: list[Shelf] = []
    seen_shelves: set[str] = set()
    seen_repos: set[str] = set()
    for raw in doc.get("shelf", []):
        sid = raw.get("id", "")
        if not SHELF_ID_RE.match(sid) or sid in seen_shelves:
            raise CurationError(f"shelf id missing, malformed or repeated: {sid!r}")
        seen_shelves.add(sid)
        title, blurb = raw.get("title", "").strip(), raw.get("blurb", "").strip()
        if not title:
            raise CurationError(f"shelf {sid}: title is required")
        picks = []
        for b in raw.get("books", []):
            repo = b.get("repo", "")
            if not REPO_RE.match(repo):
                raise CurationError(f"shelf {sid}: malformed repo {repo!r}")
            if repo in seen_repos:
                raise CurationError(f"repo {repo} appears on more than one shelf")
            year = b.get("year")
            if not isinstance(year, int) or not -3000 <= year <= 1930:
                raise CurationError(f"{repo}: year must be an integer between -3000 and 1930")
            died = b.get("died")
            if not isinstance(died, int) or isinstance(died, bool) or not -3000 <= died <= datetime.now(UTC).year:
                raise CurationError(
                    f"{repo}: died is required: the death year of the last author, translator or illustrator"
                )
            seen_repos.add(repo)
            approx = bool(b.get("year_approx", False))
            picks.append(Pick(repo, year, approx, sid, died, bool(b.get("died_approx", False))))
        if not picks:
            raise CurationError(f"shelf {sid} has no books")
        shelves.append(Shelf(sid, title, blurb, tuple(picks)))
    if not shelves:
        raise CurationError("shelves file has no [[shelf]] entries")
    return shelves
