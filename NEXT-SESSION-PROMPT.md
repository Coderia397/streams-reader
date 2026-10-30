# Handoff: nolabs-reader

Read first: `notes/00-index.md`, then `notes/10-progress.md` (newest entry), `notes/06-architecture.md` (v2 section), `notes/09-build-spec.md`.

## What this is
A personal, free replacement for the ElevenLabs Reader app. Rust/Leptos web app (installable on the user's Android phone) → Rust Cloudflare Worker `nolabs-api` (D1 `nolabs`, R2 `nolabs-media`, Queue `nolabs-render`) → private ZeroGPU Space `ibrahimar397/nolabs-engine` running Kokoro-82M on CPU (standard voices, unlimited) and Ming-3.0-tts-0.5B on GPU (expressive and cloned voices, about 4.5 min/day).

## Current state
See `notes/10-progress.md` and the acceptance table in `notes/07-plan.md`.

## Waiting on the user
- Product name and subdomain of streams.watch (recommended `listen.streams.watch`); until then the app runs on workers.dev.
- 20-minute screen-off test on the phone (check A13).

## Access
- Credentials: `~/.config/streams/*.env` (Cloudflare Global key, GitHub, HF write token, Kaggle) and `~/.config/streams/nolabs-r2.env` (bucket-scoped R2 S3 key); app secrets in `~/.config/nolabs/` (deploy token, passphrase). Never copy values into the repo or notes.
- Deploy: `cargo run -p xtask -- <bootstrap-token|setup|migrate|passphrase|build|deploy|space|smoke|status>`. No Node.
- GPU budget: a free account gets 5 GPU-min/day and maybe 3 runs; every Ming test costs a run. Check `notes/08-measurements.md` for runs used.
