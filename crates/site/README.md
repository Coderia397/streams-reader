# site-gen

Builds the public Streams Reader website (`https://listen.streams.watch`) as static files in `crates/site/dist`:
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
| `crates/web/assets/fonts/` | Inter and Literata (read only, copied to `/assets/fonts/`) |
| `assets/icons/` | Remix Icon SVGs (v4.9.1, vendored with curl; Remix Icon License v1.0), inlined |
| `assets/demo/` | the hero demo: one MP3 and one word-timing JSON per voice, made by `tools/render_demo.py` |
| `assets/js/site.js` | the only script: hero demo playback and OS detection for the download buttons |

Design values live in `brand/tokens.toml` only. `site-gen` refuses to build if `style/` contains a hex colour, an
`rgb()`/`hsl()` literal or a literal `border-radius`; change the token and rebuild the brand instead.

## The hero demo

`tools/render_demo.py` renders the Pride and Prejudice opening line (public domain) with the timestamped ONNX export of
Kokoro-82M, the same model as the standard voices, and writes the model's own per-word timings. Re-render with the
command in the script's docstring (downloads about 330 MB of model files into a cache folder you choose; delete it
afterwards). `cargo test` checks that the passage in `src/pages/home.rs` matches every timing file.
