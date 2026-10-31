# Explore catalogue builder

Builds the in-app "Explore" library from Standard Ebooks (CC0, U.S. public domain) and puts it on R2. Design and
decisions: `notes/23-explore-catalog.md`.

```sh
# build (stages every object under <out>/catalog/v1/…, writes upload-plan.json and report.json)
uv run tools/catalog/build.py --out /path/to/scratch/catalog-out
# build and upload to R2 bucket nolabs-media (credentials: ~/.config/streams/nolabs-r2.env)
uv run tools/catalog/build.py --out /path/to/scratch/catalog-out --upload
# one or two titles while editing shelves.toml (local only: --only is refused with --upload)
uv run tools/catalog/build.py --out … --only jane-austen_persuasion
# tests (converter on two small real books, metadata, covers)
cd tools/catalog && uv run --no-project --with pytest --with httpx --with pillow --with resvg-py pytest -q tests
```

Upload rules (`build.py` exits 2 or 1 and uploads nothing otherwise): `--upload` is refused with `--only` (a partial
index would empty the other live shelves) and with `--no-status-check` (a withdrawn title would be served). When any
title fails (network, licence, import, cover), nothing is uploaded unless `--allow-partial` is given; withdrawn (451)
titles are left out and are not failures. One title's failure never stops the others.

- `shelves.toml`: the curated shelves (Standard Ebooks repo names, first-publication year, and `died`: the death year
  of the last author, translator or illustrator in the edition). Under life + 70 a title is `region: "world"` from
  `died + 71`; before that the index marks it `region: "us"` and lists it in `us_only`, and the Worker must serve it,
  its body, about file and covers only to U.S. requests (`CF-IPCountry: US`).
- `epub2body/`: a Rust binary around `nolabs_import::from_epub`, so catalogue bodies are byte-for-byte what the web
  app would upload for the same EPUB. Built into the root `target/` on first use.
- `secat/`: `curation` (shelves file), `sources` (GitHub API, raw files pinned to a commit, the 451 status check),
  `opf` (metadata), `pack` (minimal EPUB without SE boilerplate), `convert` (runs epub2body), `covers` (AVIF/WebP
  thumbnails), `catalog` (entries, R2 keys, staging), `r2` (upload).

Source rules: text and covers come only from `github.com/standardebooks/<repo>` (API + `raw.githubusercontent.com`
at a pinned commit). The only standardebooks.org request is one HEAD request for each title's canonical URL, 2 s
apart, to catch withdrawals (HTTP 451 → skipped and listed under `withdrawn`). No links are followed.
