# Landing page v2 brief (Streams Reader)

Plan + finding. This note covers the ElevenReader landing page section by section, 12 Dribbble references, the licences of the public-domain libraries, the hero video plan and the "Available everywhere" section. It ends with a spec for v2.
Conclusion: match ElevenReader's structure but cut the selling. The v2 page is hero video → classics row → four story chapters (one of them the live demo) → voices → small details → Available everywhere → promise → FAQ → footer. The book row and the in-app Explore shelves use Standard Ebooks (CC0 covers and text). The hero video is a real screen recording of the app (CDP screencast + ffmpeg, AV1 + H.264, ≤ 1.5 MB). Builds on note 12 (benchmark, landing v1 brief), note 14 (phase 3) and D13–D15 (name, Streams blue #2F6BFF, ember only for the spoken word, central tokens, Remix Icon, own icons and spots).

## 2026-10-30

### How this was gathered
- elevenreader.io: a fresh headless render in Chrome Beta 1440x900 and 390x844 (`--headless=new`, throwaway profile), a dumped DOM (1.18 MB) parsed for headings, CTAs, images and links in document order, the saved HTML from note 12, and one live check in the owner's Chrome (Claude in Chrome, a new tab, read only; cookie banner answered "Reject all"). Status: verified.
- Dribbble: the owner's Chrome, connected on the first try. I fetched 14 search pages (audiobook app landing page, reading app landing, text to speech landing page, ebook reader website, ai voice app landing, audiobook website, book app website, reader app landing page, book carousel, audiobook dark, download app section, word highlight, speechify, reading app dark) and read each shot page's og:image. Previews (800x600) are in `notes/raw/design-private/dribbble/` (gitignored, third-party, never published). No likes, follows, saves or comments. Status: verified.
- Licences: the publishers' own pages, the Standard Ebooks feeds checked with curl, and the GitHub API for the Standard Ebooks org. Links are given where each fact is used.
- Screenshots saved this session: `notes/raw/design-private/web/v2-er-desktop-fold.png`, `v2-er-mobile-fold.png` (plus note 12's `landing-desktop*.png`), `dribbble/*.png|jpg`, and `se-covers/*.jpg` (20 Standard Ebooks thumbnails, CC0, kept private only to review the row).
- The live site `listen.streams.watch` currently shows only the passphrase gate (H1 "nolabs", "Unlock"). No landing page is deployed yet. The v1 landing exists in `crates/site` (copy in `crates/site/locales/en/site.ftl`, demo audio in `crates/site/assets/demo/`). Status: verified.

### Corrections (wrong beliefs found today)
- Belief (in the task brief): the Standard Ebooks OPDS feed at `standardebooks.org/feeds/opds` is open. Correction: it returns HTTP 401 (curl, 2026-10-30). So do `/feeds/opds/new-releases` and `/feeds/opds/all`. Only the Atom/RSS **new releases** feed is open (200, 15 entries, 68 KB). Full feeds and bulk downloads need Patrons Circle membership: $15/month or $150/year or more ([feeds](https://standardebooks.org/feeds), [donate](https://standardebooks.org/donate), [bulk downloads](https://standardebooks.org/bulk-downloads)). Status: verified.
- Belief: ffmpeg is available locally. Correction: `which ffmpeg` returns nothing. uvx is at `~/.local/bin/uvx`. Status: verified.
- Belief: public-domain titles are stable. Correction: Standard Ebooks returns **HTTP 451** for withdrawn titles. `jules-verne/twenty-thousand-leagues-under-the-seas/f-p-walter` answered "This ebook is no longer available, due to legal reasons". The app must re-check titles and handle withdrawals. Status: verified.

---

## Part A. Landing v2 spec

Tokens, type and motion come from `brand/tokens.toml` (D14) and note 12. Display type is Inter. One word per headline may be set in Literata italic (borrowed from refs 7, 8 and 11 below). Blue is for actions. Ember appears only on the spoken word: in the hero video, the demo and the tile mocks.

### A0. Global
- Width: content 1200 px max; gutter 16 px under 400 px and 20 px above; section gaps 136 px desktop and 96 px mobile (ElevenReader uses 136–176 px).
- Section header pattern (borrowed from ElevenReader): a 36 px rounded chip holding a Remix icon, then a grey eyebrow (15 px), then the H2 statement (30/36 px mobile, 40/46 px desktop, max 16ch).
- No cookie banner. The site sets no non-essential cookies and no trackers, which is a visible difference from ElevenReader's Cookiebot wall (it lists Meta, Google, Reddit, Tapad, YouTube, Rokt and OpenAI). Never add a tracker that would require a banner.
- Theme: Ink by default, Paper under `prefers-color-scheme: light`, with theme-color set per scheme.
- RTL: the Arabic locale mirrors the layout. Marquees reverse direction. Video and cover art are never mirrored.
- Every CTA that points at an unreleased platform shows a state (see A8), never a dead link.

### A1. Nav (sticky)
- Left: Streams Reader mark + wordmark. Centre: Library, How it works, Voices, Download, GitHub (star count fetched at build time, not at runtime). Right: "Start listening" (blue pill, 44 px).
- 64 px tall. Transparent over the hero, then `--glass` blur with a 1 px line after 8 px of scroll. On mobile: mark + "Start listening" + a menu button that opens a full-height sheet.
- No promo bar.

### A2. Hero
- Eyebrow: "Free · Private · Open source"
- H1: "Hear anything you'd read." (kept from v1). Alternatives: "Your reading list, read to you." / "Press play on any page."
- Sub: "Paste a link or open a book. A natural voice reads it to you and lights up every word as it's spoken. Free, with no hours limit on standard voices."
- CTAs: primary "Start listening" → `/app`; secondary "Get the apps" → `#everywhere`. Under them, an OS-aware line: "Works in your browser now. Android and desktop apps are coming." The line updates when the apps ship (A8 states).
- Media: the product video (Part D). Desktop: a 1280x800 app-window frame under the text, centred, 1080 px max, radius 20, with a faint blue glow behind it. Mobile: a 9:19.5 phone frame, 300 px wide, with the text above (ElevenReader also puts text first on mobile).
- Video controls copy ElevenReader's: the whole frame is an "Unmute" button; a 40 px play/pause sits bottom left; a thin progress line runs along the bottom; there is no seek bar in the hero.
- Fact row instead of ratings (we have no ratings, and none may be invented): "28 natural voices" · "No hours limit" · "Open source (MIT)" · "8 app languages". Each item is a number plus a label, with no laurels.

### A3. Classics row ("the books available in the library")
- Eyebrow: "Free library". H2: "Start with a classic." Sub: "Twenty great books, ready to hear. Public domain, from Standard Ebooks."
- Content: the 20 titles in Part C3. Covers are 2:3 at 132x198 px (desktop) and 104x156 px (mobile), radius 6, with a 1 px inner ring.
- Motion: one row moves as a slow marquee (60 s per loop, duplicated list, CSS transform only). It pauses on hover, on focus inside and on touch. With `prefers-reduced-motion` it becomes a static scroll-snap row with two 40 px arrow buttons (ElevenReader uses prev/next chevrons).
- Interaction: hovering or focusing a cover lifts it 6 px and shows a 40 px round play button. Play streams a pre-rendered 6–8 s opening line (Opus 24 kbps, about 25 KB, fetched on click). Clicking the cover goes to `/app/explore/se/{slug}`, the detail sheet (C4). Signed-out visitors land in the app's try mode.
- Licence line under the row, 12.5 px, `--text-3`: "Books and cover art from Standard Ebooks, in the U.S. public domain. Check your local laws outside the U.S." It links to `/credits#library`.
- A "See the whole library" text link → `/app/explore`.

### A4. Story chapters ("the next big thing" style)
ElevenReader uses big one-line statements under small eyebrows ("Hear the next big thing today", "This is how listening should feel", "Get lost in the story"). We tell the product as four short chapters. Desktop layout is two columns, with the media column sticky while the text column scrolls (scroll-driven, `animation-timeline: view()` where supported, plain stacking otherwise). On mobile the media comes first, then the text. Each chapter has a numbered eyebrow, an H2 statement, two sentences of body, a row of 3–4 "small detail" chips, and one media slot.

1. "01 · Bring anything". H2 "Every page can be a book you hear." Body: "Paste a link, open a PDF or EPUB, or share from any app. Menus and ads are stripped; the text stays." Chips: Links · PDF · EPUB · Markdown and text. Media: an HTML/CSS mock in which a cluttered article card collapses into clean Literata text (about 1.2 s, then still). Spot illustrations `import-link`, `import-file`, `import-paste` act as the chip icons. The idea comes from Vox's words converging into a voice (ref 7).
2. "02 · Follow along". H2 "Your eyes can follow, or rest." Body: "The sentence being read is softly lit and the spoken word is underlined in ember. Scroll away and one tap brings you back." Media: **the live demo card** from v1 (`crates/site/assets/demo`, real Kokoro audio with word timings, voices af_heart and bm_george). Upgrade the demo text to the opening of *Pride and Prejudice* so the demo and the classics row tell one story. Controls: play, voice chips, speed 1x / 1.5x, and Space to toggle playback when the card has focus.
3. "03 · Your voice". H2 "Hear your books in your *own* voice." (*own* in Literata italic). Body: "Read eight short lines aloud. Streams Reader learns your voice, and only you can use it." Chips: Eight lines · Level check · Private to you. Media: a still of the recording screen (note 12 spec) with a 24-bar level meter that animates only while in view.
4. "04 · Keep going". H2 "Start at your desk. Finish on a walk." Body: "Your place, speed and voice follow you from screen to screen. Lock your phone and it keeps reading." Media: the `sync-devices` spot plus a desktop-and-phone mock (two tilted phones, after ref 12). **Gate:** publish this chapter only when cross-device sync (phase 1) is live. Until then, use "Lock your phone and it keeps reading" with the `background-play` spot.

### A5. Voices
Keep v1 (`voices-*` strings): Standard (28 Kokoro voices, "No hours limit"), Expressive (5 Ming voices, "A fair daily share for everyone"), Your own voice. Add a 3x2 grid of voice cards using the app component (orb, name, descriptor, preview on the orb). Preview audio files are under 40 KB each and fetched on click.

### A6. Small details (features grid)
Keep the v1 items (word by word, any pace, sleep timer, picks up where you stopped, screen off, eight languages) and add "Keyboard first: Space, J, L, [ ] on desktop". A 3x3 grid on desktop and 2 columns on mobile. Each tile has a Remix icon, a 17 px title and one line.

### A7. Available everywhere — see Part E for items and states.

### A8. Promise (replaces ElevenReader's pricing block)
- Eyebrow "What it costs". H2 "Nothing. Here's how that works."
- Three columns (v1 `trust-*`): Free for real · Private by design · Open source. Below them, one plain paragraph about the expressive voices: they run on shared GPU time, so each person gets a fair daily share and standard voices have no limit.
- Optional, owner decides: a dated fact table against ElevenReader's free plan. Their side: 10 h/month free, $11/month or $99/year for Ultra, no desktop app, custom voices on Ultra only; verified on elevenreader.io/pricing on 2026-10-30, note 12. Ours: no hours limit on standard voices, desktop apps, own voice free. If used, it must show "as of {date}" and link their pricing page.

### A9. FAQ (accordion, at most 10, emitted as JSON-LD FAQPage)
Keep v1's seven questions (free, voices, own voice, screen off, files, data, languages) and add three:
- "Where do the free books come from?" "From Standard Ebooks, a volunteer project that publishes carefully edited public-domain books. The books are free to read and share in the U.S.; outside the U.S., check your local laws."
- "Is there an iPhone app?" "Not yet. On iPhone and iPad, open Streams Reader in Safari and add it to your Home Screen."
- "Is there a browser extension?" "It's on the way. It adds a button that reads the page you're on aloud in Streams Reader." Update the answer when it is published.

### A10. Closing band
H2 "Your reading list, read to you." (v1 `cta-title`), "Start listening" (blue), "Star on GitHub" (ghost).

### A11. Footer
ElevenReader's footer has 8 link groups and 9 SEO "use case" pages plus 5 comparison pages. Ours only lists pages that exist:
- Get started: Listen online · Android app · Desktop app · Chrome extension · All downloads
- Library: Free classics · Credits (Standard Ebooks)
- Help: Help · FAQ · Status · Changelog
- Project: Source code · Report a security issue · Licence (MIT)
- Legal: Privacy · Terms · Credits
- Streams: "Also from Streams: films" → streams.watch
- Bottom row: language switcher (8 languages), © year, and the voice credits line (v1 `footer-credits-line`).
- Later, as written pages land (open item): use-case pages (read PDFs aloud, read EPUB aloud, read articles aloud, for dyslexia, for ADHD) and comparison pages. ElevenReader ranks with them, but each needs real content.

### A12. SEO and metadata
- Keep note 12's title, description, canonical, OG/Twitter image 1200x630, light and dark favicons, manifest and hreflang for the 8 locales.
- JSON-LD:
  - `SoftwareApplication`: price 0; `operatingSystem` lists only what has shipped. Today that is "Web"; extend it as each app ships.
  - `FAQPage`.
  - `Organization`: Streams, `sameAs` GitHub.
  - `WebSite`.
  - Optional `ItemList` of `Book` for the classics row.
  - **No** `AggregateRating` or `Review` until real ratings exist. ElevenReader uses both.
- Captions: a WebVTT track for the hero video holding the spoken passage. It helps accessibility and gives search engines text.

### A13. Small details checklist
Skip link · visible focus rings (blue, 2 px, offset 2) · `scroll-margin-top: 80px` on anchors · OS-aware CTA labels · `content-visibility: auto` on sections below the fold · no layout shift from covers or video (fixed aspect boxes) · 48 px minimum tap targets in the marquee arrows and video button · copy buttons for checksums on /download · the hero video and demo audio pause when the tab is hidden · only one media element plays at a time (starting the demo pauses the hero and cover previews) · print style hides media · `lang` and `dir` set per locale · OG image per locale.

### A14. Performance budget (landing, first view, mobile)
| Item | Budget | Note |
|---|---|---|
| HTML | ≤ 40 KB gzip | static, no wasm (D12) |
| CSS | ≤ 25 KB gzip | one file, tokens inlined at build |
| JS | ≤ 15 KB gzip | demo player, OS detection, marquee pause, video loader; `type=module`, deferred |
| Fonts | ≤ 90 KB | Inter var + Literata (with italic) subset to Latin, woff2, `font-display: swap`; per-script subsets for ar/hi/zh loaded only on those locales |
| Hero poster | ≤ 60 KB | AVIF, preloaded; this is the LCP element |
| Hero video | ≤ 1.5 MB per file, one file per visitor | loaded after `load`, not on Save-Data or 2G (Part D) |
| Covers | ≤ 15 KB each, ≤ 300 KB for 20 | lazy, AVIF + WebP |
| Targets | LCP ≤ 2.0 s, CLS ≤ 0.02, INP ≤ 100 ms | Lighthouse mobile, slow 4G (assumed targets; measure in phase 3) |

---

## Part B. ElevenReader landing inventory (elevenreader.io, 2026-10-30)

Verified from the rendered DOM and the screenshots unless marked. Type is Waldenburg for display and Inter for body (note 12). Section headers use a 36 px icon chip, a 16 px grey eyebrow (h2) and a 30/36 → 36/42 px title (h3). Buttons are 52 px pills that scale to 0.98 when pressed (300 ms).

| # | Section | Headline / copy | Media and layout | CTAs and motion |
|---|---|---|---|---|
| 0 | Promo bar | "Your first month. Just $1." | grey strip | "Get this offer" |
| 1 | Nav | logo · Explore ▾ · Pricing · For Authors · Upload & Listen | sticky; on mobile a hamburger and "Start listening" | "Sign in" (outline), "Start listening free" (black) |
| 2 | Hero | H1 "Listen to anything in voices you'll love" (40 → 60 px, −1.8 px tracking); sub "Listen to bestselling books, PDFs, and articles. Plus, 200,000+ audiobooks included." | **book row = a 3D fanned carousel of 10 covers**, eager loaded, prev/next chevrons, a play button on each cover (audio preview); desktop has covers above the text, mobile has text first | "Start listening free", "Upload & listen" |
| 3 | Trust row | 4.7 stars / 70k+ ratings · App of the Day · 5M+ installs | laurel wreath glyphs | none |
| 4 | Two demo cards | "Read anything aloud" (drop zone: PDFs, DOCX, EPUB, Markdown, TXT; paste-a-link field; voice chip "Sir Michael Caine™"; "Listen now") and "Audiobooks, your way" (cover stack) | 2 columns on desktop, stacked on mobile | "Listen now", "Explore the library" |
| 5 | Film | eyebrow "Watch the film", H3 "This is how listening should feel" | **MP4 1920x1080, 113 s, 44.3 MB** (content-length), autoplay muted loop, no poster attribute, 16:9 at 1080 px max, radius 16; injected on scroll (it is absent from the first DOM) | a full-area "Unmute" button, play bottom left, seek line |
| 6 | Premium catalogue | "Premium eBooks & audiobooks" / "Get lost in the story" | carousel of 11 covers with title and author links to `/audiobooks/...` | "Explore library", prev/next |
| 7 | Trending genres | "A genre for every kind of story" | fanned covers and 9 genre chips | "View all" |
| 8 | Immersive listening | "Designed for how you listen" | carousel of feature cards: Explore mock, voice list ("1,000+ lifelike voices"), "Design your perfect narrator from a text prompt", "cinematic soundscapes", reader text mock, "Listen offline and across multiple devices" | "Try it free" |
| 9 | Reviews | "Loved by millions of listeners worldwide" | 25 quotes in auto-scrolling duplicated columns, "Review on App Store" | marquee |
| 10 | Available everywhere | eyebrow "Available everywhere", H3 **"Hear the next big thing today"** | 4 square tiles of 220 px, each a mini UI drawn in HTML with container-query units (`cqw`). "Listen Online" is a book page with *Tom Sawyer* and one word highlighted in #4036FF. On hover the mini UI drifts 8cqw over 5000 ms, `cubic-bezier(0.22,1,0.36,1)`, disabled under reduced motion. Grid: 2 columns on mobile, 4 from 1008 px | Listen Online · Get iOS App · Get Android App · Add to Chrome. In the owner's Chrome at 500 px the Android tile came first (assumed: order adapts to the platform) |
| 11 | Pricing | "Simple, flexible pricing" / "Start free. Upgrade anytime." | Monthly/Annual toggle ("Save 25%"); Free and Ultra cards | "Start listening free", "Start 3-day free trial" |
| 12 | FAQs | 10 questions (h3 accordion) | none | none |
| 13 | Footer | Get Started, Use Cases (9 SEO pages), Compare (5 + all), Browse, Resources, Socials, Terms, Company | none | "Cookie Settings" |

- SEO (verified):
  - Title and OG: "Free Text Read Aloud App | ElevenReader"; a description of about 150 characters.
  - OG and Twitter image: 1200x630 webp; Twitter card `summary_large_image`.
  - Canonical: `https://elevenreader.io`; theme-color #FDFCFC.
  - JSON-LD: SoftwareApplication, AggregateRating, Offer, FAQPage (10 Q&A), Organization (x3), WebSite, Review (x2).
- Images: Next.js `/_next/image`. Hero covers load eager at `w=3840` (oversized), the rest lazy.
- Sub-page, the Chrome extension page ([link](https://elevenreader.io/text-to-speech-chrome-extension), fetched):
  - Hero "Bring the web to life with natural voices", with "Add to Chrome" and "Get iOS & Android apps".
  - Features: one-click save of the main content; listen now or queue; next-chapter detection on AO3, Royal Road and FanFiction.net; EPUB import; sync to the mobile app; works on paywalled pages you are logged into, with no bypass.
  - The page does not mention highlighting or voice choice in the extension.
- What to take: the eyebrow + statement header pattern; a cover row with audio preview; film controls (unmute overlay); the four "everywhere" tiles drawn as mini UIs; a large footer.
- What to leave: the promo bar, ratings we don't have, pricing, the 44 MB film without a poster, celebrity voices, the cookie wall.

---

## Part B2. Dribbble references (12 picked from 18 reviewed)

Previews are in `notes/raw/design-private/dribbble/` under the same number.

| # | Shot | What works | Borrow for v2 |
|---|---|---|---|
| 01 | [Hedwig, mobile app landing](https://dribbble.com/shots/19509196-Landing-for-mobile-application-Hedwig) | white space; phone held at an angle; one hand-drawn circle around a word in the H1; a row of three features under the hero | an ember hand-drawn underline on one hero word, drawn once with an SVG stroke animation (the spoken-word idea) |
| 05 | [Glose, landing redesign](https://dribbble.com/shots/21284309-Glose-Landing-Page-Redesign) | diagonal composition; black cover cards with floating genre chips | genre chips floating over covers in the "See the whole library" teaser and the Explore header |
| 07 | [Vox, AI voice generator](https://dribbble.com/shots/27844434-Vox-AI-Voice-Generator-Website) | serif italic accent word in a sans headline; scattered words converging into a voice; a language grid | the Literata italic accent word; chapter 01's text-collapses-into-voice motion |
| 08 | [Book reading app landing](https://dribbble.com/shots/26376145-Landing-Page-for-Book-Reading-App) | warm serif display; centred phone; three stat counters | the fact row with real numbers only (A2) |
| 09 | [Storia, preloader for a reading app](https://dribbble.com/shots/22629899-Storia-Preloader-for-the-promo-website-of-the-reading-app) | dark ground; covers fanned around a phone; fly-in motion | the mobile hero variant: phone video with 4 classic covers fanned behind it (static on reduced motion) |
| 12 | [Smart Reading, immersive audiobook player](https://dribbble.com/shots/27780924) | two tilted phones on a warm gradient; big italic serif tag | the tilted two-device mock for chapter 04 and the Android tile |
| 14 | [Online Book Reader landing](https://dribbble.com/shots/22410486-Online-Book-Reader-Landing-Page) | a QR code "Scan to download" in the hero; covers flank the phone | a QR code in the Android tile for desktop visitors once an APK exists |
| 15 | [Book Reading App landing (The Alchemist)](https://dribbble.com/shots/27254948-Book-Reading-App-Landing-Page) | a huge thin title set behind one cover; very little else | the editorial device for chapter 02: "PRIDE AND PREJUDICE" huge and faint behind the demo card |
| 16 | [SONA, AI voice platform](https://dribbble.com/shots/27513930-SONA-AI-Voice-Generating-Platform-Landing-Page) | a working TTS box in the hero with voice and language pickers | confirms the live demo pattern; keep the demo one scroll below the video |
| 21 | [Speechify Mac app](https://dribbble.com/shots/20143067-Speechify-Mac-App) | a compact desktop player panel with a speed slider and a WPM readout | the desktop tile mock: a small player panel with "1.25× · 190 wpm" |
| 23 | [AI reading audiobook app, light/dark](https://dribbble.com/shots/27668872-AI-Reading-Audiobook-App-Light-Dark-UI) | dark literary UI; serif italic headings; one warm accent | confirms the Ink-first, literary direction (our accent is blue, ember stays on the word) |
| 24 | [MidnightReads, dark reading app](https://dribbble.com/shots/26968908-MidnightReads-Dark-Mode-Reading-App-UI-Design) | warm dark surfaces; glassy cards; a reading-stats card | glass cards for the voice previews; listening stats are a later in-app idea, not for the landing |

Reviewed and not picked: Narrali (cinematic stock-photo hero), Bookly (sticker collage, too loud), Mythics (ornate), Piqoboo, AI Resolve, AI Read and the AI voice platform shot (generic SaaS), plus two Speechify UI shots that were weaker than ref 21.

---

## Part C. Library and licences

### C1. What each source allows (status per line)
- **Standard Ebooks (SE)**: the primary source.
  - Licence: every book repo's LICENSE.md says the text and artwork are "believed to be in the United States public domain" and that contributors dedicate their work to the public domain under CC0 1.0. Checked in `standardebooks/jane-austen_pride-and-prejudice`; GitHub reports `NOASSERTION`. The site footer gives the same CC0 dedication ([about](https://standardebooks.org/about)). Verified.
  - Covers: SE covers are public-domain paintings with SE's title box, and both parts are covered by the statement above, so they can be shown and copied. Verified for the wording; cover-specific rights are "believed" PD, the same as the text.
  - Machine-readable access:
    - The open Atom new-releases feed (`/feeds/atom/new-releases`, 15 latest, with `media:thumbnail` 350x525 and EPUB enclosures). Verified.
    - The full OPDS and Atom feeds and bulk zips need Patrons Circle membership. Verified.
    - Every book is a public GitHub repo (the org has 1,542 public repos, tools included) with `images/cover.jpg`, `cover.svg` and the source text. Verified via `gh api`.
    - Stable download URLs: `/ebooks/{path}/downloads/cover-thumbnail.jpg` (350x525, 21–54 KB, measured on 20 titles) and `/downloads/{author}_{title}.epub`. Verified.
  - Etiquette: SE pages include a honeypot link labelled "Following this link will ban your IP for 24 hours", so no crawler may follow links blindly. Some titles answer 451 when withdrawn. Verified.
  - Attribution: CC0 requires none. Credit SE anyway on `/credits` and in each book's detail sheet. Don't use SE's logo or imply endorsement (assumed courtesy; their pages say nothing about trademark use).
- **Project Gutenberg (PG)**: the second source, for gaps and later for other languages.
  - Licence ([licence](https://www.gutenberg.org/policy/license.html), [permission](https://www.gutenberg.org/policy/permission.html)):
    - Works that are PD in the U.S. may be redistributed freely once **all references to Project Gutenberg are removed** (1.C).
    - If the PG name stays, the full licence must stay with the work (1.E.1, 1.E.4) and no fee may be charged without a 20% royalty (1.E.7, 1.E.8). Streams Reader charges nothing, so keeping the licence complies.
    - A mention in an acknowledgements section or colophon does not count as trademark use.
    - Non-commercial redistribution needs no permission.
    - Verified for the wording.
  - Machine access ([robot policy](https://www.gutenberg.org/policy/robot_access.html), [offline catalogs](https://www.gutenberg.org/ebooks/offline_catalogs.html)):
    - The website is for humans; robots get IP blocks.
    - Use `pg_catalog.csv` (weekly; downloaded once today, 21.3 MB, 90,931 lines including multi-line fields), the daily RDF, `today.rss`, or the harvest endpoint and mirrors with `wget -w 2`.
    - The catalogue data is public domain.
    - The OPDS 1 feed is "expected to be retired in 2027".
    - Verified.
  - Covers: the policy pages say nothing about covers. `cache/epub/84/pg84.cover.medium.jpg` exists (17 KB). PG covers are often generated title cards, so the look is inconsistent (assumed). Use our own generated typographic covers for PG titles (note 12 spec).
  - Rule for the app: keep the PG licence inside the stored EPUB, skip the PG header and footer in the audio (cut at the `*** START OF` / `*** END OF` markers), and show "From Project Gutenberg" with a link in the detail sheet.
- **Wikisource**: later, for languages and texts SE and PG lack (for example Arabic classics once an Arabic voice exists).
  - Main-namespace texts must be public domain under U.S. law. Original Wikisource content is CC BY-SA 4.0. Reusers must check PD status themselves. EPUB export works through WS Export (`ws-export.wmcloud.org`) ([copyright policy](https://wikisource.org/wiki/Wikisource:Copyright_policy), [WS Export](https://wikisource.org/wiki/Wikisource:WS_Export)). Verified from search results; the policy page itself was not opened.
  - Attribution: a link to the Wikisource page, plus "CC BY-SA 4.0" when any non-PD material (introductions, translations made on-wiki) is included.
- **Internet Archive**: not used. Rights vary item by item. WebFetch of the terms page returned only the site title, so the terms were not read. Status: open.
- **Not allowed on the landing page**: modern or publisher covers (Open Library cover images are publisher artwork), and ElevenReader's covers.

### C2. What we may do (summary)
| Use | SE | PG | Wikisource |
|---|---|---|---|
| Show covers on the landing page | yes (CC0, PD art) | use our generated covers | n/a |
| Mirror EPUBs to our R2 and import in the app | yes | yes, licence kept inside | yes, with attribution when CC BY-SA |
| Required credit | none (we credit anyway) | licence inside the file; name only with licence | link + licence when CC BY-SA |
| Bulk machine access | GitHub repos or patron feeds; open Atom for new releases | catalog CSV/RDF + harvest/mirror, 2 s wait | WS Export per title |
| Outside the U.S. | "check local laws" line | the same | the same |

### C3. Landing book row: 20 titles (all checked 200 on 2026-10-30)
SE paths (`standardebooks.org/ebooks/…`), in display order. Mixed so that neighbouring covers differ in colour:
1. mary-shelley/frankenstein
2. jane-austen/pride-and-prejudice
3. arthur-conan-doyle/the-adventures-of-sherlock-holmes
4. herman-melville/moby-dick
5. f-scott-fitzgerald/the-great-gatsby
6. bram-stoker/dracula
7. oscar-wilde/the-picture-of-dorian-gray
8. lewis-carroll/alices-adventures-in-wonderland/john-tenniel
9. fyodor-dostoevsky/crime-and-punishment/constance-garnett
10. marcus-aurelius/meditations/george-long
11. h-g-wells/the-time-machine
12. charlotte-bronte/jane-eyre
13. mark-twain/the-adventures-of-huckleberry-finn
14. sun-tzu/the-art-of-war/lionel-giles
15. virginia-woolf/mrs-dalloway
16. homer/the-odyssey/william-cullen-bryant
17. leo-tolstoy/anna-karenina/constance-garnett
18. robert-louis-stevenson/the-strange-case-of-dr-jekyll-and-mr-hyde
19. edgar-allan-poe/short-fiction
20. emily-bronte/wuthering-heights

- Not usable: `jules-verne/twenty-thousand-leagues-under-the-seas/f-p-walter` (451, withdrawn); `homer/the-odyssey/samuel-butler`, `franz-kafka/the-metamorphosis/david-wyllie` and `franz-kafka/metamorphosis/david-wyllie` (404).
- Visual check: a contact sheet of all 20 reads as one family (oil paintings with a dark title box at the bottom). Verified by eye.
- Gatsby (1925) and Mrs Dalloway (1925) are PD in the U.S. since 2021. Woolf (died 1941) and Fitzgerald (died 1940) are PD in life+70 countries too (assumed; no per-country check).
- Pipeline:
  - `xtask site covers` fetches each `downloads/cover.jpg` once, with 2 s spacing and a User-Agent naming Streams Reader with a contact address.
  - It writes AVIF (q≈50) and WebP (q≈70) at 200w and 400w, at 2:3, to R2 `site/covers/{slug}-{w}-{hash}.{avif,webp}` with `Cache-Control: public, max-age=31536000, immutable`.
  - Markup: `<picture>`, width and height set, `loading="lazy" decoding="async" fetchpriority="low"`, alt "{Title} by {Author}".
  - Estimated 8–15 KB per AVIF at 200w (assumed; measure when built).
- Previews: 6–8 s opening-line clips rendered once with Kokoro (CPU) and stored in R2. The opening lines are PD text.

### C4. In-app Explore ("Free classics") design
- Placement: a secondary entry, not a store tab, so Home stays the user's library (note 12). Library empty state → "Or start with a classic". Import sheet → "Free classics". Landing → `/app/explore`.
- Shelves, hand-curated in a repo file `catalog/shelves.toml` (SE slugs, 10–14 per shelf):
  1. Start here (the 20 above)
  2. Under two hours (novellas, short fiction)
  3. Mysteries and detectives
  4. Gothic and ghost stories
  5. Sea and adventure
  6. Love and society
  7. Early science fiction
  8. The great Russians
  9. Ideas that last (philosophy, essays)
  10. Poems to hear
  11. Just added (the open SE Atom feed, refreshed daily by the Worker cron)
- Layout:
  - Shelf: a title (20 px) and a "See all" link, then a horizontal snap row of 2:3 covers (104 px mobile, 132 px desktop).
  - Card: the cover, then title (15/20, 2 lines), then author and listening time ("6 h 40 m at 1×", from SE word count ÷ 155 wpm; assumed rate, calibrate against measured Kokoro speed).
  - A 32 px play button on the cover plays the opening-line clip.
- Detail sheet: cover, title, author, plain-text description (HTML stripped), length, subjects as chips.
  - Primary "Add to library". Secondary "Listen to chapter 1".
  - Source line: "From Standard Ebooks · Public domain in the U.S. · CC0". Small print: "Outside the U.S., check your local laws."
- Search: client-side over the shelf index (all curated titles, about 150; JSON ≤ 40 KB gzip assumed). The full SE catalogue (about 1,400 titles from the GitHub repos) is phase 2.
- Import path:
  - The Worker copies the EPUB from our R2 mirror into the user's library and runs the existing EPUB importer. Duplicates are found by SE id.
  - The R2 mirror is filled by `xtask catalog sync`, reading the GitHub repos or SE download URLs with 2 s spacing; it never touches SE's HTML listings.
  - A monthly status check marks titles that start returning 451. A withdrawn title disappears from shelves; copies already in users' libraries get a notice (open item: what to do with them).
- Languages: SE is English-only. Non-English shelves come from PG once the reading voices cover those languages. Kokoro reads es, fr, hi, it, pt, ja and zh (note 03, assumed still current).

---

## Part D. Hero video plan

| Option | Cost | Truth | Size | Localisable | Verdict |
|---|---|---|---|---|---|
| (a) Real screen recording of the app (CDP screencast → ffmpeg) | free; about half a day | it is the product | 0.5–1.2 MB (assumed) | UI is English; the spoken text is English anyway | **recommended** |
| (b) CSS/SVG phone mockup driven by real word timings | free; 1–2 days of front-end work | close to the product, drifts as the app changes | ≈ 20 KB + audio | fully (8 locales, RTL) | use only as the live demo (chapter 02), which v1 already has |
| (c) AI video via Google Flow | credits; needs the owner's sign-in | not the product (ElevenReader's film reads as AI to the owner) | large | no | not for the hero; at most a mood clip later, owner's call; not run |

Recommended recipe for (a):
1. Source: the local dev build (the owner's Chrome had `http://127.0.0.1:8093/read/…` open on 2026-10-30), seeded with the SE *Pride and Prejudice* EPUB. Not the production site, which needs the passphrase.
2. Script (14 s, one take, loops cleanly):
   - 0–1.5 s: Library with the classic at the top. Click play.
   - 1.5–10 s: the reader. The sentence wash and ember word underline move through "It is a truth universally acknowledged…", and the page auto-follows.
   - 10–12.5 s: open the voice sheet and switch to bm_george; reading continues from the same sentence.
   - 12.5–14 s: close the sheet. Hold the opening frame for the loop seam.
3. Capture: Chrome Beta headless, driven over CDP (a Python script run with `uvx --with websockets`):
   - Viewports: 1280x800 at DPR 1 (desktop file) and 390x844 at DPR 2 → 780x1688, scaled to 540x1170 (phone file).
   - `Page.startScreencast` with `format=png`. Ack every frame and store its `metadata.timestamp`.
   - Write an ffmpeg concat list with the real per-frame durations, then resample to 30 fps.
   - Clicks are sent with `Input.dispatchMouseEvent`.
   - Audio: the app's own rendered segment audio for the same passage and voice, muxed at the timestamp of the play click. Headless capture has no sound.
4. Encode (ffmpeg). It is not installed. Options: `brew install ffmpeg` (assumed to include libsvtav1, libx264, libopus and libaom), or `uvx --from imageio-ffmpeg` (codec set unknown; check `-encoders` first). Commands:
   - AV1/WebM: `ffmpeg -f concat -safe 0 -i frames.txt -i voice.wav -vf "fps=30,format=yuv420p" -c:v libsvtav1 -preset 4 -crf 40 -g 300 -c:a libopus -b:a 32k -shortest hero-1280.webm`
   - H.264/MP4 fallback (for Safari without AV1 hardware): `-c:v libx264 -preset veryslow -crf 27 -tune animation -profile:v high -pix_fmt yuv420p -movflags +faststart -c:a aac -b:a 48k hero-1280.mp4`
   - Poster: the frame at 4 s (mid-sentence highlight) as AVIF ≤ 60 KB, plus WebP.
   - Gate: each file ≤ 1.5 MB. If it is over, raise the CRF by 2 or drop to 1120x700. Screen content compresses well; the 0.5–1.2 MB estimate is assumed and gets measured at build.
5. Markup and behaviour:
   - Markup: `<video muted playsinline loop preload="none" poster>` with two `<source>`s. AV1 goes first with its codecs string; the MP4 comes second. A phone-sized pair is selected by `media="(max-width: 700px)"` on `<source>` (assumed supported in current Chrome, Firefox and Safari; otherwise a 10-line JS chooser).
   - A WebVTT captions track.
   - JS starts playback only when all hold: after `load`, in view (IntersectionObserver), `prefers-reduced-motion` is not `reduce`, `navigator.connection.saveData` is false, and `effectiveType` is not 2g.
   - It pauses when out of view or when the tab is hidden.
   - "Unmute" and play/pause buttons as in A2.
   - Reduced motion or no autoplay: the poster plus a 56 px "Play video" button. Nothing moves until the visitor presses it.

---

## Part E. "Available everywhere" section

- Eyebrow "Available everywhere". H2 "Hear the next chapter anywhere." Alternative: "Start on one screen, finish on another."
- Four tiles. Each is a 220 px square mini-UI drawn in HTML/CSS with container-query units (ElevenReader's technique), with the action pill underneath. Grid: 2x2 on mobile, 4 across from 1008 px. The tile matching the visitor's platform goes first and gets a "Your device" chip (v1 string `platform-you`). Hover drift: 6cqw over 4 s, none under reduced motion.

| Tile | Mini-UI | Action label | State now | State later |
|---|---|---|---|---|
| Web | a book page with the ember-underlined word (Pride and Prejudice line) | "Listen online" → `/app` | Available ("Chrome, Edge, Firefox, Safari") | the same |
| Android | the phone mini-player with lock-screen controls | "Get the Android app" | Coming soon: a disabled pill plus a link "Install the web app" (PWA from Chrome) | 1) "Download APK · v{x} · {size} MB" with SHA-256 and the changelog, and a QR code for desktop visitors (ref 14). 2) The official Google Play badge once listed (Play developer registration is $25 one time; assumed, not checked today) |
| Desktop | a compact player panel with "1.25× · 190 wpm" (ref 21) | OS-detected: "Download for Mac" (Universal .dmg) / "Download for Windows" (.msi, x64) / "Download for Linux" (AppImage), plus an "Other platforms" link → `/download#desktop` | Coming soon: "Watch releases" → GitHub releases | the live download with version, size, minimum OS and "Updates itself" |
| Chrome extension | a browser toolbar with the Streams Reader button and a "Reading this page" toast | "Add to Chrome" → the Chrome Web Store listing | Coming soon | live; on Firefox the label becomes "Add to Firefox" once on AMO; on Safari the tile shows "Not available in Safari yet" |

- iOS: no tile. One line under the grid: "iPhone and iPad: open Streams Reader in Safari and add it to your Home Screen."
- OS detection (≈ 1 KB JS):
  1. Use `navigator.userAgentData?.platform` ("macOS", "Windows", "Linux", "Android", "Chrome OS"), with the UA string as the fallback.
  2. Treat a "Mac" UA with `navigator.maxTouchPoints > 1` as iPad.
  3. Chrome OS gets Web first, then Android.
  4. On phones the Desktop tile shows "Mac, Windows, Linux" and links to `/download` without guessing.
  5. Server HTML ships the neutral labels; JS only re-labels and reorders, so there is no layout shift (tiles are fixed size).

Minimal MV3 extension ("Read this page with Streams Reader"):
- Manifest: permissions `activeTab`, `scripting`, `contextMenus`, `storage`; host permission only `https://listen.streams.watch/*`; a background service worker; command `Alt+Shift+R`. No broad host permissions and no remote code. Total under 100 KB with Readability bundled.
- Flow:
  1. A toolbar click, the shortcut, or the context-menu item ("Read this page" or "Read selection") injects Mozilla Readability (Apache-2.0) into the active tab and extracts `{title, byline, lang, url, html}` or the selected text.
  2. The extension opens or focuses `https://listen.streams.watch/app/import`. **No page content goes in the URL.**
  3. It hands over the payload with `chrome.tabs.sendMessage` to a content script that runs only on that origin. The script passes it to the app with `postMessage` and an origin check.
  4. The app imports the page under the user's own session (sign-in first if needed) and starts at the first sentence.
- It works on pages the user can see, including subscriptions they are logged into, because it reads the rendered DOM. It bypasses nothing.
- Phase 2: play in place (an overlay mini-player with word highlight, audio in an offscreen document) and next-chapter detection for web-novel sites (ElevenReader has both).
- Publishing: Chrome Web Store registration is a one-time fee, $5 per Chromium's 2020 post and 2026 third-party guides. Google's page states no amount ([register](https://developer.chrome.com/docs/webstore/register)). Status: assumed $5. The same code should work on Edge Add-ons and Firefox AMO (assumed).

---

## Decisions proposed (owner to confirm)
- P1 (assistant): Standard Ebooks is the only source for the landing row and Explore v1; PG comes later; Internet Archive is not used. Reason: CC0, consistent covers, open GitHub repos. Rejected: PG covers (inconsistent), Open Library covers (publisher art).
- P2 (assistant): hero video = a real screen recording (option a). Rejected: AI video for the hero (not the product, costs credits); the CSS mock as the hero (kept as the demo).
- P3 (assistant): no ratings, reviews or pricing blocks. A fact row replaces ratings and the Promise section replaces pricing. An optional dated comparison with ElevenReader is left to the owner.
- P4 (owner): whether to join SE's Patrons Circle ($15/month or $150/year) for full feed access. Not needed for v1, because the curated shelves plus the GitHub repos and the open Atom feed are enough.

## Open items
- Hero video: build the capture script and encode. Unblocks when the dev build runs locally with the SE EPUB seeded, and ffmpeg is installed (`brew install ffmpeg`, or uvx imageio-ffmpeg with AV1 confirmed).
- Measure real AVIF cover sizes and video sizes against the A14 budget. Unblocks at the phase 3 build.
- Chapter 04 copy depends on cross-device sync going live (phase 1).
- Android, desktop and extension tiles move from "Coming soon" to live as phases 4 and 6 ship. Each needs a version, size and checksum source (the release manifest in R2).
- Withdrawn SE titles already in a user's library: keep, hide or flag. The owner decides.
- Internet Archive terms not read (WebFetch returned only the page title). Unblocks by reading the terms in a browser, only if IA is ever needed.
- Kokoro's reading languages for non-English shelves: re-check against note 03 before building those shelves.
- Use-case and comparison SEO pages: write them only with real content. The owner decides whether to include comparisons.
