# Architecture

Plan. A layered reader: a Rust web app on the phone, a Rust Cloudflare Worker as the always-on backend, and a thin Python Space on ZeroGPU that only runs the speech models. **Current: version 2 (first section).** Version 1 is kept below it, superseded by v2.

## 2026-10-30 — version 2 (current)

Produced from the five-lens review of v1 (product, feasibility, software, audio, security; raw: [raw/2026-10-30-arch-review.md](raw/2026-10-30-arch-review.md)). Panel rating: v1 4.5/10, v2 projected 7/10. Everything in v1 not changed here still holds.

### What changed and why
| # | v1 | v2 | Reason |
|---|---|---|---|
| 1 | Synchronous render for Kokoro, queue for Ming | One async path: `POST /api/render` claims cache misses in D1 (`INSERT … ON CONFLICT DO NOTHING RETURNING`) and enqueues them; a Queue consumer (15 min wall) calls the Space; the app fetches `/api/audio/{key}` with backoff | A phone request dies with the screen off; `waitUntil` only gives 30 s |
| 2 | Ogg Opus, one `<audio>` switching `src` | WebM Opus; playback through MediaSource (`SourceBuffer` mode `sequence`) for a gapless timeline per document; src switching only as fallback | Chrome MSE does not accept Ogg; gaps and background `play()` failures on One UI |
| 3 | −18 LUFS per segment | One static gain per render batch toward −16 LUFS, then at most ±2 dB (Ming) / ±4 dB (Kokoro) per segment, sample peak ≤ −1.5 dBFS | Per-segment normalisation flattened dynamics (correction 3 in note 02) |
| 4 | Clips keep model silence | Space trims leading/trailing silence (−45 dBFS, 30 ms guard, 5 ms fades) and appends a fixed pause by boundary type (sentence / paragraph / section) | Uneven rhythm at every clip boundary |
| 5 | 150–600 chars per segment | Kokoro 120–320 chars, never splitting a sentence; Ming one paragraph ≤ 500 chars | Kokoro rushes above ~400 tokens |
| 6 | Worker parses imports | App parses everything (EPUB, HTML via dom_smoothie, MD, TXT, PDF via pdf.js); Worker only fetches raw HTML (SSRF rules below); segment index stored as one R2 JSON per document | 10 ms CPU limit |
| 7 | Loose cache key | Versioned, length-prefixed key over engine, model revision, voice fingerprint (preset id or sha256 of reference + transcript), mode, emotion, tags, normaliser/segmenter versions, boundary pause, codec and loudness profiles, text. Computed in Rust (`crates/core`) on Worker and app; speed excluded | Stale audio after re-recording or model updates |
| 8 | Free-form GPU calls | GPU queue `max_retries=0`; job row + deterministic output keys written before the call; Space writes `jobs/{id}.json` marker; "runs limit" / "quota" errors set `deferred` with `retry_at`; GPU ledger and a pure budget function in core cap requests at 280 GPU-s/day | Each retry can burn one of ~3 daily runs |
| 9 | Ming "IP voices with emotion variants" | Modes `ip` (preset, neutral), `tts` (reference + transcript, optional tags), `emotion` (reference + emotion only). English presets US_Emma, US_James, US_Stella, UK_George, UK_Charlotte have no variants; emotion on a preset uses an anchor clip rendered once with `ip` | Ming rejects controls from other tasks (verified in `tts_tasks.py`) |
| 10 | Progress unspecified | Progress = (doc, char offset, updated_at); IndexedDB every 5 s, synced on pause / hidden / 30 s; resume one sentence back | Seconds differ per voice and engine |
| 11 | Player = play/pause | Speed 0.5–3x on the client (`playbackRate`, pitch kept), ±15 s, paragraph prev/next, sleep timer (15/30/45/60, end of chapter, 10 s fade), tap a sentence to play from it, "Continue" card, rolling offline cache of every fetched clip, share target, first-run screen with Samsung battery steps | ElevenReader parity |
| 12 | Clone: record 20–60 s + Whisper | Guided script read line by line, getUserMedia with DSP off, WAV through an AudioWorklet, level meter, clipping/noise checks; the script is the transcript (no Whisper); Space keeps the best 10–18 s sentence-aligned span | Ming caps references near 20 s; browser DSP smears timbre |
| 13 | Global API Key in tooling, passphrase unspecified | xtask mints scoped tokens from the Global key once; refuses to mutate resources without the `nolabs-` prefix. Generated 128-bit passphrase, HMAC compare, D1 lockout after 20 failures/hour, `__Host-` cookie with server-side expiry and a revocation epoch, no CORS, same-origin check on writes, strict CSP, imported content rendered as text only, pdf.js pinned with `isEvalSupported:false` | Account holds 15 production Workers and the Streams data |
| 14 | Keepalive only | Cron every 30 min: Space runtime check and restart, lease recovery, deferred GPU jobs; R2 lifecycle deletes `audio/kokoro/` after 90 days | Space sleeps after 48 h, rebuilds wipe disk |
| 15 | All 28 Kokoro voices | Curated 9 (af_heart default, af_bella, af_nicole, bf_emma, am_michael, am_fenrir, am_puck, bm_george, bm_fable), rest under "More"; text normaliser drops `[12]` citations, footnote marks, turns URLs into domains, joins hyphenated line ends | Low-grade voices and read-out citations sound amateur |

### Deferred to phase 2 (decision D6)
LLM director, Whisper ASR check, MeloTTS fallback, Kaggle bulk renderer, word timings for Ming, pronunciation dictionary, bookmarks with notes, whole-document download button (the rolling cache and pre-render on import cover most of it), Workers Static Assets upload session (tonight the web build is embedded in the Worker).

### Code layout v2
```
crates/core     pure: proto (HTTP DTOs, engine contract v1, ApiError), text (normaliser, segmenter), cache_key,
                engine_error (classifier for ZeroGPU strings), budget (GPU ledger), job (claim/lease state machine)
crates/import   EPUB (zip + quick-xml), HTML (dom_smoothie), Markdown, plain text → core::text (web only)
crates/server   application services over traits Repo, Blob, Engine, Queue, Clock; fakes + rusqlite repo running the real migrations
crates/worker   workers-rs adapters (D1, R2, Queue, Space client over Gradio SSE), router, auth, cron, embedded web build
crates/web      Leptos 0.8 CSR app; player state machine in Rust; interop/*.js plain ES modules (MSE player, Media Session, recorder worklet, pdf.js)
xtask           tokens, D1 migrations, R2/queue setup, Worker upload, Space upload, secrets, smoke test
space/          nolabs_engine (contracts, engines, audio, storage, service) + vendor/ming3 + tests
migrations/     NNNN_name.sql, tracked in a _migrations ledger
contracts/      JSON fixtures shared by Rust and pytest tests
```

### Milestones
- A: import (paste, URL, EPUB, TXT/MD, share), Kokoro playback through the async path, gapless MSE, speed, skip, sleep timer, resume, rolling cache, first-run screen. Verified on the deployed site in Chrome; the 20-minute screen-off check needs the user's phone.
- B: clone flow, Ming rendering (preset, cloned, emotion), GPU ledger and quota UX.
- C (phase 2): director, Kaggle bulk, ASR check, MeloTTS, word timings for Ming.

### Assistant's rating of v2 (2026-10-30)
7/10 as designed, the same as the panel's projection. It loses points only on facts that need measurement, not on design: the Space's CPU speed for Kokoro (decides whether long books keep up at 2x), the real ZeroGPU run limit (decides how many minutes a day the cloned voice gets), and Samsung background behaviour (decides whether the web app is enough or a native shell is needed). Each has a fallback recorded above. Rating is revisited in note 08 after measurements.

## 2026-10-30 — version 1 (superseded by version 2)

### Goals (from the chats, in priority order)
1. Replaces ElevenLabs Reader on an Android phone: import anything, press play, listen with the screen off, pick up where you left off.
2. Expressive reading inferred from the text, many good voices, the user's own cloned voice.
3. £0, no card, FOSS models, nothing on the laptop, user owns every piece.
4. Rust wherever possible; quality code with no review burden on the user.
5. Beautiful and professional.

### Layers

```
 Phone (installed web app, Rust/Leptos + small TS glue)
   │  HTTPS, same origin, session cookie
   ▼
 Cloudflare Worker "nolabs-api" (Rust, workers-rs)      ← always on, no cold start
   ├─ Static assets: the web app
   ├─ D1 "nolabs": documents, progress, voices, segments index, jobs, director cache
   ├─ R2 "nolabs-media": document bodies, audio (Ogg Opus), voice reference clips
   ├─ Workers AI: director LLM (gemma-4-26b), Whisper (reference transcripts), MeloTTS (emergency fallback)
   ├─ Queue "nolabs-render": expressive (GPU) render jobs, 15 min consumer
   └─ Cron: keep the Space awake, recover stuck jobs, report quota
   │  HTTPS + HF token (private Space)
   ▼
 Hugging Face Space "nolabs-engine" (ZeroGPU, Gradio 6.17.3, Python 3.10)
   ├─ render_cpu  → Kokoro-82M on CPU   (no GPU quota)       → uploads Opus to R2
   ├─ render_gpu  → Ming-3.0-tts-0.5B on GPU (one call/batch) → uploads Opus to R2
   ├─ design_voice → Ming voice design (GPU)
   └─ voices, health
```

### Engine tiers
| Tier | Engine | Where | Cost | Used for |
|---|---|---|---|---|
| Standard | Kokoro-82M, 28 English voices (54 total) | Space CPU | none | Default narration of everything |
| Expressive | Ming-3.0-tts-0.5B + LLM director | Space GPU | 5 GPU-min/day (maybe 3 runs) | Your cloned voice, emotional fiction, voice design, previews |
| Fallback | MeloTTS | Workers AI | about 19 neurons per audio minute | Space asleep or broken; app never goes silent |
| Bulk expressive (phase 2) | Ming-3.0-tts-0.5B | Kaggle T4 job | about 30 GPU-h/week | Whole books in the cloned voice, rendered overnight |

### Data flow: press play
1. The web app imports the document (paste, URL, EPUB, PDF, TXT, Markdown, HTML) and segments it locally in Rust (`nolabs-text`): paragraphs → sentences → segments of 1–4 sentences, 150–600 characters, so each clip is over 5 s (Android media notification) and under the engines' limits.
2. The app uploads the document (title, source, segments) to the Worker. Body goes to R2, metadata and the segment index to D1.
3. On play, the app asks for a window of segments: `POST /api/render {doc, voice, from, count}`.
4. The Worker computes each segment's cache key `sha256(engine, voice, settings, normalised text)`. Hits return immediately. Misses for the Standard tier are sent to the Space `render_cpu` in one call; the Worker waits on the SSE stream and records the manifest (key, duration, word timings) in D1.
5. The app plays `GET /api/audio/{key}` (R2 stream with Range support) on one audio element, highlights the current word or sentence, and asks for the next window when two segments remain.
6. Expressive tier: `POST /api/render` with an expressive voice creates a job; the Queue consumer runs the director on Workers AI (structured JSON: emotion label + paralinguistic tags at word offsets, validated so the text never changes), then calls `render_gpu` with the whole batch in one GPU call. The app shows progress and plays when ready.

### Audio
- Space encodes Ogg Opus 24 kHz mono, 48 kbps, with ffmpeg (preinstalled), after a static gain to −18 LUFS per segment (pyloudnorm). Static gain only, so the dynamics inside a segment stay; no `loudnorm` compression (correction 3 in note 02).
- Space writes audio straight to R2 over the S3 API, so the Worker never handles audio bytes (10 ms CPU limit).

### Voices
- Catalogue = Kokoro presets + Ming built-in voices (with emotion variants) + the user's voices.
- Clone: record 20–60 s in the app (guided script with emotional range) → R2 → Whisper transcript (Workers AI) → editable → saved voice `{ref_key, transcript}`.
- Design: describe a voice → Ming VOICE_DESIGN renders a 15 s sample in one GPU call → saved as a reference clip, then used like a clone.

### Security
- Single user. Passphrase → HMAC-signed session cookie (HttpOnly, Secure, SameSite=Strict, 180 days). Every `/api` route except login requires it.
- Secrets: HF token, R2 S3 keys, session key live only in Worker secrets / Space secrets. The Space is private.
- The Worker's URL fetcher (for article import) is authenticated and refuses private-network addresses.

### Code layout (Cargo workspace + Space package)
```
crates/core     shared types, DTOs, ids, cache keys, errors (no I/O)
crates/text     normalisation, segmentation, importers (EPUB/HTML/MD/TXT), director span application
crates/worker   Cloudflare Worker: routes, D1 repo, R2 store, Space client (Gradio SSE), queue consumer, cron
crates/web      Leptos app: pages, components, state, api client; interop/ (TS: player, media session, pdf.js)
xtask           build + deploy: Cloudflare API (D1 migrations, R2, queue, secrets, assets, script), HF Space upload, smoke tests
space/          Python package nolabs_engine (engines, audio, storage, contracts) + vendored Ming
migrations/     D1 SQL migrations
notes/          this folder
```

### Testing and verification
- Rust: unit tests in core/text/worker logic (segmentation, cache keys, SSE parser, director validation, auth); `cargo clippy -D warnings`; wasm builds for worker and web.
- Python: pytest for contracts, audio and storage with engines faked; smoke test against the live Space.
- End to end: xtask `smoke` drives the deployed API (login → import → render → fetch audio → duration check); browser test in Chrome on the deployed site.
- Independent review agents read every component before deploy.

### Phases
1. Tonight: Space (Kokoro + Ming), Worker, web app, deploy, end-to-end on the live URL.
2. Next: Kaggle bulk renderer, offline downloads, word-level highlighting for Ming (via Whisper alignment).
3. Later: native Android shell if web playback with the screen off proves unreliable on the user's phone.

## Open items
- Review panel scores and v2 changes: pending (this note, next section).
- Product name and subdomain: the user decides.
