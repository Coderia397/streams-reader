"""The converter on two small real books: pack the SE source, run the app's importer, check the DocumentBody."""

from __future__ import annotations

import hashlib
import io
import json
import zipfile
from pathlib import Path

import pytest
from conftest import BOOKS, src_files

from secat import convert, opf, pack

KINDS = {"heading", "paragraph", "quote", "list_item"}
BOILERPLATE = ("Standard Ebooks", "Uncopyright", "public domain", "colophon")


def run(repo: str, binary: Path, tmp_path: Path) -> tuple[dict, convert.BodyMeta, bytes]:
    epub = tmp_path / f"{repo}.epub"
    epub.write_bytes(pack.pack(src_files(repo)))
    out = tmp_path / f"{repo}.json"
    meta = convert.convert([(epub, out)], binary)[out]
    assert not isinstance(meta, str), meta
    raw = out.read_bytes()
    return json.loads(raw), meta, raw


def test_pack_keeps_text_and_drops_boilerplate():
    epub = pack.pack(src_files("t-s-eliot_poetry"))
    with zipfile.ZipFile(io.BytesIO(epub)) as z:
        names = z.namelist()
        assert names[0] == "mimetype" and z.getinfo("mimetype").compress_type == zipfile.ZIP_STORED
        assert z.read("mimetype") == b"application/epub+zip"
    assert "epub/text/poetry.xhtml" in names and "epub/content.opf" in names
    assert not any(n.endswith(("imprint.xhtml", "colophon.xhtml", "uncopyright.xhtml")) for n in names)


def test_pack_requires_package_document():
    with pytest.raises(ValueError):
        pack.pack({"META-INF/container.xml": b"<container/>"})


@pytest.mark.parametrize("repo", BOOKS)
def test_body_shape_and_counts(repo, converter, tmp_path):
    body, meta, raw = run(repo, converter, tmp_path)
    assert body["v"] == 1
    blocks = body["blocks"]
    assert blocks and {b["kind"] for b in blocks} <= KINDS
    assert all(b["text"] == b["text"].strip() and b["text"] for b in blocks)
    # Counts are those the app stores in public.documents.
    reading = "\n\n".join(b["text"] for b in blocks)
    assert meta.char_count == len(reading)
    assert meta.block_count == len(blocks)
    assert meta.heading_count == sum(b["kind"] == "heading" for b in blocks)
    assert meta.body_bytes == len(raw) and meta.sha256 == hashlib.sha256(raw).hexdigest()
    # The importer's word count lands within 2% of SE's own count.
    se_words = opf.parse(src_files(repo)[pack.OPF]).se_word_count
    assert se_words and abs(meta.word_count - se_words) / se_words < 0.02
    # SE's imprint, colophon and licence pages are not read aloud.
    for phrase in BOILERPLATE:
        assert phrase.lower() not in reading.lower(), phrase


def test_eliot_poems_have_headings_and_lines(converter, tmp_path):
    body, meta, _ = run("t-s-eliot_poetry", converter, tmp_path)
    headings = [b["text"] for b in body["blocks"] if b["kind"] == "heading"]
    assert headings[0] == "Poetry"
    assert "The Love Song of J. Alfred Prufrock" in headings and "The Waste Land" in headings
    assert meta.title == "Poetry"
    texts = [b["text"] for b in body["blocks"]]
    assert any(t.startswith("Let us go then, you and I,") for t in texts)


def test_prophet_reads_in_spine_order(converter, tmp_path):
    body, meta, _ = run("kahlil-gibran_the-prophet", converter, tmp_path)
    texts = [b["text"] for b in body["blocks"]]
    assert texts[0] == "The Prophet" and body["blocks"][0]["kind"] == "heading"
    assert texts[1] == "By Kahlil Gibran."
    love = texts.index("Then said Almitra, Speak to us of Love.")
    clothes = texts.index("And the weaver said, Speak to us of Clothes.")
    assert love < clothes
    assert meta.author == "Kahlil Gibran"


def test_converter_reports_a_broken_book_without_stopping(converter, tmp_path):
    bad, good = tmp_path / "bad.epub", tmp_path / "good.epub"
    bad.write_bytes(b"not a zip")
    good.write_bytes(pack.pack(src_files("kahlil-gibran_the-prophet")))
    res = convert.convert([(bad, tmp_path / "bad.json"), (good, tmp_path / "good.json")], converter)
    assert isinstance(res[tmp_path / "bad.json"], str)
    assert isinstance(res[tmp_path / "good.json"], convert.BodyMeta)


@pytest.mark.xfail(
    strict=True,
    reason="nolabs-import keeps only the last <meta property='role'> per creator; Eliot's author roles end with 'dto'",
)
def test_importer_finds_author_with_several_roles(converter, tmp_path):
    _, meta, _ = run("t-s-eliot_poetry", converter, tmp_path)
    assert meta.author == "T. S. Eliot"
