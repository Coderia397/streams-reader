"""Reads the catalogue metadata from a Standard Ebooks package document (src/epub/content.opf).

Handles both metadata generations SE has used: the current one (`schema:abstract` short text, HTML long text in
`dc:description`, plain URL `dc:identifier`) and the older one (`dc:description` short text, `se:long-description`,
`url:`-prefixed identifier)."""

from __future__ import annotations

import html
import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from html.parser import HTMLParser

NS = {"opf": "http://www.idpf.org/2007/opf", "dc": "http://purl.org/dc/elements/1.1/"}
MAX_OPF_BYTES = 2 * 1024 * 1024
SE_URL = "https://standardebooks.org/ebooks/"


class OpfError(ValueError):
    pass


@dataclass
class BookMeta:
    url: str
    title: str
    authors: list[str]
    translators: list[str] = field(default_factory=list)
    illustrators: list[str] = field(default_factory=list)
    language: str = "en"
    abstract: str = ""
    description: str = ""
    subjects: list[str] = field(default_factory=list)
    genres: list[str] = field(default_factory=list)
    rights: str = ""
    se_word_count: int | None = None


class _Text(HTMLParser):
    """HTML → plain text: paragraphs become blank-line separated, tags are dropped."""

    BLOCKS = frozenset({"p", "div", "li", "blockquote", "br"})

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []

    def handle_starttag(self, tag, attrs):
        if tag in self.BLOCKS:
            self.parts.append("\n\n")

    def handle_data(self, data):
        self.parts.append(data)


def html_to_text(fragment: str) -> str:
    p = _Text()
    p.feed(fragment)
    p.close()
    paras = ("".join(p.parts)).split("\n\n")
    return "\n\n".join(" ".join(x.split()) for x in paras if x.strip())


def parse(opf: bytes) -> BookMeta:
    if len(opf) > MAX_OPF_BYTES:
        raise OpfError("package document too large")
    if b"<!DOCTYPE" in opf or b"<!ENTITY" in opf:
        raise OpfError("package document declares a DTD or entities")
    try:
        root = ET.fromstring(opf)
    except ET.ParseError as e:
        raise OpfError(f"package document is not well-formed: {e}") from e
    md = root.find("opf:metadata", NS)
    if md is None:
        raise OpfError("package document has no metadata")

    def texts(tag: str) -> list[str]:
        return [" ".join((e.text or "").split()) for e in md.findall(tag, NS) if (e.text or "").strip()]

    def metas(prop: str) -> list[str]:
        return [(m.text or "").strip() for m in md.findall("opf:meta", NS) if m.get("property") == prop]

    roles: dict[str, set[str]] = {}
    for m in md.findall("opf:meta", NS):
        if m.get("property") == "role" and m.get("refines", "").startswith("#"):
            roles.setdefault(m.get("refines")[1:], set()).add((m.text or "").strip())

    def people(role: str) -> list[str]:
        out = []
        for tag in ("dc:creator", "dc:contributor"):
            for e in md.findall(tag, NS):
                if role in roles.get(e.get("id", ""), set()) and (e.text or "").strip():
                    out.append(" ".join(e.text.split()))
        return out

    ident = (texts("dc:identifier") or [""])[0].removeprefix("url:")
    if not ident.startswith(SE_URL):
        raise OpfError(f"identifier is not a Standard Ebooks URL: {ident!r}")
    title = (texts("dc:title") or [""])[0]
    authors = people("aut") or texts("dc:creator")
    if not title or not authors:
        raise OpfError("title or author missing")

    # dc:description holds escaped HTML in the current generation; ElementTree has already unescaped one level.
    desc_el = md.find("dc:description", NS)
    raw_desc = (desc_el.text or "") if desc_el is not None else ""
    long_old = metas("se:long-description")
    abstract = (metas("schema:abstract") or [""])[0]
    if long_old:
        description, abstract = html_to_text(html.unescape(long_old[0])), abstract or html_to_text(raw_desc)
    elif "<p" in raw_desc:
        description = html_to_text(raw_desc)
    else:
        description = ""
        abstract = abstract or html_to_text(raw_desc)

    word_counts = metas("schema:wordCount") or metas("se:word-count")
    return BookMeta(
        url=ident,
        title=title,
        authors=authors,
        translators=people("trl"),
        illustrators=people("ill"),
        language=(texts("dc:language") or ["en"])[0],
        abstract=" ".join(abstract.split()),
        description=description,
        subjects=texts("dc:subject"),
        genres=metas("schema:genre") or metas("se:subject"),
        rights=(texts("dc:rights") or [""])[0],
        se_word_count=int(word_counts[0]) if word_counts and re.fullmatch(r"\d+", word_counts[0]) else None,
    )
