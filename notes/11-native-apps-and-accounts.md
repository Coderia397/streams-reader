# Native apps, self-updating, accounts for several users (architecture v3 delta)

Plan. The user asked (2026-10-30, evening) for real apps on macOS, Linux, Windows and Android (the Pixel 8, other Android phones, an old laptop), updates that install themselves, use by the user and other people, and a managed backend service so less backend code is ours. Decision: one Rust/Leptos UI shared by the web app and Tauri 2 native apps, with native audio, media controls, offline storage and updaters; managed auth for several users. Status: open (planned after milestone A ships on the web).

## 2026-10-30

### What the user asked for (verbatim intent)
- "I don't want it to be only a web app and I don't want just wrappers of Android around."
- Supported on Mac, Linux, Windows, the user's phone and other Android phones, plus an old laptop at the parents' house.
- "It installs itself … seamless to the user": automatic updates.
- "Me or others": several users.
- Use Supabase (or Firebase) so less backend work is ours; tell the user which is better.

### Decision D9 (assistant): Tauri 2 apps sharing the Leptos UI, with native layers
Why Tauri 2: it is Rust end to end, builds macOS, Windows, Linux, Android (and iOS later) from one project, bundles the UI inside the app (no website wrapper; works offline), and lets each platform add native code. Rejected: Flutter / Kotlin Multiplatform (not Rust, second UI codebase); Slint or Dioxus native renderers (would mean rewriting the UI, Android media integration is immature); a Trusted Web Activity or WebView wrapper (what the user explicitly does not want).

Native layers so it is not a wrapper:
| Need | macOS / Windows / Linux | Android |
|---|---|---|
| Playback that survives screen off / minimised | Rust audio engine (WebM demux + Opus decode → cpal output) in the app process | Media3/ExoPlayer in a foreground `MediaSessionService` (Tauri plugin in Kotlin), gapless playlist of local files |
| System media controls, headset keys | `souvlaki` (macOS Now Playing, Windows SMTC, Linux MPRIS) | MediaSession notification, lock screen, Bluetooth, Android Auto later |
| Offline downloads | audio cached on disk in the app data dir | same, app-private storage |
| Open files / share | file associations (.epub .pdf .txt .md) | share target (links, text, files) |
| Updates | `tauri-plugin-updater`, signed manifest + bundles on R2 behind `listen.streams.watch/updates/` | in-app updater: download signed APK from the same channel, install through PackageInstaller (Android asks the user once per update unless the app is its own installer of record) |
| Builds | GitHub Actions matrix (macos-14 universal, windows-latest, ubuntu-22.04 AppImage + .deb) | Actions or local (NDK 27, SDK 36/37, Android Studio JBR present on the Mac) |

### Accounts: managed auth
- Supabase is preferred over Firebase: Postgres (our schema is SQL), plain HTTP from Rust, row-level security, realtime progress sync between devices, email OTP sign-in, the user already uses it, no Node tooling. Firebase would add a second cloud with Node-based tools and a NoSQL store.
- Blocked (verified 2026-10-30): `POST /v1/projects` returned 400 — owner **Promarcos397** (the only member of org `pstream`) already has 2 active free projects; only `pstream` (`emvyjhleyhimxxdqybsn`, eu-west-1) is visible to the token. Unblock: the user pauses or deletes one project, or chooses an alternative below.
- Alternatives if no slot is freed: (a) a `nolabs` schema inside the `pstream` project (shares auth users and the 500 MB quota with Streams); (b) Firebase Auth only, through its REST API from Rust, data staying in D1.
- Several users changes: data rows get `user_id`; invite-only sign-up so strangers cannot spend the GPU budget; per-user share of the daily expressive (Ming) budget; the Kokoro CPU queue is shared and fair-queued per user.

### Things the user may have forgotten (offered for confirmation)
1. Library and position sync across devices (start on the phone, continue on the laptop).
2. Offline downloads for trips.
3. Share from the browser or other apps into the reader; open EPUB/PDF with it.
4. Lock screen, headset and car controls (Android Auto).
5. Invite-only accounts and a fair split of the daily expressive minutes between users.
6. Arabic and other languages (Ming covers 7 languages, Kokoro 9; no Arabic yet; Chatterbox Multilingual V3 would add it).
7. Whole books in the cloned voice overnight (Kaggle bulk renderer).
8. Export and backup of the library.

### Supabase project created (verified 2026-10-30, ~19:50)
- The user paused project `pstream-backend` (org "Kinemora Org"); the slot freed. Project **nolabs**, ref `atxbmgfuxjtcwmoaldqy`, region eu-west-2, org `pstream`, status ACTIVE_HEALTHY. URL, ref and DB password in `~/.config/nolabs/supabase.env` (mode 600). Free-plan limits are per project (500 MB database, 1 GB storage), so `pstream` is unaffected.
- Free projects pause after 7 days without activity: the Worker cron touches the database daily.

### Speed: architecture, not just caching (assistant plan, 2026-10-30)
1. On-device Standard voice in the native apps: Kokoro-82M as ONNX (int8, about 90 MB) run by the `ort` crate on desktop and Android. Rendering moves to the listener's own CPU: no shared server queue, works offline, scales with users. The Space CPU stays for the web app and as fallback. Expected: desktop several times realtime; Pixel 8 to be measured.
2. Streaming per segment: the Space reports each segment as soon as it is uploaded (Gradio generator, `generating` events) instead of at batch end, so the first audio starts after one short segment.
3. Pipelining inside the Space: synthesis of segment N+1 overlaps encoding and upload of segment N (thread pool), removing encode/upload from the critical path.
4. Kokoro on ONNX Runtime on the Space CPU too (int8 quantised), assumed 2-3x faster than PyTorch CPU; measure.
5. Look-ahead scheduling: the client asks for the next chapter while the current one plays, using measured render speed to keep a fixed number of minutes ready.
6. Expressive tier: GPU calls batch a whole chapter per run (fewest runs), and the Kaggle overnight job takes whole books.

## Open items
- Old laptop OS (Windows or Linux): not needed to proceed, both are built.
- Android self-update without a prompt needs the app to be its own installer of record; verify on the Pixel.
