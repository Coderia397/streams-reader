# 44. The Worker serves the landing site and the web app

Finding + decision + reference. Conclusion: the Worker now embeds `crates/site/dist` at `/` and `crates/web/dist` under
`/app/`, with per-surface security headers, and `cargo xtask build` builds site, app and Worker in one pass. The
release Worker is 13.93 MiB of the 64 MiB limit. Not deployed.

## 2026-10-31

### What changed (verified)
- `crates/server/src/static_assets.rs`: content types (adds mp4, webm, avif, mp3, xml with charset, LICENSE) and cache
  policy by URL path. App: hashed names immutable, everything else `no-cache` with ETag. Site: follows
  `crates/site/HEADERS.md` (HTML `max-age=0, must-revalidate`, hashed css/js immutable, fonts and demo 30 days, brand
  and favicon 7 days, robots/sitemap/llms 1 h, `.well-known` and the IndexNow key 1 day). Other `/assets/*` without a
  hash (covers, previews, spots, hero film) get 7 days; HEADERS.md did not list them (assumed acceptable).
- `crates/server/src/static_routes.rs`: the routing table. `/` and `/<lang>/` serve `index.html`; `/ar` → 301 `/ar/`;
  `/download/` → 301 `/download`; a folder is a language root when it holds `404.html`, so no language list is
  hardcoded. Unknown paths get `<lang>/404.html` or `/404.html` with status 404 and `no-store`. `/app` → 301 `/app/`;
  `/app/*` without an extension → the app shell; a missing `/app/*.ext` → 404. `/auth/*` → 302 `/app/auth/*` with the
  query and `no-store` (old sign-in emails link to `<site>/auth/confirm`).
- `crates/server/src/security_headers.rs`: one header set per surface (site, app, API = `/api`, `/hooks`, `/updates`).
  Site CSP is the one in HEADERS.md (`script-src 'self'`, `style-src 'self'`, `connect-src 'self'`). App and API CSP:
  `connect-src 'self' https://atxbmgfuxjtcwmoaldqy.supabase.co wss://atxbmgfuxjtcwmoaldqy.supabase.co` (from the
  `SUPABASE_URL` var, falling back to the project URL), `script-src 'self' 'wasm-unsafe-eval'`. All surfaces: HSTS
  2 years with includeSubDomains (no `preload`, which only applies to registrable domains), nosniff,
  `X-Frame-Options: DENY`, `frame-ancestors 'none'`, `object-src 'none'`. `interest-cohort` from HEADERS.md is left out
  because Chrome logs it as an unknown feature.
- `crates/worker/build.rs` embeds both dists into one table sorted by path (binary search), skipping hidden files
  except `.well-known`; a missing build becomes a placeholder page. `crates/worker/src/assets.rs` serves ETag/304,
  HEAD, single byte ranges (206/416; Safari needs ranges for `<video>`), and redirects that keep the query.
- `xtask/src/cmd/build.rs`: `site-gen`, then `trunk build --release`, then `worker-build --release`; prints each size
  and fails above 64 MiB uncompressed, warns above 32 MiB.
- `crates/web/sw.js` line 1 (web app, outside this task's crates): its comment contained the literal `__PRECACHE__`, so
  `postbuild.py` pasted the multi-line precache list into a `//` comment and `sw.js` failed with
  `SyntaxError: Unexpected token ']'` (seen in headless Chrome). The service worker never installed. The comment was
  reworded; after the rebuild the worker registers.

### Wrong belief, corrected
- `xtask/src/cmd/build.rs` assumed a 3 MiB gzip limit on Workers Free. Corrected: 64 MiB uncompressed on Free and Paid,
  no compressed limit, startup 1 s, 128 MB memory per isolate (developers.cloudflare.com/workers/platform/limits,
  page dated 2026-10-29, read 2026-10-31). Note 03 already had 64 MiB.

### Decision
- 2026-10-31, assistant (task scope): no host redirect from listen.streams.watch to reader.streams.watch yet, although
  HEADERS.md asks for one. Supabase `site_url`, its redirect allowlist and the auth email hook still use
  `listen.streams.watch` (D34 sweep open), so a redirect now could break sign-in. Both hosts serve the same content;
  site pages carry `reader.streams.watch` canonical URLs. Rejected: redirect now (auth risk).
- 2026-10-31, assistant: embed both builds in the Worker as asked, not Workers Static Assets. Static Assets would make
  asset requests free and keep them off the 100k requests/day count, but needs the assets upload session in
  `deploy.rs`. Open below.

### Measurements (verified)
`cargo xtask build`, 2026-10-31, release:
| Part | Size |
|---|---|
| `crates/site/dist` | 260 files, 7.53 MiB (hero film 2.8 MiB, covers 1.0 MiB, previews and demo 1.3 MiB) |
| `crates/web/dist` | 146 files, 5.30 MiB |
| Worker `index_bg.wasm` | 13.90 MiB (gzip 7.72 MiB) |
| Worker total | 13.93 MiB of 64 MiB |

Real pages: native preview (`scratchpad/preview`, the same `static_routes`, `static_assets` and `security_headers`
over both dists) driven by CDP in headless Chrome Beta, at 390x844 mobile and 1440x900: `/`, `/ar/`, `/download`,
`/de/nope` (404, German page), `/app/`, `/app/settings`. 0 CSP violations, 0 console errors, 0 px horizontal
overflow, service worker registered on the app pages. curl checked 21 routes for status, location, content type and
cache policy.

Tests: `cargo test --workspace --exclude nolabs-worker` all pass (server 131, including 16 new routing and header
tests; xtask build 5). Clippy `-D warnings` clean native (workspace) and wasm (`nolabs-worker`, `nolabs-web`).

## Open items
- Startup time and memory of the 13.9 MiB Worker on Cloudflare: unblocked by the lead's deploy; check the first
  requests in the dashboard (limit 1 s startup).
- Free plan request count: a landing view loads about 30 files through the Worker (assumed, not counted), so
  100k requests/day is roughly 3,000 views. Unblocked by moving `/assets/*` and `/app/*` to Workers Static Assets (upload session in `deploy.rs`,
  `_headers` file for CSP and cache), when traffic needs it.
- listen → reader host redirect: unblocked by the D34 sweep (Supabase site URL and allowlist, mail links, brand
  `SITE_URL`).
- `cargo fmt --check` reports a diff in `crates/mail/examples/preview.rs` (another agent's work in progress).
