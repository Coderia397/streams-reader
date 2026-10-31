# Web app review fixes, round 3 (2026-10-31)

Finding + decision. Conclusion: all 8 round-3 reviewer defects in `crates/web` are fixed at the root and verified on the rendered page against the local mock. The startup wasm is 39 % smaller (brotli) because the parsers moved into a Web Worker. PDF import was broken before this round (`doc.destroy is not a function`) and is fixed.

## 2026-10-31

### What changed
- Modal sheets: `Sheet` (`src/components/sheet.rs`) renders a native `<dialog class="sheet">` opened with `showModal()` in an Effect on mount. It saves `document.activeElement` before opening and focuses it again on cleanup, after `close()`. The dialog's `close` event (Escape, Android back) sets the signal. A click whose point is outside the dialog's box closes it (backdrop). `aria-labelledby` points at `h2.sheet__title` (unique id per sheet). `.sheet-scrim` is gone; `.sheet::backdrop` uses `t.color("scrim")`. The UA dialog box is reset in `_sheet.scss`.
- Toasts in an open sheet: `showModal` makes the app's toast region inert, so `ToastHost in_sheet=true` renders a copy inside the dialog and `:root:has(.sheet[open])` hides the global one (`_toast.scss`).
- Focus not obscured (WCAG 2.4.11): `html { scroll-padding-block-end: tab bar + mini-player + safe area + space(2) }`, and from `md` `mini-h + space(8)` (`base/_layout.scss`).
- Plurals: `voices-budget-numbers`, `voice-budget-left`, `voice-ask-left` (`$left` and `$runs`), `engine-gpu-left` use Fluent selects in all 8 locales. ar has zero/one/two/few/many/other (مرة واحدة, مرتان, مرات; ثانية واحدة, ثانيتان, ثوانٍ). zh keeps one form. Test `counts_of_runs_and_seconds_take_plural_forms` (en and ar, runs 1 and 2).
- Add sheet: split into `components/add_sheet/{mod.rs,forms.rs}`. Validation errors go to `error: RwSignal<Option<String>>`, rendered as `<p class="field__error" id="add-error" role="alert">` under the field, with `aria-invalid` and `aria-describedby`. Typing clears it, and switching form or reopening clears it too. Visible `<label class="field__label">` on all three fields; new keys `add-link-label`, `add-paste-title-label`, `add-paste-text-label` in 8 locales. `.field__error` in `_input.scss` (`danger`, `text("xs")`).
- Truncation: `.row-item__hint` and `.voice-row__meta` clamp at two lines. The Settings GPU row puts the numbers under the label (`row-item__hint`) instead of in the value column.
- Focus ring corners: `border-radius: t.radius("md")` on `.hero__open` and `.doc-row__main`. Also on `.stack > .row-item`, the rows that stand alone in a stack (reader More sheet, Appearance). The rendered page showed their rings still square, which the reviewer had not listed.
- Search snippets: `word_start` / `word_end` in `reading/search.rs` move the cut to a whitespace boundary (a run with no whitespace, such as Chinese, keeps the cut). Test `snippets_start_and_end_on_word_boundaries`.
- Import worker: new crate `crates/web-import` (`nolabs-web-import`). The lib holds the message contract (`Job`, `Reply`, `Fault`, `PdfLine`; JSON over `postMessage`, with EPUB bytes transferred). Feature `worker` adds `job::run` over `nolabs-import` and the bin `nolabs-import-worker`. trunk builds it with `data-type="worker"`. `tools/postbuild.py` moves it to hashed names under `/app/import/`, writes a loader (`importScripts` + `wasm_bindgen`, no inline script) and fills `<meta name="import-worker">`. `src/import_flow/worker.rs` starts the worker when the Add sheet opens (or on the first parse), queues jobs until it says ready, and terminates it once nothing is pending. `sw.js` does not precache `/app/import/`. It caches those files on first use into the shell cache of their version. The app no longer depends on `nolabs-import`.
- PDF import: `interop/pdf.js` called `doc.destroy()`, which the vendored pdf.js 6.4.299 does not have, so every PDF import failed with "Couldn't read that PDF (doc.destroy is not a function)". It now calls `task.destroy()` on the loading task. This bug existed before this round. It was found when the PDF path through the worker was tested.

### Decisions (assistant, under the task's delegation)
- The worker is terminated after each finished job instead of being kept alive. This gives back the parser's memory after a large book. The cost is a reload from the HTTP or SW cache on the next import (assumed small; not measured).
- The GPU row uses the label/hint layout. A second value-column variant was rejected, because the hint already wraps to two lines and the Voices page uses the same layout.
- The Arabic `[zero]` form reads "0 مرة", which matches the `[other]` wording.

### Bundle size (verified)
Measured with `trunk build --release` (wasm-opt from trunk), `wc -c`, `gzip -9c | wc -c` and Python `brotli.compress(quality=11)`:

| File | Raw B | gzip -9 B | brotli 11 B |
|---|---|---|---|
| Startup wasm before (`target/wasm-opt/release/nolabs-web_bg.wasm`, 05:59 build) | 2,928,824 | 1,182,458 | 906,779 |
| Startup wasm after (`nolabs-web-<hash>_bg.wasm`) | 1,884,573 | 720,548 | 554,026 |
| Import worker wasm (loads when the Add sheet opens) | 1,306,475 | 560,296 | 441,981 |

The startup wasm is 35.7 % smaller raw and 38.9 % smaller in brotli. The startup JS glue is 88,843 B raw. Before this round the app had no worker. Part of the worker's size is shared code (serde_json, nolabs-core) that now ships twice.

### Verification
- `cargo test -p nolabs-web -p nolabs-web-import --features nolabs-web-import/worker`: 135 + 3 passed (verified).
- `cargo clippy --target wasm32-unknown-unknown -p nolabs-web --all-targets -- -D warnings`, the same for `nolabs-web-import --features worker --bins --lib`, and native `-p nolabs-web-import --features worker --all-targets`: clean (verified). `cargo fmt --check` on both crates is clean.
- `trunk build` (debug, `STREAMS_READER_AUTH_URL=http://127.0.0.1:8099/supa`) and `trunk build --release`: success. check_tokens 0, check_style 0 (verified).
- Rendered page (headless Chrome Beta over CDP, mock `app-complete/mock.py` with the app's CSP, scripts in session scratchpad `app-complete/r4/`):
  - `check.py` in ink-ar-390, ink-de-390, ink-en-390, ink-en-1440 and paper-en-390, 46-57 checks each, 0 failed, 0 console errors (verified). It covers every sheet: add, more, voice, speed, sleep and contents, plus sign out on every device and clear saved audio in en. For each it checks that `dialog.open` is modal, focus moves in, 25 Tab presses stay inside, Escape closes, focus returns to the opener, the close button returns focus, a backdrop click closes and a click inside does not. It also checks the toast inside an open sheet (visible, on top, global copy hidden), the add-sheet labels, the inline errors (below the field, `aria-invalid`, `aria-describedby`, no toast) and that typing clears them. It confirms the import worker is not fetched at start and is fetched on the Add sheet, and runs imports through it: paste, link, Markdown, EPUB and PDF.
  - `obscure.py` at 390, real Tab presses, en: Library 0/44, Settings 0/60, Voices 0/54 focus stops under the tab bar (before: 10/60, 4/33, 8/54) (verified).
  - `trunc.py 390` in all 8 languages (sign-in, welcome, library, voices, record, settings, reader, 4 sheets): no clipped text (verified).
  - Screenshots `r4/shots/{de-390,ar-390,en-1440}/`: de GPU row "42 von 60 s · 2 Durchläufe" in full, the ring on the 2.5× chip fully above the tab bar, "Amerikanisch · Weiblich" on two lines, the hint "Beim Abspielen ist nur das Cover zu sehen" in full, ar "42 من 60 ث · مرتان", search snippets cut on word boundaries, rounded rings on the continue card and library rows (verified).

## Open items
- The Worker (`crates/worker`) must serve `/app/import/*` with the same CSP (`worker-src 'self'`, `script-src 'self' 'wasm-unsafe-eval'` already allow it) and long cache headers for the hashed names. Unblocked by: the Worker owner at deploy.
- `crates/web/dist` in the repo is from 07:29 and predates this round. Rebuild with `trunk build --release` before deploying.
- Shared code duplicated between the app and worker wasm could be trimmed (for example `nolabs-core` features). Unblocked by: a size budget decision from the owner.
