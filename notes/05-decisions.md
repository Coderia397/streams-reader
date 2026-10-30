# Decision log

Decisions, newest first. Each has date, decision, reason, alternatives rejected, and who decided.

## 2026-10-30

### D14. Streams blue accent and one central brand token file
- Decision: accent = Streams primary blue #2F6BFF (hover #4A82FF, active #2358D6, soft #6E9BFF, deep #1D4BBD, from `streams-frontend-rs/brand/colors.json`), surfaces near Streams' void #0B0B0C / panel #16161A / paper #F3F1EC; ember stays only for the word being spoken. Supersedes note 12's violet.
- One source of truth: `brand/tokens.toml` → `brand/build.py` → `brand/dist/tokens/{_tokens.scss,tokens.css,tokens.json}`. Every colour, radius, spacing, type size, shadow, motion and component dimension (buttons, inputs, cards, sheets, chips, tap targets) comes from it; app icons and illustrations read their palette from it too. Web and site SCSS may not contain raw hex values (build check).
- UI icons: Remix Icon (vendored SVGs, compiled in). Brand: custom app-icon system for every platform plus a drawn spot-illustration family (20 named spots).
- Decided by: user (blue, central tokens, Remix Icon, custom icons), assistant (structure).

### D13. Product name: Streams Reader
- Decision: the product is **Streams Reader**, a sub-brand of Streams (streams.watch), at `listen.streams.watch`. The user proposed "Streams Reader / Streams Books / Streams something" and delegated the choice.
- Reason: one brand family with the user's film app; says exactly what it is; "listen" (the address) is the action, "Reader" is the product.
- Rejected: standalone names. Checked 2026-10-30 with the naming skill script (.com via Verisign RDAP, crates.io, GitHub user/org; .app returned unknown): rawi, sada, lilt, murmur, verso, hakawati, lectern, cadenza, qari, raawi — all 10 .com taken and all 10 GitHub names taken; crates free for rawi, sada, hakawati, qari, raawi. Trademarks and app stores not checked.
- Follow-up: display name lives in one constant per surface; GitHub repo renamed to `streams-reader`; internal resource names (`nolabs-*`) kept until a planned rename refactor.
- Decided by: assistant on the user's direction ("take care of everything").

### D9. Native apps with Tauri 2, and all eight extra features are in scope
- Decision: Tauri 2 apps for macOS, Windows, Linux, Android sharing the Leptos UI, with native playback, media controls, downloads and self-updating (note 11). In scope (user: "yes for all the eight things"): cross-device sync, offline downloads, share into the app / open files, lock-screen + headset + car controls, invite-only accounts with a fair split of expressive minutes, Arabic, whole books in the cloned voice overnight (Kaggle), library export/backup.
- Also in scope (user, later the same evening): final name and logo; home-screen widget (resume/play); import from Google Drive and Kindle; notification when a long render finishes; "read this page" button for desktop browsers; several users on every platform.
- Decided by: user (features), assistant (Tauri).

### D8. Domain: listen.streams.watch
- Decision: the app lives at `listen.streams.watch` (custom domain on the Worker `nolabs-api`, Cloudflare zone streams.watch). Supersedes the "name chosen later" part of D2.
- Alternatives offered: `read.streams.watch`, `reader.streams.watch`.
- Decided by: user, 2026-10-30.

### D7. R2 credentials in the Space are bucket-scoped, not presigned URLs (for now)
- Decision: the Space holds an S3 credential derived from a Cloudflare user token limited to Object Read + Write on `nolabs-media` only (token name "nolabs-engine-r2", created 2026-10-30). Presigned URLs from the Worker are phase 2.
- Reason: the Space writes several objects per segment (audio + sidecar JSON + job marker); presigning each from the Worker adds a round trip per object. A bucket-scoped key cannot touch `pstream-data` or anything else.
- Rejected: account-wide R2 admin key (security review blocker).
- Decided by: assistant.

### D6. Scope for tonight: milestones A and B; director, ASR, MeloTTS, Kaggle in phase 2
- Decision: see note 06 v2, "Milestones" and "Deferred to phase 2".
- Reason: all five reviewers judged v1's scope too large; the listening loop is the product.
- Decided by: assistant, on the review panel's synthesis.

### D5. A little TypeScript is allowed for browser glue
- Decision: Rust everywhere; a small TypeScript `interop/` layer only where browser APIs are awkward from wasm-bindgen (audio element / MediaSource playback, Media Session, PDF text via pdf.js).
- Reason: same split the user's `streams-frontend-rs` already uses (`interop/`, esbuild).
- Rejected: TypeScript frontend (user wants Rust); pure web-sys for everything (slower to build well).
- Decided by: user ("a bit of TypeScript"), scope by assistant.

### D4. All design decisions delegated to the assistant
- Decision: the assistant decides architecture, models, naming of internals, hosting layout, and runs review, testing and deployment itself.
- Decided by: user ("All decisions are yours").

### D3. Node is not part of the product or the day-to-day deploy
- Decision: no Node runtime anywhere in the product. Cloudflare deploys go through a Rust deploy tool calling the Cloudflare API, or through GitHub Actions. A local Node 26.13.0 + wrangler 4.156.0 was installed in `~/.local/node` on 2026-10-30 as a fallback; remove it once the Rust deployer works.
- Reason: the user dislikes Node ("usually").
- Decided by: user preference, approach by assistant.

### D2. Domain: subdomain of streams.watch, name chosen later by the user
- Decision: deploy to `*.workers.dev` first; attach a subdomain of `streams.watch` once the user picks a name. Assistant recommendation: `listen.streams.watch`.
- Reason: free, HTTPS, isolated DNS record, stable origin for the installed web app (changing origin later loses the phone's local audio cache and the install).
- Rejected: a new domain (costs money); keeping `workers.dev` permanently (less premium, ties origin to account name).
- Decided by: assistant recommendation; final name open with the user.

### D1. Follow the user's existing Streams conventions
- Decision: Leptos CSR + Trunk frontend, Rust services, Space deploy pattern copied from `streams-engine` (`deploy/space/app.py` launcher, routes under `/gradio_api/<name>/`, GitHub Actions deploy, keepalive cron).
- Reason: proven on the same accounts; the user maintains these patterns already.
- Decided by: assistant.

## Open items
- Subdomain name: waiting on the user.
