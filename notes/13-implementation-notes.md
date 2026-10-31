# Implementation notes from the build agents

Reference. Decisions, assumptions and facts the seven build agents reported (workflow `wf_598e8a73-8f8`, 2026-10-30), recorded here because each agent could only write its own directory. Conclusion: the components are built and tested natively; most remaining assumptions concern live Cloudflare/HF API shapes and are checked on first deploy.

## 2026-10-30

### Pinned model revisions (read from the HF API on 2026-10-30 by the space agent)
- Kokoro: `hexgrad/Kokoro-82M@f3ff3571791e39611d31c381e3a41a3af07b4987`
- Ming: `inclusionAI/Ming-3.0-tts-0.5B@7247f4c94a5b2e987404ed28a5cfec41b1a4ca61`
Both are the Worker vars `KOKORO_REV` / `MING_REV` and part of every cache key.

### Decisions taken by agents (accepted by the lead)
- core `job::apply`: a Kokoro `LeaseExpired` with attempts ≥ `MAX_CPU_ATTEMPTS` goes to Failed (spec said Queued); prevents endless re-renders.
- core `budget::admit`: applies `max_call_s` to multi-item batches (shrinks to a prefix that fits one call); a single oversized item is admitted at the 180 s cap.
- core normaliser cleans citations, brackets and footnote marks before rewriting URLs, so it is idempotent.
- backend: D1 schema in `migrations/0001_init.sql`; logout bumps the session epoch (signs out every device); failed-segment cooldowns Kokoro 60 s, Ming 1 h, 60 s when the GPU never ran; ledger rows deleted for refused GPU calls; interactive Kokoro job sizes 1, 3, 12 (first audio fast); Ming jobs packed up to 2,866 chars per call. `ADAPTER_VERSION` in `crates/server/src/config.rs` must be bumped when the Space changes how it drives a model.
- player: `timestampOffset` after each sequence-mode append equals the exact segment end (verified in Chrome by the agent); jump/next/previous move in place when the target is buffered, otherwise rebuild the MediaSource; previous restarts the segment if more than 3 s in; the sleep countdown counts playing time only; end-of-section stops 0.9 s into the 1.2 s section pause.
- space: `design_voice` and `catalog` endpoints removed (not in contract v1; voice design needs a contract type later). Emotion items on a Ming preset render a ~150-char anchor clip once per Space process (about +9 GPU-s on that call).
- import limits (`nolabs_import::limits`): text/Markdown/HTML up to 64 MB; EPUB up to 200 MB compressed and uncompressed, 100:1 per-entry ratio cap above 1 MiB; PDF up to 20k pages; DRM EPUBs → `ImportError::Unsupported`. `PdfLine.y` is the pdf.js baseline (`transform[5]`), `height` the font height in the same units, lines in reading order, one Vec per page.

### Assumptions to verify on first deploy (status: assumed)
- D1 `POST /query` with several statements is atomic (migrations rely on it; they use IF NOT EXISTS where possible).
- Cloudflare request/response shapes in xtask (permission group names, queues consumer fields, R2 lifecycle/domains/cors, script secrets/schedules/subdomain) — partly verified: bootstrap-token, setup and migrate succeeded on 2026-10-30 after one fix (Queues answer `"errors": null`).
- HF runtime response has a `sha` field; `hf upload --exclude` syntax in huggingface_hub 0.36.2.
- R2 bucket-scoped token allows HeadBucket (Space `health.storage_ok`).
- soxr resampler present in the Space's ffmpeg (falls back to ffmpeg's own resampler if not).
- Reference preparation thresholds (dynamic range ≥ 12 dB, pauses ≥ 200 ms, 4–30 chars/s) tuned only on synthetic audio.
- `EngineError::classify` Sleeping-by-text rule guessed from typical HF wording.
- PDF and readability heuristics tested on synthetic inputs only.

### Deploy order
`cargo xtask bootstrap-token` → `passphrase` → `setup` → `migrate` → `space` → `build` → `deploy` → `smoke`. Resources created 2026-10-30: D1 `nolabs` (`d6d8cdc1-52d2-4b0b-9ec8-58f1aef1e542`), Queue `nolabs-render` (`99bea8e7efd048dab297a0e827a4cba1`), R2 lifecycle rule `nolabs-expire-kokoro-audio`, deploy token `nolabs-deploy` (expires 2027-01-28). Ids also in the gitignored `deploy/state.toml`.

## 2026-10-31 player round 2 fixes (crates/player only)
Nine defects from the Pixel 8 seek/jump/tracking report fixed; harness 70/70 scenarios pass (verified: `uv run --script crates/player/harness/run.py`, 710 s with the machine at load 13 to 33 from other agents; about 135 s on an idle machine).
- A seek, jump or next whose target is the segment being fetched keeps the build (`seek::is_next_fetch`, `Shared::hold`). Scenario 80: one build, 17 asked for once, playback at 7.25 s (was 11.0 s).
- Segment lengths are kept per document (`seek::Lengths`, recorded on each append, cleared on load/unload), so a skip back across segments already played rebuilds once where the target is. Scenario 91: one rebuild at 15, landed 605 ms later. Skips across segments never fetched still rebuild one segment at a time (each length is unknown until fetched).
- SegmentChanged is sent only for a target whose segment is certain, not for each hop.
- Lock screen: a 50 ms probe after `playing`/`seeked` republishes the standing position until the clock has run with it for 3 rounds (Chrome starts the clock about 250 ms after `playing`, longer under load: verified in the raw logs), plus a refresh on any `timeupdate` that drifted more than 100 ms.
- Play with nothing buffered goes to Buffering; play in Error wakes the fetch loop (checks every 250 ms) and restarts the backoff; end-of-section sleep on the document's last section also sends Ended.
- Fallback (blob) path: one pending source switch per element in `interop/dom.js`; seeks during a switch move its offset; the playhead reads the pending offset. Verified with the scratchpad `fallback.py` probes (all pass).
- Harness noise seen on 2026-10-31 (not player bugs): external pauses of the harness Chrome while other Chrome/ffmpeg jobs ran, and Chrome applying a rate change about 150 ms late (scenario 20 gapless ratio 1.18).

## Open items
- Fallback path on a real iPhone: assumed to be what Safari uses; not tested on a device.
- Each "assumed" line above: verified or corrected by the first deploy and smoke test.
