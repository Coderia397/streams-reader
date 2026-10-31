# Serving the site: routes, content types and cache headers

For the owner of the Worker (`crates/worker`). `site-gen` writes everything into `crates/site/dist`; the Worker serves
that folder at `/` on `https://reader.streams.watch` (canonical since D34; `listen.streams.watch` answers with a permanent
redirect to the same path), next to the app (`/app/`) and the API (`/api/`). The whole folder
is rebuilt on every run, and the stylesheet and script names carry a content hash, so embed the folder as a whole.

## Routes

| Request path | File in `dist/` | Notes |
|---|---|---|
| `/` | `index.html` | English is the default language |
| `/ar/`, `/es/`, `/fr/`, `/de/`, `/pt/`, `/hi/`, `/zh/` | `<lang>/index.html` | `/ar` (no slash) → 301 to `/ar/` |
| `/download`, `/help`, `/privacy`, `/terms`, `/credits` | `<page>/index.html` | `/download/` → 301 to `/download` (the canonical form has no trailing slash) |
| `/<lang>/<page>` | `<lang>/<page>/index.html` | same rule per language |
| `/app/…`, `/api/…` | not in `dist/` | the app and the API, unchanged |
| any other path without a file | `404.html`, or `<lang>/404.html` when the path starts with a language prefix | send it with **status 404** |

Rule of thumb: try the exact file; for a path without an extension try `<path>/index.html`; otherwise the 404 page.
Do not redirect by `Accept-Language`: every language has its own URL and `hreflang` alternates, which is what search
engines expect. The language menu in the footer links between them.

## Content types

| Extension | Content-Type |
|---|---|
| `.html` | `text/html; charset=utf-8` |
| `.css` | `text/css; charset=utf-8` |
| `.js` | `text/javascript; charset=utf-8` |
| `.json` | `application/json` |
| `.xml` (`sitemap.xml`) | `application/xml; charset=utf-8` |
| `.txt` (`robots.txt`, `llms.txt`, `.well-known/security.txt`, IndexNow key) | `text/plain; charset=utf-8` |
| `.svg` | `image/svg+xml` |
| `.png` | `image/png` |
| `.ico` | `image/x-icon` |
| `.woff2` | `font/woff2` |
| `.mp3` | `audio/mpeg` (answer `Range` requests with 206 if convenient; the clips are about 64 KB, so 200 also works) |

## Cache-Control

| Path | Cache-Control | Why |
|---|---|---|
| `/assets/site.<hash>.css`, `/assets/site.<hash>.js` | `public, max-age=31536000, immutable` | name changes with the content |
| `/assets/fonts/*` | `public, max-age=2592000` | 30 days; names are stable, files rarely change |
| `/assets/demo/*` (mp3 + timings json) | `public, max-age=2592000` | 30 days; re-rendered only by `tools/render_demo.py` |
| `/assets/brand/*` (favicons, social card, spot illustrations) | `public, max-age=604800` | 7 days; the brand build may update them |
| `/favicon.ico` | `public, max-age=604800` | browsers fetch it on their own |
| HTML pages (`*.html`, every route above) | `public, max-age=0, must-revalidate` + `ETag` | always fresh after a deploy; 304s keep it cheap |
| 404 responses | `no-store` | |
| `/robots.txt`, `/sitemap.xml`, `/llms.txt` | `public, max-age=3600` | |
| `/.well-known/security.txt`, `/<indexnow-key>.txt` | `public, max-age=86400` | |

## Security headers for the site

The pages need no inline script, inline style or event handler (checked by `tests/site.rs`). The only `<script>`
elements are the external `/assets/site.<hash>.js` (hero demo and OS detection) and JSON-LD data blocks, which a
CSP does not execute or block. Suggested headers for every site response:

```
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; media-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()
X-Frame-Options: DENY
```

The script only changes element styles through the CSSOM (`element.style.transform`), which `style-src 'self'`
allows. It fetches `/assets/demo/<voice>.json` and plays `/assets/demo/<voice>.mp3` (same origin).

## IndexNow

The key file is `/<key>.txt` at the root, with the key as its body (the key is in `src/config.rs`, `INDEXNOW_KEY`). After
a deploy that changes pages, POST the changed URLs to `https://api.indexnow.org/indexnow` with that key and
`keyLocation: https://reader.streams.watch/<key>.txt`.
