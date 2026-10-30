# v3 plan: accounts, languages, landing page, native apps, speed

Plan. Everything the user added on the evening of 2026-10-30, sequenced after milestone A (web listening loop) is live. Conclusion: six phases, each built by parallel agents in separate directories, reviewed, deployed and committed before the next. Earlier notes: 06 (v2), 11 (native apps and accounts), 12 (design benchmark, in progress).

## 2026-10-30

### Requests covered (user, 2026-10-30 evening)
Native apps for macOS, Windows, Linux, Android; self-updating; several users; Supabase backend; web app with landing page; SEO files like Streams (robots.txt etc.); compare with ElevenLabs Reader; catchy but clean design; ~8 UI languages; emails handled like Streams; continuous refactoring; everything wired together; the eight features (sync, downloads, share/open files, lock-screen/headset/car controls, invite-only + fair budget split, Arabic, cloned-voice books overnight, export/backup); name and logo; home-screen widget; Google Drive and Kindle import; render-finished notification; "read this page" browser button; speed improvements beyond caching.

### Decisions
- D10 (assistant): Supabase project `nolabs` (`atxbmgfuxjtcwmoaldqy`) holds accounts and each user's data (documents metadata, progress, bookmarks, voices metadata, settings, invites, budget shares) with row-level security; the apps talk to it directly over PostgREST and Realtime for cross-device sync. The Cloudflare Worker keeps what needs secrets or heavy I/O: the render pipeline (D1 `renders`, `jobs`, `gpu_ledger`), audio from R2, document bodies in R2 (ownership checked against Supabase with the caller's JWT), the URL fetcher, the auth email hook, the update channel. JWTs verified in the Worker against the project's JWKS.
- D11 (assistant, following Streams): UI text in Fluent `.ftl` files, 8 languages: English, Arabic (RTL), Spanish, French, German, Portuguese, Hindi, Chinese (Simplified). Emails: Supabase "Send Email" hook → route on the Worker (signature-verified) → Brevo HTTP API from `no-reply@streams.watch`, wording from the same `.ftl` files.
- D12 (assistant): the landing page and policy pages are prebuilt static HTML (no wasm), served by the Worker with SEO files modelled on `streams-frontend-rs/public` (robots.txt welcoming search and AI crawlers, sitemap.xml, llms.txt, security.txt, IndexNow key, Open Graph image, manifest). The app (Leptos wasm) lives under `/app`.

### Phases
| Phase | Content | Owner dirs | Gate |
|---|---|---|---|
| A (now) | Web listening loop on `listen.streams.watch` (current build) | all | smoke + browser + Pixel test |
| 1 Accounts | Supabase schema + RLS + invites; Worker JWT auth replacing the passphrase; per-user budget share; email hook + Brevo; web sign-in (email code) | `supabase/`, `crates/server`, `crates/worker`, `crates/web` | tests + live sign-in |
| 2 Languages + design | Fluent i18n (8 languages, RTL), design system from note 12, polish pass | `crates/web`, `crates/i18n` | screenshots per language |
| 3 Landing + SEO | static site, legal pages, OG images, SEO files, download page with OS detection | `site/`, `crates/worker` | Lighthouse-style checks, links |
| 4 Native apps | Tauri 2 app (`apps/desktop`, `apps/android`), native audio + media controls, downloads, file open/share, updater, release CI to R2 | `apps/`, `.github/` | builds for 4 platforms, install on Pixel |
| 5 Speed | on-device Kokoro (ONNX via `ort`) in native apps; Space streaming per segment + pipelined encode; ONNX Kokoro on Space CPU | `crates/tts-local`, `space/` | measured RTF before/after |
| 6 Features | widget, Drive import, Kindle (exported files; no public Kindle API), notifications, browser extension, Kaggle bulk renderer, Arabic voice (Chatterbox Multilingual), export/backup | various | per feature |

### Added 2026-10-30 (user, late evening)
- Landing page v2 with ElevenReader-level richness in our design: hero product video, a row of library books, "available everywhere" (listen online, Android, desktop, add to Chrome), storytelling sections, small details. Brief: note 15 (research agent, in progress). Built after the first site version lands.
- In-app "Explore" library of DRM-free public-domain books (Standard Ebooks CC0 editions first, Project Gutenberg second), one tap to add and listen. Licensing verified in note 15 before use.
- Chrome extension ("Add to Chrome"): read the current page aloud through Streams Reader (phase 6, moved earlier if time allows).

## Open items
- Phase order may change if milestone A testing on the Pixel shows playback problems (native Android player moves up).
- Kindle: Amazon offers no import API; feasible only for user-exported files (assumed).
