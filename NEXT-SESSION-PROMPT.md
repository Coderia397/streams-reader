# Handoff: nolabs-reader (product name: Streams Reader)

Read first: `notes/00-index.md`, then `notes/10-progress.md` (newest entry), `notes/15-accounts-phase-1.md` (accounts, current), `notes/14-v3-plan.md`, `notes/06-architecture.md` (v2 section), `notes/09-build-spec.md`.

## What this is
A free replacement for the ElevenLabs Reader app for the owner and invited users. Rust/Leptos web app (and later Tauri apps) → Supabase project `atxbmgfuxjtcwmoaldqy` for accounts and each user's library/progress (RLS) → Rust Cloudflare Worker `nolabs-api` (D1 `nolabs`, R2 `nolabs-media`, Queue `nolabs-render`, auth email hook) → private ZeroGPU Space `ibrahimar397/nolabs-engine` running Kokoro-82M on CPU and Ming-3.0-tts-0.5B on GPU (about 4.5 min/day, split per user).

## Current state
See `notes/10-progress.md`. Phase 1 (accounts) is built and applied to Supabase; the Worker with it is not deployed yet.

## Waiting on the user
- OAuth credentials for Google / Microsoft / Apple (Apple needs a paid developer membership), then `cargo xtask auth-provider …`.
- 20-minute screen-off test on the phone (check A13).

## Access
- Credentials: `~/.config/streams/*.env` (Cloudflare Global key, GitHub, HF write token, Kaggle, Supabase management token, Brevo) and `~/.config/streams/nolabs-r2.env`; app secrets in `~/.config/nolabs/` (`cloudflare-deploy.env`, `app.env` with `AUDIO_URL_KEY`, `supabase.env` with URL, DB password and `SEND_EMAIL_HOOK_SECRET`, optional `oauth.env`). Never copy values into the repo or notes. The Supabase service-role key is fetched at runtime by xtask and never stored or deployed.
- Supabase: `cargo xtask <supabase-migrate|supabase-auth|supabase-owner|supabase-check|invite|auth-provider>`; live schema check `python3 -I supabase/tests/live_rls_check.py`. Never touch project `emvyjhleyhimxxdqybsn` (Streams).
- Deploy: `cargo xtask <bootstrap-token|setup|migrate|build|deploy|space|smoke|status>`. No Node.
- GPU budget: a free account gets 5 GPU-min/day and maybe 3 runs; every Ming test costs a run. Check `notes/08-measurements.md` for runs used.
