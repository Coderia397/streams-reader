# apps/: Streams Reader native apps

Tauri 2 apps for Android, macOS, Windows and Linux around the same Leptos UI as `listen.streams.watch/app`.
Architecture, decisions and the phased plan: [notes/18-native-apps-architecture.md](../notes/18-native-apps-architecture.md).

| Path | Crate | What |
|---|---|---|
| `bridge/` | `streams-native-bridge` | UI ↔ shell contract: command/event names, payloads, `NativeBridge` trait, `TauriBridge` (wasm32) |
| `audio-engine/` | `streams-audio` | desktop playback: WebM/Opus → Sonic time-stretch → cpal, gapless |
| `android-plugin/` | `tauri-plugin-streams-media` | Android layer in Kotlin: Media3 `MediaSessionService`, keystore secrets, shares, APK self-update |
| `tauri/src-tauri/` | `streams-reader-app` | the app: commands, feeder, storage, `tauri.conf.json`, capabilities |

This is its own Cargo workspace (own `Cargo.lock` and `target/`), so the root workspace never resolves Tauri.

```sh
cd apps
cargo test                                   # bridge only (default member)
cargo clippy -p streams-audio --no-default-features --all-targets -- -D warnings   # without cmake/libopus
```

Full app builds run in CI only: `.github/workflows/release.yml` (manual dispatch).
