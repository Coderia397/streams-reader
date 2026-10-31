# Decision log

Decisions, newest first. Each has date, decision, reason, alternatives rejected, and who decided.

## 2026-10-31

### D35. Streamed renders level each segment against the batch so far
- Decision: the Space streams one item result per finished segment, so segment N's batch loudness is that of segments 0..N, not the whole batch; per-segment clamp and peak ceiling unchanged. Needs Worker `ADAPTER_VERSION` 2 (together with the BART fallback of D32). Rejected: whole-batch loudness (blocks streaming), per-segment targets with no batch term. Details: [22-engine-speed.md](22-engine-speed.md).
- Decided by: assistant under the engine-speed track delegation, 2026-10-31; owner may veto.

## 2026-10-30

### D34. Main address becomes reader.streams.watch; Google sign-in live
- Decision (owner asked "since it is Streams Reader, should we fix the domain?"; assistant agreed): `reader.streams.watch` becomes the canonical address, `listen.streams.watch` stays as a permanent redirect. Supersedes the address in D8. Status: domain attached 2026-10-31 01:25 with the new narrow `cargo xtask domain <hostname>` (only reader/listen.streams.watch accepted); a raw API call had been blocked by the auto-mode classifier. Verified: https://reader.streams.watch/ 200 in 0.17 s. Open: sweep Supabase site URL/redirects, emails, site canonical URLs and sitemap, CSP, app config and docs.
- Google sign-in (2026-10-31, done with the owner in Chrome): Google Cloud project `streams-reader`; Google Auth Platform: external, In production, basic scopes only, no logo (a logo forces Google's brand verification; add the pixel mark later together with verification once the privacy/terms pages are live); OAuth web client "Streams Reader web": JS origins https://listen.streams.watch and https://reader.streams.watch, redirect https://atxbmgfuxjtcwmoaldqy.supabase.co/auth/v1/callback; client id 292244780535-d9cko9tpp51neioc73jb1vcm99jkb0uc.apps.googleusercontent.com (public); secret only in `~/.config/nolabs/oauth.env` (mode 600). Enabled in Supabase with `cargo xtask auth-provider google` (verified: /auth/v1/settings external.google = true). The owner ticked Google's API Services User Data Policy himself.
- Decided by: owner (domain question, Google go-ahead), assistant (setup).

### D33. Motion: product motion design for the hero, Blender for the character film
- Decision: the landing hero is product motion design of the real UI (the live interactive demo in crates/site, plus a code-driven motion reel and feature loops in media/motion rendered through headless Chrome with brand tokens and real fonts); the voxel character film is the emotional piece and moves to Blender (scripted bpy: real armature with IK, PBR lighting, depth of field, motion blur), rendered on Kaggle's free GPU (media/blender). The three.js film stays as v1.
- Reason: the owner asked for "a bit more professional"; leading brands lead with crisp UI motion; Blender gives correct rigs and cinema lighting for free, scriptable and reproducible; nothing heavy runs on the laptop.
- Rejected: Blender for UI motion (worse type and brand fidelity); Unreal (heavy, not headless-friendly); paid AI video.
- Decided by: assistant on the owner's question, 2026-10-31.

### D27–D31. Native apps architecture
- Recorded in [18-native-apps-architecture.md](18-native-apps-architecture.md): `apps/` is its own Cargo workspace (D27); the Rust core owns playback, render-ahead, downloads, session and updates while the webview only displays (D28); Sonic for pitch-preserving speed on desktop, signalsmith as fallback (D29); Android plays local files fed ≥ 3 min ahead (D30); release artefacts in R2 bucket `streams-reader-releases`, one minisign updater key (D31). Accepted by the assistant 2026-10-31.

### D32. On-device Standard voice: Kokoro fp32 ONNX with misaki-rs and a BART fallback, no espeak
- Decision: native apps render Kokoro locally with `onnx-community/Kokoro-82M-v1.0-ONNX-timestamped` (fp32; int8 measured slower on this Mac), `ort`, `misaki-rs` 0.8.0 (MIT) for G2P with the Apache-2.0 BART model for unknown words; espeak-ng is excluded (GPL-3.0). The Space switches its unknown-word fallback to the same BART model so device and server pronounce alike (this re-keys the Kokoro cache once). Android renders ahead and uses the server for live playback until the Pixel 8 is measured. Evidence: [19-on-device-tts.md](19-on-device-tts.md).
- Decided by: assistant on the owner's delegation, 2026-10-31.

### D25. The repository is public
- Decision: `Coderia397/streams-reader` made public on 2026-10-30 (the landing page promises open source; public repos get unlimited free CI minutes incl. macOS). Before publishing, a scan of every commit on every branch for HF/Cloudflare/GitHub/Supabase/Brevo/Resend/Kaggle tokens, service-role JWTs, passphrases, session keys, R2 secrets and private keys found nothing; the owner's email is not in tracked files. README added; topics set.
- Decided by: owner ("you can make it public"; reaffirmed with permission to use the GitHub admin token for account work).
- Correction 2026-10-31: the first attempt to record this decision (commit 0df1908) did not write it, because its insertion anchor only existed on the accounts branch; recorded here instead.

### D26. Mark v2: the pixel book (designer B, with the judges' fixes)
- Correction 2026-10-30 (critic round, applied by the brand fixer): "at 32 px and below the bare blue mark" and "inner arc on rows 5-11" are superseded. The bare mark is now only for web favicons; every desktop/app icon is the tile at every size (16 and 32 px carry a 10x10 hand reduction, `brand/src/mark-small.txt`; 24 px the 16 grid on 1 px cells), matching Streams' desktop set. Inner arc is rows 4-11 (concentric with the outer arc, centre row 7.5). `cell_for` rounds ties up (64 px: 3 px cells, 192 px: 8). Android adaptive cells shrink to keep the corners inside the 66 dp safe zone (2/4/5/8/11 px). favicon.svg uses blue-400 under prefers-color-scheme: dark. OG faint mark is 288 px at (888, 156). macOS body is a superellipse (n = 5); open: check it in the macOS 26 Dock before shipping the icns. Verified: `brand/build.py` rebuild and `brand/tests` (33 tests) pass.
- Decision: B's concept (a page stack drawn like the Streams cube's depth band, which bows into a sound arc) became the Streams Reader mark. Scores were A 199, B 206, C 0 (C submitted nothing). Applied fixes:
  - The silhouette closes at the bottom right with the cube's three-step diagonal, so it shares the cube's outline cells.
  - The folded corner was replaced by a book spine (column 2), so the mark reads as a book, not a document.
  - The text lines are 5, 3 and 4 pixels long.
  - The inner voice arc is symmetric on rows 5-11 and never touches the page edge or the outer arc.
  - The ember pixel is removed from every app icon, favicon and Android layer. It survives only in `streams-reader-mark-ember.svg`, for 128 px and up.
  - The white and one-colour marks are pure.
  - The tile gradient is faint (2.4 % lightness). The tile gets A's quarter-cell blue-800 shadow at 32 % and the hairline rim.
  - A's 640 px mark is used on the 1024 tile (40 px cells).
  - Every export is drawn at its own size with whole-pixel cells. At 32 px and below the bare blue mark is used, as in Streams' favicon.
- Rejected: B's folded corner (read as a Google Docs or file icon), the ember pixel on the tile (judges: it reads as a stray dot and breaks white-on-blue), B's 2.83 dp Android cells and the blur filter in the foreground layer, and A's book-and-equaliser (wide, flat, no cube silhouette).
- Old mark and icons: `brand/archive/v1/`.
- Decided by: assistant on the owner's delegation (D17); owner may veto.
### D18–D24. Accounts phase 1 decisions
- Recorded in [17-accounts-phase-1.md](17-accounts-phase-1.md) (Supabase schema and RLS, invite-only sign-up for every provider, email OTP only (no passwords), identity linking rules, per-user GPU share, JWT verification in the Worker, email hook through Brevo). Renumbered from the agent's D13–D19 when merged.

### D17. Brand icon direction v2: pixel-grid sibling mark + voxel mascot illustrations
- Owner feedback on brand v1 (contact sheet): "a bit unprofessional … kinda like it, not fully". Assistant's diagnosis: the cursor-waveform mark reads as an X/bone and vanishes on a dark tile; spots look clip-art (heavy navy fills, uneven strokes, circle plates).
- Decision: app icon = a Streams-family mark on the same 16×16 pixel grid as Streams' pixel-cube S (an open book whose pages become sound bars) on a Streams-blue tile, white mark, legible at 16 px; illustrations = the voxel character from the film as a recurring mascot, rendered from the same 3D scene as transparent PNG/WebP spots (20 presets, `media/spots/`); UI icons stay Remix Icon.
- Reason: one world across logo family, film, landing and in-app illustrations; made once, reused; distinctive rather than stock.
- Decided by: assistant (owner asked for a suggestion and delegated decisions); owner may veto.

### D16. Landing v2 open questions (note 15), decided by the assistant on the owner's delegation
- Standard Ebooks: no Patrons Circle; source books and covers from the public GitHub repos (CC0) and the open Atom feed; honour HTTP 451 withdrawals; never follow their crawler-trap links.
- A dated, factual comparison with ElevenReader (features, price, platforms) is included; no disparaging claims; date and sources on the section.
- Withdrawn titles already in a user's library stay; they leave the Explore shelves.
- Hero media: the 3D film studio loop (note 16, owner's idea) replaces the plain screen recording recommended in note 15; the screen recording stays as the fallback.

### D15. Every component goes through a find-fix loop on the real thing
- Decision: after each build phase, adversarial testers exercise the real rendered/running component (real browser, real audio, the phone over ADB), report only reproduced defects, a separate fixer repairs them at the root with tests, and the round repeats until no new defect is found. First applied to the player (workflow `wf_a6ae6ada-16b`, harness in `crates/player/harness/`).
- Reason: the owner found seek and tracking problems on the Pixel that unit tests missed (Streams lesson: score the real page, not fixtures).
- Decided by: user ("make sure you have a cycle of improve"), assistant (method).

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

D10–D12 are in note 14; D13–D19 (accounts phase 1: no service key in the Worker, atomic invite redemption, no passwords, per-user GPU share, audio URL tokens, brand constant, Bearer + CORS) are in note 15.

## Open items
- Subdomain name: decided as `listen.streams.watch`, product name "Streams Reader" (owner, 2026-10-30).
