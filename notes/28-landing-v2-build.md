# Landing v2 build (crates/site)

Finding + reference. Landing v2 of note 15 is built in `crates/site` and verified on the rendered pages (en and ar at 390, 768 and 1440 px, light and dark, reduced motion). It is not deployed: the Worker serves `crates/site/dist` when its owner embeds it (see `crates/site/HEADERS.md`).

Correction 2026-10-31 (second pass, below): fonts are now 70,124 B for all three faces, not 154 KB; the italic face is preloaded. The critic's 15 defects are fixed; see "Critic fixes".

## 2026-10-31

### What is on the page (order of note 15)
Hero (eyebrow, H1 with the Literata italic accent word and an ember underline drawn once, OS-aware download CTA with an honest "Coming soon", film loop + interactive voice demo, fact row) → glow arc → "Classics to start with" (20 Standard Ebooks covers, numbered, marquee, preview clips) → four story chapters (alternating sides from 900 px) → voices (5 voice cards with previews + your voice, three tiers) → "More reasons to listen" (four blue-to-indigo cards with spots) → small details (8) → Available everywhere (Web / Desktop / Android / Chrome extension; the visitor's platform first) → promise + dated comparison with ElevenReader (D16) → FAQ (10, `details`/`summary`, height animated with `::details-content`) → closing CTA → footer.

Interactive demo: 4 samples (story, article, poem, late night) × 4 Kokoro voices, plus Emma (`ming:US_Emma`) on the late-night sample, reused from the film's Ming voice-over (no GPU run spent). Verified in headless Chrome with a scripted probe: play starts, the pill moves and words light up, voice switch keeps playing, picking Emma switches to the late-night sample with a status line, no console errors.

### Measured sizes (site-gen on 2026-10-31, raw bytes on disk; gzip -9 in brackets)
| File | Size |
|---|---|
| Landing HTML en / ar | 161,917 (24,549) / 167,978 (26,564) |
| Landing CSS | 40,442 (9,604); budget 40,960 raw |
| Inner-page CSS | 20,531 (5,442) |
| site.js (only script) | 18,610 (5,752); budget 25,600 raw |
| Hero poster AVIF portrait / landscape (LCP, preloaded) | 10,685 / 12,185; budget 122,880 |
| Hero loop portrait WebM / MP4 | 435,372 / 884,411 (loaded after `load`, in view, never on Save-Data, 2g or reduced motion) |
| Hero loop landscape WebM / MP4 | 484,353 / 977,531 |
| Fonts preloaded: Inter / Literata; Literata italic on first use | 48,256 / 52,496; 53,728 |
| Covers, 20 titles × 4 files | 3,082 to 17,234 each, all lazy |
| Demo clips (WebM/Opus) | 20,818 to 53,428 each, fetched on first play |
| Inner pages HTML (download / help / privacy / terms / credits / 404, en) | 33,748 / 26,867 / 14,632 / 14,121 / 18,387 / 12,791 |

Measured with a Python `gzip` script over `crates/site/dist` and the site-gen summary line. First view on a phone, assumed (not measured in Lighthouse): HTML 24.5 KB + CSS 9.6 KB + JS 5.8 KB gzip + three fonts 154 KB + poster 10.7 KB ≈ 205 KB before the film. Fonts exceed note 15's 90 KB font budget (open item).

### Wrong beliefs found
- Belief (v1 SCSS): `@extend %disc` / `%ri-sm` / `%lift` placeholders in `_tk.scss` reach every module that `@use`s it. Correction: grass 0.13.4 emits nothing for an `@extend` across `@use` modules, silently. The section eyebrow chips, cover play buttons, the recording button and every small icon were unstyled squares. Fixed with grouped selectors in `_base.scss` and at the top of `_home.scss`; placeholders replaced by mixins. Status: verified on the rendered page.
- Belief: `.topbar__gh { display: none }` hides the GitHub button on phones. Correction: `.btn` in `_components.scss` comes later with the same specificity and shows it, which pushed the logo mark to zero width at 390 px. Fixed with `.topbar .topbar__gh`. Verified.
- Belief: the hero stage could be a one-column grid without a track size. Correction: the implicit `auto` track grew to the min-content width of the tabs and voice chips, so the page scrolled sideways (scrollWidth 448 at 390 px). Fixed with `minmax(0, 1fr)`; the voice chips now wrap. Verified with an overflow probe (scrollWidth equals clientWidth for en and ar at 390 and 768).

### Decisions (assistant, under the owner's delegation D4 and D34)
- Site origin is `https://reader.streams.watch` (canonical, sitemap, hreflang, robots, llms.txt, IndexNow key location). Reason: D34 made it the main address and left "site canonical URLs and sitemap" open. `listen.streams.watch` must answer with a permanent redirect (Worker owner).
- Section gaps follow note 15 A0 (96 px phones, 136 px wide): each section pads half of it; the promise band keeps its larger inner padding.
- CSS stays under 40 KB raw by post-processing in `css.rs` (fold `calc(<length>*<number>)`, `transparent` → `#0000`, merge repeated `@media (min-width)` and reduced-motion blocks in breakpoint order) instead of dropping design. Rejected: dropping the mini-UIs of the device tiles (2.8 KB) or measuring the budget gzipped.
- Phones: sample tabs drop their icons and share the width; the film's play button moves to the top corner, because the demo card covers the film's bottom edge.

### Tests (cargo test -p nolabs-site: 32 unit + 9 integration, all pass; clippy -D warnings clean; brand/check_tokens.py 0 findings)
New: JSON-LD types are exactly SoftwareApplication, FAQPage, Organization, WebSite, ItemList, with no rating or review; landing asset budget (CSS, JS, HTML, posters, film files, covers, clips); every demo clip has its WebM and timings that move forward inside the text; every icon name used in the page sources is vendored (this caught the missing `book-2-line`; `book-shelf-line` v4.9.1 was added from jsDelivr); calc folding; media packing.

### Screenshots
Local only, not committed: `/private/tmp/claude-501/-Users-ibrahim-code-nolabs-reader/a97fc270-2dc8-4cdf-895f-e925bee528d9/scratchpad/landing-v2/shots/` (`{en,ar}-{390,768,1440}-NN.png`), contact sheets in `.../landing-v2/sheets/`, dark, reduced-motion and keyboard-focus shots in `.../landing-v2/one/`. Script: `.../landing-v2/shoot.py` (CDP on port 9471; port 9333 is used by another agent's Chrome).

## 2026-10-31 — Critic fixes (second pass)

All 15 defects from the critic run (crops in the session scratchpad `landing-v2/critic/`) fixed at the root and checked on the rendered pages in headless Chrome Beta (en and ar at 390, 768, 1440; de, fr, es, pt, hi, zh probed at 390 and 1440; iPhone, Android and Windows user agents; reduced motion). Status: verified unless marked.

| Defect | Fix | Check (verified) |
|---|---|---|
| Arc ends in a hard cut | `.arc` mask fades to transparent at 35 %; fill `radial-gradient(60% 12% …)`; 2 px top border kept | screenshots en/ar 390, 768, 1440 |
| 404 buttons unstyled | `.hero__ctas` and `.cta__actions` replaced by one `.actions` in `_components.scss` (hero, closing band, 404); `.platform__body` got `min-width: 0`; `menu__btn` dropped; `platforms--detail` hook became `data-detected-first` | 404 at 1440: three buttons at y = 445, 12 px gaps, 32 px below the lede; new test `every_class_on_an_inner_page_is_styled_by_the_inner_stylesheet` (fails on `hero__ctas`, checked by reintroducing it) |
| Keyboard focus on off-screen covers | site.js: first keyboard focus (`:focus-visible`) in the marquee adds `.is-manual` (static scroll-snap row, arrows) and `scrollIntoView`; pointerdown outside drops it; reduced motion sets it too | Tab probe 1440: covers 1-5 at x = 152, 362, 572, 782, 992; 390: x = 16, 184; ar 390: 218 |
| 10 look-alike spots (D37) | spots out of chapters, promise and reasons; promise uses the 36 px icon chip; reasons a 56 px Remix glyph in a soft disc; chapter 01 mock sized to its content (`aspect-ratio: auto`, no fixed `.clean` height) | 0 `class="spot"` in en/ar landing |
| Comparison clipped on phones | under 600 px the table is stacked rows: label as a small caption, two value cells side by side under the two-column header | screenshots en/ar 390; no element wider than its box |
| Arabic accent in faux italic | `:lang(ar/hi/zh) .accent { font-style: normal }` (hero and chapter 03) | ar 1440 screenshot |
| Arabic voice chips one per row | under 600 px a 2-column grid, odd last chip full width, labels ellipsize | `.voices` 148 px tall (3 rows) in all 8 locales at 390 |
| Tile pills wrap, chips misaligned | status chip moved into `.tile__actions`, which has fixed rows (status, action, link or its 44 px room) and sits at the bottom; pills `nowrap`; title icon beside a wrapping name; shorter Android/desktop action labels in 8 languages | probe: no tile button overflows in 8 locales; chip and pill tops equal across each row |
| Tap targets < 44 px | `.tab` (covers the speed chip) and `.toc__link` `min-height: tap-min` | heights 44 on landing tabs and /help, /ar/help TOC |
| iOS button promised an app | `data-label-ios` "Add to Home Screen" (8 languages) + `data-href-ios` → `/download#install`; badge removed on iOS | iPhone UA: "Add to Home Screen -> /download#install", no badge |
| Demo below the fold at 1440×900 | from 1200 px the hero is two columns: head (start-aligned) beside the demo, film below at full width; H1 capped at the 3xl token (72 px); hero top padding 40 px. The demo now comes before the film in the source (keyboard order follows the wide layout; phones place rows with `grid-row`) | `.play` bottom = 653 px at 1440×900 in en, de, fr, ar (target ≤ 860) |
| Fonts 154 KB | `tools/subset_fonts.py` (uv, fonttools 4.55.0, brotli 1.1.0): upright Latin-1 + U+2010-2027, U+2190-2193, weight axis limited to 400-700; italic limited to 450-550 and to the 38 characters the site sets in italic; all three preloaded | Inter 27,856 + Literata 33,056 + italic 9,212 = 70,124 B; tests `fonts_stay_within_budget`, `italic_subset_covers_every_italic_string` |
| Page repeats itself | "Small details" cut to 4 items told nowhere else (keyboard, eight languages, dark and light, install as an app); "Your own voice" reasons card replaced by "Classics, free"; chapter 02 ghost title removed (it also overlapped the text on phones) | en landing height at 390: 13,872 px (was 14,198) |
| Rank numerals barely read | numeral at 1.15 × cover height, 2 px `text-3` stroke on `bg` fill, cover padded 0.5 × cover (0.98 for two digits) so it overlaps about a quarter; static row starts at the content gutter (`--gutter` moved to `:root`, `.marquee` is a size container) | reduced motion: first cover x = 16 at 390, 152 at 1440 (= heading x) |
| "More reasons" lacked the header pattern | same `head()` markup: star icon chip, eyebrow "Why people stay" (8 languages), `.section__title` | screenshots |

### Measured sizes (site-gen 2026-10-31, raw bytes; gzip -9 in brackets; Python `gzip` over `crates/site/dist`)
| File | Size |
|---|---|
| Landing HTML en / ar | 163,618 (25,959) / 169,672 (28,208) |
| Landing CSS | 40,852 (9,731); budget 40,960 |
| Inner CSS | 20,291 (5,427) |
| site.js | 19,356 (6,023); budget 25,600 |
| Fonts (three faces, all preloaded) | 70,124; budget 92,160 |

CSS headroom is 108 B. To stay under it, `css.rs` now also folds same-unit `calc()` sums, writes block-direction sides as physical properties (`margin-block-start` → `margin-top`; the site only uses horizontal writing), drops light-theme values equal to the dark defaults, removes spaces around `*` and `/`, and merges `@media (max-width:…)` blocks (placed before the min-width blocks). Unit tests cover the new passes.

### Wrong beliefs found
- Belief (critic): the Literata italic file loads only for one hero word. Correction: the top bar's "Reader", the footer tagline, the demo poem and the reader mock's "Chapter 1" are italic too, on every page; the subset covers all of them and is preloaded.
- Belief (critic): cutting about 100 px of hero spacing and moving the card up would put `.play` above 860 px. Correction: measured, the head ends at y = 597 and the play button sits 448 px into the card, so stacking cannot fit 900 px; the two-column hero was needed (decision below).
- Belief: `.mock--clean { aspect-ratio: 4/3 }` overrides `.mock`. Correction: `.mock`'s 600 px rule is merged later by `pack_media` and wins at equal specificity; the override needs `.mock.mock--clean`.

### Decisions (assistant, under the owner's "you take the lead", 2026-10-31)
- Two-column hero from 1200 px (head beside the demo, film below). Reason: the owner's ElevenLabs reference keeps the demo in the first view; measured above. Rejected: shrinking the stacked hero (cannot reach 860 px).
- "Small details" holds 4 items, the reasons card "Your own voice" becomes "Classics, free", the chapter 02 ghost title goes. Reason: the critic's repetition finding and the 40 KB CSS budget. Rejected: dropping the voice tiers (they carry the engine names, Kokoro-82M and Ming-3.0-tts).
- Spot renders are still copied to `dist/assets/spots/` (132 KB) but no page uses them; kept for the about-4 redrawn spots of D37.

## 2026-10-31 — Review round 3 (11 defects)

All 11 fixed at the root and checked in headless Chrome Beta against `crates/site/dist` served on 127.0.0.1:8793. Status: verified unless marked. Probes and shots: session scratchpad `landing-v2/r3/` and `landing-v2/shots6/` (local only).

| Defect | Fix | Check (verified) |
|---|---|---|
| #faq overshoot at 1440 (top −1,118 px) | removed `content-visibility: auto; contain-intrinsic-size: auto 900px` from `main > .section` (`_layout.scss`) | `tools/check_anchors.py`: 7 anchors (page and footer) × 390/768/1440, en and ar, all at top = 80 px; test `landing_layout_keeps_anchor_targets_and_order_stable` |
| Film drawn above the demo below 1200 px, focus jumped up off screen | no `grid-row` outside lg: head → demo → film in source and on screen; demo overlap margin dropped (normal top margin), film toggle at the bottom corner everywhere; demo's backdrop blur and translucency dropped (nothing under it now) | Tab from the speed chip → film toggle at y = 440 (390), 530 (768), 468 (1440), all in view; demo top in the first view at 390 |
| Faux italic tagline in ar/hi/zh | `:is(em, i, .accent, .footer__tagline):is(:lang(ar), :lang(hi), :lang(zh)) { font-style: normal }`, matched on the element so English inserts keep italic | computed `font-style: normal` for tagline and accent in ar, hi, zh; `italic` in en |
| Empty chapter cards | `.mock` sized by content (no aspect-ratio); only the lock card keeps a shape: 5/4, 2.2/1 with 24 % side padding from 600 px (narrower container → smaller phone), 5/4 from 900 px; phone `width: 52cqw`, centred | bottom gap = card padding (21 px at 390, 33 px at 768/1440) for cards 01-03; phone bleeds the bottom edge at every width |
| Time label 0:07 → 0:08 → down | `DemoText.duration` = clip `d`; server label and site.js both use it; site.js keeps the label while the next clip loads | label sequence on play at 1440: 0:08 0:08 0:08 0:08 0:07 … 0:06 (monotonic); test `demo_time_label_is_the_clip_duration` |
| Mirrored transport in RTL | `dir="ltr"` on `.mini__bar`, `.mini__ctl` (chapter 04 and Android tile) and `.mini__panel` (desktop tile) | ar: computed direction ltr, icons at x = 359, 419, 480 (back, pause, forward); test `rtl_keeps_transport_controls_and_key_names_left_to_right` |
| First focus in classics row off screen; prev arrow live at 0 | site.js: on first keyboard entry, re-place the cover in the next frame (`block: "center"`); arrows get `aria-disabled` at the ends (stays focusable) and do nothing; faded at 0.4 | first focused cover at y = 384 (390, two runs) and 474 (768), fully in view; arrows: start `-1:true`, end `1:true` in en and ar |
| Two footer links named "Credits" | Library column link is `footer-book-sources` ("Where the books come from", 8 languages) | test `titles_and_footer_link_names_do_not_repeat` (all pages, 8 languages) |
| Poster hidden ~1 s by the entrance | `.film` and `.hero__title` use `glide` (transform only, no opacity) | LCP element at 1440 is `IMG.film__poster`; H1 at 390 |
| Reason titles on 1 or 2 lines | `.reason__title { min-height: 2lh }` from 600 px | text top 88 px in all four cards, en/de/fr/ar at 768 and 1440 |
| Copy slips | `title()` drops the suffix when the text already names the brand; /download title from `nav-download` ("Download · Streams Reader", ar "التنزيل · Streams Reader"); Pride and Prejudice moved to "Free library"; design group renamed "Type and icons" (8 languages); ar key list J, L, [, ] wrapped in U+2066…U+2069 | curl titles; credits group order; a side-by-side render showed the isolated line differs from the mirrored one |

### Sizes after round 3 (site-gen 2026-10-31, raw; gzip -9 in brackets)
Landing CSS 40,954 (9,722), budget 40,960, headroom 6 B. site.js 20,205 (6,296). Landing HTML en 163,686 (25,968), ar 169,748 (28,222). cargo test -p nolabs-site: 33 unit + 16 integration pass; clippy -D warnings clean; brand/check_tokens.py 0 findings.

### Wrong beliefs found
- Belief (v2 build): `content-visibility: auto` on sections is a free rendering win. Correction: its 900 px estimate moved smooth-scroll targets as sections rendered; #faq landed at −1,118 px at 1440. Removed.
- Belief: with absolute insets in container units, side padding on the container shrinks the phone. Correction: insets resolve against the padding box while `cqw` uses the content box, and `aspect-ratio` gives the phone a min-content minimum width (the 11cqw clock forced 250 px). Fixed with an explicit `width: 52cqw` and auto margins.

## 2026-10-31 — Review round 4 (6 defects)

All 6 fixed at the root and checked in headless Chrome Beta against `crates/site/dist` on 127.0.0.1:8793. Status: verified. Probes and shots: session scratchpad `landing-v2/r5/` (local only).

| Defect | Fix | Check (verified) |
|---|---|---|
| Classics row: from cover 11 on, every second focused cover snapped back off screen | site.js focusin scrolls with `inline: "start"` (both calls), matching `scroll-snap-align: start`; `.is-manual .marquee__list` gets 64 px end padding (`tap-min` + `space-5`) so the last cover clears the "More books" arrow | `r5/kb/kb.py`, Tab ×22 from the film toggle: all 20 covers in view, 0 off screen. Cover x: en 390 16..222 (last 120..326), ar 390 168..374 (last 63..269), en 768 20..226 (last 498..704), ar 768 542..748 (last 63..269); crops `r5/kb/row2.png` |
| Demo card height changed with the caption | all four captions (caption, switch, locked, error) rendered stacked in one grid cell (`.demo__note`, `grid-area: 1/1`), inactive ones `visibility: hidden`, the stack `aria-hidden`; a separate `.sr-only` `aria-live` node carries the text; `data-msg-*` attributes removed | `r5/demo/shift.py`: card height, H1 top and film top identical across story → Emma → article → poem in en, ar, de, fr, hi at 390, 768, 1440 (for example en 1440 618/246/770 throughout) |
| /credits licence badge stretched to row height below 600 px | `.credit__licence { align-self: start }` | badge 26 px tall at 390, en and ar; `r5/cred-en390-top.png` |
| /credits name links 23 px tall | `.credit { position: relative }`, `.credit__name::after { inset: 0 }`; focus ring drawn on the `::after` | row hit area 358×102 at 390, 728×77 at 768; elementFromPoint near the row bottom hits the link; ring visible on the whole row |
| 404 `<title>` kept the heading's full stop | new `lost-page-title` in 8 locales ("Page not found" …); test `titles_and_footer_link_names_do_not_repeat` now also fails on `.。।!?！？` before " · " | titles of all 8 404 pages |
| Reasons row (scroll row below 600 px) not reachable by keyboard in Safari | `ul.reasons` gets `tabindex="0" aria-labelledby="reasons-title" data-scroller` (list role kept: `role=region` on a `ul` would orphan its items); site.js sets `tabIndex` to −1 where the row does not scroll | 390 en/ar: Tab from the title lands on the list with a ring, ArrowRight scrolls it (scrollLeft 795 en); 768 and 1440: `tabIndex` −1, no dead stop |

### Bytes
To fit the demo caption stack, `css.rs` now writes `@media(`/`@supports(` without the space (12 B), `main:focus`, `.demo:focus`, `.marquee__viewport:focus` share one rule in `_base.scss`, and `.demo__source` and `.compare__asof` share one rule. Landing CSS 40,929 (gzip 9,742), headroom 31 B; site.js 20,784 (6,530); inner CSS 20,458. cargo test -p nolabs-site: 33 unit + 16 integration pass; clippy -D warnings clean; brand/check_tokens.py 0 findings in crates/site.

### Wrong beliefs found
- Belief (round 3): re-placing the cover once on first focus keeps every cover in view. Correction: `inline: "nearest"` disagrees with the start snap points, and the snap pulled every second cover back off screen from cover 11 on.

## Open items
- CSS headroom is 31 B under 40,960 (round 4). The next style change needs a cut first; candidates measured: the two `ri-sm` icon rules and the three `28px` icon rules have identical bodies (about 25-50 B if grouped). Open; unblocks when the next CSS change is made.
- ~~Fonts are 154 KB against note 15's 90 KB budget~~ superseded 2026-10-31: 70,124 B after `tools/subset_fonts.py`.
- When an italic string changes (tagline, accent words, poem), rerun `tools/subset_fonts.py`; `cargo test` fails until then.
- Unused spot renders in `dist/assets/spots/`: remove or replace once D37's redrawn spots exist (owner of D37).
- Lighthouse LCP/CLS/INP not measured (unblocks after deploy).
- The Worker must serve `crates/site/dist` at `/` and redirect `listen.streams.watch` → `reader.streams.watch` (Worker owner).
- Chapter 04 still uses the screen-off copy; switch to cross-device copy when sync is live (note 15 A4 gate).
- `tools/render_demo.py` (v1 MP3 demo) is unused since the v2 clips; delete it or keep it for reference (owner's call, harmless).
