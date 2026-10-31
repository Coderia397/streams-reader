"""Fetch the landing page's "Classics to start with" covers and opening lines from Standard Ebooks' public GitHub repos.

Standard Ebooks dedicates its covers and texts to the public domain (CC0 1.0; the books are believed to be in the U.S.
public domain). This script never crawls standardebooks.org: it sends one HEAD request per title to the book's own
page only to honour HTTP 451 withdrawals (2 s apart, never following links), then reads the composed cover
(`src/epub/images/cover.svg`), the reading order (`src/epub/content.opf`) and the first chapter from
raw.githubusercontent.com.

Writes into crates/site/assets/covers/: <slug>-200.avif/.webp and <slug>-300.avif/.webp (2:3, each at most 15 KB), covers.json (title,
author, Standard Ebooks URL, opening line) and CREDITS.md.

    uv run --no-project --python 3.12 --with resvg-py==0.2.6 --with pillow==11.3.0 \
        python -I crates/site/tools/covers.py --out crates/site/assets/covers [--work <scratch dir>]
"""
from __future__ import annotations

import argparse
import html
import io
import json
import pathlib
import re
import time
import urllib.error
import urllib.request

import resvg_py
from PIL import Image

UA = "StreamsReader-site-build/1 (+https://reader.streams.watch; help@streams.watch)"
# Display order (note 15, C3): neighbouring covers differ in colour.
BOOKS = [
    "mary-shelley/frankenstein",
    "jane-austen/pride-and-prejudice",
    "arthur-conan-doyle/the-adventures-of-sherlock-holmes",
    "herman-melville/moby-dick",
    "f-scott-fitzgerald/the-great-gatsby",
    "bram-stoker/dracula",
    "oscar-wilde/the-picture-of-dorian-gray",
    "lewis-carroll/alices-adventures-in-wonderland/john-tenniel",
    "fyodor-dostoevsky/crime-and-punishment/constance-garnett",
    "marcus-aurelius/meditations/george-long",
    "h-g-wells/the-time-machine",
    "charlotte-bronte/jane-eyre",
    "mark-twain/the-adventures-of-huckleberry-finn",
    "sun-tzu/the-art-of-war/lionel-giles",
    "virginia-woolf/mrs-dalloway",
    "homer/the-odyssey/william-cullen-bryant",
    "leo-tolstoy/anna-karenina/constance-garnett",
    "robert-louis-stevenson/the-strange-case-of-dr-jekyll-and-mr-hyde",
    "edgar-allan-poe/short-fiction",
    "emily-bronte/wuthering-heights",
]
FRONT = re.compile(r"^(titlepage|imprint|dedication|epigraph|halftitlepage|preface|introduction|foreword|"
                   r"translators?-|notice|explanatory|etymology|extracts|the-life|biographical|note|"
                   r"author|contents|preliminary|prefatory)")
WIDTHS = (200, 300)
MAX_BYTES = 15_000  # per cover file (note 15, A14)
# Opening lines read by the preview clips, checked by hand against the first chapter. Where the automatic pick was
# wrong (front matter, footnote markers) or too long for a 6-12 s clip, the line is set here.
LINES = {
    "frankenstein": "You will rejoice to hear that no disaster has accompanied the commencement of an enterprise which "
                    "you have regarded with such evil forebodings.",
    "moby-dick": "Call me Ishmael. Some years ago, never mind how long precisely, having little or no money in my purse, "
                 "and nothing particular to interest me on shore, I thought I would sail about a little and see the "
                 "watery part of the world.",
    "dracula": "Left Munich at 8:35 P.M., on 1st May, arriving at Vienna early next morning; should have arrived at "
               "6:46, but train was an hour late.",
    "the-picture-of-dorian-gray": "The studio was filled with the rich odour of roses, and when the light summer wind "
                                  "stirred amidst the trees of the garden, there came through the open door the heavy "
                                  "scent of the lilac.",
    "alices-adventures-in-wonderland": "Alice was beginning to get very tired of sitting by her sister on the bank, and "
                                       "of having nothing to do.",
    "crime-and-punishment": "On an exceptionally hot evening early in July a young man came out of the garret in which "
                            "he lodged in S. Place and walked slowly, as though in hesitation, towards K. bridge.",
    "meditations": "From my grandfather Verus I learned good morals and the government of my temper.",
    "jane-eyre": "There was no possibility of taking a walk that day.",
    "the-adventures-of-huckleberry-finn": "You don't know about me without you have read a book by the name of The "
                                          "Adventures of Tom Sawyer; but that ain't no matter.",
    "the-art-of-war": "Sun Tzu said: The art of war is of vital importance to the State.",
    "the-odyssey": "Tell me, O Muse, of that sagacious man who, having overthrown the sacred town of Ilium, wandered "
                   "far and visited the capitals of many nations.",
    "anna-karenina": "Happy families are all alike; every unhappy family is unhappy in its own way.",
    "the-strange-case-of-dr-jekyll-and-mr-hyde": "Mr. Utterson the lawyer was a man of a rugged countenance that was "
                                                 "never lighted by a smile.",
    "short-fiction": "True! Nervous, very, very dreadfully nervous I had been and am; but why will you say that I am "
                     "mad?",
    "wuthering-heights": "I have just returned from a visit to my landlord, the solitary neighbour that I shall be "
                         "troubled with.",
}


def get(url: str, method: str = "GET") -> tuple[int, bytes]:
    req = urllib.request.Request(url, method=method, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, (r.read() if method == "GET" else b"")
    except urllib.error.HTTPError as e:
        return e.code, b""


def strip_tags(s: str) -> str:
    s = re.sub(r"<(abbr|span|i|em|b|a)[^>]*>", "", s)
    s = re.sub(r"</(abbr|span|i|em|b|a)>", "", s)
    s = re.sub(r"<br\s*/?>", " ", s)
    s = re.sub(r"<[^>]+>", "", s)
    s = html.unescape(s).replace(" ", " ").replace("⁠", "")
    return re.sub(r"\s+", " ", s).strip()


def opening_line(raw: str, opf: str) -> str:
    spine = re.findall(r'<itemref idref="([^"]+)"', opf)
    first = next((i for i in spine if re.match(r"(chapter|letter|book|part|stave)-0*1(\.|-)", i)), None)
    if first is None:
        first = next((i for i in spine if not FRONT.match(i)), spine[0])
    hrefs = dict(re.findall(r'<item href="([^"]+)" id="([^"]+)"', opf))
    by_id = {v: k for k, v in hrefs.items()}
    status, body = get(f"{raw}/src/epub/{by_id.get(first, 'text/' + first)}")
    if status != 200:
        return ""
    doc = body.decode("utf-8")
    doc = re.sub(r"<header.*?</header>", "", doc, flags=re.S)
    doc = re.sub(r"<blockquote.*?</blockquote>", "", doc, flags=re.S)
    for p in re.findall(r"<p[^>]*>(.*?)</p>", doc, flags=re.S):
        text = strip_tags(p)
        if len(text.split()) >= 4:
            return first_sentences(text)
    return ""


def first_sentences(text: str, limit: int = 170) -> str:
    out = ""
    for m in re.finditer(r".+?[.!?;](?:[’”\"')]*)(?=\s|$)", text):
        cand = (out + " " + m.group(0).strip()).strip() if out else m.group(0).strip()
        if out and len(cand) > limit:
            break
        out = cand
        if len(out) >= 60:
            break
    return out or text[:limit]


def render(svg: bytes, w: int) -> Image.Image:
    png = bytes(resvg_py.svg_to_bytes(svg_string=svg.decode("utf-8"), width=w, height=w * 3 // 2))
    return Image.open(io.BytesIO(png)).convert("RGB")


def save_under(img: Image.Image, f: pathlib.Path, fmt: str) -> int:
    """Highest quality (from 60 down to 16) that keeps the file under MAX_BYTES."""
    for q in range(60, 15, -4):
        img.save(f, fmt.upper(), quality=q, **({"speed": 4} if fmt == "avif" else {"method": 6}))
        if f.stat().st_size <= MAX_BYTES:
            break
    return f.stat().st_size


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, type=pathlib.Path)
    ap.add_argument("--work", type=pathlib.Path, help="cache downloaded SVGs here")
    ap.add_argument("--skip-status", action="store_true",
                    help="re-encode from the --work cache without the 451 check (only right after a checked run)")
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    books = []
    for path in BOOKS:
        parts = path.split("/")
        slug = "-".join(parts[1:2])
        repo = "_".join(parts)
        page = f"https://standardebooks.org/ebooks/{path}"
        cache = a.work / f"{repo}.svg" if a.work else None
        if a.skip_status and cache and cache.exists():
            status = 200
        else:
            status, _ = get(page, "HEAD")
            time.sleep(2)
        if status == 451:
            print(f"skip {path}: 451 withdrawn")
            continue
        if status != 200:
            print(f"skip {path}: HTTP {status}")
            continue
        raw = f"https://raw.githubusercontent.com/standardebooks/{repo}/master"
        if cache and cache.exists():
            svg = cache.read_bytes()
        else:
            code, svg = get(f"{raw}/src/epub/images/cover.svg")
            if code != 200:
                print(f"skip {path}: cover {code}")
                continue
            if cache:
                cache.parent.mkdir(parents=True, exist_ok=True)
                cache.write_bytes(svg)
        opf_cache = a.work / f"{repo}.opf" if a.work else None
        if opf_cache and opf_cache.exists():
            opf = opf_cache.read_text()
        else:
            opf = get(f"{raw}/src/epub/content.opf")[1].decode("utf-8")
            if opf_cache:
                opf_cache.write_text(opf)
        title = strip_tags(re.search(r"<dc:title[^>]*>(.*?)</dc:title>", opf, re.S).group(1))
        author = strip_tags(re.search(r"<dc:creator[^>]*>(.*?)</dc:creator>", opf, re.S).group(1))
        line = LINES.get(slug) or opening_line(raw, opf)
        sizes = {}
        for w in WIDTHS:
            img = render(svg, w)
            for fmt in ("avif", "webp"):
                f = a.out / f"{slug}-{w}.{fmt}"
                sizes[f.name] = save_under(img, f, fmt)
        books.append({"slug": slug, "title": title, "author": author, "url": page, "line": line})
        print(f"{slug:40s} {sizes}  | {line}")
    (a.out / "covers.json").write_text(json.dumps({"source": "Standard Ebooks (CC0 1.0)", "books": books},
                                                  indent=1, ensure_ascii=False) + "\n")
    rows = "\n".join(f"| {b['title']} | {b['author']} | <{b['url']}> |" for b in books)
    (a.out / "CREDITS.md").write_text(
        "# Cover credits\n\nEvery cover here is from [Standard Ebooks](https://standardebooks.org), resized. Standard "
        "Ebooks dedicates its covers and editions to the public domain under\n[CC0 1.0](https://creativecommons.org/"
        "publicdomain/zero/1.0/); the cover paintings and the books are believed to be in the United States public "
        "domain.\nOutside the U.S., check your local laws. Made by `crates/site/tools/covers.py` from the public "
        "GitHub repositories.\n\n| Title | Author | Standard Ebooks |\n|---|---|---|\n" + rows + "\n")


if __name__ == "__main__":
    main()
