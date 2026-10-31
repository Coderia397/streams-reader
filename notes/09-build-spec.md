# Build spec: interfaces between components

Reference for every implementation agent. Each component owns its own directory; these are the only cross-component agreements. Architecture: [06-architecture.md](06-architecture.md) v2. Contracts in code: `crates/core/src/proto/{api,engine,error}.rs` (source of truth; do not change field names without updating every consumer).

## 2026-10-30

### Names
- Product working name: **nolabs** (display name in one constant `APP_NAME` in the web crate; the user picks the final name later).
- Cloudflare: Worker `nolabs-api`; D1 database `nolabs`; R2 bucket `nolabs-media` (exists); Queue `nolabs-render`; cron `*/30 * * * *`.
- Hugging Face: Space `ibrahimar397/nolabs-engine` (exists, private, ZeroGPU), base URL `https://ibrahimar397-nolabs-engine.hf.space`.
- Local credentials (never in the repo): `~/.config/streams/{cloudflare,huggingface,github,nolabs-r2}.env`; new files go to `~/.config/nolabs/` with mode 600.

### Worker bindings and secrets (xtask creates, worker reads)
| Name | Kind | Value |
|---|---|---|
| `DB` | D1 | `nolabs` |
| `MEDIA` | R2 | `nolabs-media` |
| `RENDER_QUEUE` | Queue producer | `nolabs-render` (the same Worker is the consumer; `max_retries` 0, `max_batch_size` 1, `max_batch_timeout` 0) |
| `SPACE_URL` | var | `https://ibrahimar397-nolabs-engine.hf.space` |
| `SPACE_REPO` | var | `ibrahimar397/nolabs-engine` |
| `KOKORO_REV` | var | `hexgrad/Kokoro-82M@<commit>` |
| `MING_REV` | var | `inclusionAI/Ming-3.0-tts-0.5B@<commit>` |
| `APP_VERSION` | var | git short sha |
| `HF_TOKEN` | secret | token used for `Authorization: Bearer` to the Space and the HF runtime API |
| `SUPABASE_URL` | var | `https://atxbmgfuxjtcwmoaldqy.supabase.co` |
| `SUPABASE_ANON_KEY` | var | the project's publishable key (public) |
| `SITE_URL` | var | `https://listen.streams.watch` (`nolabs_core::brand::SITE_URL`) |
| `AUDIO_URL_KEY` | secret | 64 hex chars (32 random bytes), signs `?t=` audio tokens; generated into `~/.config/nolabs/app.env` |
| `SEND_EMAIL_HOOK_SECRET` | secret | `v1,whsec_…` (Standard Webhooks), from `~/.config/nolabs/supabase.env` |
| `BREVO_API_KEY` | secret | from `~/.config/streams/brevo.env` |
| `SUPABASE_JWT_SECRET` | secret, optional | legacy HS256 secret, only if the project signs sessions with it (it does not: ES256) |

Superseded 2026-10-30 by note 15: `SESSION_KEY` and `PASSPHRASE_HASH` (passphrase login removed; `deploy` deletes them).

### Audio
- Container/codec: WebM Opus (`audio/webm; codecs="opus"`), mono. Kokoro 24 kHz source, 32 kbps, profile id 1. Ming 44.1 kHz source resampled to 48 kHz (soxr), 56 kbps, profile id 2. Both: `-application audio -vbr on -frame_duration 20`.
- Loudness: one batch gain toward −16 LUFS, per-segment correction ≤ ±4 dB (Kokoro) / ±2 dB (Ming), sample peak ≤ −1.5 dBFS. No limiter or compressor.
- Silence: trim leading/trailing below −45 dBFS keeping 30 ms, 5 ms fades, then append pause: sentence 250 ms, paragraph 600 ms, section 1200 ms (Kokoro); Ming the same.
- R2 layout: `audio/{kokoro|ming}/{key}.webm` + sidecar `audio/{engine}/{key}.webm.json` (an `EngineItemResult`); `jobs/{job_id}.json` (full `EngineResponse`, written last); `docs/{doc_id}/body.json`; `voices/{voice_id}/raw.wav`, `voices/{voice_id}/ref.wav`; `previews/{voice_id}.webm`.

### Space endpoints (Gradio api_name; one JSON string in, one out)
`render_cpu` (Kokoro, concurrency id `cpu`, limit 1), `render_gpu` (Ming, concurrency id `gpu`, limit 1), `prepare_reference` (CPU, concurrency `cpu`), `health` (no queue concurrency limit, never touches `@spaces.GPU`). Request/response types: `crates/core/src/proto/engine.rs`. Errors are raised as `gr.Error("bad request: …")` or `gr.Error("engine error: …")`; ZeroGPU limit errors pass through with their original text so `EngineError::classify` can read them.

### Import crate API (`crates/import`, used by the web app)
```rust
pub struct Imported { pub title: String, pub author: Option<String>, pub blocks: Vec<nolabs_core::proto::api::Block> }
pub enum ImportError { Empty, TooLarge(&'static str), Malformed(String), Unsupported(String) }  // thiserror
pub fn from_text(text: &str, title_hint: Option<&str>) -> Result<Imported, ImportError>;
pub fn from_markdown(md: &str, title_hint: Option<&str>) -> Result<Imported, ImportError>;
pub fn from_html(html: &str, url: Option<&str>) -> Result<Imported, ImportError>;   // readability via dom_smoothie
pub fn from_epub(bytes: &[u8]) -> Result<Imported, ImportError>;                     // zip + quick-xml, spine order
pub struct PdfLine { pub text: String, pub y: f32, pub height: f32 }
pub fn from_pdf_pages(pages: &[Vec<PdfLine>], title_hint: Option<&str>) -> Result<Imported, ImportError>;
```
Blocks come out already passed through `nolabs_core::text::normalise_blocks`.

### Player crate API (`crates/player`, wasm only, used by the web app)
```rust
pub struct Track { pub segment: u32, pub bytes: js_sys::Uint8Array, pub duration_ms: u32,
                   pub words: Vec<nolabs_core::proto::api::WordTiming>, pub ends_section: bool }
pub type Provider = Rc<dyn Fn(u32) -> LocalBoxFuture<'static, Result<Track, PlayerError>>>; // resolves when audio is ready
pub struct MediaInfo { pub title: String, pub artist: String, pub album: String, pub artwork_url: Option<String> }
pub enum Sleep { Off, After { ms: u32 }, EndOfSection }
pub enum PlayerState { Idle, Loading, Buffering, Playing, Paused, Ended, Error(String) }
pub struct Snapshot { pub state: PlayerState, pub segment: u32, pub segment_pos_ms: u32, pub word: Option<usize>,
                      pub buffered_ahead_ms: u32, pub rate: f64, pub sleep_left_ms: Option<u32> }
pub enum PlayerEvent { Snapshot(Snapshot), SegmentChanged(u32), Ended, SleepFired, Error(String) }
impl Player {
  pub fn new(provider: Provider, on_event: Rc<dyn Fn(PlayerEvent)>) -> Result<Player, PlayerError>;
  pub fn load(&self, info: MediaInfo, segment_count: u32, start_segment: u32);
  pub fn play(&self); pub fn pause(&self); pub fn toggle(&self);
  pub fn seek_by(&self, delta_ms: i32); pub fn jump_to(&self, segment: u32);
  pub fn next(&self); pub fn previous(&self);
  pub fn set_rate(&self, rate: f64); pub fn set_sleep(&self, sleep: Sleep);
  pub fn snapshot(&self) -> Snapshot; pub fn unload(&self);
}
```
Playback: MediaSource + one SourceBuffer `audio/webm; codecs="opus"`, mode `sequence`; ≥ 3 min buffered ahead; Media Session metadata, position state and actions (play, pause, seekbackward/forward ±15 s, previoustrack/nexttrack = previous/next segment, seekto); sleep timer checked on `timeupdate` with a 10 s volume fade; `preservesPitch` on. Fallback when `MediaSource.isTypeSupported` fails: blob URLs on one `<audio>`.

### Web app
Leptos 0.8.22 CSR, trunk. Routes: `/` library, `/read/:id` reader + player, `/voices`, `/voices/new` (clone), `/settings`, `/login`, `/welcome` (first run). Visual direction: see the web agent prompt; display name from `APP_NAME`.

## Open items
- None.
