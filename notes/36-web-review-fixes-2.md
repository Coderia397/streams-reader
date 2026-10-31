# Web app review fixes, round 2 (2026-10-31)

Finding + decision. Conclusion: all 13 round-2 reviewer defects in `crates/web` (plus the brand layer they touch) are fixed at the root and verified on the rendered page against the local mock. Three suggested fixes were changed because they would not have worked as written (Fluent `NUMBER()`, the colour check regex, the chip-row class); see Decisions.

## 2026-10-31

### What changed
- Focus ring: new brand mixin `t.focus-ring-inset` (outline offset `-focus-width`). Used on `.row-item`, `.voice-row__main` and `.mini__body`, which sit inside containers with `overflow: hidden`. The first and last child of `.group__card` / `.list-card` take the card's inner radius (`card-radius - border-width`), and `.voice-row__main` inherits it, so the ring follows the corners.
- Link form: `novalidate` on the Paste a link form. Bare addresses go through `normalise_url`, and junk shows the translated `add-link-invalid` toast instead of Chrome's English bubble.
- Bookmarks: `.bookmark`, `.bookmark__jump`, `.bookmark__text` styled (icon in the `.row-item__icon` column, 44 px jump target, one-line truncation). `icon-btn--sm` dropped. Saving a place whose sentence already has a bookmark shows `bookmark-exists` and adds nothing (`saved_in_segment`, unit tested). A tap before the sheet's list has loaded fetches it first, and a second tap while a save is in flight is ignored. Async writes use `try_` so a closed sheet cannot panic.
- Search hits: `dir="auto"` on `.hits__item`, so English snippets keep their own direction in Arabic.
- Numbers: `i18n/number.rs` installs a Fluent formatter on every bundle. It applies `NUMBER()` options and the language's decimal separator (`Lang::decimal_separator`: comma for es, fr, de, pt). `NUMBER()` is now registered (`add_builtins`). New keys in all 8 locales: `size-b`, `size-kb`, `size-mb`, `size-gb`, `speed-value`. `format::bytes` and `format::speed` use them. French uses o/ko/Mo/Go, Arabic بايت/كيلوبايت/ميغابايت/غيغابايت.
- Hit areas: `t.hit-area` now grows both axes to `--tap-min` with `min(0px, (100% - tap-min) / 2)` insets. Added to `.play-btn--small` and `.tabbar__brand`.
- Contents sheet: `Sheet`'s `tall` prop is now a `Signal<bool>`. Contents passes `chapters > 8` (`TALL_AFTER`). Search, voice and language sheets keep `tall=true`.
- Picked sentence: an underline instead of the per-line outline (D38).
- Settings default speed: new `chip-row--thirds` (3-column grid below `sm`, flex row from `sm`). The sleep sheet keeps `chip-row--wrap`.
- Record page: `.topbar` spans the window from `md` (`margin-inline: calc(50% - 50vw)`, matching padding).
- Sign out on every device: opens a confirm sheet (`account-sign-out-all-confirm`, `-line`, `-action`, all 8 locales) before `sign_out(true)`.
- Classes without rules: added `.btn--lg` (new token `button-height-lg = 48px`), `.input--mono` (placeholder stays in the UI face), `.field--reveal` (rise animation), `.auth__email`. Removed `auth__glow` div, `is-stuck`, `is-hidden-text`, `chunk` and the unused `Scrubber compact` prop. `tools/check_style.py` now fails a build when a `class="…"` or `class:name=` in `src` has no selector in the built CSS (allowlist `UNSTYLED_OK`: `sheet__close`). It runs in the trunk post-build hook against the staging CSS, or with `--css <file>`.
- Generative colour: new `[generative]` table in `brand/tokens.toml` (cover and orb lightness, chroma, alpha; emitted as `--cover-*` / `--orb-*`, read with `t.gen()`). `_cover.scss` and `_orb.scss` draw in `oklch(...)` from those tokens. `cover.rs::BASE_HUES` moved to OKLCH degrees. `brand/check_tokens.py` now flags any colour function (rgb, hsl, hwb, oklch, oklab, lab, lch) whose arguments hold a number outside `var(...)`. Test cases are in `brand/tests/test_tokens.py`.

### Decisions (assistant, under the task's delegation)
- Fluent's `NUMBER($n, maximumFractionDigits: 1)` alone would not work. fluent-bundle 0.16 ignores `maximumFractionDigits` and always writes "." (`FluentNumber::as_string`, verified in the crate source), and the bundles did not register `NUMBER` at all. The fix is a bundle formatter plus `add_builtins`. Arabic, Hindi and Chinese keep "." because the app writes Latin digits everywhere.
- The reviewer's regex `\b(?:rgba?|…)\(` would also flag the new token-only `oklch(var(--…))` calls, and then every line would need `tokens-ok`. The check flags literal numbers inside a colour function instead.
- The 3 + 3 grid is a new modifier, not a change to `chip-row--wrap`, because the sleep sheet uses `--wrap` with seven chips.
- A duplicate bookmark shows a short toast ("This place is already bookmarked.") instead of doing nothing silently.
- Mid-row corners: the voice row ring is rounded at all four corners (inherited radius); this reads as a focus pill and was accepted on the close-up.

### Verification (verified)
- `cargo test -p nolabs-web`: 133 passed (130 before, plus `a_place_is_saved_once_per_sentence`, `rounds_trims_and_localises`, `numbers_follow_the_language`).
- `cargo clippy -p nolabs-web --target wasm32-unknown-unknown -- -D warnings` and `cargo clippy -p nolabs-web --tests -- -D warnings`: clean.
- `uv run --no-project --python 3.12 -m unittest discover -s brand/tests`: 33 OK (7 skipped). `brand/check_tokens.py crates/web/style crates/site/style`: 0 findings.
- `trunk build` (debug, `STREAMS_READER_AUTH_URL=http://127.0.0.1:8099/supa`): success; check_tokens 0, check_style 0, "markup classes checked against main-<hash>.css". Negative check: a fake `class="a b--x"` against CSS with only `.a` is reported.
- Rendered page (headless Chrome over CDP, scratchpad `app-complete/fix2/`, mock `app-complete/mock.py`):
  - `shoot.py`, all 8 combinations (Ink/Paper × en/ar × 390/1440): 23 shots each, 0 failed scenes, 0 console errors.
  - `probe2.py` for paper-en-390, ink-ar-390, paper-ar-1440, paper-de-390:
    - Focus: Tab presses to Sign out and Sign out on every device, and to `.voice-row__main`. Outline 2 px, offset -2px, ring inside the card on all of them. Close-ups `probe2/ink-ar-390/focus-*.png`.
    - Link: `noValidate` true; "not a url" shows the translated message (en, ar, de); "example.com/post" starts the import.
    - Bookmarks: after clearing, two taps give 1 row and the `bookmark-exists` toast. The jump button is 44 px tall. The icon is in the same column as the other rows (x 37 in en, 333 in ar) and centred on the label (same mid-y). The remove button is 44×44.
    - Search hits: `dir=auto`, computed `ltr` for English snippets in Arabic (screenshot `probe2/ink-ar-390/search.png`).
    - Contents: 3 chapters, `sheet--tall` off, sheet height 262 px at 390 and 254 px at 1440.
    - Hit areas (elementFromPoint, scrolled to centre): `.play-btn--small` 44×44, `.tabbar__brand` 44 tall at 1440, speed-sheet 1× chip 44×44.
    - Settings chips at 390: two rows of three, each 103 px wide; one row at 1440.
    - Numbers: de Settings shows "21,6 kB" and chips "0,8× … 2,5×"; ar shows "21.6 كيلوبايت". The ar Latin-text scan no longer lists the size.
    - Record at 1440: `.topbar` spans 0–1440, no horizontal scroll, back button on the page column.
    - Sign out on every device opens the dialog ("Sign out on every device? / Sign out everywhere"); the page stays on `/app/settings`.
  - Reviewer audit (`audit.py`) rerun for paper-en-390, ink-ar-390 and paper-ar-1440. These findings are gone: the 40×40 play button, the 32 px rail mark, the 42 px 1× chip and the 28 px bookmark rows. What the audit still lists at 390 (x26 to x30) is controls partly under the fixed tab bar at the viewport's bottom edge; scrolled to the centre they measure 44 (assumed harmless, as in round 1).

## Open items
- Server-side cleanup of `ready = false` rows older than a day (Worker owner), carried over from note 34.
- The `.topbar` has no bottom hairline while the sticky footer has a top one. This is consistent with `.reader__bar` and is left for an owner look.
