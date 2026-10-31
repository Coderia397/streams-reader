# Web app review fixes (2026-10-31)

Update 2026-10-31: round 2 is in [36](36-web-review-fixes-2.md). It resolves the speed row (3 + 3 grid at 390) and `.btn--lg` (48 px rule) open items below, and replaces the picked-sentence outline with an underline (D38).

Finding + decision. Conclusion: all 13 reviewer defects in `crates/web` are fixed at the root and verified on the real rendered page (8 combinations: Ink/Paper × en/ar × 390/1440). Two of the reviewer's suggested fixes were replaced because they failed on the real page (chip grid, `match-parent`); see Decisions.

## 2026-10-31

### What changed
- Sheets: the speed, voice and sleep sheets are now rendered beside `.dock`, not inside it. `.dock` has a `backdrop-filter`, which made it the containing block of their fixed scrim and panel. The reader owns the one `ContentsSheet`; `Dock` gets an `on_contents` callback.
- Picked sentence: `.seg.is-picked` is a 1 px `brand-text` inset outline with no fill and 0.1em padding (note 12). Before, it had the same wash as the spoken sentence.
- RTL time row: `.scrubber__times > :last-child` uses `justify-self: end; text-align: right`, because the row is always LTR.
- Hit areas: a new brand mixin `t.hit-area` (in `brand/brandkit/scss_api.scss`) adds a `::before` that is `--tap-min` tall. It is used on `.chip`, `.segmented__opt`, `.btn--sm`, `.disclosure` and the new `.link-btn` style. `.link-btn` had no CSS before. `.scrubber__range` is 44 px tall. Wrapped chip rows have a 12 px row gap, so the hit areas of the rows do not overlap.
- Chip rows: the settings default speed row wraps (`chip-row--wrap`); `chip-row--scroll` was removed. The library's three ways in are now `AddTiles compact` (icon above the label, 3 equal columns, label clamped to 2 lines). The empty library uses the same component at full size.
- Sign-in providers: `stack--snug` (gap space 2).
- Voice row: the name is `<span class="voice-row__label" dir="auto">`, and `.voice-row__name` wraps, so the badge moves under the name. The name never breaks.
- Library meta line: each part is wrapped in FSI/PDI (`meta.rs::isolate`), with a test.
- Import: the body is cached in IndexedDB before the upload. If `put_body` fails, the Worker DELETE removes the row; the local copy is removed only if that delete succeeds. Library rows have three states (`meta::upload_state`): Ready, Preparing (spinner with `role=status`, "Preparing…") and Stuck (`updated_at` more than `UPLOAD_STALE_MS` = 120 s ago: "Upload didn't finish" in danger, with Try again and Delete). Retry re-sends the cached body. If there is none, it re-reads the row and, if the row is still not ready, says "This file is no longer on this device…". New keys in all 8 locales: `library-preparing`, `library-upload-failed`, `library-retry-gone`, `action-delete`.
- Titles: `.mini__title` and `.doc-row__title` use `dir="auto"` (no inner `<bdi>`), with a physical `text-align` per page direction.
- Toasts and dialogs: on railed pages at `md`, the toasts start after `--rail-w` and the `.sheet` dialog is shifted by `--rail-w / 2`. `.app--no-mini` sets `--mini-h: 0px` when no document is loaded.
- Progress: library playback bars have `progress--playback`, which is LTR like the mini-player and the scrubber. Budget bars (settings/about) still follow the page direction.
- Tokens: `[component] glass-blur = "20px"`, `glass-saturate = "140%"` and a `t.glass` mixin replace five raw `blur(20px)` call sites. Raw `160ms` and `60ms` became `t.dur("fast")`. `tools/check_style.py` now rejects raw `NNms` and `blur(Npx)`. The reduced motion `0.01ms` lines are marked `tokens-ok`.

### Decisions (assistant, under the task's delegation)
- Library quick actions are compact tiles, not the suggested `repeat(3, 1fr)` chip grid with truncation. Measured at 390 with `fix1/chips.py` (CDP, scrollWidth vs clientWidth of the label, 68 px available): the label was cut in en, es, fr, de, pt and hi ("Abrir um arquivo" needs 112 px). Rejected: horizontal scroll (the reviewer said it looks broken) and wrapping (2 + 1 layout).
- Voice name: the badge wraps under the name. The suggested truncation alone showed "M…" at 390 (verified in a screenshot).
- `text-align: match-parent` is not applied in Chrome (the computed value stays `start`, verified with the probe), so the code uses `text-align: left` with a `[dir="rtl"]` override.
- The playback LTR rule is scoped to `progress--playback`, so the budget bars are not forced LTR.

### Verification (verified)
- `cargo test -p nolabs-web`: 130 passed (128 before plus `upload_states` and `meta_line_isolates_each_part`).
- `cargo clippy -p nolabs-web --target wasm32-unknown-unknown -- -D warnings` and `--tests`: both clean.
- `uv run --no-project --python 3.12 -m unittest discover -s brand/tests`: 33 OK (7 skipped). `brand/check_tokens.py crates/site/style`: 0 findings.
- `trunk build` (debug, `STREAMS_READER_AUTH_URL=http://127.0.0.1:8099/supa`): success, with check_tokens and check_style at 0 findings.
- `shoot.py`, all 8 combinations: 23 shots each, 0 failed scenes, 0 console errors.
- `fix1/probe.py` for ink-en-390, paper-ar-390 and ink-ar-1440 (getBoundingClientRect, plus elementFromPoint up and down from the centre for hit height):
  - Every sheet is outside `.dock`, the scrim covers the viewport (390x844 and 1440x900), and a tap on the scrim closes the sheet. At 1440 the sheets fit in the viewport (speed y 302–599, contents 90–810).
  - Hit heights: chips, `.btn--sm`, `.segmented__opt`, `.disclosure` and `.link-btn` are all 44. The scrubber is 44/42: the chapter chip's hit area overlaps it by 2 px.
  - The Arabic chapter label is 0 px from the right end of the row. The provider gap is 8 px. The voice labels are on one line and not truncated. The meta lines carry isolates.
  - At 1440, the toast and the action dialog are 0 px off the main column's centre. The toast is 8 px above the mini-player.
- Screenshots: scratchpad `app-complete/fix1/shots/<theme>-<lang>-<size>/` (local, not kept in the repo).

## Open items
- The settings default speed row wraps to 5 + 1 chips at 390. It is not clipped, but it is unbalanced. A design decision (for example a 3 + 3 grid) unblocks it.
- Retry for a stuck row cannot re-upload when the cached body is gone (other device, cleared storage). It says so and offers Delete. Server-side cleanup of `ready = false` rows older than a day would unblock this fully (Worker owner).
- `.btn--lg` (used by the sign-in providers) has no rule in `_button.scss`; it is a no-op class (assumed harmless, open).
