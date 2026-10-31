# Chrome Web Store listing: Streams Reader — read this page

Everything the Chrome Web Store dashboard asks for, ready to paste, plus the screenshot plan. The extension's own
strings (name, summary, menus, errors) live in `_locales/*/messages.json`; the store pulls the name and summary from
there for each language. Design and handoff details: `notes/23-chrome-extension.md`.

## Item

| Field | Value |
|---|---|
| Name (from `ext_name`) | Streams Reader — read this page |
| Summary (from `ext_description`, 132 max) | Listen to any article in Streams Reader. One click sends the page you're on to your library and starts reading it aloud. |
| Category | Productivity › Tools (alternative: Accessibility) |
| Languages | English (default), Arabic, Spanish, French, German, Portuguese (Brazil), Hindi, Chinese (Simplified) |
| Homepage | https://listen.streams.watch/ |
| Support | https://github.com/Coderia397/streams-reader/issues |
| Privacy policy | https://listen.streams.watch/privacy (**open**: the page does not exist yet; the store rejects a listing that handles website content without one) |
| Package | `uv run extensions/chrome/package.py` → `extensions/chrome/dist/streams-reader-chrome-<version>.zip` |
| Visibility at first | Unlisted, until the web app receives handoffs in production |

## Detailed description (English; translate with the other locales)

Streams Reader turns articles into natural-sounding audio, with each word highlighted as it is read.

This extension adds one button. On any article, click it (or press Alt+Shift+R) and the page opens in Streams Reader,
saved to your library and ready to play. Right-click to read only the text you selected, or a page you have a link to.

- Works on pages you can see, including articles behind your own subscriptions, because it reads the page as it is
  shown to you. It does not get around paywalls or sign-ins.
- Nothing runs in the background. The extension reads a page only when you click the button, use the shortcut or
  pick a menu item.
- No account details, history or browsing data are collected. The page you choose goes to your own Streams Reader
  library and nowhere else.
- Free and open source (MIT): https://github.com/Coderia397/streams-reader

## Single purpose (dashboard field)

Send the article on the current tab, the selected text, or a linked page to the user's Streams Reader library so it
can be listened to.

## Permission justifications (dashboard fields)

| Permission | Why |
|---|---|
| `activeTab` | Read the current page only after the user clicks the button, presses the shortcut or uses the menu. No access to any other tab or site. |
| `scripting` | Run the one-shot extraction script in that tab (rendered HTML, or the selected text). |
| `contextMenus` | The "Read this page", "Read selection" and "Read linked page" menu items. |
| `storage` | Hold the chosen page in memory (`storage.session`, never written to disk, cleared when the browser closes) until the Streams Reader tab collects it. A page the tab does not collect is deleted about 5 minutes after it was sent. |
| `alarms` | Delete a page the Streams Reader tab did not collect about 5 minutes after it was sent, even if the extension is not used again. One timer, set when a page is sent. |
| Host access to `https://listen.streams.watch/app/*` (from the content script) | Hand the page to the Streams Reader web app in the tab the extension opened, without putting page content in the address bar. |
| Remote code | None. All code ships in the package; the CSP is `script-src 'self'`. |

## Data usage disclosure (dashboard checkboxes)

- Collects: **Website content** (the page or selection the user chose), only on the user's action, sent to the user's
  own Streams Reader account.
- Does not collect: personally identifiable information, health, financial, authentication, personal
  communications, location, web history, user activity.
- Certify: not sold to third parties; not used or transferred for purposes unrelated to the single purpose; not used
  for creditworthiness or lending.

## Notes for the reviewer

1. Install, open any article (for example https://standardebooks.org/ebooks/ — public domain), click the toolbar
   button. A Streams Reader tab opens next to it with the article imported. Signing in is required to save to a
   library; a test account can be provided on request.
2. The page reaches the app through `chrome.storage.session` and a content script on listen.streams.watch only; see
   `src/app-bridge.js`. The URL opened is always `https://listen.streams.watch/app/import?from=extension`.

## Screenshots plan

Store rules: 1280×800 (preferred) or 640×400 PNG/JPEG, 1 to 5 images, no rounded corners or padding added by us.
Captured in headed Chrome at a 1280×800 window, device scale 1, light theme, with the extension loaded unpacked from
`dist/unpacked`. Article content is public domain (Standard Ebooks, CC0) or written by us, so the images can be
published; no third-party product UI other than Chrome itself.

| # | Shot | Shows | Caption |
|---|---|---|---|
| 1 | A Standard Ebooks short story in Chrome, pointer on the Streams Reader toolbar button with its tooltip | The one-click entry point | One click to listen to this page |
| 2 | The Streams Reader tab that opens, mid-paragraph, word highlight visible, player bar | The result | Words light up as they're read |
| 3 | Right-click menu on a selected paragraph, "Streams Reader › Read selection" | Selection and link reading | Read just the part you need |
| 4 | Library with three articles imported from different sites, source domain under each title | Saved for later, synced | Everything you send is saved to your library |
| 5 | Arabic UI (RTL) reading an Arabic public-domain text | The 8 languages | In eight languages, including Arabic and Hindi |

Promo images (optional, improve placement): small tile 440×280 (mark + "Listen to any page"), marquee 1400×560 (shot
2 cropped with the wordmark). Both from `brand/dist` assets; the brand owner signs off.

Blocked: shots 2, 4 and 5 need the web app's `/app/import` receiver (see the note) deployed. Shots 1 and 3 can be taken
now.

## Release checklist

1. Bump `version` in `manifest.json`.
2. `uv run --no-project python -m unittest discover -s extensions/chrome/tests -p 'test_*.py'` and
   `uv run extensions/chrome/tests/e2e.py` pass.
3. `uv run extensions/chrome/package.py`; record size and sha256 in the note's progress log.
4. Upload the zip in the developer dashboard (one-time $5 registration, status assumed in note 15). First upload:
   copy the item's public key into the note so a pinned extension ID is available if ever needed.
5. Edge Add-ons accepts the same zip (assumed; not tried).
