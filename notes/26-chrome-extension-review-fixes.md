# 26 Chrome extension: review fixes

Finding + decision. Nine reviewer defects in `extensions/chrome/` (note 23) were fixed at the root and re-verified in headless Chrome.
Conclusion: the extension now refuses the Web Store, re-delivers to a reloaded app tab inside the lease, deletes uncollected pages on a timer (new `alarms` permission), keeps shadow-DOM article text, and its PDF hand-off is tested. Note 23's affected claims carry a dated correction.

## 2026-10-31

### Fixes (all verified by the tests named)

| # | Defect | Fix | Test |
|---|---|---|---|
| 1 | Web Store pages went to the app as a `link` (`viewer = !isScriptable(url)`) | The read decision moved to `src/collect.js` (page script runner injected). Unscriptable pages return `restricted` before anything runs; only a PDF counts as a viewer page | unit "collect refuses the Web Store…" (both store hosts, page and selection, extractor never called) |
| 2 | A reload inside the 30 s lease got `handoff:none` | `claim()` treats an entry leased to the asking tab as free; other tabs still wait for the lease | unit "the tab holding the lease gets the handoff again at once"; e2e lost-reply case now reloads at once with the real 30 s lease: claims `[true, true, false]`, reload asked 481 ms after the first claim |
| 3 | Expired entries stayed in `storage.session` until the next use | Store takes `scheduleSweep(when)`; `put` arms alarm `handoff-sweep` for the earliest expiry + 1 s, `sweep` re-arms or clears it; `background.js` listens at top level | unit "put arms the sweep…", "a failing scheduler does not fail the put"; e2e alarm armed 300,935 ms ahead after a put, and a stale entry removed by the alarm in 0.8–1.3 s with the alarm not re-armed |
| 4 | A page that clears its selection on right-click gave "no readable text" | `collect()` falls back to Chrome's `selectionText` when the page script returns `empty` (keeps the page's `lang`) | unit; e2e fixture `clears-selection.html` clears on `contextmenu`, payload text equals Chrome's copy, `lang` `fr` |
| 5 | PDF hand-off untested; failed injection gave `failed` | In the catch branch a `.pdf` path still goes as a link; `file:` PDFs give `restricted` | unit (both branches); e2e `fixtures/doc.pdf` (592 bytes): payload `link` with the PDF URL. In Chrome 158 headless the script does run in the viewer and reports `application/pdf` |
| 6 | Text inside open shadow roots was dropped | `extractPage` finds open roots (nested too); when any exist and `getHTML` exists (Chrome 125+), serialises with `getHTML({serializableShadowRoots, shadowRoots})`, parses with `DOMParser` (inert), flattens each `template[shadowrootmode]` into its host with slots filled as rendered, then applies the drop list. Otherwise the old clone path | unit (slot, fallback, nested root, closed root excluded, unassigned child dropped, no-`getHTML` path); e2e fixture `<pull-quote>` web component in `article.html` |
| 7 | Badge reset set an empty tab title (shows the extension name); overlapping errors cleared each other | Reset to `action_title`; per-tab `Map` of timers, cleared before a new one | e2e: cleared state `['', 'Read this page in Streams Reader']`; a second error 1 s later keeps "!" past the first timer (test build badge 2 s) |
| 8 | `--app-origin https://x/` built an extension that never delivers | `normalise_origin()` (no trailing slash, lower case, no default port) in `build()` and `main()`; `origin_pattern()` refuses any path, query, fragment, user name or bad port, and runs before anything is deleted | `test_package.py` (6 new cases) |
| 9 | `package.py --check` deleted the loaded `dist/unpacked` | `--check` builds in a `tempfile.TemporaryDirectory` | `test_check_does_not_touch_out`; by hand: `dist/unpacked` still present after `--check` |

### Decision D-EXT2: `alarms` permission (assistant, under the track's delegation)

Decision: add `alarms` (permissions now `activeTab, alarms, contextMenus, scripting, storage`; `ALLOWED_PERMISSIONS` in `package.py` and the e2e permission check updated). Reason: the store listing and note 23 say uncollected pages are deleted after 5 minutes; without a timer that was false for hours. Chrome shows no install warning for `alarms` (assumed from Chrome's permission warning list, not seen at an upload yet). Rejected: keeping four permissions and rewording the privacy text to "until the app collects it, the next use of the extension, or the browser closes", because the shorter retention is the better privacy statement. `STORE.md` now justifies `alarms` and says "about 5 minutes" (alarms can fire late when the machine sleeps).

### Behaviour change the web app must follow

Without an ack, the same tab gets the payload again at once. The receiver must ack before asking again and dedupe by `payload.id` if two asks can overlap (note 23, step 7, updated). `tests/fixtures/app/receiver.js` already acks before resolving.

### Measurements

| What | Number | How |
|---|---|---|
| e2e checks | 42 of 42 pass, 4 consecutive runs | `uv run extensions/chrome/tests/e2e.py`, Chrome Beta 158.0.8127.2 headless, macOS, local server |
| Browser unit tests | 37 pass | `tests/unit.html`, run inside the e2e |
| Packaging tests | 17 pass | `uv run --no-project python -m unittest discover -s extensions/chrome/tests -p 'test_*.py'` |
| Store zip | 24,565 bytes, 23 files, sha256 `1bd067e27673…675b11` | `uv run extensions/chrome/package.py`, version 0.1.0 |
| Click to payload in the app | 0.51 s (n = 1) | e2e timer, 1,708-character fixture page |
| Mutation check | old claim filter fails the new unit test | scratch copy with `e.leasedTo !== tabId` removed, e2e run, copy deleted |

### Open items

- Install prompt wording with `alarms` added: assumed no extra warning; unblocks at the first store upload.
- Real PDF sites where `executeScript` cannot run in the viewer: covered only by the unit test of the `.pdf` path fallback; a PDF served without a `.pdf` path in that case still gives `failed`. Unblocks with a manual check in headed Chrome on such a site.
- Shadow DOM on Chrome 116–124 (no `getHTML`): content inside shadow roots is still lost there. Open, low priority (minimum version could be raised to 125).
- Everything open in note 23 (web app receiver, headed manual check, translations, store listing) is unchanged.
