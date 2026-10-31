"""Package-document metadata, curation file, catalogue entries and cover thumbnails."""

from __future__ import annotations

import io
from pathlib import Path

import pytest
from conftest import CATALOG, load_fixture, src_files
from PIL import Image

from secat import catalog, convert, covers, curation, opf, pack

OLD_OPF = b"""<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:identifier id="uid">url:https://standardebooks.org/ebooks/jane-austen/persuasion</dc:identifier>
<dc:title id="title">Persuasion</dc:title>
<dc:creator id="author">Jane Austen</dc:creator>
<meta property="role" refines="#author" scheme="marc:relators">aut</meta>
<dc:description id="description">A story of second chances.</dc:description>
<meta id="long-description" property="se:long-description" refines="#description">
&lt;p&gt;Anne Elliot &lt;i&gt;was&lt;/i&gt; persuaded.&lt;/p&gt;&lt;p&gt;Eight years later&amp;hellip;&lt;/p&gt;
</meta>
<meta property="se:subject">Fiction</meta>
<meta property="se:word-count">83658</meta>
<dc:language>en-GB</dc:language>
</metadata></package>"""


def test_current_generation_opf():
    m = opf.parse(src_files("kahlil-gibran_the-prophet")[pack.OPF])
    assert m.url == "https://standardebooks.org/ebooks/kahlil-gibran/the-prophet"
    assert m.title == "The Prophet" and m.authors == ["Kahlil Gibran"]
    assert m.abstract and "<" not in m.abstract
    assert m.description.startswith("The Prophet is") and "<" not in m.description
    assert m.se_word_count and m.se_word_count > 10_000
    assert m.language.startswith("en")


def test_old_generation_opf():
    m = opf.parse(OLD_OPF)
    assert m.url == "https://standardebooks.org/ebooks/jane-austen/persuasion"
    assert m.abstract == "A story of second chances."
    assert m.description == "Anne Elliot was persuaded.\n\nEight years later…"
    assert m.genres == ["Fiction"] and m.se_word_count == 83658


def test_opf_rejects_entities_and_foreign_ids():
    with pytest.raises(opf.OpfError):
        opf.parse(b'<!DOCTYPE x [<!ENTITY a "b">]><package/>')
    with pytest.raises(opf.OpfError):
        opf.parse(OLD_OPF.replace(b"standardebooks.org", b"example.org"))


def test_author_with_several_roles():
    m = opf.parse(src_files("t-s-eliot_poetry")[pack.OPF])
    assert m.authors == ["T. S. Eliot"]


def test_licence_text_of_fixtures_is_accepted():
    for repo in ("kahlil-gibran_the-prophet", "t-s-eliot_poetry"):
        assert catalog.licence_ok(load_fixture(repo)["LICENSE.md"].decode())
    assert not catalog.licence_ok("All rights reserved.")


def test_shelves_file_is_valid():
    shelves = curation.load(CATALOG / "shelves.toml")
    assert len(shelves) == 11
    assert 55 <= sum(len(s.picks) for s in shelves) <= 70


def test_curation_rejects_duplicates():
    doc = {
        "version": 1,
        "shelf": [
            {"id": "a", "title": "A", "books": [{"repo": "x-y_z", "year": 1900, "died": 1910}]},
            {"id": "b", "title": "B", "books": [{"repo": "x-y_z", "year": 1900, "died": 1910}]},
        ],
    }
    with pytest.raises(curation.CurationError):
        curation.parse(doc)


def test_listening_minutes_at_15_chars_per_second():
    assert catalog.listening_minutes(900) == 1
    assert catalog.listening_minutes(901) == 2
    assert catalog.listening_minutes(1) == 1
    assert catalog.listening_minutes(64710) == 72


def test_cover_thumbnails():
    svg = (
        b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1400 2100">'
        b'<rect width="1400" height="2100" fill="#2F6BFF"/><path d="M 50,1620 1350,1620 1350,2050 50,2050 Z"/></svg>'
    )
    thumbs = covers.render(svg)
    assert sorted((t.width, t.height, t.fmt) for t in thumbs) == [
        (200, 300, "avif"),
        (200, 300, "webp"),
        (400, 600, "avif"),
        (400, 600, "webp"),
    ]
    for t in thumbs:
        assert Image.open(io.BytesIO(t.data)).size == (t.width, t.height)
        assert catalog.cover_key("b", t).startswith(f"catalog/v1/covers/b-{t.width}-")


def test_entry_and_index_shape(tmp_path: Path):
    pick = curation.Pick("kahlil-gibran_the-prophet", 1923, False, "poetry", 1931)
    meta = opf.parse(src_files(pick.repo)[pack.OPF])
    body = convert.BodyMeta("The Prophet", "Kahlil Gibran", 64710, 12364, 811, 1, 88475, "ab" * 32)
    up = catalog.stage(tmp_path, catalog.body_key(pick.repo, body.sha256), b"{}", "application/json", catalog.IMMUTABLE)
    about_data = catalog.about_bytes(meta)
    about = catalog.stage(tmp_path, catalog.about_key(pick.repo, about_data), about_data, "application/json", "x")
    entry = catalog.book_entry(pick, meta, body, up, about, [], "c0ffee")
    assert entry["about"].startswith("catalog/v1/books/kahlil-gibran_the-prophet/about-")
    assert "description" not in entry and b"The Prophet is" in about_data
    assert entry["body"]["key"] == "catalog/v1/books/kahlil-gibran_the-prophet/body-abababababab.json"
    assert entry["listening_minutes"] == 72 and entry["licence"] == "CC0-1.0"
    assert entry["source"]["url"] == meta.url
    assert (entry["region"], entry["world_from"], entry["last_death"]) == ("world", 2002, 1931)
    shelf = curation.Shelf("poetry", "Poems to hear", "", (pick,))
    doc = catalog.index([shelf], {pick.repo: entry}, [])
    assert doc["v"] == 1 and doc["shelves"][0]["books"][0]["id"] == pick.repo
    assert doc["us_only"] == []
    assert catalog.index_bytes(doc).startswith(b'{"v":1')


# Titles still in copyright under life + 70 in 2026 (death year of the last author or translator, recorded in note 27).
US_ONLY_2026 = {
    "agatha-christie_the-mysterious-affair-at-styles": 1976,
    "t-s-eliot_poetry": 1965,
    "bertrand-russell_the-problems-of-philosophy": 1970,
    "e-m-forster_a-room-with-a-view": 1970,
    "w-e-b-du-bois_the-souls-of-black-folk": 1963,
    "apsley-cherry-garrard_the-worst-journey-in-the-world": 1959,
    "sun-tzu_the-art-of-war_lionel-giles": 1958,
    "yevgeny-zamyatin_we_gregory-zilboorg": 1959,
}


def test_every_pick_has_a_death_year_and_the_right_region():
    picks = [p for s in curation.load(CATALOG / "shelves.toml") for p in s.picks]
    us = {p.repo: p.died for p in picks if p.region(2026) == "us"}
    assert us == US_ONLY_2026
    blue_castle = next(p for p in picks if p.repo == "l-m-montgomery_the-blue-castle")
    assert (blue_castle.died, blue_castle.region(2026)) == (1942, "world")
    giles = next(p for p in picks if p.repo == "sun-tzu_the-art-of-war_lionel-giles")
    assert giles.world_from == 2029 and giles.region(2028) == "us" and giles.region(2029) == "world"


def test_curation_requires_a_death_year():
    for died in (None, "1900", True, 9999):
        book = {"repo": "x-y_z", "year": 1900} | ({} if died is None else {"died": died})
        with pytest.raises(curation.CurationError, match="died is required"):
            curation.parse({"version": 1, "shelf": [{"id": "a", "title": "A", "books": [book]}]})


def test_index_lists_us_only_ids():
    a = curation.Pick("a-b_c", 1920, False, "s", 1976)
    b = curation.Pick("d-e_f", 1900, False, "s", 1910)
    entries = {p.repo: {"id": p.repo, "region": p.region(2026)} for p in (a, b)}
    doc = catalog.index([curation.Shelf("s", "S", "", (a, b))], entries, [])
    assert doc["us_only"] == ["a-b_c"]
    assert [e["id"] for e in doc["shelves"][0]["books"]] == ["a-b_c", "d-e_f"]
