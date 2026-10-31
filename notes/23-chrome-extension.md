# 23 Chrome extension "Streams Reader — read this page"

Finding + decision + reference. The MV3 extension in `extensions/chrome/` is built and passes its tests in headless Chrome; the web app does not receive handoffs yet.
Conclusion: the page travels as rendered HTML through `chrome.storage.session` and a content script on `listen.streams.watch/app/*` that answers the app's `postMessage` request, so nothing goes in the URL. Section "What the web app must implement" is the work order for the `crates/web` owner.

## Correction 2026-10-31 (review fixes, see [26-chrome-extension-review-fixes.md](26-chrome-extension-review-fixes.md))

These claims below are superseded by 26:
- "Pages the extension may not script (`chrome://`, the Web Store) are refused": the Web Store was in fact sent to the app as a link. Fixed; now verified by unit test.
- "Chrome's PDF viewer is handed over as a `link` payload": was untested. Now verified in headless Chrome 158 (e2e).
- "entries expire after 5 min" / "The handoff waits up to 5 min": expired entries were removed only on the next use of the extension. An `alarms` sweep now removes them about 5 min after sending; permissions are five, not four.
- "an unacknowledged lease can be claimed again" (only after 30 s): the tab holding the lease may now re-claim at once, so a reload inside the lease receives the page. Work order step 7 changes accordingly.
- Measurements table: e2e is now 42 checks, browser unit tests 37, packaging tests 17, zip 24,565 bytes.

## 2026-10-31

### What exists (verified)

- `extensions/chrome/`: manifest, 9 source modules under `src/`, 8 locales, brand icons copied from `brand/dist/png/icon-{16,32,48,128}.png`, `package.py`, `STORE.md`, `README.md`, tests. File map in `extensions/chrome/README.md`.
- Entry points: toolbar button, `Alt+Shift+R` (`_execute_action`), context menu "Read this page" / "Read selection" / "Read linked page" (the last is extra to the brief; it reuses the app's existing link import).
- Permissions: `activeTab`, `contextMenus`, `scripting`, `storage`. No `host_permissions`. The content script's match `https://listen.streams.watch/app/*` gives host access to that path only; expect the install prompt "Read and change your data on listen.streams.watch" (assumed wording, check at first upload).
- CSP `extension_pages`: `default-src 'none'; script-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`. Chrome 158 accepted it (verified by loading). No remote code, no `fetch`, no `eval`; `package.py` fails the build on these.
- `incognito: not_allowed`, so a private-window page is never sent into the normal profile's app tab.
- Locales: `en ar es fr de pt_BR hi zh_CN`. Chrome has no plain `pt` or `zh` locale folder, so `pt_BR` (the app's Portuguese is Brazilian: "Excluir", "ouvindo") and `zh_CN` (Simplified, as the app). Translations are the assistant's; status assumed until a native speaker reads them. Register matches the app: German "du", French "vous", Spanish "tú".
- No Readability copy in the extension. The app's importer (`crates/import`, `from_html` → dom_smoothie) does the extraction, so there is one extractor to maintain. The extension only removes `script, style, noscript, template, iframe, object, embed, canvas, video, audio, svg` and stylesheet/preload/icon links to shrink the payload, and falls back to `article, main, [role=main]` when the page is over 6 MiB.
- Pages the extension may not script (`chrome://`, the Web Store) are refused with a "!" badge and a tooltip for 6 s. Chrome's PDF viewer is handed over as a `link` payload so the app fetches it.

### Decision D-EXT1: handoff channel (assistant, under the track's delegation)

Decision: the service worker stores the payload in `chrome.storage.session` bound to the app tab it opens (`/app/import?from=extension`). A content script on `listen.streams.watch/app/*` (top frame, `document_start`) answers the page's `handoff:ready` postMessage by claiming the payload from the worker and posting it back. Claim leases the entry for 30 s; the app's `handoff:ack` deletes it; an unacknowledged lease can be claimed again; entries expire after 5 min.

Reasons:
- Nothing in the URL, history, server logs or Referer. `storage.session` is memory only, cleared on browser exit, and not readable by content scripts (default access level).
- Pull, not push: the app asks when it is mounted and signed in, so there is no race with wasm start-up and no timer guessing.
- Same code works in Edge and (with small manifest changes) Firefox, which has no `externally_connectable` for web pages.
- The worker checks every claim: sender is this extension, `sender.origin` is the app origin, path under `/app/`, `frameId == 0`, and the tab is the one opened for that handoff (or that tab has closed: reload, sign-in elsewhere). The bridge ignores messages whose `event.source` is not the top window, so a cross-origin iframe in the app cannot claim (tested).

Rejected:
- Content in the URL (query or fragment): history, sync, logs, length limits. Ruled out by the brief.
- `externally_connectable` (the app calls `chrome.runtime.sendMessage(EXT_ID, …)`): needs no host access and no content script, but the app must know each store's extension ID (Chrome, Edge differ; unpacked dev builds differ) and Firefox does not support it. Kept as a possible second channel later.
- Web Share Target: only works when the PWA is installed and needs a service worker POST handler in the app; extensions cannot trigger it reliably.
- Form POST from an extension page to a Worker endpoint: sends article bytes through our server (the importer runs in the browser by design, note 09) and needs CSRF and auth design.
- Injecting into the app tab with `chrome.scripting`: needs the same host access plus load-timing guesses.
- One-shot delete on claim: superseded in the same session, see "Wrong belief" below.

### Wrong belief, corrected (verified)

Belief: deleting the payload when the app claims it is enough. Correction: on this machine under load (load average 90, other workflows building), `store.claim` took 1.5 s, 2.0 s and 3.1 s (3 failing runs out of 9, measured by a wrapped `store.claim` in the e2e test worker), the reply reached the mock app after its 4 s timeout, and the article was lost. Fix: lease plus ack (above) and a 10 s receiver timeout. The e2e test now forces a lost reply (receiver timeout 0, reload) with a 1.5 s test lease and checks that the reloaded page gets the same handoff.

### Other findings

- Without the `tabs` permission the extension cannot read the URL of its own app tab, even with the content script matching it (`chrome.tabs.get` returned no `url`/`pendingUrl` for `http://localhost:PORT/app/import`, Chrome 158). The test reads the tab URL from DevTools instead. Verified.
- Branded Chrome ignores `--load-extension` since Chrome 137 (assumed, from release notes, not tried here). The test loads the extension with CDP `Extensions.loadUnpacked` over `--remote-debugging-pipe` with `--enable-unsafe-extension-debugging`, which works in Chrome Beta 158.0.8127.2 (verified).
- Attaching DevTools to a page and awaiting a promise across that page's `location.reload()` left the reloaded page's iframe unloaded (test artefact, 2 of 2 runs). The e2e test waits 6 s before attaching in that scenario. Do not retry the attach-before-reload variant.

### Measurements

| What | Number | How |
|---|---|---|
| Store zip | 21,353 bytes, 22 files, sha256 `2de5c331f4f8…a54079` | `uv run extensions/chrome/package.py`, version 0.1.0 |
| e2e checks | 29 of 29 pass, 4 consecutive runs | `uv run extensions/chrome/tests/e2e.py`, Chrome Beta 158.0.8127.2 headless, macOS, local server |
| Browser unit tests | 24 pass | `tests/unit.html`, run inside the e2e |
| Packaging tests | 11 pass | `uv run --no-project python -m unittest discover -s extensions/chrome/tests -p 'test_*.py'` |
| Click to payload in the app | 0.53–0.75 s (n = 6), of which 0.4 s is the mock's own deliberate wait | e2e timer from `read()` to `__mockApp`, 1,553-character fixture page |

### What the web app must implement (owner: crates/web)

The reference implementation is `extensions/chrome/tests/fixtures/app/receiver.js` (60 lines of JS, exercised by the e2e test). Port it to Rust/wasm-bindgen.

1. **Route** `/app/import` (the extension always opens exactly `https://listen.streams.watch/app/import?from=extension`). It shows the import progress screen. The query carries no data; treat it only as "came from the extension".
2. **Signed out**: the guard (`app.rs`, `redirect`) currently sends every non-public route to `/sign-in` and drops the path. Remember `from=extension` for this tab (sessionStorage key, for example `sr-pending-extension-import`), and after sign-in in this tab return to `/app/import?from=extension`. If sign-in finishes in another tab (magic link), this tab's session changes too; on that change, run step 4 again. The handoff waits up to 5 min.
3. **Detect** the extension: `document.documentElement.dataset.streamsReaderExtension` (the extension version, set at `document_start` on `/app/*` only). Absent → do not ask, show the normal import screen.
4. **Ask** only when signed in and ready to import:
   `window.postMessage({ source: "streams-reader-app", type: "handoff:ready", v: 1, requestId }, location.origin)` with a fresh random `requestId`.
5. **Receive**: a `message` listener that accepts only `event.source === window`, `event.origin === location.origin`, `data.source === "streams-reader-extension"`, `data.requestId === requestId`. `data.type` is `"handoff:payload"` (with `data.payload`) or `"handoff:none"`. Time out after 10 s.
6. **Validate** the payload again (the page cannot know who posted): see the schema below; reject unknown `kind`, non-http(s) `url`, `title` over 300 chars, `html` over 6 MiB UTF-8, `text` over 1,048,576 UTF-16 units, `link` without `url`.
7. **Acknowledge** at once after a valid payload: `window.postMessage({ source: "streams-reader-app", type: "handoff:ack", v: 1, id: payload.id }, location.origin)`. Without the ack the extension offers the same payload again: at once to the same tab (a reload or a second ask), to other tabs after 30 s. So ask again only after acking, and dedupe by `payload.id` if two asks can overlap (superseded wording, see 26). Ack also when the import later fails, so the user is not shown the same failure twice.
8. **Import** through the existing pipeline (`import_flow/run.rs`):
   - `kind: "page"` → new `ImportSource::Page { html, url, title }` → `nolabs_import::from_html(&html, url.as_deref())`, `SourceKind::Url`, `source_url = url`. If the imported title is empty, use `payload.title`.
   - `kind: "selection"` → `ImportSource::Paste { text, title: Some(payload.title) }`, keep `source_url = url` (needs a small change in `parse`, which sets `None` for Paste).
   - `kind: "link"` → existing `ImportSource::Link { url }` (also used for PDFs open in Chrome's viewer; the link import must cope with a PDF response or show its error).
   - `payload.lang` (BCP 47 or null) may pick the default voice language.
9. Then open `/read/:id` and start at the first sentence (note 15, flow step 4).
10. Never copy the payload into the URL, history state, logs, analytics or error reports.
11. Content scripts are not affected by the app's CSP; no CSP change is needed.

Payload v1 (all fields always present except `html`/`text`; no other fields):

| Field | Type | Notes |
|---|---|---|
| `v` | 1 | protocol version |
| `id` | UUID string | ack with it |
| `kind` | `"page"` \| `"selection"` \| `"link"` | |
| `createdAt` | number | ms since epoch |
| `url` | string \| null | http(s) only; null for `file:` pages |
| `title` | string | trimmed, ≤ 300 chars, may be empty |
| `lang` | string \| null | from `<html lang>` |
| `html` | string | `page` only: rendered HTML, scripts/styles/media removed, starts with `<!DOCTYPE html>` |
| `text` | string | `selection` only |

Messages (all via `window.postMessage(msg, location.origin)` on the app's top window):

| From | `type` | Other fields |
|---|---|---|
| app | `handoff:ready` | `source: "streams-reader-app"`, `v: 1`, `requestId` |
| extension | `handoff:payload` | `source: "streams-reader-extension"`, `v: 1`, `requestId`, `payload` |
| extension | `handoff:none` | `source: "streams-reader-extension"`, `v: 1`, `requestId` |
| app | `handoff:ack` | `source: "streams-reader-app"`, `v: 1`, `id` |

Suggested Rust shape (assumed, not compiled): a `interop::extension` module with `fn installed() -> bool` and `async fn take_handoff() -> Option<Handoff>` (web_sys `MessageEvent`, `serde_wasm_bindgen::from_value`, a `futures` oneshot with a `gloo_timers` 10 s timeout), and `Handoff` as a serde enum tagged on `kind`.

### Requests to other owners

- crates/web: the receiver above.
- crates/site: a `/privacy` page (the store requires a privacy policy for an extension that handles website content), and the Chrome extension tile linking to the store item once published.
- brand: a 128 px store icon with the art in the central 96 px and a 16 px transparent margin (Chrome's guideline, assumed from its docs); today `icon-128.png` is the full-bleed tile.

### Open items

- Web app receiver: open; unblocks when crates/web implements the section above. The extension is unusable in production until then.
- Manual check of the real toolbar click, the context menu and `Alt+Shift+R` in a headed Chrome: open; headless Chrome has no toolbar, so the e2e test calls `read()` directly and grants `127.0.0.1` as a test-only host permission. Unblocks with 5 minutes in a headed browser.
- Native-speaker review of ar, es, fr, de, pt_BR, hi, zh_CN strings: open.
- Portuguese (Portugal) users see English (only `pt_BR` ships): open; unblocks by adding `pt_PT` with the same strings if the owner wants it.
- Store listing: privacy page, screenshots 2/4/5 (need the receiver live), $5 registration (assumed, note 15), first upload: open; steps in `extensions/chrome/STORE.md`.
- In-page "Reading this page" toast from note 15: not built; errors show on the toolbar badge only. Open, low priority.
- Firefox build (`browser_specific_settings`, background scripts instead of a service worker): open, not tried.
