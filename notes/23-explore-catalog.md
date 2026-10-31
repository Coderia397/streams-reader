# 23. Explore catalogue: Standard Ebooks pipeline and what is on R2

Kind: finding + decision + reference. The Explore library data pipeline is built (`tools/catalog/`), and its output is on R2. That output is 66 Standard Ebooks titles on 11 shelves, 397 objects, 38.7 MB under `nolabs-media/catalog/v1/`. Every body is the exact `DocumentBody` the web app's EPUB importer would produce. The app and Worker still need a read route and an "Add to library" path (section 5).

Correction 2026-10-31 (later the same day): section 4's year caveat named the wrong titles and the wrong risk; superseded by [27](27-catalog-rights-and-upload-safety.md), which lists the 8 titles still in copyright under life + 70 and the `region` / `us_only` fields. The index shape in section 5 gained `region`, `world_from`, `last_death` and `us_only` (note 27).

Builds on note 15 (C1 licences, C4 Explore design) and note 17 (documents table, R2 `docs/<user_id>/<doc_id>/body.json`).

## 2026-10-31

### 1. What was built (verified)
- `tools/catalog/build.py`: a uv script (PEP 723). Per title it does the following:
  - Pins the repo's head commit through the GitHub API.
  - Fetches `LICENSE.md`, `src/META-INF/container.xml`, `src/epub/content.opf`, the `src/epub/text/*.xhtml` files and the compiled `src/epub/images/cover.svg` from `raw.githubusercontent.com` at that commit (cached on disk).
  - Checks the licence text.
  - Checks the title's canonical standardebooks.org URL with one HEAD request, 2 s apart. A 451 answer skips the title and lists it under `withdrawn`.
  - Packs a minimal EPUB and converts it.
  - Renders the thumbnails.
  - Writes `catalog.json`, `upload-plan.json` and `report.json`. With `--upload` it also puts the plan on R2.
- `tools/catalog/epub2body/`: a Rust binary (its own `[workspace]`, built into the root `target/`) around `nolabs_import::from_epub`. Its output is `serde_json::to_vec(&DocumentBody)`, and its counts are the same as `crates/web/src/import_flow/meta.rs::build_meta` (`reading_text`, `word_count`). Reasons:
  - Bodies, char offsets and cache keys match what a user would get by importing the same EPUB.
  - There is no second normaliser to drift.
- `tools/catalog/secat/`: one small module per step (`curation`, `sources`, `opf`, `pack`, `convert`, `covers`, `catalog`, `r2`).
- `tools/catalog/shelves.toml`: the curation (section 3).
- EPUB packing rule: SE's boilerplate pages are left out (imprint, colophon, uncopyright, half-title, list of illustrations, endnotes, toc), so the audio starts at the title page ("Title. By Author. Translated by …") and then the book. Footnote references and endnotes are already skipped by the importer's `epub:type` rules.
- Covers: resvg renders SE's compiled `cover.svg` (the painting is embedded as JPEG and the title lettering is already turned into paths, so no fonts are needed). Pillow then writes AVIF q50 and WebP q70 at 200x300 and 400x600.
- Supabase: `supabase/migrations/0005_catalog.sql` adds `documents.catalog_id` and a unique index on (user_id, catalog_id). **Not applied.**
- Tests:
  - `tools/catalog/tests`: 17 pass and 1 is an expected failure (xfail, see section 6).
  - The tests run the converter on two small real books, *The Prophet* and T. S. Eliot's *Poetry*. Their fixtures are zips of the repo sources at a pinned commit (54 KB and 52 KB).
  - They check the following:
    - the body's shape and kinds;
    - the char, block and heading counts, the byte count and the sha;
    - the word count is within 2% of SE's count;
    - the boilerplate is gone;
    - spine order;
    - poem headings.
  - They also cover the OPF parser (both SE metadata generations), the curation checks, keys, listening minutes and thumbnails.
  - `epub2body` has 2 Rust unit tests.

### 2. Measurements (verified)
- Full build:
  - Command: `uv run tools/catalog/build.py --out <scratch>/out --upload`, at 2026-10-31 ~01:25 UTC+3.
  - Result: 66/66 titles, 0 withdrawn, 397 objects, 38,744,722 bytes, 202 s with a warm source cache. The cold run took 397 s, including the 66 SE status checks at 2 s spacing.
- R2 read-back: `catalog.json` was fetched through the S3 API with `application/json` and `public, max-age=300`. The Pride and Prejudice body was fetched; its length and sha256 match the index and it carries `immutable`. `list_objects_v2 catalog/` gives 397 objects and 38.7 MB.
- `catalog.json`: 90,645 bytes raw, 20,504 bytes gzip -9. Long descriptions and subjects were moved to per-book `about` files; with them inline the index was 50,949 bytes gzip.
- Bodies:
  - 64,710 to 1,806,658 chars per title, 567.7 h of listening in total.
  - The largest body is Poe's *Short Fiction* at 1.94 MB.
  - Words per title from the importer, divided by SE's `schema:wordCount`: 0.980 to 1.014. Only *The Art of War* is outside ±1.5%.
- Thumbnails, average and maximum per format and size (66 each):

  | Size | Format | Average | Maximum |
  |---|---|---|---|
  | 200w | AVIF | 5.9 KB | 10.9 KB |
  | 200w | WebP | 9.0 KB | 19.3 KB |
  | 400w | AVIF | 20.4 KB | 43.0 KB |
  | 400w | WebP | 33.4 KB | 73.8 KB |

  - Note 15 guessed 8–15 KB for the 200w AVIF; the measured average is lower.
  - A contact sheet of all 66 was checked by eye: every cover shows its painting and title box.
- Listening time: `ceil(chars / 15 / 60)` minutes at 15 chars/s. This is an assumed Kokoro 1x rate; it agrees within 4% with note 15's 155 wpm on Pride and Prejudice (12 h 40 m against 13 h 08 m). Recalibrate it against real render timings.

### 3. Catalogue (66 titles, all CC0 / U.S. public domain, all checked 200 on standardebooks.org on 2026-10-31)
**The great classics** (`classics`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Pride and Prejudice | Jane Austen | 1813 | 122,172 | 12 h 40 m | 0.75 MB |
| Jane Eyre | Charlotte Brontë | 1847 | 187,821 | 18 h 54 m | 1.16 MB |
| Wuthering Heights | Emily Brontë | 1847 | 116,830 | 11 h 56 m | 0.72 MB |
| Crime and Punishment | Fyodor Dostoevsky (tr. Constance Garnett) | 1866 | 204,583 | 20 h 57 m | 1.28 MB |
| Moby Dick | Herman Melville | 1851 | 212,981 | 22 h 11 m | 1.30 MB |
| The Great Gatsby | F. Scott Fitzgerald | 1925 | 48,691 | 4 h 58 m | 0.33 MB |

**Sea and adventure** (`adventure`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Treasure Island | Robert Louis Stevenson | 1883 | 68,668 | 6 h 41 m | 0.41 MB |
| The Three Musketeers | Alexandre Dumas (tr. William Robson) | 1844 | 230,009 | 23 h 56 m | 1.57 MB |
| Around the World in Eighty Days | Jules Verne (tr. George Makepeace Towle) | 1872 | 63,154 | 6 h 46 m | 0.42 MB |
| The Call of the Wild | Jack London | 1903 | 32,028 | 3 h 15 m | 0.19 MB |
| King Solomon’s Mines | H. Rider Haggard | 1885 | 81,921 | 8 h 08 m | 0.49 MB |
| The Prisoner of Zenda | Anthony Hope | 1894 | 54,082 | 5 h 19 m | 0.35 MB |

**Mysteries and detectives** (`mystery`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| The Adventures of Sherlock Holmes | Arthur Conan Doyle | 1892 | 105,083 | 10 h 24 m | 0.65 MB |
| The Hound of the Baskervilles | Arthur Conan Doyle | 1902 | 59,494 | 5 h 53 m | 0.37 MB |
| The Mysterious Affair at Styles | Agatha Christie | 1920 | 57,065 | 5 h 56 m | 0.41 MB |
| The Moonstone | Wilkie Collins | 1868 | 196,965 | 19 h 49 m | 1.20 MB |
| The Innocence of Father Brown | G. K. Chesterton | 1911 | 79,380 | 8 h 09 m | 0.49 MB |
| The Thirty-Nine Steps | John Buchan | 1915 | 41,293 | 3 h 58 m | 0.24 MB |

**Love and society** (`romance`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Persuasion | Jane Austen | 1817 | 83,532 | 8 h 37 m | 0.50 MB |
| North and South | Elizabeth Gaskell | 1855 | 182,967 | 18 h 17 m | 1.10 MB |
| A Room With a View | E. M. Forster | 1908 | 67,197 | 6 h 56 m | 0.45 MB |
| The Age of Innocence | Edith Wharton | 1920 | 102,573 | 10 h 40 m | 0.65 MB |
| The Blue Castle | L. M. Montgomery | 1926 | 69,703 | 7 h 10 m | 0.44 MB |
| Far from the Madding Crowd | Thomas Hardy | 1874 | 138,927 | 14 h 15 m | 0.89 MB |

**Early science fiction** (`science-fiction`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Frankenstein | Mary Shelley | 1818 | 78,022 | 8 h 05 m | 0.46 MB |
| The Time Machine | H. G. Wells | 1895 | 32,565 | 3 h 19 m | 0.19 MB |
| The War of the Worlds | H. G. Wells | 1898 | 60,458 | 6 h 14 m | 0.37 MB |
| Journey to the Center of the Earth | Jules Verne (tr. F. A. Malleson) | 1864 | 72,352 | 7 h 32 m | 0.48 MB |
| Flatland | Edwin A. Abbott | 1884 | 33,683 | 3 h 32 m | 0.21 MB |
| A Princess of Mars | Edgar Rice Burroughs | 1912 | 67,271 | 6 h 50 m | 0.40 MB |
| We | Yevgeny Zamyatin (tr. Gregory Zilboorg) | 1924 | 64,540 | 6 h 33 m | 0.40 MB |

**Ideas that last** (`philosophy`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Meditations | Marcus Aurelius (tr. George Long) | c. 180 | 45,532 | 4 h 34 m | 0.26 MB |
| Tao Te Ching | Laozi (tr. James Legge) | c. 400 BCE | 25,286 | 2 h 36 m | 0.15 MB |
| The Art of War | Sun Tzu (tr. Lionel Giles) | c. 500 BCE | 23,532 | 2 h 24 m | 0.15 MB |
| The Prince | Niccolò Machiavelli (tr. W. K. Marriott) | 1532 | 34,981 | 3 h 39 m | 0.21 MB |
| On Liberty | John Stuart Mill | 1859 | 50,976 | 5 h 32 m | 0.30 MB |
| The Problems of Philosophy | Bertrand Russell | 1912 | 43,042 | 4 h 32 m | 0.26 MB |

**Poems to hear** (`poetry`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Poetry | John Keats | c. 1821 | 94,898 | 9 h 43 m | 0.57 MB |
| Poetry | Edgar Allan Poe | c. 1849 | 15,613 | 1 h 34 m | 0.10 MB |
| Poetry | W. B. Yeats | c. 1928 | 63,869 | 6 h 19 m | 0.40 MB |
| Poetry | T. S. Eliot | c. 1922 | 14,608 | 1 h 30 m | 0.09 MB |
| Gitanjali | Rabindranath Tagore | 1912 | 14,606 | 1 h 24 m | 0.09 MB |
| The Prophet | Kahlil Gibran | 1923 | 12,364 | 1 h 12 m | 0.09 MB |

**Short stories** (`short-stories`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Short Fiction | Edgar Allan Poe | c. 1845 | 319,516 | 33 h 28 m | 1.94 MB |
| The Strange Case of Dr. Jekyll and Mr. Hyde | Robert Louis Stevenson | 1886 | 25,726 | 2 h 34 m | 0.15 MB |
| Dubliners | James Joyce | 1914 | 67,895 | 6 h 51 m | 0.43 MB |
| The Turn of the Screw | Henry James | 1898 | 42,830 | 4 h 14 m | 0.26 MB |
| Short Fiction | M. R. James | c. 1911 | 160,950 | 15 h 48 m | 0.92 MB |
| Short Fiction | Kate Chopin | c. 1897 | 132,634 | 13 h 32 m | 0.84 MB |

**For younger listeners** (`children`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Alice’s Adventures in Wonderland | Lewis Carroll | 1865 | 26,874 | 2 h 41 m | 0.17 MB |
| The Wind in the Willows | Kenneth Grahame | 1908 | 59,354 | 6 h 00 m | 0.36 MB |
| The Wonderful Wizard of Oz | L. Frank Baum | 1900 | 39,517 | 3 h 50 m | 0.24 MB |
| Peter and Wendy | J. M. Barrie | 1911 | 47,284 | 4 h 43 m | 0.31 MB |
| The Secret Garden | Frances Hodgson Burnett | 1911 | 81,185 | 7 h 55 m | 0.51 MB |
| The Jungle Book | Rudyard Kipling | 1894 | 51,680 | 5 h 02 m | 0.31 MB |

**History and lives** (`history`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Narrative of the Life of Frederick Douglass | Frederick Douglass | 1845 | 40,601 | 4 h 06 m | 0.23 MB |
| Up from Slavery | Booker T. Washington | 1901 | 77,049 | 7 h 46 m | 0.44 MB |
| The Autobiography of Benjamin Franklin | Benjamin Franklin | 1791 | 64,638 | 6 h 44 m | 0.38 MB |
| Eminent Victorians | Lytton Strachey | 1918 | 98,878 | 10 h 35 m | 0.60 MB |
| The Worst Journey in the World | Apsley Cherry-Garrard | 1922 | 240,579 | 23 h 51 m | 1.36 MB |
| Commentaries on the Gallic War | Julius Caesar (tr. W. A. McDevitte, W. S. Bohn) | c. 50 BCE | 75,104 | 7 h 57 m | 0.44 MB |

**Essays and arguments** (`essays`)

| Title | Author | Year | Words | Listening | Body |
|---|---|---|---|---|---|
| Walden | Henry David Thoreau | 1854 | 107,175 | 10 h 43 m | 0.60 MB |
| A Vindication of the Rights of Woman | Mary Wollstonecraft | 1792 | 83,334 | 9 h 02 m | 0.51 MB |
| The Souls of Black Folk | W. E. B. Du Bois | 1903 | 69,640 | 7 h 22 m | 0.42 MB |
| Orthodoxy | G. K. Chesterton | 1908 | 63,929 | 6 h 32 m | 0.36 MB |
| Essays | Ralph Waldo Emerson | c. 1841 | 133,312 | 13 h 39 m | 0.76 MB |

Listening hours per shelf: classics 91, adventure 54, mystery 54, romance 65, science fiction 42, philosophy 23, poetry 21, short stories 76, children 30, history 60, essays 47.

### 4. Licences (verified per title by the build)
- The build rejects a title unless its `LICENSE.md` contains both "United States public domain" and "CC0 1.0". All 66 passed.
- The text and cover art are "believed to be in the United States public domain", and SE's contributions are dedicated under CC0 1.0.
- `catalog.json` carries `licence` (id `CC0-1.0`, URL, and the notice "Public domain in the U.S. Outside the U.S., check your local laws."), a `credit` line, and each book's `source` (SE URL, GitHub repo, commit).
- CC0 needs no attribution. Credit SE anyway on `/credits` and in the detail sheet (note 15 C1), without SE's logo.
- Superseded by 27 (2026-10-31): the claim below is wrong. *The Blue Castle* is public domain under life + 70 (Montgomery died 1942); for *We* the term runs from the translator Zilboorg (died 1959), not Zamyatin; a "check your local laws" notice does not make serving the 8 titles in note 27 to UK or EU users lawful. Old text: Year caveat (assumed): *The Blue Castle* (1926) and *We* (1924, Zilboorg translation) are in the U.S. public domain but may still be in copyright in life+70 countries (Montgomery died 1942, Zamyatin 1937). The detail sheet's "check your local laws" line covers this. Remove them if the owner wants worldwide-clean shelves only.

### 5. What the app and Worker must add (open; owners: Worker + web app)
R2 layout (bucket `nolabs-media`):
- `catalog/v1/catalog.json`: the index (short cache).
- `catalog/v1/books/{id}/body-{sha12}.json`: the `DocumentBody` v1 (immutable).
- `catalog/v1/books/{id}/about-{sha8}.json`: `{v, description, subjects}` (immutable).
- `catalog/v1/covers/{id}-{200|400}-{sha8}.{avif|webp}` (immutable).

Index shape:
- Top level: `{v:1, generated_at, chars_per_second, licence, credit, shelves:[{id, title, blurb, books:[…]}], withdrawn:[{id,url}]}`.
- Each book: `{id, title, author, translators, year, year_approx, language, abstract, genres, chars, words, blocks, listening_minutes, body:{key, bytes, sha256}, about, cover:{alt, sizes:[{w,h,avif,webp}]}, licence, region, world_from, last_death, source:{name,url,repo,commit}}`. `region` and the top-level `us_only` list were added on 2026-10-31 (note 27).
- `id` is the SE GitHub repo name, for example `jane-austen_pride-and-prejudice`.

Requests:
1. Worker: a public, unauthenticated read of `catalog/v1/*`, so signed-out try mode can browse.
   - Proposed route: `GET /api/catalog` returns `catalog.json`. `GET /api/catalog/{rest}` streams `catalog/v1/{rest}` only when `{rest}` matches `^(books/[a-z0-9_-]+/(body|about)-[0-9a-f]+\.json|covers/[a-z0-9_-]+-(200|400)-[0-9a-f]{8}\.(avif|webp))$`.
   - Pass through the stored `content-type` and `cache-control`, and support `If-None-Match`.
   - Alternative: a public custom domain on the bucket limited to `catalog/` (Cloudflare cannot limit an r2.dev or custom domain to a prefix, so this exposes the whole bucket; rejected).
2. "Add to library": copy the body; do not re-import.
   - The app makes a 16-char id and inserts the `documents` row from the index entry:
     - `title`, `author`;
     - `source='epub'`, `source_url=source.url`, `language`;
     - `char_count=chars`, `word_count=words`, `block_count=blocks`, `body_bytes=body.bytes`;
     - `catalog_id=id`, once 0005 is applied.
   - The app then calls a new `POST /api/documents/{doc_id}/from-catalog {catalog_id}`.
   - The Worker checks the row is the caller's and that `body_bytes` equals the catalogue entry, copies `catalog/v1/books/…/body-….json` to `docs/<user_id>/<doc_id>/body.json` inside R2 (no client upload of up to 2 MB), and sets `ready`.
   - Fallback with no new write route: the app GETs the body through request 1 and uses the existing `PUT /api/documents/{id}/body`.
3. Supabase: apply `0005_catalog.sql` (adds `catalog_id` plus a unique index per user), so a second "Add" opens the existing copy and a withdrawn title can be flagged in libraries.
4. Web app Explore screen (note 15 C4):
   - Use `<picture>` with an AVIF/WebP `srcset` at 200w and 400w, `width=200 height=300`, and `cover.alt`.
   - Show listening time from `listening_minutes`.
   - Load the detail sheet's long text from `about`.
   - Hide titles listed in `withdrawn`.
5. Refresh: re-run `uv run tools/catalog/build.py --out … --upload` monthly or after editing `shelves.toml` (never with `--only`; refused since 2026-10-31, note 27).
   - Keys are content-addressed, so unchanged objects are skipped and the index is written last.
   - Old body keys stay until cleaned. Open: add a prune step once the app no longer references them.

### 6. Findings and corrections
- Bug in `nolabs-import` (found by the tests; owner: import crate):
  - `epub/package.rs` stores one role per creator in `refined_roles: HashMap<String, String>`, so the last `<meta property="role" refines="#author">` wins.
  - SE lists several roles per creator. T. S. Eliot's are `ann`, `aut`, `dto`, so `aut` is lost and the author comes out as `None`.
  - Every user who imports such an SE EPUB gets no author.
  - Fix: keep a set of roles per id, and accept the creator if any role is `aut`. When fixed, the xfail test `test_importer_finds_author_with_several_roles` turns into a failure (strict); then remove its marker.
  - The catalogue is not affected, because it takes authors from the OPF parser in `secat/opf.py`.
- Correction to note 15 C4:
  - The pipeline is a uv script in `tools/catalog/`, not `xtask catalog sync`, and the curation file is `tools/catalog/shelves.toml`, not `catalog/shelves.toml`.
  - The shelves are the 11 in section 3 at about 6 titles each (66), not 10–14 per shelf. "Start here" and "Just added" are not built. "Just added" needs the open SE Atom feed, which is a Worker cron job (open).
  - Note 15 assumed that "Add to library" means the Worker copies the EPUB and runs the importer. Correction: the importer runs only in the web app (wasm), so the catalogue stores the converted body and the Worker only copies it.
- SE metadata comes in two generations, and the parser handles both:
  - Current: `schema:abstract` short text, an HTML `dc:description`, a plain URL identifier, `schema:wordCount`.
  - Older: `dc:description` short text, `se:long-description`, a `url:` identifier, `se:word-count`.
- Candidate repos that do not exist (GitHub 404 on 2026-10-31, so not retried):
  - joseph-conrad_heart-of-darkness (only `joseph-conrad_short-fiction`)
  - e-m-forster_the-machine-stops
  - charlotte-perkins-gilman_the-yellow-wallpaper
  - emily-dickinson_poetry
  - william-shakespeare_sonnets (`william-shakespeare_poetry` exists)
  - katherine-mansfield_short-fiction
  - j-m-barrie_peter-pan (`peter-and-wendy` exists)
  - virginia-woolf_a-room-of-ones-own
  - washington-irving_the-legend-of-sleepy-hollow
  - xenophon_the-anabasis_h-g-dakyns
  - howard-carter_a-c-mace_the-tomb-of-tut-ankh-amen
  - jonathan-swift_short-works
  - epictetus_the-enchiridion_george-long (`epictetus_short-works_george-long` exists)
  - t-s-eliot_the-waste-land (it is inside `t-s-eliot_poetry`)
  - jules-verne_journey-to-the-centre-of-the-earth_frederick-amadeus-malleson (the real repo name is `…center…_f-a-malleson`)

### 7. Decisions (assistant, under the task's delegation; 2026-10-31)
- D-cat-1: store converted bodies, not EPUBs, on R2.
  - Reason: the importer is wasm-only, and the body must be byte-exact for `body_bytes` and the cache keys.
  - Rejected: mirroring EPUBs, which would mean re-importing on every add.
- D-cat-2: convert with the Rust importer, not a Python port.
  - Reason: one normaliser and identical offsets.
  - Rejected: a Python reimplementation of `tree_blocks` and `normalise`, which would drift.
- D-cat-3: covers from the GitHub `src/epub/images/cover.svg`, not SE's `downloads/cover.jpg`.
  - Reason: the task said to take sources only from GitHub, and the SVG gives the same composite.
  - Rejected: SE download URLs.
- D-cat-4: book id = the SE repo name.
  - Reason: stable, readable, and maps 1:1 to the source.
  - Rejected: nolabs ids, which mean nothing outside our database.
- D-cat-5: put long descriptions in per-book `about` files.
  - Reason: the index stays at 20.5 KB gzip, against 51 KB with them inline.

### Open items
- Worker routes and the app's Explore screen (section 5, requests 1, 2 and 4). Unblocked by the Worker and web app owners.
- Applying `0005_catalog.sql`. Unblocked by the owner, or by the agent that owns Supabase, running `cargo xtask supabase-migrate` together with the app change.
- Importer author bug (section 6). Unblocked by the import crate owner.
- Opening-line preview clips (note 15 C4, 6–8 s Kokoro clips). Not built; needs Space render time. Unblocked by a decision to spend it.
- The "Just added" shelf from the SE Atom feed, and a monthly 451 re-check in the Worker cron. Not built.
- Pruning superseded body, about and cover keys after a rebuild. Not built; needed only once titles change.
- Calibrating the 15 chars/s listening rate against measured Kokoro timings (note 08). Unblocked by one long render.
