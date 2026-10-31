# Native apps architecture (Tauri 2: Android, macOS, Windows, Linux)

Plan + reference + decisions for phase 4 of the v3 plan (notes 11, 14; D9). Conclusion: one Tauri 2 app per platform around the unchanged Leptos UI, where the Rust core (not the webview) owns playback, the render-ahead feeder, downloads, the session and updates; the UI reaches it through one `NativeBridge` trait (`apps/bridge`). Android plays through a Kotlin Media3 `MediaSessionService`; desktop through our own Rust engine (symphonia + libopus + Sonic + cpal) with souvlaki media controls. Build order: Android debug APK on the Pixel 8 first, desktop CI builds second, the updater third. Scaffold is in `apps/` and `.github/workflows/release.yml`; nothing was built with Tauri or Gradle (disk limit).

## 2026-10-30

### What exists now (verified 2026-10-31 on this Mac)
| Path | What | Check |
|---|---|---|
| `apps/Cargo.toml` | separate Cargo workspace for the apps (see D27) | `cargo generate-lockfile` in `apps/`: 599 packages resolved, every version and feature below exists (verified) |
| `apps/bridge` | crate `streams-native-bridge`: IPC names, payloads, `NativeBridge` trait, `TauriBridge` (wasm32) | `cargo clippy --all-targets -D warnings` clean on host and `--target wasm32-unknown-unknown`; 7/7 tests pass (wire shapes, names) |
| `apps/audio-engine` | crate `streams-audio`: desktop engine skeleton (clock, playlist, Sonic adapter, module docs) | clippy clean, 5/5 tests (with `--no-default-features`: no cmake on this Mac, so bundled libopus was not compiled) |
| `apps/android-plugin` | crate `tauri-plugin-streams-media` + Gradle library with 7 Kotlin files | not compiled (needs Gradle + Tauri; assumed correct against the Tauri 2.12.5 Kotlin sources read in the crate) |
| `apps/tauri/src-tauri` | crate `streams-reader-app`, `tauri.conf.json`, `capabilities/default.json`, command modules | not compiled (Tauri tree ≈ 1–2 GB of target); JSON parsed; script check: every `cmd::ALL` entry has a handler in `lib.rs` and an `allow-*` grant (verified) |
| `.github/workflows/release.yml` | `workflow_dispatch` only; meta → web → desktop matrix + Android → publish | YAML parsed (verified); never run |
Target data of the checks went to the scratchpad (≈ 600 MB), not the repo.

### Process model
- **One OS process per app.** Rust core = Tauri runtime + tokio tasks (feeder, downloads, updater, session refresh) + on desktop the audio decode thread and the OS audio callback thread; souvlaki on the main thread (macOS/Windows) or its own zbus thread (Linux). The webview runs out of process where the OS does that (WKWebView WebContent process, WebView2 `msedgewebview2`, WebKitGTK `WebKitWebProcess`); on Android the WebView runs in the app process.
- **Android:** `MainActivity` (Tauri's `WryActivity`, the WebView) + `PlaybackService` (Media3 `MediaSessionService`, foreground of type `mediaPlayback` while playing) + `libstreams_reader_app_lib.so` (the Rust core). The plugin in the activity talks to the service through a `MediaController`, so the service keeps playing when the activity is destroyed.
- **Rule:** anything that must continue while the webview is hidden, throttled or gone lives in the core. The UI shows state and sends intent. Background webview timers on Android are assumed throttled while the screen is off (Chromium behaviour; to verify in phase 1), which is why render-ahead moves out of the web `RenderSession` into the native feeder.

### Workspace layout (D27)
`apps/` is its own Cargo workspace (`apps/Cargo.toml`, own `Cargo.lock`, `apps/target/`), members `bridge`, `audio-engine`, `android-plugin`, `tauri/src-tauri`, default member `bridge`. Root crates come in by path (`nolabs-core`). The brief asked for the app crate as a root-workspace member outside `default-members`; that still puts Tauri's ~600-package graph (WebKitGTK, WebView2, JNI, cpal, libopus) in the root `Cargo.lock`, which 6 parallel agents share, and a single bad feature would break resolution for all of them. The coordinator also limited this work to `apps/`. A separate workspace keeps the root lock and builds untouched and lets the app have its own release profile (opt-level 3 for the audio crates, `s` elsewhere; the root is `z` for the Worker wasm).

### Frontend in the app
- The UI is the same `crates/web` build. It is compiled with `BASE = "/app"` and absolute `/app/...` asset URLs (`Trunk.toml public_url = "/app/"`). Tauri serves `frontendDist` at the origin root and resolves `path` → `path.html` → `path/index.html` → `index.html` (read in `tauri-2.12.5/src/manager/mod.rs`). So `frontendDist = "../frontend"`, a staging folder with the dist under `app/` plus a copy of `index.html` at the root for deep-route fallbacks; the window URL is `app/`. The brief said "frontendDist pointing at crates/web/dist"; pointing it there directly would serve `index.html` in place of every `/app/*.js` and `.wasm` (assumed from the resolver code, not run). CI's `web` job builds and stages it; `apps/.gitignore` ignores `apps/tauri/frontend/`.
- Dev: `trunk serve` on 8090 + `devUrl: http://localhost:8090/app/`.
- Origins: `tauri://localhost` (macOS, Linux), `http://tauri.localhost` (Windows, Android); the Worker's CORS list already has both (D24).

### Detection and the `NativeBridge` trait (contract for the `crates/web` owner)
- `streams_native_bridge::tauri::is_tauri()` checks `globalThis.isTauri` / `__TAURI_INTERNALS__`. At start the UI calls `TauriBridge::connect().await`: `app_info` → `AppInfo { platform, version, bridge_version, channel, capabilities }`. If it fails, the UI uses `WebBridge` (to be written in `crates/web`, reports `Capabilities::default()` and keeps today's code paths). `bridge_version` mismatch → "update the app" screen.
- Calls go through Tauri's public global API (`app.withGlobalTauri: true`): `__TAURI__.core.invoke(cmd, args)` and `__TAURI__.event.listen(name, cb)`. Payloads cross as JSON text, so the wire format is exactly serde's; no JS bundle and no serde-wasm-bindgen.
- `trait NativeBridge` (in `apps/bridge/src/lib.rs`): `info()`, `player_load/control/snapshot`, `download_start/cancel/remove/list`, `storage_usage`, `cache_clear`, `auth_store/session/refresh/sign_out/open_browser`, `update_check/install`, `open_take_pending`, `file_read`, `subscribe(handler) -> Subscription`. Default methods return `ErrorCode::Unsupported`, so `WebBridge` writes only what the web has.
- Switching the player: in `crates/web/src/state/player/open.rs`, `if bridge.info().capabilities.native_player { NativePlayer(bridge, LoadArgs { doc_id, voice_id, engine, items: render_items(&segments), known, start_segment, info, rate, sleep, skip, autoplay }) } else { nolabs_player::Player::new(session.provider(), …) }` behind one `ActivePlayer` enum with the same methods (`play`, `pause`, `seek_by`, …). Native `PlayerEvent::Snapshot` carries `at_ms`; the UI extrapolates `segment_pos_ms + (now − at_ms) × rate` per animation frame for word highlighting, and gets word timings from `PlayerEvent::SegmentReady`.
- Other web changes when native: skip service-worker registration; auth hands the session to `auth_store` and asks `auth_session`/`auth_refresh` instead of refreshing itself; Settings shows downloads and update channel when the capabilities say so. Capabilities switch on in `commands/app.rs` as each phase lands (all `false` now except `open_files`).

### IPC (all names in `streams_native_bridge::proto::{cmd, event}`)
| Command | Args → result |
|---|---|
| `app_info` | → `AppInfo` |
| `player_load` | `{ args: LoadArgs }` |
| `player_control` | `{ command }`: `play`, `pause`, `toggle`, `seekBy{deltaMs}`, `seekTo{segment,ms}`, `next`, `previous`, `setRate{rate}`, `setSleep{sleep}`, `setSkip{skip}`, `unload` |
| `player_snapshot` | → `Snapshot` |
| `download_start` / `download_cancel` / `download_remove` / `download_list` | `{ args: DownloadArgs }` / `{ docId }` / `{ docId }` / → `DownloadInfo[]` |
| `storage_usage`, `cache_clear` | → `StorageUsage` |
| `auth_store` | `{ session: NewSession }` → `SessionView` (never returns the refresh token) |
| `auth_session`, `auth_refresh`, `auth_sign_out` | → `SessionView?` / `SessionView` / `()` |
| `auth_open_browser` | `{ url }`, only `https://<ref>.supabase.co/auth/v1/authorize?…` accepted |
| `update_check`, `update_install` | `{ channel }` → `UpdateInfo?` / `()` |
| `open_take_pending`, `file_read` | → `OpenRequest[]` / `{ path }` → raw bytes (`tauri::ipc::Response`, `ArrayBuffer`), only inside `imports/` |

Events: `streams:player` (`PlayerEvent`: snapshot ~4/s while playing, segmentChanged, segmentReady, ended, sleepFired, error), `streams:download`, `streams:update`, `streams:open`, `streams:auth-callback { url }`. Errors are always `{ code, message }` (`unsupported`, `not_signed_in`, `network`, `storage`, `busy`, `invalid`, `internal`). ACL: `build.rs` registers `cmd::ALL` through `AppManifest::commands`, so each command needs its `allow-<name-with-hyphens>` grant in `capabilities/default.json` (naming verified in `tauri-utils` `acl/build.rs`). The webview gets no plugin permissions: updater, deep link, opener and streams-media are driven from Rust.

### Android layer (`apps/android-plugin`)
- Versions (verified 2026-10-30, Maven metadata + AAR manifests): **Media3 1.11.1** stable (2026-09-10), minSdk 23, minCompileSdk 36; 1.12.0-alpha02 exists (not used). androidx.core 1.18.0 (1.19.1 needs compileSdk 37). compileSdk 36, minSdk 24 (Tauri default), targetSdk 36. Gradle layout copied from `tauri-plugin-dialog` 2.8.4.
- `PlaybackService : MediaSessionService`: ExoPlayer with `AudioAttributes(USAGE_MEDIA, AUDIO_CONTENT_TYPE_SPEECH)` and `handleAudioFocus = true` (ExoPlayer requests and abandons focus, ducks/pauses for calls and navigation), `setHandleAudioBecomingNoisy(true)` (unplugged headphones / Bluetooth drop → pause), `WAKE_MODE_LOCAL`, seek increments 15 s; `onTaskRemoved` stops only when not playing. Manifest: `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MEDIA_PLAYBACK`, `foregroundServiceType="mediaPlayback"`, intent filter `androidx.media3.session.MediaSessionService`, `POST_NOTIFICATIONS`.
- Notification, lock screen, Bluetooth/headset keys: Media3's `DefaultMediaNotificationProvider` from the session (from API 33 System UI draws it; Google advises against customising). Artwork: `MediaMetadata.artworkUri` = local file of the cached cover (1.11.0 fixed a `ForegroundServiceStartNotAllowedException` from async artwork loads). Custom layout: ±15 s buttons. The service leaves foreground after 10 min paused (Media3 behaviour).
- `DocumentPlayer : ForwardingSimpleBasePlayer`: ExoPlayer holds one `MediaItem` per segment (local `file://` WebM/Opus, appended as the feeder lands them); the session sees one item for the whole document (title, author, one progress bar = sum of segment durations, estimates until known), and seeks map document time ↔ (item, offset).
- Gapless: ExoPlayer plays the playlist back to back; Media3 1.11.1 trims Opus pre-skip but not WebM end padding (fixed in 1.12.0-alpha01, androidx/media#2576). Our segments end in 250–1200 ms of appended silence, so the padding is extra silence, not a click. Audio offload stays off (no Opus gapless in offload).
- Rate: `setPlaybackSpeed` (ExoPlayer's Sonic processor, pitch kept). Sleep timer and skip size: custom `SessionCommand`s (TODO).
- Android 17 (API 37; Pixel 8 reported updated 2026-06-16, secondary source): "background audio hardening" applies to every app: background playback, focus requests and volume changes need a visible activity or a non-short foreground service; for apps targeting 37 the service also needs while-in-use capability, gained by starting from a visible activity, the notification, a widget or a media key. We start playback from the activity, so fine; test with `adb shell cmd audio set-enable-hardening throw`. Android 15: `BOOT_COMPLETED` may not start `mediaPlayback` services (we do not).
- Secrets: `SecretStore.kt`, AES-256-GCM key in the Android Keystore, ciphertext in private SharedPreferences (Jetpack security-crypto is deprecated). Shares: `ShareIntake.kt` (SEND text/URL/file, VIEW epub/pdf/md/txt; content URIs copied to `files/imports/` at once). Updater: `ApkInstaller.kt`.
- Events Kotlin → Rust: a `tauri::ipc::Channel` created in Rust and passed to `setEventHandler` (Kotlin side `app.tauri.plugin.Channel`, deserialised from `__CHANNEL__:<id>`; verified in `Channel.kt`; a Rust-created channel reaching Kotlin is assumed). Kotlin sends `BridgeEvent` envelopes so Rust re-emits them unchanged.

### Desktop audio engine (`apps/audio-engine`, crate `streams-audio`)
Stack (crates.io, 2026-10-30; research report in this session):
| Layer | Crate | Why |
|---|---|---|
| Demux | `symphonia` 0.6.1, feature `mkv` only | pure Rust; maps `A_OPUS`, passes OpusHead, applies CodecDelay; does not trim DiscardPadding |
| Decode | `symphonia-adapter-libopus` 0.3.0 (libopus 1.6.1 via `opusic-sys`, bundled with cmake) | drops pre-skip; cmake + C compiler exist on all GitHub hosted images; fallback pure-Rust `opus-decoder` 0.1.1 |
| Stretch | `libsonic` 0.1.0 (pure Rust Sonic) | built for speeding up speech, latency of a few pitch periods, tiny CPU; ExoPlayer uses the same algorithm, so both platforms sound alike. Risk: one release; backups: `signalsmith-stretch` 0.1.3 (feature `signalsmith`, MIT, C++ + libclang) or compiling `sonic.c` with `cc` |
| Output | `cpal` 0.18.2 | CoreAudio / WASAPI / ALSA (`libasound2-dev` in CI). README: CoreAudio needs macOS 14.2+ → `minimumSystemVersion: "14.2"` |
| Resample | `rubato` 5.0.1 | only if the device refuses 48 kHz |
| Ring | `rtrb` 0.4.1 | wait-free SPSC between decode thread and callback |
Rejected: rodio 0.23 (pulls symphonia 0.5, little control over gapless and DSP placement), rubberband (GPL), bungee bindings (young), tdpsola (AGPL), timestretch (music-tuned, untested on speech).

Design: one decode thread decodes segments back to back into one PCM stream (never restarting the output), trims each file to its sidecar `duration_ms`, applies gain (volume × 10 s sleep fade), stretches (bypassed at 1.0), pushes 20 ms chunks plus a `clock::Tag` into a 400 ms ring; the callback pops, writes silence on underrun and counts frames; `clock` maps frames played → (segment, source ms) at any rate. Look-ahead lives on disk (≥ 3 min), not in memory, so a rate change is heard within 400 ms. Seek inside a segment = decode from its start and discard (segments are 5–40 s). Device loss → pause and re-open on the new default device. Media controls: souvlaki 0.9.3 with `use_zbus` (pure Rust MPRIS, no libdbus), created on the main thread, Windows HWND from the main window; reports document-level metadata and position.

### Storage, offline downloads, feeder
- Layout (`apps/tauri/src-tauri/src/paths.rs`): app data dir → `audio/{kokoro|ming}/{key}.webm` + `.json` sidecar (content-addressed like R2, shared by all documents), `downloads/{doc_id}.json` (pins clips), `covers/`, `imports/`, `updates/`, `state.json`. Unpinned clips evicted LRU above 1 GiB (setting). The webview keeps its IndexedDB (bodies, prefs) as on the web.
- Feeder (`feed.rs`, phase 1): per loaded document a tokio task does what the web `RenderSession` does (`POST /api/render` for the window, status polls with backoff, `GET /api/audio/{key}?t=…` into `.part` → rename, sidecar, `Append` to the engine, `SegmentReady` to the UI), keeping ≥ 3 min ahead; a download runs the same loop to the end. Proposal for the web owner: move the pure `crates/web/src/provider/table.rs` into `nolabs-core` so both share the window/backoff rules.

### Session, deep links
- The core is the only refresher and the only holder of the refresh token (Supabase rotates refresh tokens and revokes on reuse; two refreshers would sign users out, and the core needs tokens while the webview sleeps). UI signs in as on the web, then `auth_store`; afterwards `auth_session` / `auth_refresh`.
- Keystores: `keyring-core` 1.0.1 with `apple-native-keyring-store` 1.0.2 (`keychain`), `windows-native-keyring-store` 1.1.0, `zbus-secret-service-keyring-store` 1.0.1 (`rt-tokio-crypto-rust`); Android: Kotlin `SecretStore`. Linux without a Secret Service: 0600 file fallback, shown in Settings (assumed acceptable).
- OAuth: `auth_open_browser(authorize URL, redirect_to=nolabs://auth/callback)` opens the system browser (`tauri-plugin-opener`); `tauri-plugin-deep-link` 2.6.3 (`desktop.schemes: ["nolabs"]`, `mobile: [{ scheme: ["nolabs"], host: "auth" }]`) delivers the URL; the core emits `streams:auth-callback`; the UI (holding the PKCE verifier) exchanges the code. Windows/Linux deliver the link as argv of a second instance → `tauri-plugin-single-instance` 2.5.5 with `deep-link` feature. Redirects `nolabs://auth`, `nolabs://auth/**` are already allowed in Supabase (note 17). Email codes need no redirect at all (primary path).

### CSP and asset protocol
CSP in `tauri.conf.json` (object form): `default-src 'none'`, `script-src 'self' 'wasm-unsafe-eval'`, `style-src 'self' 'unsafe-inline'`, `img-src/media-src` self + `https://listen.streams.watch`, `connect-src 'self' ipc: http://ipc.localhost https://listen.streams.watch https://<ref>.supabase.co wss://<ref>.supabase.co`, `base-uri/form-action/frame-ancestors 'none'`. Tauri adds nonces/hashes to the CSP; a nonce in `style-src` makes browsers ignore `'unsafe-inline'`, which would break Leptos inline styles, so `dangerousDisableAssetCspModification: ["style-src"]` (CSP3 behaviour; assumed to matter for our `style=` attributes). The asset protocol is disabled: the webview never reads audio or local files (bytes come through `file_read`).

### Updater (desktop), keys, R2 layout, rollout
- `tauri-plugin-updater` 2.14.1 (verified in source): static manifest; platform keys looked up as `{os}-{arch}-{installer}` then `{os}-{arch}` (`installer` ∈ `app`, `msi`, `nsis`, `appimage`, `deb`, `rpm`); `requireSignedVersion: true` (new in 2.14: the minisign trusted comment must carry the announced version, which blocks replaying an old signed build as a "new" one; safe because we have no older releases). Windows `installMode: passive`. `createUpdaterArtifacts: true` → `.app.tar.gz`, `.AppImage`, `.msi`, `-setup.exe` + `.sig`.
- Keys: one minisign pair generated once with `cargo tauri signer generate -w ~/.config/nolabs/tauri-updater.key` (password-protected, mode 600, plus an offline copy). Public key → `plugins.updater.pubkey` (placeholder now; the publish job refuses to run while it is the placeholder). Private key + password → GitHub secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The same key signs Android APK manifests. Losing it means shipping a manual reinstall to every user.
- R2: a new bucket `streams-reader-releases` (not `nolabs-media`: the Space's R2 credential can write there, D7), served by the Worker at `/updates/*` and `/download/*`:
  ```text
  updates/desktop/{stable|beta}/latest.json      Cache-Control: max-age=300
  updates/android/{stable|beta}/latest.json      { version, versionCode, url, size, sha256, signature, notes, pubDate, minSdk }
  releases/{version}/Streams.Reader_{version}_universal.dmg | …_universal.app.tar.gz(.sig) | …_x64_en-US.msi(.sig)
  releases/{version}/Streams.Reader_{version}_x64-setup.exe(.sig) | …_amd64.AppImage(.sig) | …_amd64.deb
  releases/{version}/streams-reader-{version}.apk | .aab                      Cache-Control: immutable
  ```
  Desktop `latest.json`: `{ version, notes, pub_date, platforms: { "darwin-aarch64-app", "darwin-x86_64-app", "windows-x86_64-nsis", "windows-x86_64-msi", "linux-x86_64-appimage", "linux-x86_64-deb": { url, signature } } }`.
- Publishing (`cargo xtask release-publish --version --channel --dir`, not written; xtask is outside this scope): upload versioned files first, then the manifest, so a client never sees a manifest pointing at missing files; R2 token scoped to the releases bucket only.
- Rollout: `beta` first (owner's devices), `stable` after a day without issues. Staged percentage later in the Worker: the app sends `x-install-id` (random per install) and the Worker returns 204 outside `rollout_percent`. Rollback = put the previous manifest back (installed clients stay; ship a patch version). `x-nolabs-api: 2` + Worker 409 `upgrade_required` → `UpdateInfo.required`.

### Android self-update reality (verified 2026-10-30, PackageInstaller.SessionParams docs)
- Needs `REQUEST_INSTALL_PACKAGES` and the user turning on "Install unknown apps" for Streams Reader once. Silent update with `setRequireUserAction(USER_ACTION_NOT_REQUIRED)` (API 31+) when: the app is updating itself (or is installer of record / update owner), holds `UPDATE_PACKAGES_WITHOUT_USER_ACTION`, and the new APK targets ≥ 34 on Android 16 / ≥ 35 on Android 17 (floor keeps rising). Otherwise `STATUS_PENDING_USER_ACTION` → system dialog (handled). How the app was first installed (adb, browser) does not matter for self-updates. `setRequestUpdateOwnership` only works on first install; not needed.
- Developer verification: enforced since 2026-09-30 on certified devices in Brazil, Indonesia, Singapore, Thailand; global in 2027. adb installs are exempt; updates of unregistered apps fail unless the user enabled the advanced flow (24 h wait, restart, biometric). Free "Limited Distribution" account (up to 20 devices, no ID) or $25 full account. Register `watch.streams.reader` with the release signing key before 2027. Losing that key = cannot register or update.
- Android refuses updates signed by another certificate: debug APKs (phase 1) and release APKs cannot update each other; uninstall once when switching.

### CI release pipeline (`.github/workflows/release.yml`)
- `workflow_dispatch` only, inputs `targets` (android-debug / android / desktop / all), `channel`, `publish`.
- `web` (ubuntu-24.04): trunk 0.21.14 (the version on this Mac), stage under `app/`.
- `desktop` matrix: **macos-15** (arm64) `universal-apple-darwin` → `.app` + `.dmg`; windows-latest (Server 2025) → `.msi` + NSIS `.exe`; ubuntu-22.04 (glibc 2.35 for older distros) → `.AppImage` + `.deb`. tauri-cli 2.12.4 via cargo-binstall. Without the signing secret it builds with `createUpdaterArtifacts: false`. The brief named macos-14: it is deprecated and retired on 2026-11-02 (brownouts already running, GitHub changelog 2026-10-01; runner-images README marks it deprecated), so macos-15.
- `android` (ubuntu-24.04): Temurin 17, SDK 36, build-tools 36.0.0, NDK 27.2.12479018 (version string assumed; note 11 says NDK 27), Rust targets aarch64 + armv7; `cargo tauri android init --ci` when `gen/android` is not committed yet (uploaded as an artefact to commit); debug: `--debug --apk --target aarch64`; release: `--apk --aab`, then zipalign + apksigner / jarsigner with the keystore from secrets, so the generated Gradle files need no signing edits.
- `publish`: `environment: release`, runs `cargo xtask release-publish`.
- Cost: the repo is public since 2026-10-30 (D25), so standard hosted runners are free, macOS included (GitHub billing docs, verified 2026-10-30). For reference if it were private: Free plan = $12/month of included usage; Linux $0.006/min, Windows $0.010, macOS $0.062 (10.3× Linux). A full release is estimated (assumed durations: macOS 35 min, Windows 25, Linux 20, Android 30, web 8) at ≈ $2.17 + 0.25 + 0.12 + 0.18 + 0.05 ≈ $2.8, about 4 releases a month on the Free allowance before anything else. Artefact storage on Free private repos is 500 MB; retention is 3–7 days.

### Decisions (assistant, under the owner's delegation D4; proposed numbers, the lead assigns final ones in note 05)
- **D27** 2026-10-31: `apps/` is a separate Cargo workspace. Reason and rejected alternative: "Workspace layout" above. Decided by: assistant.
- **D28** 2026-10-30: the Rust core owns playback, render-ahead, downloads, the session and updates; the webview is a view. Reason: Android webview throttling with the screen off; one refresher for rotating tokens. Rejected: keeping `RenderSession` in the webview and passing audio bytes to native over IPC.
- **D29** 2026-10-30: Sonic (`libsonic`) for pitch-preserving rate on desktop, signalsmith as fallback. Reason: speech-specific, pure Rust, same algorithm as ExoPlayer. Rejected: rubberband (GPL), bungee, tdpsola, timestretch.
- **D30** 2026-10-30: Android plays local files only (feeder downloads first, ≥ 3 min ahead). Reason: offline cache for free, no auth headers in ExoPlayer, exact durations. Rejected: ExoPlayer streaming from `/api/audio` with a CacheDataSource (possible later for a faster first segment).
- **D31** 2026-10-30: release artefacts in a separate R2 bucket `streams-reader-releases`; updater key = minisign, one key for desktop and the Android manifest; `requireSignedVersion` on from the first release. Reason: the Space's credential can write `nolabs-media`.

### Phased plan with acceptance checks
**Phase 1: Android debug APK on the Pixel 8 with background playback**
1. Web owner: `WebBridge`, `ActivePlayer` switch, skip SW when native, `auth_store` path. Check: web app unchanged in a browser (existing tests + Pixel Chrome smoke).
2. Run `release.yml` with `targets=android-debug`; commit the `gen-android` artefact into `apps/tauri/src-tauri/gen/android`. Check: workflow green, APK artefact present.
3. Implement Kotlin `open`/`append`/`control`/events + `DocumentPlayer` state mapping; Rust `engine.rs` Android path, `feed.rs`, keystore session; flip `native_player`, `keystore` to true on Android. Check list (on the Pixel 8 over adb, `adb install -r app-universal-debug.apk`):
   - app starts, signs in with an email code, library loads (CSP/CORS OK);
   - a 30 min document plays with the screen off for 30 min, including segments rendered after the screen went off;
   - lock-screen/notification controls, Bluetooth headset play/pause, ±15 s work; document title and whole-document progress shown;
   - a phone call pauses and resumes; unplugging/disconnecting headphones pauses;
   - swipe from recents while playing: playback continues;
   - rate 2x keeps pitch; no audible gap at segment boundaries (listen at 3 boundaries);
   - `adb shell cmd audio set-enable-hardening throw` produces no exception during play/pause from the notification;
   - share a URL and an EPUB from another app → import screen.
**Phase 2: desktop builds in CI**
4. Implement `streams-audio` (decode, ring, cpal, Sonic, clock), souvlaki, desktop keystore; `targets=desktop`. Check: three green matrix jobs; `.dmg`, `.msi`/`.exe`, `.AppImage`/`.deb` artefacts; on the Intel Mac (macOS 26) the universal app opens (right-click → Open while unsigned), plays a document gaplessly, media keys and Now Playing work, rate 0.75/1.5/2/3x A/B (Sonic vs signalsmith) by ear on one Kokoro and one Ming chapter, CPU of the decode thread < 5 % of one core at 2x (measure with Activity Monitor); the old laptop (Windows or Linux) installs and plays.
5. Offline downloads (both platforms). Check: download a 1 h document, airplane mode, play start to end; storage figures in Settings match `du`.
**Phase 3: updater**
6. Generate the minisign key, put the public key in `tauri.conf.json`, add secrets, create the R2 bucket + Worker routes, write `cargo xtask release-publish`. Check: install version 0.1.0 from R2, publish 0.1.1 to `beta`, desktop app updates and restarts on all three OSes; Android: first update asks for "Install unknown apps" once, the next one (0.1.2) installs without a prompt on Android 17; a manifest with a tampered signature or an older signed version is refused.
7. Android release signing key + developer verification registration. Check: release APK installs over adb, `apksigner verify --print-certs` shows the release cert, registration shows `watch.streams.reader` with that cert.

## 2026-10-31
- Correction (assistant): during research a `curl -o` without `cd` left `tauri-plugin-opener.crate` (+ an unpacked folder) in the repo root; the coordinator deleted it. All downloads since go to the scratchpad. Cause: relative output path in a command without a `cd`.

## Sources
- Tauri 2.12.5, tauri-utils, tauri-build 2.7.4, tauri-plugin 2.7.2, tauri-plugin-updater 2.14.1, tauri-plugin-deep-link 2.6.3, tauri-plugin-dialog 2.8.4: crate sources from crates.io, read in the scratchpad (asset resolver `src/manager/mod.rs`; Kotlin `mobile/android/.../plugin/{Plugin,Invoke,Channel}.kt`; ACL slugs `acl/build.rs`; updater `config.rs`, `updater.rs`).
- Updater guide: https://v2.tauri.app/plugin/updater/
- Media3: https://dl.google.com/android/maven2/androidx/media3/media3-exoplayer/maven-metadata.xml, https://developer.android.com/jetpack/androidx/releases/media3, https://developer.android.com/media/media3/session/background-playback, https://github.com/androidx/media/issues/2576
- Android: https://developer.android.com/reference/android/content/pm/PackageInstaller.SessionParams, https://developer.android.com/about/versions/17/changes/bg-audio, https://developer.android.com/about/versions/15/behavior-changes-15, https://developer.android.com/developer-verification, https://android-developers.googleblog.com/2026/03/android-developer-verification-rolling-out-to-all-developers.html, https://developer.android.com/google/play/requirements/target-sdk
- Rust audio crates: crates.io API per crate; https://github.com/pdeljanov/Symphonia, https://github.com/aschey/symphonia-adapters, https://github.com/ZirekHQ/libsonic-rs, https://github.com/colinmarc/signalsmith-stretch-rs, https://github.com/RustAudio/cpal, https://github.com/Sinono3/souvlaki
- GitHub Actions: https://docs.github.com/en/billing/concepts/product-billing/github-actions, https://github.com/actions/runner-images, https://github.blog/changelog/2026-10-01-github-actions-macos-14-runner-image-retirement

## Open items
- Web integration (`WebBridge`, `ActivePlayer`, SW skip, auth via bridge): unblocks phase 1 step 1; owner of `crates/web`.
- `crates/web` path dependency on `apps/bridge` and the move of `provider/table.rs` into `nolabs-core`: web owner decides.
- Rust-created `Channel` reaching Kotlin, and the manifest merge of share intent filters into Tauri's `MainActivity`: assumed; unblocked by the first CI Android build.
- Android webview timer throttling with the screen off: assumed; measured in phase 1 (if timers keep running, the feeder is still kept, for the token rule).
- `cargo xtask release-publish`, R2 bucket `streams-reader-releases`, Worker routes `/updates/*`, `/download/*`, `x-install-id` rollout: unblocked by the lead/xtask and Worker owners in phase 3.
- Updater key generation and GitHub secrets: owner action (or assistant with tauri-cli installed in CI), phase 3.
- Code signing: macOS notarisation needs the Apple Developer Program ($99/year); Windows SmartScreen reputation needs a certificate or Azure Trusted Signing; until then users see Gatekeeper/SmartScreen warnings. Owner decides.
- Developer verification registration (free Limited Distribution or $25): owner action before 2027; needed earlier if any user is in BR/ID/SG/TH.
- ubuntu-22.04 runner retirement date: unconfirmed (one blog says April 2027); revisit then, or build the AppImage in a container.
- macOS icon superellipse check in the macOS 26 Dock (D26 open item) before the first `.dmg`.
- NDK 27.2.12479018 string and `cargo binstall tauri-cli` prebuilt availability: verified only by the first workflow run.
