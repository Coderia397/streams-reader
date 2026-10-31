# Streams Reader — read this page (Chrome extension)

Manifest V3 extension: a toolbar button, the Alt+Shift+R shortcut and a context menu send the current page, the
selected text or a linked page to the Streams Reader web app. The page's own rendered HTML is sent; the app's
importer (`crates/import`) finds the article. Nothing goes in the URL: the payload waits in `chrome.storage.session`
and a content script on `listen.streams.watch/app/*` hands it to the app when the app asks for it.

Design, the message protocol and what the web app must implement: `notes/23-chrome-extension.md`.
Store listing and screenshots: `STORE.md`.

| Path | What |
|---|---|
| `manifest.json` | permissions `activeTab`, `alarms`, `contextMenus`, `scripting`, `storage`; no host permissions; CSP `default-src 'none'; script-src 'self'` |
| `src/background.js` | service worker: wires the button, menu and messages |
| `src/actions.js` | the read flow and the error badge |
| `src/collect.js` | what one action sends: page HTML, selection, link (PDF viewer), or a refusal (pure, page script injected) |
| `src/extract.js` | functions injected into the page (rendered HTML or selection) |
| `src/payload.js` | payload building, validation, URL rules (pure) |
| `src/handoff-store.js` | pending handoffs: lease on claim (same tab may re-claim), delete on ack, 5 min expiry swept by an alarm (pure, storage and scheduler injected) |
| `src/claim.js` | who may claim (app origin, `/app/`, top frame) |
| `src/app-bridge.js` | content script on the app: postMessage bridge |
| `src/config.js` | app origin and limits |
| `_locales/` | en, ar, es, fr, de, pt_BR, hi, zh_CN |
| `tests/` | `unit.js` (browser unit tests), `e2e.py` (headless Chrome), `test_package.py`, fixtures incl. the reference receiver `fixtures/app/receiver.js` |

```sh
uv run extensions/chrome/package.py                 # checks + dist/streams-reader-chrome-<version>.zip
uv run extensions/chrome/package.py --check         # checks only, in a temp dir (a loaded dist/unpacked is kept)
uv run --no-project python -m unittest discover -s extensions/chrome/tests -p 'test_*.py'
uv run extensions/chrome/tests/e2e.py               # needs Chrome or Chrome Beta installed
```

Try it by hand: `chrome://extensions` → Developer mode → Load unpacked → `extensions/chrome/dist/unpacked` (after
running `package.py`). For a local app build use `package.py --app-origin http://localhost:PORT`.
