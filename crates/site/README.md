# site-gen

Builds the public Streams Reader website (`https://reader.streams.watch`) as static files in `crates/site/dist`:
the landing page, `/download`, `/help`, `/privacy`, `/terms`, `/credits` and `404.html`, each in eight languages
(English at `/`, then `/ar/` (right to left), `/es/`, `/fr/`, `/de/`, `/pt/`, `/hi/`, `/zh/`), plus `robots.txt`,
`sitemap.xml` (with `hreflang` alternates), `llms.txt`, `.well-known/security.txt` and the IndexNow key file.

```sh
cargo run -p nolabs-site --bin site-gen            # build into crates/site/dist
cargo run -p nolabs-site --bin site-gen -- --check # only lint style/ for raw colours and radii
cargo test -p nolabs-site                          # unit tests + a full build checked end to end
```

How the Worker serves it (routes, content types, cache and security headers): [HEADERS.md](HEADERS.md).

## Inputs

| Input | Used for |
|---|---|
| `locales/<lang>/site.ftl` | every visible string (Fluent). English is the source; `llms-*` messages are English only |
| `style/*.scss` | the one stylesheet, compiled with grass on top of `brand/dist/tokens/_tokens.scss` |
| `brand/dist/tokens/tokens.json` | `theme-color` and fallback favicon colours |
| `brand/dist/web/` | favicons, touch icon, social card (copied to `/assets/brand/web/`) |
| `brand/spot/*.svg` (or `brand/dist/spot/`) | spot illustrations (copied to `/assets/brand/spot/`) |
| `brand/mark/streams-reader-mark-currentcolor.svg` | the mark, inlined in the top bar and footer |
| `assets/fonts/` | Inter and Literata subsets made by `tools/subset_fonts.py` from `crates/web/assets/fonts/` (upright: Latin-1 and punctuation, weights 400-700; italic: only the characters set in italic, listed in `literata-italic.chars.txt`), copied to `/assets/fonts/` and all three preloaded |
| `assets/icons/` | Remix Icon SVGs (v4.9.1, vendored with curl; Remix Icon License v1.0), inlined |
| `assets/demo/` | the hero voice demo: 4 samples × 4 Kokoro voices plus one Ming sample, WebM/Opus + word timings, and `demo.json`; made by `tools/demo_clips.py` |
| `assets/previews/` | 6-8 s opening-line clips for the classics row, made by `tools/demo_clips.py` |
| `assets/covers/` | 20 Standard Ebooks covers (CC0) as AVIF/WebP at 200 and 300 px, `covers.json`, `CREDITS.md`; made by `tools/covers.py` |
| `assets/media/` | the hero film loop (WebM AV1 + MP4, landscape and portrait, each ≤ 1.5 MB) and its posters; copied by `tools/media.py` |
| `assets/spots/` | voxel spot renders (280 px WebP) from `media/dist/spots`; copied by `tools/media.py` |
| `assets/js/site.js` | the only script (≤ 25 KB): demo player, film loader, OS detection, marquee, reveal fallback, menu |

Design values live in `brand/tokens.toml` only. `site-gen` refuses to build if `style/` contains a hex colour, an
`rgb()`/`hsl()` literal or a literal `border-radius`; change the token and rebuild the brand instead.

## The hero demo

`tools/demo_clips.py` renders every standard-voice clip once through the live API (`POST /api/render`, `kokoro:*`
voices only, never `ming:*`, so no GPU time is spent) and reuses the film's existing Ming voice-over for the
expressive sample. `cargo test` checks that every clip has its timings and that the timings fit the text.

## Budgets (checked by `cargo test`)

Landing stylesheet ≤ 40 KB and script ≤ 25 KB (raw bytes), posters ≤ 120 KB, covers ≤ 18 KB each, each film file
≤ 1.5 MB, the three fonts ≤ 90 KB together. Every class on an inner page must have a rule in the inner stylesheet. The SCSS compiles through grass, then `css.rs` drops unused custom properties, shortens their names, folds
`calc()` products and same-unit sums, writes block-direction sides physically (`margin-top`), drops light-theme
values equal to the dark defaults, merges repeated `@media` blocks and drops the space in `@media (`.
