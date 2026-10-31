# Streams Reader brand

The visual identity of Streams Reader and the single source of truth for every colour, size and asset in the product. Everything is generated from `tokens.toml` by one script; change a value, rebuild, and the web app, the landing site, the emails, the native app icons and the illustrations all follow.

```sh
uv run --script brand/build.py          # tokens, SVG sources, every PNG/ICO/ICNS, contact sheet (about 45 s)
uv run --script brand/build.py tokens   # token layer only (instant)
uv run --script brand/build.py art      # tokens + SVGs, no raster exports
uv run --script brand/build.py check    # contrast check only; exit 1 on a failure
uv run --no-project --python 3.12 --with resvg-py==0.2.6 --with pillow==11.3.0 \
  --with "fonttools[woff]==4.59.0" --with uharfbuzz==0.51.7 -m unittest discover -s brand/tests
```

Without the `--with` packages the render and export tests skip and the token and grid tests still run.

Dependencies are pinned in the script header (resvg-py, pillow, fonttools, uharfbuzz); uv installs them. The wordmark and the Open Graph text are outlined from the Inter and Literata files in `crates/web/assets/fonts` (read only), so no output depends on an installed font. On macOS the `.icns` is written by `iconutil`; elsewhere by Pillow.

## Tokens

`tokens.toml` holds raw colours (`[palette]`), three themes (`[theme.ink]`, `[theme.paper]`, `[theme.night]`), the scales (radius, space, type, motion), component dimensions (`[component]`) and the artwork palette (`[art.color]`). A value may point at another one: `"{palette.blue-500}"`, `"{palette.ember-400/0.26}"` (same colour at 26 % opacity), `"{radius.pill}"` (emitted as `var(--radius-pill)` so it stays live).

Generated in `dist/tokens/`:

| File | For |
|---|---|
| `_tokens.scss` | web app and landing site. Custom properties on `:root` (Ink by default, Paper under `prefers-color-scheme: light` when no theme is set) and `:root[data-theme="ink" \| "paper" \| "night"]`, plus Sass maps, functions and component mixins |
| `tokens.css` | the same custom properties for plain CSS |
| `tokens.json` | Rust (emails, site generator, Tauri): every colour as css, hex, alpha and composited-on-bg hex, every scale, the art palette |
| `contrast.md` | every checked pair with its WCAG ratio |

Use in SCSS (path relative to the importing file):

```scss
@use "../../../brand/dist/tokens/tokens" as t;

.play { @include t.icon-button(play-size, primary); }
.save { @include t.button(primary); }
.more { @include t.button(ghost, sm); }
.speed-chip { @include t.chip; }          // [aria-pressed="true"] is the selected state
.field { @include t.input; }
.row { padding: t.space(4); border-radius: t.radius(md); color: t.color(text-2); }
.title { @include t.type("h1"); }
.word { @include t.word-pill; }  .sentence { @include t.sentence-wash; }
@include t.bp(md) { … }   @include t.reduced-motion { … }
```

`t.color()`, `t.space()`, `t.radius()`, `t.text()`, `t.size()` and friends return `var(--…)`, so theme switching needs no rebuild, and an unknown name fails the Sass build. Components are defined once as mixins here (`button`, `icon-button`, `chip`, `input`, `card`, `sheet`, `word-pill`, `sentence-wash`, `type`); app code includes them instead of restyling. `t.gen()` reads the `[generative]` table (lightness, chroma and alpha for the CSS-drawn covers and orbs, written as `oklch(t.gen("orb-l-max") t.gen("orb-c") var(--hue-a))`). `focus-ring-inset` draws the ring inside rows that fill a clipping card, and `hit-area` grows a small control to `--tap-min` in both directions.

Colour roles: `brand` fills primary buttons and play (white `on-brand` label); `brand-hover`/`brand-active` keep that label at AA; `brand-soft` is the tinted fill of selected chips and soft buttons; `brand-text` is brand-coloured text and icons on any surface; `ember` is only the word being spoken (underline, caret, recording); `word-pill` and `sentence-wash` are the reader highlight.

The Streams primary `#2F6BFF` gives a white label 4.50:1, a hair under AA, so UI fills use `blue-ui` `#2F6AFF` (4.54:1, one green step away, visually identical); artwork keeps `#2F6BFF`. The Streams hover `#4A82FF` gives white 3.5:1, so it is kept for artwork only and the hover fill is `#2A62F0` (5.1:1).

### Enforcing the token layer

`check_tokens.py` fails on hex colours, `rgb()/hsl()` literals and raw `border-radius` values in any SCSS outside `dist/tokens`:

```sh
uv run --no-project --python 3.12 --script brand/check_tokens.py crates/web/style site/style
```

A line ending in `// tokens-ok: <reason>` is allowed. Wire it into the web and site builds next to the Sass step.

### Contrast (generated, lowest ratio per token across the backgrounds it is checked on)

Text tokens are checked on bg and surface-1 to surface-3 at 4.5; the white label on brand, hover and active at 4.5; brand, focus and ember as UI elements on bg to surface-2 at 3.0; body text on the highlight fills at 7.0. Alpha colours are composited on the theme background first.

<!-- contrast:start -->
| Token | Min | ink | paper | night |
|---|---|---|---|---|
| text | 7 | 9.66 on word-pill | 13.44 on word-pill | 10.03 on word-pill |
| text-2 | 4.5 | 6.21 on surface-3 | 5.52 on surface-3 | 6.40 on surface-3 |
| text-3 | 4.5 | 4.63 on surface-3 | 4.78 on surface-3 | 4.75 on surface-3 |
| brand-text | 4.5 | 5.50 on surface-3 | 4.51 on surface-3 | 6.02 on surface-3 |
| success | 4.5 | 8.02 on surface-3 | 4.60 on surface-3 | 8.77 on surface-3 |
| warning | 4.5 | 8.17 on surface-3 | 4.92 on surface-3 | 8.93 on surface-3 |
| danger | 4.5 | 5.39 on surface-3 | 4.79 on surface-3 | 5.89 on surface-3 |
| on-brand | 4.5 | 4.54 on brand | 4.54 on brand | 4.54 on brand |
| brand | 3 | 3.65 on surface-2 | 3.68 on surface-2 | 3.94 on surface-2 |
| focus | 3 | 6.16 on surface-2 | 4.95 on surface-2 | 6.64 on surface-2 |
| ember | 3 | 9.42 on surface-2 | 4.32 on surface-2 | 10.16 on surface-2 |
<!-- contrast:end -->

## Mark (v2, decision D17)

A book whose page stack bows into a sound arc, drawn on the same 16x16 pixel grid as the Streams pixel cube. The source is `src/mark.txt` (one character per pixel); every SVG and PNG is generated from it. The first mark (the speaking cursor) and its icons are kept in `archive/v1/` for history.

```
...##########...   row 0: back line, as the cube
..#..........#..   rows 1-2: the cube's two-step top-left diagonal; top-right corner rounds into the arc
.#............#.
###########....#   rows 3-15: 13-row front face, as the cube
#.#.......#.#..#   column 2: the book's spine
#.#.......#..#.#   columns 12-13: inner arc (the voice), rows 4-11, centred on row 7.5 like the outer arc
#.#.......#..#.#   column 15: outer arc (the back pages)
#.#.#####.#..#.#   text lines 5, 3 and 4 pixels long
#.#.......#..o.#   o: the ember pixel, shown only in streams-reader-mark-ember.svg
#.#.###...#..#.#
#.#.......#..#.#
#.#.####..#.#..#
#.#.......#....#
#.#.......#...#.   rows 13-15: the cube's three-step bottom-right diagonal
#.#.......#..#..
#############...
```

Construction rules (checked by `tests/test_mark.py`):

- The outline shares the cube's cells: the back line on row 0, the top-left diagonal, the 13-row front face, the bottom edge to column 12, the right line on column 15 and the bottom-right diagonal. In a dock or launcher, Streams and Reader have the same outline.
- The body, the three text lines and the inner arc never touch, not even diagonally. There is at least one empty pixel between the voice arc and the page edge, and between the voice arc and the outer arc, on every row.
- The mark is flat colour, one colour per file, with no gradient, outline or shadow on the pixels themselves. Depth belongs to the tile.
- Pixels are always whole. The mark is drawn at an integer cell size at an integer origin, and each export size is drawn directly, never scaled from a master. The bare mark is exact at 16, 32, 48 and 64 px. On the 1024 tile the mark is 640 px (40 px cells, `art.mark-on-tile = 0.625`), so it is also exact at 512, 256 and 128.

Files in `mark/` (256 px default size, viewBox 16x16, `shape-rendering="crispEdges"`):

| File | Use |
|---|---|
| `streams-reader-mark.svg` | primary: Streams blue `#2F6BFF` on Ink, Paper or Night; favicon; lockups |
| `streams-reader-mark-white.svg` | pure white, on brand blue or dark photos |
| `streams-reader-mark-black.svg`, `safari-pinned-tab.svg` | one-colour print, Safari mask icon |
| `streams-reader-mark-currentcolor.svg` | inline SVG that follows the text colour |
| `streams-reader-mark-ember.svg` | blue with the ember pixel at the centre of the voice arc. Only at 128 px and up, for the landing page and large lockups. Never in favicons, app icons or Android layers |

- Clear space: 2 cells (1/8 of the mark side) on every side. In lockups, the gap from the mark to the wordmark is half a cap height.
- Minimum sizes: bare mark 16 px (1 px cells, web favicons only); blue tile 16 px. Horizontal lockup: 96 px wide.
- Small tiles: at 16 and 32 px the tile carries `src/mark-small.txt`, a 10x10 hand reduction (front face, spine, two text lines, outer arc; no voice arc) on 1 and 2 px cells (3 and 6 px margins), as Streams redraws its cube for its 16 px desktop icon. At 24 px the tile carries the 16 grid on 1 px cells (4 px margin).
- Mark-to-tile ratio: `icons.cell_for` rounds ties up, so the mark is 62.5-75 % of the tile at every size (64 px: 3 px cells, 192 px: 8 px cells).
- Cell sizes: use 16, 32, 48 or 64 px for the bare mark on screen. Other sizes soften.

Do:
- Keep the mark white on the blue tile and blue on Ink or Paper.
- Use one colour in each variant.
- Draw new sizes with `icons.mark_at`, which snaps the mark to whole pixels.
- Change the mark only by editing `src/mark.txt` and rebuilding.

Don't:
- Put the ember pixel on anything below 128 px or on any app icon.
- Add a gradient, outline or glow to the pixels.
- Resample a large PNG down to a small one.
- Rotate, stretch or animate the outline. Only the voice arc may pulse with audio.
- Put the blue mark on mid-tone grounds where it drops below 3:1.

### Tile

The tile is Streams blue with a faint vertical gradient: `art.color.tile-top` `#356FFF` to `tile-bottom` `#2967FF`, 2.4 % lightness in all, so it sits next to the flat Streams tile without looking like a different system. It has a 22 % corner radius, as Streams' tile does. Under the pixels sits a flat offset shadow: `tile-shadow`, which is blue-800 at 32 %, a quarter cell down and right (`art.shadow-offset`), drawn only when cells are 4 px or more. A 2 px top-light rim (`tile-rim`, white at 14 % fading out by 45 % of the height) is drawn on tiles of 128 px and up. All of these colours come from `tokens.toml`.

## Wordmark

"Streams" in Inter 640 at −2.2 % tracking (the parent's sans) and "Reader" in Literata italic 480 (the product's reading face), outlined. Lockups in `wordmark/`: horizontal and stacked, each `on-dark` (white), `on-light` (void), `mono-white`, `mono-black`, and the wordmark alone. Do not reset it in live text or swap the faces.

## App icons (`app-icon/`)

These are the masters. The raster exports call the same functions (`brandkit/icons.py`) at each target size, so every PNG has whole-pixel cells.

| File | Use |
|---|---|
| `icon.svg` | rounded tile 1024, 22 % radius, mark 640 (Linux, PWA "any", Android legacy launcher, avatars) |
| `icon-square.svg` | square tile (Windows tiles and Store, apple-touch-icon, Play Store). The platform applies the mask |
| `icon-macos.svg` | macOS template: 824 body at 100 on the 1024 grid with continuous-curvature corners (superellipse, n = 5, a = 412, 256 points; the rim follows it), drop shadow (y 10, blur 10, 30 %), mark 512. Not yet checked in the macOS 26 Dock |
| `icon-maskable.svg` | PWA maskable, full bleed. The mark's corners stay inside the central 80 % circle (32 px cells at 1024) |
| `android-adaptive-foreground/background/monochrome.svg` | 108 dp layers. The cell is the largest whole pixel that keeps the mark's square corners inside the 66 dp safe-zone circle: mdpi 2, hdpi 4, xhdpi 5, xxhdpi 8, xxxhdpi 11 px (32 to 44 dp marks). The foreground has a flat shadow and no filters. The monochrome layer is the bare mark with no shadow |
| `favicon.svg` | the bare blue mark on the 16 grid, as Streams' favicon. Under `prefers-color-scheme: dark` it is blue-400 `#6E9BFF` (4.48:1 on Chrome's dark tab `#35363A`; blue-500 is 2.68:1). The PNG and ICO favicons stay blue-500 |
| `splash.svg`, `splash-icon.svg` | 2732 square splash (the tile at 20 % on Void); Android 12 splash icon (a 192 dp blue disc with the mark on 7 dp cells) |
| `og-image.svg` | 1200x630 social image. The lockup's mark is on 3 px cells, with the faint large mark on 18 px cells at x 888-1176, y 156-444 (35 px clear of the headline) |

Size rule for the exports: only the web favicons (`favicon.svg`, `favicon.ico`, `favicon-*.png`) and the email mark use the bare blue mark. Every app and desktop icon is the tile at every size (Windows .ico 16-256, Linux hicolor 16-512, the macOS 16 pt slot at 1x and 2x, Tauri, `png/icon-*`), as Streams' desktop set is.

## Spot illustrations (removed)

Removed on 2026-10-31 (decision D40): the v1 drawings were below the brand's bar. Empty, success and onboarding states use one Remix Icon line glyph in a soft `brand-soft` disc instead. The old drawings and their sources are kept for reference in `archive/spot-v1/`.

## Exports (`dist/`)

| Folder | Contents |
|---|---|
| `web/` | favicon.ico (16/32/48, the bare mark), favicon.svg, favicon-16/32/48/96.png (bare mark), apple-touch-icon.png (180), icon-192/512.png, icon-maskable-192/512.png, safari-pinned-tab.svg, og-image.png, splash-2732.png |
| `png/` | the rounded icon at 16 to 1024 |
| `android/` | mipmap-mdpi..xxxhdpi (ic_launcher, ic_launcher_round, adaptive foreground/background/monochrome), mipmap-anydpi-v26 XML, drawable-*/splash_icon.png, playstore-icon.png |
| `macos/` | icon.icns and icon.iconset |
| `windows/` | icon.ico (16 to 256), Square*Logo.png, StoreLogo.png |
| `linux/hicolor/` | streams-reader.png 16 to 512 and scalable SVG |
| `tauri/` | the icon set `tauri.conf.json` expects (32x32, 128x128, 128x128@2x, icon.png, icon.icns, icon.ico, Square*Logo, StoreLogo) |
| `wordmark/`, `mark/` | PNG lockups at 64 and 128 px high (the horizontal lockup's mark lands on 4 and 8 px cells), every mark variant at 512 |
| `email/` | for crates/mail: `streams-reader-logo-on-light.png` / `-on-dark.png` (48 px high, mark on 3 px cells) and `@2x` (96 px), `streams-reader-mark.png` 48/96, `streams-reader-avatar.png` 96/192 |
| `contact-sheet.png` | on Ink and on Paper: the masters, the Android adaptive and themed previews, Streams' tile for the family check, the size ladder 144 to 16 px, the 16 px mark enlarged 8x, lockups and the OG image |

## Consumers copy, never edit

`brand/` is the only place these files are made. Other directories import or copy from `brand/dist` and re-copy after a rebuild:

- Web app (`crates/web`): `@use` the tokens; copy `dist/web/*` into its assets.
- Landing site: `@use` the tokens; `dist/web/og-image.png`, favicons, `wordmark/streams-reader-horizontal-on-dark.svg` and `-on-light.svg`.
- Native apps: `dist/tauri/*` into `src-tauri/icons/`, `dist/android/*` into the Android `res/`, `dist/linux/hicolor` for packaging.
- Emails: `dist/email/streams-reader-logo-on-light.png` (and `@2x`), colours from `tokens.json`.
- `crates/web/tools/make_icons.py` still draws the v1 icons for the web app. Replace its output with `dist/web/*`.
