# Accounts, credentials and local environment

Reference. Which accounts the build uses, what already exists in them, and where credentials live. No secret values are recorded here.

## 2026-10-30

### Credentials (location only)
All in `~/.config/streams/` on the Mac, mode 600, one `.env` file per service. Never copied into the project.
- `cloudflare.env`: email + Global API Key, zone id, account id. Full account access.
- `github.env`: fine-grained token, all repositories, all permissions.
- `huggingface.env`: HF token, role `write`.
- `kaggle.env`: Kaggle API token.
- Also present but not used by this project: brevo, resend, mail, supabase, tmdb, colab_job.

### What exists in each account (verified via API, 2026-10-30)
- Hugging Face: user `ibrahimar397`, not PRO, cannot pay. One Space: `ibrahimar397/streams-engine` (Gradio, `zero-a10g`, running). Slot 2 is free.
- Cloudflare: account "Ibrahimar397@gmail.com's Account"; workers.dev subdomain `ibrahimar397`; 15 existing Workers (Streams project and others); R2 enabled with bucket `pstream-data`; no D1 databases; no KV namespaces; Pages projects `streams-app`, `pstream-frontend`, two school sites; zone `streams.watch` (Free plan, active).
- GitHub: user `Coderia397`, free plan.

### Local machine
- macOS, Intel i7-9750H. Disk: 11 GB free of 183 GB (measured with `df -h ~`). Keep build artefacts small.
- Installed: rustc/cargo 1.98.1, rustup targets wasm32-unknown-unknown, aarch64-linux-android, trunk 0.21.14, worker-build 0.8.7, gh 2.100.0, uv 0.12.15, git 2.50.1, Python 3.9.6 (system).
- Not installed: node/npm, wrangler, ffmpeg, docker, hf CLI.

### Phone (verified over ADB, 2026-10-30)
- Google Pixel 8 (`shiba`), Android 17, SDK 37, Chrome installed. adb: `~/Library/Android/sdk/platform-tools/adb`, USB serial 38151FDJH00ERU.

### Existing resources this project must not touch
Space `streams-engine`, every existing Worker, bucket `pstream-data`, the `streams.watch` zone, existing Pages projects. New resources get a `nolabs-` prefix.

## Open items
- None.
