# Design benchmark (ElevenReader) and design brief for nolabs

Finding + plan. ElevenReader's Android app (v1.7.30) and public marketing pages were studied screen by screen. Its reader screen is excellent, while the rest of the app is a store with a reader attached. The one conclusion: nolabs should copy the reader and player patterns closely (paragraph wash plus word pill, chapter chip, re-centre button, sheets), and win on everything ElevenReader spends on selling: a home screen built around your own documents, no meters or upsells, honest import states, live settings, desktop apps, and a dark, literary identity.

Sources: Pixel 8 over ADB (screenshots and UI dumps in `raw/design-private/android/`, gitignored), public pages elevenreader.io, elevenreader.io/pricing and elevenlabs.io/text-reader (HTML and a headless render in `raw/design-private/web/`). Personal data seen in the app is not copied here.

## 2026-10-30

### How the observations were made
- Device: Pixel 8, 1080x2400 px, density 410 dpi override (`adb shell wm density`), so 1 dp = 2.5625 px. Every dp value below is a pixel bound from `uiautomator dump` divided by 2.5625. Status: verified.
- Colours were sampled from screenshots with Pillow at single pixels. Status: verified (light theme only; the app follows the system theme and changing it was out of scope).
- The player window sets `FLAG_SECURE` (verified with `dumpsys window`: `fl=... SECURE`). While the full player chrome was up, one screenshot came back black. Later captures of the same screen worked, so the black frame may have come from screen state rather than the flag. Status: open. The UI dump worked in every case.
- The logged-in web reader was not seen. The Claude in Chrome extension returned "not connected" 3 times, so that source was dropped as the brief required. Status: open (see open items).
- Side effect: the first relaunch restored the player, and my Library-tab tap at (752,2255) landed in the player's Bookmark area. A playback of about 45 s then started (the trigger was not identified) and was paused with `KEYCODE_MEDIA_PAUSE`. The bookmarks list showed "No bookmarks found", so no bookmark was added. The title's position moved from 7:03 to 7:48. Nothing else was changed. The phone was left on its home screen with the media session PAUSED. Status: verified.

### (a) ElevenReader, screen by screen

Global system (verified unless marked)
- Colour: monochrome light UI. Background #FCFCFC; chips, segment controls and icon-button circles #F0F0F0 to #F3F3F3; primary text and filled controls #202020; the selected tab pill #E2E2E2; sheet scrim #A3A3A3 (black at about 35 %). Book covers and the voice orbs supply all the colour. The marketing site uses the same palette (#FDFCFC, #F5F3F1, #202020, #646363) with orange #FF8E00 as the only accent.
- Type: Inter everywhere in the app. Page titles are large: "Home" and "Library" sit in a 40 dp line (about 32 sp), and section titles are about 22 sp. Reading text is Inter by default, with a 24 dp line height (about 18 sp text). Marketing pages use Waldenburg for display type and Inter for body text (from the woff2 files in the page HTML).
- Shape: pills everywhere (filter chips 52 dp high, CTA buttons 55 dp). Round icon buttons are 44 dp. Sheets have a top radius of about 30 dp. Covers have a small radius of about 4 dp.
- Navigation: a 5-tab bottom bar (Home, Explore, Import, Library, Voices), 64 dp high, with the selected item marked by a grey pill behind the icon. A floating mini-player sits 12 dp in from the screen edges, 56 dp high, radius about 16 dp, with a soft shadow.
- Motion: not measured frame by frame. Seen: sheets slide up, the player controls auto-hide ("Auto Fullscreen"), and the word highlight moves word by word. Status: assumed from use; no timings recorded.

Home
- Layout: large title on the left; search and a combined "Try Ultra" plus avatar pill on the right. Below: "Continue listening" as a horizontal carousel (64x96 dp cover, title, two-line blurb, a progress ring with "2 % · 10 hrs left"), a "Bring your own" chip row (Write text, Paste a link, Upload a file), then store rails: What's trending, Top picks, a 2x3 category grid, What's new.
- Works: Continue listening comes first; import chips are one tap away; the progress copy is in hours left, not percent alone.
- Weak: four of the five rails sell catalogue titles. The upsell sits in the header. Scrolled content runs under the status bar with no scrim, so text collides with the clock (verified on Home and Library).

Explore (store)
- Filter chips (Tags, Genre, Price, Rating, Narrated by), then rails with preview buttons on the covers. Not relevant to nolabs beyond the chip pattern.

Import sheet (centre tab)
- Bottom sheet: title "Import content", subtitle, close X (no drag handle). Rows have a 44 dp grey circle icon and a 17 sp label: Paste a link, Upload a file, Scan text, Write text. A second group, "Popular websites", lists Kindle, Archive of Our Own and More.
- Paste link: a full screen with one outlined field (placeholder "https://") and a disabled Next pill. The keyboard opens at once. The clipboard was not offered as a suggestion (assumed: the clipboard may have been empty).
- Works: one sheet holds every way in. Weak: Paste link is a full page for one field, and there is no share-sheet hint.

Library
- Large title, search, a more menu, and segment pills (Saved selected in #202020 with white text; Collections; Archive). Rows are 212 dp high: a 120x180 dp cover, author (grey), title (17 sp), a two-line description, and progress ("2 % • 10 hrs left") or a green check when finished. Hairline dividers.
- Collections: Read later (count), Create a personal collection, Downloaded, plus a "By type" group (Books, Imports).
- Archive: imported files without a cover get a blank peach gradient tile and "File Import" as the author.
- Weak (verified): raw HTML shows in descriptions on 2 items (`<div><p>…`, `<p class="description">`). A failed import shows only "Error importing document" in red, with no reason and no retry, and duplicate failed rows stay. File names are used as titles unchanged. Rows are very tall, so only about 3.5 fit on screen.

Reader + player (the best screen)
- Header: a collapse chevron or back button, title and author (2 lines), More (three dots).
- Text: about 18 sp Inter at a 24 dp line height, with a 20 dp side gutter. The current paragraph gets a pale wash (#DDF5FF, the "Sky" preset) whose shape follows the ragged last line. The current word gets a stronger rounded pill (#73DBF2). Highlight presets: Sky, Fall (peach and orange), Highlight (grey and yellow), Classic (green).
- "Centre highlighted content": a black pill button with a follow icon appears above the dock when you scroll away from the playing paragraph.
- Dock (bottom 600 px, about 234 dp): a chapter chip (list icon and chapter title, which opens Contents), a scrubber (6 dp track, #202020 fill on #E0E0E0), a times row (elapsed | "4h left" in the book, centred | chapter remaining), and a transport row (sleep, −15, a 56 dp play, +30, speed shown as "1x"). The secondary row has Soundscapes, a voice chip (orb plus name, in a pill) and Bookmark.
- Interaction: a single tap on the text shows a toast that covers the top of the text: "Double tap to play from here, or change behavior in Player Preferences". Controls auto-hide during playback (immersive), and a tap brings them back.
- More sheet: book header, two tiles (Contents, Sleep timer), then rows: Hide text (audio only), Preferences, Voices, Soundscapes, Pronunciations | Contents (a duplicate of the tile), Queue, Bookmarks, Search, Rate this title. A "Get ULTRA" upsell card sits at the bottom.
- Contents: chapters with durations and a download button for each. The current chapter shows a play glyph and a progress ring.
- Bookmarks empty state: the title, then centred "No bookmarks found" on an otherwise blank sheet.

Preferences (Player Preferences)
- Font: Libre Baskerville, OpenDyslexic3, Inter (default). Each is previewed with a pangram in its own face.
- Font size slider; background swatches System, Light, Paper (cream), Dark.
- Highlight colour presets, previewed as mini text cards.
- Playback controls: forward and back intervals (30 s / 15 s), Autoplay on open.
- Device controls: Auto Fullscreen; Double tap to play; mapping for the headset and car Next, Previous, Forward and Back buttons (default skip 30 s / 15 s).
- Reading mode: Reading Sync (keep reading while paused), Text display (Continuous scroll or pages), page-turn tap zones, swipe to turn, page-turn animation.
- Works: deep and well grouped. Every subpage has a Reset to default.

Speed sheet
- "Reading speed: 1x" title, a slider from 0.25x past 3x (the pricing page says up to 4.0x), five 56 dp circular presets (0.5, 0.75, 1, 1.5, 2; the selected one has a 2 dp black ring), and a full-width black "Save settings" button.
- Weak: the change does not apply until Save is pressed.

Sleep timer sheet
- Title "Fall asleep…". A list: When the current chapter ends, 2 h, 1 h 30, 1 h, 45, 30, 15, 10, 5 min, Off. Then "Save settings".

Voice picker (from the player) and the Voices tab
- Header: "Voice", search, filter, close. Segment pills: Explore, Recents, Favorites, Created.
- An Expressiveness slider (Subtle / Balanced / Expressive) with waveform glyphs at each end and an info button.
- "Recommended for this read": wide colour cards (a celebrity photo or an orb, name, "Popular in English", heart).
- List rows (90 dp): a 56 dp orb avatar (an abstract blurred gradient sphere, unique per voice) that doubles as the preview button, name (17 sp, semibold), descriptor (traits • category), and a heart.
- Footer: Reset and Save pills, side by side.
- Voices tab: Trending voices (a two-column horizontal pager), "Design your own narrator" (a black + circle, "Create new voice"), and Voice Collections cards.
- Voice Design: a full-screen text prompt ("Start typing qualities…"), example chips with orbs ("Bold Stage Woman"), an info button and "Create voice".
- Works: the orbs give every voice an identity without faces. The preview sits on the avatar. The expressiveness control is easy to understand. Weak: the choice needs Save; previews use a fixed sample rather than the document's text (assumed, not played).

Account
- Avatar, a FREE badge, name, email, then "Hours remaining" as two monospaced digit tiles ("0 9") with the refresh date, an Ultra card with five blurred pastel gradient swatches, Get more hours, Refer & earn, then a settings list (Referrals, Account & support, Content preferences, Usage, Terms, Privacy, Licenses) and a danger zone (Delete account, Reset password, Sign out).
- Works: the digit tiles are playful. Weak: the account page mostly sells.

Search
- A pill search field with the keyboard opening at once. Below it: store recommendations (Recent bestsellers, Recommended collections). Search covers the catalogue, not your own imports (assumed from the suggestions shown; no query was typed).

Marketing site (elevenreader.io; verified from HTML and a headless render)
- Promo bar ("Your first month. Just $1."), then a nav: wordmark, Explore ▾, Pricing, For Authors, Upload & Listen | Sign in, a black "Start listening free" pill.
- Hero: a fanned 3D carousel of book covers; H1 "Listen to anything in voices you'll love"; a one-line sub; CTAs "Start listening free" (black) and "Upload & listen" (outline); a trust row with laurels (4.7 stars, App of the Day, 5M+ installs).
- Next: two large cards, an upload drop zone ("Drop a file or click to upload", PDF/DOCX/EPUB/MD/TXT, "Paste a link here…", voice picker plus "Listen now") and an audiobook stack with a play button and voice selector. Then "Watch the film" (full-width video), a premium book rail, trending genres (fanned covers and genre chips), "Designed for how you listen" (a feature card carousel: phone UI, voice list, a coral prompt card, soundscape), ratings, "Available everywhere" (Listen Online, Get Android App), pricing (Free $0: 10 h/month, up to 4.0x, sleep timer, bookmarks; Ultra $11/mo or $8.25/mo yearly: unlimited imports capped at 24 h/day, 200K titles, offline, custom voices), 10 FAQs, and a large SEO footer (comparison pages against Speechify, NaturalReader, Audible and others, plus a Chrome extension link).
- Section pattern: a small icon chip and grey eyebrow label, then a 32 to 40 px title. Generous white space.
- SEO: title "Free Text Read Aloud App | ElevenReader"; description, OG and Twitter cards at 1200x630, canonical, light and dark favicons, a manifest, and JSON-LD for SoftwareApplication, AggregateRating, FAQPage, Organization and WebSite. theme-color #FDFCFC.

What ElevenReader does well (to keep)
1. The paragraph wash plus word pill highlight, with the wash shaped to the text.
2. The re-centre follow button that appears only when you scroll away.
3. The chapter chip above the scrubber, and book-level "time left" in the times row.
4. Abstract gradient orbs as voice avatars, with the avatar as the preview button.
5. Highlight colour presets with live mini previews; a dyslexia-friendly font option.
6. Headset button mapping and skip intervals that you can change.
7. One import sheet; consistent sheet anatomy (title, close, rows with circle icons).
8. A monochrome chrome that lets the content carry the colour.

What is weak (to beat)
1. Store-first: the Home rails, the Explore tab and the Voices tab all sell.
2. Upsells in the header, the More sheet and the Account page; an hours meter.
3. Raw HTML in descriptions; raw file names as titles; dead-end import errors.
4. No status-bar scrim on scroll.
5. A mini-player with no progress or buffering state.
6. Speed, sleep and voice changes need "Save".
7. A hidden double-tap gesture taught by a toast that covers the text.
8. Contents listed twice in the More sheet.
9. Bare empty states.
10. Search ignores your own documents (assumed).
11. No desktop app (web and mobile only, per the "Available everywhere" section).

### (b) Feature parity against the nolabs plan (notes 06 v2 and 11)

"Plan" means the feature is already in 06 v2 or 11. "Gap" means ElevenReader has it and we do not plan it yet. Each gap gets a recommendation.

| Area | ElevenReader | nolabs plan | Status / action |
|---|---|---|---|
| Import: paste text, URL, file (EPUB, PDF, TXT, MD, HTML, DOCX) | yes | yes except DOCX | Gap: DOCX. Add `from_docx` (zip and quick-xml, the same approach as EPUB) to `crates/import` |
| Share into the app / open files | Android share | share target, file associations (11) | plan |
| Scan text (camera OCR) | yes | no | Gap, phase 2: Android ML Kit text recognition in the native plugin; no web version |
| Kindle / AO3 connectors | yes | no | Skip (needs their sign-in; out of scope) |
| Continue listening | carousel | "Continue" card | plan; spec below |
| Library filters: saved, archive, collections, downloaded, by type | yes | not specified | Gap: add Archive (swipe), Finished, Downloaded filter; collections in phase 2 |
| Library search | catalogue only | no | Gap: search titles and full text locally (IndexedDB or SQLite index) |
| Search inside a document | yes | no | Gap: add to the reader More sheet |
| Word highlight + sentence wash | yes | Kokoro word timings (09 Track.words); Ming in phase 2 | plan; Ming falls back to sentence-only highlight |
| Re-centre follow button | yes | `reading/follow.rs` exists | plan; spec below |
| Tap to play from a sentence | double tap | tap sentence | plan; use tap → "Play from here" pill (spec below) |
| Speed | 0.25 to 4x | 0.5 to 3x | Gap (small): widen to 0.5 to 3.5x, 0.05 steps |
| Skip | −15 / +30, configurable | ±15 | Gap: make both intervals configurable (5, 10, 15, 30, 60) |
| Paragraph or chapter prev/next | via Contents | paragraph prev/next | plan |
| Sleep timer | 5 min to 2 h, end of chapter | 15/30/45/60, end of chapter, 10 s fade | Gap (small): add 5, 10, 90, 120 min; we already beat them on the fade |
| Contents / chapters with durations | yes | not specified | Gap: chapter list from EPUB nav or headings; durations estimated from characters and measured speed |
| Queue | yes | no | Gap: "Play next" / "Add to queue" on library rows |
| Bookmarks | yes | phase 2 (with notes) | Move a plain bookmark (no note) into milestone B |
| Hide text (audio only) | yes | no | Gap (cheap): a cover-art player view |
| Fonts | 3 including OpenDyslexic | Literata + Inter | Gap: add Atkinson Hyperlegible and OpenDyslexic as reading options |
| Background themes | System, Light, Paper, Dark | System, Paper, Ink | plan; add an OLED "Night" (#000) option for Ink |
| Highlight colour presets | 4 | one accent | Gap (cheap): 4 presets: Violet (default), Ember, Mint, Sky |
| Headset / car button mapping | yes | Media Session actions | Gap: a settings row mapping next/prev to skip or paragraph |
| Autoplay on open | setting | not specified | Gap (cheap) |
| Pages vs continuous scroll | both | continuous | Skip (continuous only) |
| Pronunciations | yes | phase 2 dictionary | plan (phase 2) |
| Soundscapes | yes | no | Skip; possible later |
| Voice catalogue, preview, favourites, recents | yes | curated 9 + More, previews in R2 | Gap: favourites and recents |
| Expressiveness control | slider | Ming emotion / tags | Partial: show "Expressive" as a voice variant toggle, not a slider (GPU budget) |
| Voice design from a prompt | yes | v1 had Ming voice design; not in v2 milestones | Gap, phase 2 (one GPU run per design) |
| Own cloned voice | Ultra only | milestone B, free | plan; a headline advantage |
| Offline | Ultra, per chapter | rolling cache; native offline downloads (11) | plan; add an explicit "Download" action in native apps |
| Sync across devices | web and mobile | Supabase realtime (11) | plan |
| Lock screen, notification, car | yes | Media Session / Media3 (11) | plan |
| Languages | 32 | Kokoro 9, Ming 7, no Arabic | Gap, known (note 11 item 6) |
| Desktop apps | none | Tauri macOS / Windows / Linux | nolabs advantage |
| Export audio | no (FAQ topic; answer assumed no) | not planned | Opportunity, phase 2: export a chapter as WebM/M4A |
| Usage meter / paywall | 10 h/month free | none for Kokoro; GPU minutes for Ming | Show only the expressive budget, as a small meter in the voice sheet |

### (c) Design brief for nolabs

#### Brand direction
- Idea: "the page that speaks". Dark, literary, quiet, with one living element: the word being spoken. The chrome stays monochrome, as ElevenReader's does, but on ink instead of white. Two signature colours carry the identity: Violet is the voice (accent, play, selection) and Ember is the spoken word (the live highlight cursor, recording, and the hero demo).
- Voice: plain and warm. Short verbs ("Paste a link", "Listen", "Pick a voice"). No "AI" in headlines. Numbers instead of hype ("Free. No hours limit on standard voices.").
- The name is pending, so every mark below works without letters, and the wordmark is set in Inter Display Semibold at −2 % tracking, in lower case.

Logo concepts (name-independent; 64-unit grid; strokes scale to 1.5 px at a 16 px favicon)
1. Speaking cursor (recommended). A text I-beam cursor whose vertical stem is a 5-point waveform. Serifs: two horizontal bars, 20 units wide and 4 thick, at y=10 and y=54. The stem runs from y=14 to y=50 as a polyline through x=32±0, ±6, ±10, ±6, ±0 (a single smooth sine period), with a 4-unit stroke and round caps. App icon: a squircle (radius 18) in Ink #0B0B0F, with the cursor in Violet #7C6CFF and the middle wave peak dotted in Ember #FFB45C (a 6-unit circle). Why it works: it is the reading caret that sounds, it animates naturally (the wave breathes while audio plays), and it becomes the hero's highlight cursor.
2. Open page, three bars. One open book page seen from above as two curved leaves (paths like the current `logo.rs`, simplified to one stroke of 4 units), with the right leaf ending in three vertical bars 4 units wide at heights 10, 18, 12, spaced 4 apart, as if the page turns into sound. White on a Violet squircle. This is the evolution of the current mark, and the safest choice.
3. Quote that speaks. Two quotation marks built from filled 14-unit circles with tapered tails. The left one is solid Violet. The right one is drawn as two concentric arcs (sound waves, 4-unit stroke, 90° sweep) in Ember. It reads as "quote" and "sound" at once. At 16 px it reduces to a dot and one arc.
- For every concept: monochrome versions in Ink and Paper text colours; a minimum size of 16 px; clear space of 25 % of the mark's height.

#### Colour tokens

Contrast was computed with the WCAG 2.1 formula (Python, sRGB; ratios shown against bg / surface / surface-2 / surface-3). Changes from `crates/web/style/_tokens.scss` are marked "changed".

Ink (dark, default)
| Token | Value | Use | Contrast |
|---|---|---|---|
| --bg | #0B0B0F | app background | |
| --surface | #15151B | cards, dock | |
| --surface-2 | #1C1C24 | raised cards, sheets | |
| --surface-3 | #252530 | toasts, hovered rows | |
| --text | #EEEEF3 | primary text | 17.0 / 15.7 / 14.6 / 13.1 |
| --text-2 | #A3A3B2 | secondary text | 7.9 / 7.3 / 6.8 / 6.1 |
| --text-3 | #86869E (changed from #74748A, which was 4.31 / 3.99 / 3.71) | captions, timestamps; not on surface-3 | 5.5 / 5.1 / 4.8 / 4.3 |
| --accent | #7C6CFF | fills, icons, focus rings (UI elements need 3:1) | 5.1 / 4.7 / 4.4 / 3.9 |
| --accent-text | #9387FF (new) | accent-coloured text and links on any surface | 6.7 / 6.2 / 5.8 / 5.2 |
| --accent-strong | #6655F0 | filled buttons with white labels | white 5.1 |
| --ember | #FFB45C (new) | live word cursor, record state, brand spark | 11.2 on bg; Ink text on Ember 11.2 |
| --word | rgba(124,108,255,0.30), which is #2D2857 on bg (new) | current word pill | text 11.7 |
| --sentence | rgba(124,108,255,0.10), which is #161527 on bg | current sentence or paragraph wash | text 15.5, text-2 7.2 |
| --success / --warning / --danger | #3DD6A6 / #F5B544 / #FF6B7A | status | ≥ 5.5 on all surfaces |
| Night (OLED option) | --bg #000000, --surface #0E0E12 | true black reading | |

Paper (light)
| Token | Value | Use | Contrast |
|---|---|---|---|
| --bg | #F6F2EA | warm paper | |
| --surface | #FFFDF8 | cards | |
| --surface-2 | #EFE9DE | chips, sheets | |
| --surface-3 | #E6DFD2 | pressed rows | |
| --text | #1C1A16 | | 15.6 / 17.1 / 14.4 / 13.1 |
| --text-2 | #5F5A51 | | 6.1 / 6.7 / 5.7 / 5.2 |
| --text-3 | #67614F (changed from #8A8478, which was 3.33) | | 5.5 / 6.1 / 5.1 / 4.7 |
| --accent | #5A48E0 | fills, icons; white on it 6.1 | 5.5 / 6.0 / 5.1 / 4.6 |
| --accent-text | #4B3BD1 (new) | links and accent text | 6.6 / 7.3 / 6.1 / 5.6 |
| --ember | #9A4D00 for text, #FFB45C at 0.35 for the word fill (new) | | 5.5 / 6.0 / 5.1 / 4.6 |
| --word | rgba(90,72,224,0.22), which is #D4CDE8 on bg | | text 11.3 |
| --success | #0C6E52 (changed from #138A69, which was 3.86) | | 5.6 / 6.1 / 5.2 / 4.7 |
| --warning | #8F4A06 (changed from #A86A00) | | 6.0 / 6.6 / 5.5 / 5.0 |
| --danger | #B42336 (changed from #C2283B, which was 4.33 on surface-3) | | 5.8 / 6.4 / 5.4 / 4.9 |

Highlight presets (the word pill and sentence wash pair): Violet (default, as above); Ember (word #FFB45C at 0.32, wash at 0.08); Mint (#3DD6A6 at 0.28 / 0.08); Sky (#5EC8F2 at 0.28 / 0.08). Text stays --text in every preset. Check: every word fill is over 10:1 with text on bg (the same alpha range as the computed values above; assumed for Mint and Sky, so verify when implemented).

Voice orbs: each voice gets a 3-stop radial gradient generated from its voice fingerprint (hue = fingerprint mod 360, two neighbours at ±40°, OKLCH L 0.62 to 0.78, C 0.12), with a soft inner highlight at the top left. This gives the ElevenReader orb idea, but deterministic and in our palette.

#### Type scale
- UI: Inter variable, with `font-feature-settings: "cv11", "ss01"` and tabular numbers (`tnum`) for every time and percentage.

| Role | Size / line | Weight | Tracking |
|---|---|---|---|
| Display (landing) | clamp(44px, 7vw, 76px) / 1.02 | 650 | −0.03em |
| H1 page title | 28 / 34 | 650 | −0.02em |
| H2 section | 20 / 26 | 600 | −0.01em |
| Title (rows, sheet titles) | 17 / 22 | 600 | 0 |
| Body | 15 / 22 | 450 | 0 |
| Label (buttons, chips) | 14 / 20 | 550 | 0 |
| Caption / meta | 12.5 / 16 | 500 | +0.01em |
| Overline (eyebrows) | 11 / 14 | 600 | +0.08em, upper case |

- Page titles are deliberately smaller than ElevenReader's (about 32 sp) so the content starts higher. On scroll, the title collapses into a 56 px blurred top bar.
- Reading: Literata variable (opsz axis on), default 19 px / 1.65, sizes 16 to 28 px in 1 px steps, measure 64ch maximum (`--column: 760px` is already about right at 19 px), paragraph gap 0.85em, `hanging-punctuation: first`, hyphenation on for narrow columns. Alternatives in Settings: Inter (18 / 1.6), Atkinson Hyperlegible (19 / 1.65), OpenDyslexic (18 / 1.8).

#### Spacing, radius, elevation
- Spacing (4-based): 2, 4, 8, 12, 16, 20, 24, 32, 40, 56, 72, 96. Gutter 16 px under 400 px wide and 20 px above (already in the tokens); landing sections are separated by 96 / 72 px (desktop / mobile).
- Radius: --r-xs 6 (tags, word pill), --r-s 10 (inputs, small cards), --r-m 14 (rows, toasts), --r-l 18 (cards, mini-player), --r-xl 24 (sheets, hero card), --r-pill 999. App icon squircle: 28 % of the side.
- Elevation: Ink uses surface steps and 1 px lines (--line), never shadows, except the mini-player (--shadow-2). Paper uses --shadow-1 for cards and --shadow-2 for floating elements.
- Tap targets are at least 44 px (48 px for transport controls); the play button is 64 px.

#### Component specs
- Library row: 96 px high. A 48x72 cover (radius 6, --cover-ring 1 px inset). Title 17/22, 600, 2 lines. Meta 12.5/16 in --text-3: `source · 3 h 12 m left · voice name`. A 3 px progress track under the meta line (--surface-3 track, --accent fill), hidden at 0 %. Trailing: a 40 px round play button that turns into a pause icon with the progress ring when that row is playing.
  - States: Importing (a shimmer on the title line and "Reading the file…"); Preparing audio ("Getting 3 minutes ready" with an indeterminate 3 px bar); Failed (the meta line in --danger with the reason in plain words, e.g. "This EPUB has no readable text", plus an inline "Retry" text button); Finished (a check glyph and "Finished", in a muted row).
  - Swipe left to Archive, with a toast offering Undo. A long press or right click opens a menu: Play next, Add to queue, Rename, Download (native), Archive, Delete.
  - Generated cover when none exists: a 2-stop linear gradient at 160° in two hues derived from a title hash, the first letter in Literata 28 px at 70 % white, and a 14 px source glyph (link / PDF / EPUB / text) at the bottom right. Never a blank tile.
- Continue-listening hero: a full-width card, radius 24, --surface-2, padding 16. Cover 72x108 on the left. Title 20/26. Line 2: "Chapter 4 · 23 min left in chapter". A 4 px progress bar for the whole document. A 56 px Violet play button at the bottom right. The background is a radial wash of the cover's dominant colour at 18 % from the top left. With 2 or more in-progress documents it becomes a snap carousel that shows 24 px of the next card.
- Reader: text column as above. Sentence wash: the --sentence fill on the current sentence (ElevenReader washes the paragraph; a sentence matches our segmenting and is easier to follow), drawn per line box with radius 6 and joined into one shape (CSS `box-decoration-break: clone` on an inline span). Current word: an --word pill with radius 4 and a 2 px Ember underline 1 px below the baseline (the "speaking cursor"). Follow: auto-scroll keeps the current line in the band 30 to 45 % from the top. A manual scroll pauses follow, and a "Back to voice" pill (40 px, --surface-3, follow icon) appears above the dock, bottom right.
- Tap to play: a single tap on a sentence selects it (a 1 px outline) and shows a floating "▶ Play from here" pill above that sentence for 4 s. A double tap plays at once (for power users). No instructional toast.
- Player dock (in the reader): --glass background with `backdrop-filter: blur(20px) saturate(140%)` and a 1 px --line top border; padding 12 / 16; height 232 px (as now).
  - Row 1: chapter chip (list icon and chapter title, 13 px, opens Contents).
  - Row 2: scrubber, a 4 px track that grows to 6 px with a 16 px thumb while dragging; buffered-ahead audio shown at 35 % accent; a 44 px tall hit area; the time bubble shows while dragging.
  - Row 3: times in 12.5 px tabular numbers: elapsed | "1 h 12 m left" (whole document) | chapter remaining.
  - Row 4: sleep, back-skip, 64 px play (Violet filled; the icon morphs between play and pause), forward-skip, speed ("1.25×" as text).
  - Row 5: voice chip (20 px orb and name, pill, 36 px), bookmark, more.
  - While playing, the dock collapses after 4 s without touch to a 72 px strip (scrubber and play only) and expands on tap or scroll up. Keyboard on desktop: Space play/pause, J/L −/+ skip, K play/pause, [ ] speed −/+ 0.1, ↑/↓ previous/next paragraph, B bookmark, F follow.
- Mini-player: floating, 64 px high, 8 px above the tab bar, 12 px side insets, radius 18, --surface-2 with --shadow-2. A 40 px cover (radius 6), title (15/20, 600) and a meta line ("Chapter 4 · 23 m left"), a 40 px back-skip and a 44 px play. A 2 px progress line along the bottom edge, inset by the radius, with a lighter buffered segment. Swipe up or tap opens the reader with a 280 ms shared-element transition of the cover. Swipe down stops playback, with Undo.
- Sheets: top radius 24; a 36x4 drag handle (--line-strong) 8 px from the top; a 56 px header (title 17/22, 600, and a 40 px close button); maximum height 92 vh; snap points at 50 % and 92 %; --backdrop scrim. On wide screens (720 px or more) the voice picker and settings open as a 400 px right side panel and other sheets as a centred 480 px dialog. Settings apply live (speed, sleep, voice, highlight), and only destructive changes ask for confirmation. No Save buttons.
- Speed sheet: the current value large (32 px tabular), a slider from 0.5 to 3.5 in 0.05 steps with haptic ticks at 0.25 steps, and presets as 44 px pills: 0.8, 1, 1.2, 1.5, 2, 2.5.
- Sleep sheet: pills for 5, 10, 15, 30, 45, 60, 90 min; "End of chapter"; "End of document"; Off. While active, the dock's sleep icon shows the minutes left (e.g. "23") and the last 10 s fade out.
- Voice card with preview: a list row 72 px high: 44 px orb, name (15/20, 600), descriptor (12.5 px --text-3, e.g. "Warm · British · Narration"), an engine badge ("Standard" in --surface-3, or "Expressive" in an Ember outline with a GPU-minutes hint), and a heart.
  - Preview: tapping the orb plays a sentence taken from the open document (cached through the normal render path; fallback: a fixed line). While playing, the orb's ring pulses with the live amplitude (AnalyserNode RMS mapped to a 0 to 4 px ring) and a progress arc draws around it.
  - Selected: a 2 px Violet ring and a check, applied at once from the current sentence without restarting the document.
  - Sections: Your voices (clones first), Favourites, Recommended for this text (language match), All.
- Recording screen (clone): full screen with a progress header ("Line 3 of 8" and a thin 8-segment bar). The script line is centred in Literata 24/34, the next line at 40 % opacity and the previous at 25 %. The 80 px record button turns Ember while recording, with a ring scaled by RMS. A level meter of 24 horizontal bars shows a green band at −18 to −6 dBFS, amber above −6 and red at clipping (−1 dBFS or more). A room check before the first line ("Room is quiet" / "Background noise: try a quieter room"). After each line: chips for Level, Clipping and Noise, plus Retake. The final step shows a "Build my voice" button, then a preview that plays the user's voice reading the first lines of their current document.
- Toasts: bottom centre, 8 px above the mini-player or dock, 48 px minimum height, radius 14, --surface-3, 15 px text, at most one action (Undo, Retry, View). 4 s, or 6 s with an action. Swipe to dismiss. One visible at a time, the rest queued. `role="status"`, aria-live polite (assertive for errors).
- Empty states: always an icon or illustration (the logo cursor at 64 px in --text-3 with the Ember dot), a 17 px headline, a one-line explanation and at most two actions.
  - Library: "Nothing to listen to yet", with three 72 px action tiles (Paste a link, Open a file, Paste text) and "Tip: share any page from your browser to {name}".
  - Bookmarks: "Press the bookmark button while listening to save a spot."
  - Search: "No matches in your library" plus "Search inside documents" as a fallback.
  - Offline: a slim banner, "Offline · playing downloaded audio".

#### Motion rules
- Durations: 120 ms press feedback; 180 ms standard (fades, colour); 240 ms sheet enter and 200 ms exit; 280 to 320 ms the cover shared-element route change. Easing: enter `cubic-bezier(0.2, 0.8, 0.2, 1)` (current --ease); exit `cubic-bezier(0.4, 0, 1, 1)`.
- Word highlight: the pill glides between words with a transform and width transition of 90 ms ease-out. Across a line break it fades out and in over 90 ms instead of sliding diagonally. The sentence wash crossfades in 160 ms.
- Auto-scroll: smooth over 400 ms when the jump is less than 1.5 viewports, otherwise instant. Never scroll while the user's finger is down.
- Play/pause icon morph 160 ms. The scrubber thumb scales from 0 to 1 in 120 ms on drag. Pulses are allowed only for the record ring and the playing cursor in the logo.
- Sheets track the finger 1:1. A flick faster than 0.5 px/ms, or a drag past 40 % of the height, dismisses.
- `prefers-reduced-motion`: no glide (instant word swaps), no pulses, crossfades capped at 120 ms, no parallax on the landing page.

#### Landing page
Order and copy (placeholders in braces):
1. Nav (sticky, 64 px, blurred Ink): mark and wordmark | How it works, Voices, Download, GitHub (star count) | "Open app" pill (Violet). No promo bar.
2. Hero (one viewport, centred):
   - Eyebrow: "Free · Open source · No account to try".
   - H1: "Hear anything you'd read." Alternatives: "Your reading list, read to you." / "Press play on any page."
   - Sub: "Paste a link or drop a book. A natural voice reads it to you and lights up every word as it's spoken. Free, with no hours limit."
   - Primary CTA: auto-detected "Download for macOS" / "for Windows" / "for Linux" / "Get the Android app" (from `navigator.userAgentData.platform`, falling back to the UA string), with "v{x.y} · 38 MB · updates itself" underneath. Secondary: "Open in browser".
   - Live demo card (720 px wide, radius 24, --surface): two short paragraphs in Literata 22 px; a play button; voice chips (3 voices with orbs); a speed toggle (1× / 1.5×).
   - The audio is a pre-rendered static WebM per voice with a word-timing JSON (no backend call, so it works when the Space sleeps). The words highlight live with the same Violet pill and Ember underline as the app, so the demo is the product. The text is static and authored by us.
   - Visual signature: behind the card, a large faint speaking-cursor waveform that breathes with the audio amplitude.
3. How it works (three steps, each with a short looping UI clip): "Add anything: links, PDFs, EPUBs, notes, or share from any app" → "Pick a voice: nine natural voices, or your own" → "Listen anywhere: screen off, lock screen, headphones, car".
4. Voices: a 3x2 grid of voice cards with preview (the same component as the app), plus a wide card: "Your voice, reading your books. Read eight short lines. That's it." with "Clone your voice" (it notes that expressive voices have a daily limit).
5. Reading features strip (icon and one line each): word-by-word highlight · speed up to 3.5× · sleep timer that fades out · offline listening · picks up where you left off on any device.
6. Free, private, open (three columns):
   - "Free, for real.": no hours meter on standard voices, no card, no trial.
   - "Private by design.": "Your documents live in your account. No ads, no trackers, no selling data."
   - "Open source.": "Every line is on GitHub. Fork it, host it, audit it." State only claims that are true at launch (for example, whether there is any analytics).
7. Download (anchor #download): the detected platform as a large card at the top, then a grid of macOS (Universal .dmg), Windows (.msi), Linux (AppImage / .deb), Android (.apk) and Web. Each shows the version, size, minimum OS and "Updates itself". A link to checksums and the changelog.
8. FAQ (accordion, at most 9): Is it really free? Which voices are there? Can it use my own voice? Does it play with the screen off? Does it work offline? Which files can I import? Where is my data stored? Which languages? Why no iPhone app yet?
9. Footer: mark, one-line tagline, GitHub, Changelog, Status, Privacy, Terms, and credits ("Voices by Kokoro-82M (Apache 2.0) and Ming-3.0") © {year}.
- SEO: title "{Name}: free read-aloud app with natural voices" (60 characters or fewer); a meta description of 150 characters or fewer; canonical; OG/Twitter image 1200x630 showing a highlighted sentence on Ink; theme-color #0B0B0F (dark) and #F6F2EA (light) via media; light and dark favicons; JSON-LD SoftwareApplication (offers price 0, operatingSystem "macOS, Windows, Linux, Android, Web", applicationCategory "MultimediaApplication") and FAQPage; a sitemap; `lang="en"`.
- Performance: the landing page is static HTML/CSS with one small JS module for the demo and OS detection, demo audio under 200 KB per voice, fonts subset to Latin with `font-display: swap`, LCP under 1.5 s on 4G (assumed target).

#### Ten improvements over ElevenReader
1. Home is your library: the Continue hero, then your documents and the import actions. No store, no upsell in the header.
2. No hours meter for standard voices; the only meter is the expressive (GPU) budget, shown inside the voice sheet when relevant.
3. Settings apply live, with Undo; no "Save settings" on speed, sleep or voice.
4. Tap a sentence for a "Play from here" pill instead of a hidden double tap taught by a toast that covers the text.
5. Honest imports: clean titles from metadata, generated covers, plain-language errors with Retry, and HTML always stripped from descriptions.
6. The mini-player shows progress and buffered audio; the dock shows buffered-ahead audio on the scrubber.
7. Voice previews read your own document, and switching voice continues from the current sentence.
8. Your own cloned voice for free, with a guided recording screen that checks level, clipping and noise.
9. Native desktop apps with keyboard shortcuts and system media keys, plus the same library synced to Android and the web.
10. Ink theme by default for night listening, with an OLED Night option, a status-bar scrim and a blurred collapsing header, and real empty states.

### Open items
- See the logged-in ElevenReader web reader (layout of the desktop player, keyboard shortcuts). Unblocks when the Claude in Chrome extension is connected in the owner's Chrome.
- Choose a logo concept (1, 2 or 3). The owner decides, ideally with the final name.
- Measure ElevenReader's highlight timing and transitions frame by frame (screen recording over `adb shell screenrecord`) if exact motion matching matters. Not needed for the build.
- Verify the Mint and Sky highlight contrast and the voice-orb OKLCH values in the browser when implemented.
- Confirm the voice-preview cost: a document sentence preview needs one Kokoro render per voice (CPU, cheap). For expressive voices, use the stored preview to protect the GPU budget.
- Fix the existing tokens in `crates/web/style/_tokens.scss` (text-3 in both themes, Paper success/warning/danger, new accent-text, ember and word tokens). This is a code task for the web agent.
