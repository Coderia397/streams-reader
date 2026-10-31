# Expressiveness: results, product recommendation and implementation plan


> **Correction 2026-10-31 (lead):** the recommendation below ("Standard becomes Kokoro + director") is superseded. The owner listened to the Kokoro and Ming demo clips and judged them not expressive (Kokoro af_heart "sounds good but lacks expression"; the others lack naturalness and quality). The prosody index measures pitch and pace variation, not naturalness or emotion, so it can rate a flat-sounding voice as expressive; it is a filter, not the judge. The director only changes speed and blends and cannot fix that. Decision now: the engine is chosen by the owner's blind listening, from the bake-off models that score above Kokoro with good quality (Chatterbox, IndexTTS-2.5, Higgs; Qwen3-TTS and Fish pending, notes 25, 29, 30, 37, 38). Open: how a GPU model is served within the free budget (queued or overnight renders, or paid GPU).
Finding + decision proposal + plan. It joins the baseline (note 20), Ming on Kaggle (note 32) and the director on Kokoro (note 33) into one answer to the owner's "our voices are not expressive enough".
Conclusion: the gap is mostly in how we drive Kokoro, not in which model we use. Kokoro with the LLM director moves af_heart from 67 % to 87 % of the human readers (EI 0.31 → 0.67), with no character voices. Ming's default preset scores the same as plain Kokoro (65 %). Standard should become Kokoro + director. Ming stays the tier for cloned voices. Kaggle is a queued bulk renderer for Ming books only, not a live path. None of this has been checked by ear yet; the listening pack below is for that.

Filed as note 35, not `20-expressiveness.md` as the task asked: note 20 is already the baseline, and the notes rules ask for the next free number.

## 2026-10-30 (study ran 2026-10-31; dated by the task)

### How everything here was measured
- EI = expressiveness index from `tools/expressiveness/prosody.py measure` with the calibration frozen in `calibration.json` (note 20). It weighs pitch 0.5, energy 0.25 and timing 0.25. 0 % is the F0-flattened af_heart control and 100 % is three LibriVox readers matched by genre. Status: verified on real output. That the index tracks what a listener hears is assumed and has not been checked.
- Passages: `tools/expressiveness/passages/{1-suspense,2-dialogue,3-reflective}.txt`, 656 / 610 / 633 chars. Each EI below is the mean of the three passages.
- Kokoro plain: live API renders (note 20). Kokoro + director: local Kokoro 0.9.4 at the same weights revision. af_heart is the mean of seeds 1-3; bm_george is seed 1 (note 33). Ming: private Kaggle T4, seed 1234, one run (note 32).

### Results (verified unless marked)
| condition | EI | % of human | suspense | dialogue | reflective | source |
|---|---|---|---|---|---|---|
| Kokoro af_heart plain (app today, live API) | 0.32 | 68 | 0.60 | 0.39 | -0.02 | note 20 |
| Kokoro af_heart plain (local, 3 seeds) | 0.31 (sd 0.03) | 67 | 0.43 | 0.51 | 0.00 | note 33 |
| Kokoro af_heart + director, own voice (a_blend) | 0.67 (sd 0.05) | 87 | 0.67 | 0.81 | 0.52 | note 33 |
| Kokoro af_heart + director + character voices (a_chars) | 0.90 (sd 0.02) | 101 * | 0.67 | 1.52 | 0.52 | note 33 |
| Kokoro bm_george plain | -0.57 | 16 | -0.60 | -0.56 | -0.55 | note 33 |
| Kokoro bm_george + director (a_blend) | 0.27 | 65 | 0.16 | 0.48 | 0.18 | note 33 |
| Ming US_Emma, paragraph IP (as the app sends it) | 0.28 | 65 | 0.32 | 0.28 | 0.24 | note 32 |
| Ming US_Emma, emotions + tags per sentence | 0.36 | 70 | 0.42 | 0.42 | 0.26 | note 32 |
| Ming clone of af_heart, emotions + tags | 0.51 | 78 | 0.08 | 1.14 | 0.31 | note 32 |
| Ming UK_George, paragraph IP | 1.00 ** | 106 | 0.81 | 1.38 | 0.81 | note 32 |
| LibriVox readers | 0.89 | 100 | 1.03 | 0.47 | 1.17 | note 20 |

\* Inflated. Character voices widen the dialogue pitch-level spread to 5.1 st, against 1.67 st for the human reader. On the two narration passages a_chars equals a_blend.
\*\* Assumed inflated. There are 8.2 F0 jumps over 4 st per 1,000 voiced frames, against 0.3 for Kokoro bm_george, and the slope sd is 6.5-7.7 st/s against 3.9 for the readers. That pattern fits creaky phrase endings. Not yet checked by ear.

Where the gain comes from (af_heart, 3 seeds, Δ EI against plain; note 33):
- One call per sentence: +0.11.
- Director pauses: +0.08.
- Speed of 4-12 % per sentence: +0.06.
- Partner-voice blend: +0.11.
- Character voices: +0.23, almost all of it on dialogue.

Pause variety, which note 20 found to be the biggest gap: the ln-pause sd is 0.38 with fixed pauses and 0.46-0.47 with director pauses. The readers are at 0.58. Ming with fixed pauses is at 0.32-0.39. Pauses are set by our pipeline, not by the model.

Rejected in note 33, with the measurements there:
- Emphasis by splitting the sentence: -0.03, and it caused the only content WER error.
- Duration emphasis: +0.01.
- In-call speed and F0 hooks: lower on narration, and 12-17 % of the inserted silences cut into sound.

Artefacts (note 33, `director/artefacts.py`, `asr_check.py` with Parakeet TDT 0.6B v3):
- Clipping: 0 samples in any condition.
- Joins between calls: 7.3-8.7 dB log-mel distance, the same as Kokoro's own sentence gaps (8.0-8.1).
- WER: 0.0-0.8 %.
- Pitch breaks: 28-31/min on sentence calls against 12-19/min on paragraph calls. Not checked by ear.

### Listening pack (verified: built and sized; not listened to)
- Path: `/private/tmp/claude-501/-Users-ibrahim-code-nolabs-reader/a97fc270-2dc8-4cdf-895f-e925bee528d9/scratchpad/expr/pack/`. It holds 18 WebM/Opus clips (32 kbps mono, 24 kHz, 30-51 s each, 2.84 MB in total) and `index.md`, which gives each clip's per-passage EI and what to listen for.
- Made by `uv run tools/expressiveness/make_pack.py --scratch <scratch>/expr --out <scratch>/expr/pack`. It joins the existing renders and re-encodes them. It reads EI from `tools/expressiveness/results/` and does no new measurement or rendering.
- Six clips per passage:
  1. Kokoro af_heart plain (live API).
  2. Kokoro af_heart with the recommended director mapping (a_chars, seed 1).
  3. Ming US_Emma IP.
  4. Ming US_Emma with emotions and tags.
  5. The highest EI measured on that passage. That is Ming UK_George IP for suspense and reflective. For dialogue it is Kokoro bm_george a_chars at 1.97, leaving out the rejected in-call hook path.
  6. 30 s of the LibriVox reader. It is a different text and is there only to calibrate the ear.

### Recommendation for the product (proposal, by the assistant; the owner decides)
1. **Standard voice (default) = Kokoro + director, recommended mapping** (`tools/expressiveness/director/kokoro_map.py` RECOMMENDED):
   - One Kokoro call per sentence. A quote and its speech tag are separate calls, with the tag read neutral 150 ms after the quote.
   - `speed` = 1 ± (4 % + 8 % × intensity). The director's pace sets the direction. When pace is normal and intensity ≥ 0.4, the emotion sets it.
   - The director's `pause_after_ms` replaces the fixed 250/600 ms. Section breaks keep 1,200 ms as a floor (assumed; not tested).
   - Partner blend at weight 0.45 × intensity when intensity ≥ 0.3. Neutral sentences are never blended.
   - Character voices on fiction dialogue as a user setting, default on. This depends on the listening check: the EI gain here is inflated, and the real benefit is telling speakers apart, which nothing has measured yet.
   - When the director is unavailable or out of budget, keep the per-sentence path with boundary pauses (k_sent, +0.11, free), not today's paragraph calls.
2. **Ming:** keep it for cloned and designed voices, which Kokoro cannot do. Do not market it as "more expressive":
   - US_Emma scores 0.28 against 0.67 for Kokoro + director.
   - The ZeroGPU budget is ~4.5 min of audio/day (note 08).
   - Feed the same director output into Ming's EMOTION mode. On the af_heart clone that gave +0.37, mostly on dialogue (+0.72) with suspense -0.16, one seed.
   - UK_George could become a Ming preset with "expressive" in its name if the creak check passes. Pending listening.
3. **Kaggle:** yes as a queued bulk renderer for Ming, no for anything live.
   - Speed: a T4 runs at RTF 2.24, 0.143 GPU-s/char (note 32, 294 items). At the assumed ~30 GPU-h/week quota that is ~13 h of audio/week on one T4, or ~27 h with one worker per GPU (assumed).
   - That is about one 10-hour book a week. It fits a user's "render this book expressively" request and the first chapter of catalogue titles. It does not fit the whole 66-title Explore catalogue: at ~10 h each that is ~660 h of audio, or ~50 weeks (assumed).
   - Limits: the account allows 2 batch GPU sessions at once, shared with experiments (verified 2026-10-31, refused 05:51-06:43). The quota is per account and there is no service level. So it is opt-in: the user sees a queue position and an ETA, never a live stream.
4. **Director model and cost:**
   - Model: `@cf/google/gemma-4-26b-a4b-it` on Workers AI, JSON schema output, PROMPT_VERSION 1.
   - Measured cost: 190 neurons for 1,899 chars, 0.10 neuron/char (`usage` in `director/out/*.json`, one call per passage).
   - Free budget: 10,000 neurons/day is ~100k chars/day, or ~101 min of directed audio per day across all users. Kokoro reads 16.2-17.0 chars/s on these passages: 656 chars in 39 s, 610 in 36 s, 633 in 39 s.
   - Over the free tier: $0.011 per 1,000 neurons on Workers Paid (verified, [Cloudflare pricing](https://developers.cloudflare.com/workers-ai/platform/pricing/), 2026-10-31). A 600k-char book is ~60k neurons, about $0.55 past the free part. Workers Paid itself costs $5/month (assumed, not re-checked).
   - Latency: 24.6-64.9 s per ~630-char call, and dialogue (64.9 s) is slower than its own playback (36 s). Direction must run ahead of playback, not on the play tap.
   - Proposed policy: direct at import time for the first ~20k chars of each document (~2k neurons), then keep a window ahead of the playback position.
   - Per-user cap: a share of the daily neurons, as `core::budget` does for GPU seconds.
   - Cache: in R2 by sha256(prompt version, model id, window text), so a re-read costs 0.
   - Fallback model to measure: `@cf/qwen/qwen3-30b-a3b-fp8` (note 03).

### Implementation plan (open; owners in brackets; acceptance checks numbered)
Order: D1 → D2 → D3 → D4. D5 is independent and comes after the owner says yes to Kaggle bulk.

**D1. Director schema and validation (`crates/core`, new `src/director/`).**
- Types: `Direction`, a passage-level struct with genre, characters and quote → speaker, and `SentenceDirection`. They mirror `tools/expressiveness/director/schema.json`, minus `emphasis_words`, which is dropped.
- `validate(text, sentences, raw) -> Result<Direction, DirectorError>`, ported from `director.py validate`:
  - one entry per sentence from `text::sentence`;
  - clamp intensity to [0, 1] and confidence to [0, 1];
  - `pause_after_ms` within [80, 2,000];
  - 2-sentence hysteresis on narrator emotion;
  - tags anchored within ±3 word indices, and dropped in nonfiction.
- `kokoro_plan(direction, narrator) -> Vec<KokoroSentencePlan {char_range, voice_mix: [(id, weight)], speed, pause_after_ms}>`. It ports `kokoro_map.RECOMMENDED`, with the partner and character tables in `voices.rs`.
- Cache key: add `direction_fingerprint` to `cache_key::KeyInput` (sha256 of the canonical `KokoroSentencePlan`s) and bump `KEY_VERSION` to 2.
- Checks:
  - C1: the shared fixtures in `contracts/fixtures/director/` (the three study passages with their cached gemma output) give the same plan as `kokoro_map.py`, field by field.
  - C2: malformed LLM output (wrong sentence count, out-of-range numbers, unknown emotion) gives a typed error or a clamped value and never a panic. This is property-tested.
  - C3: the same text and plan give the same key, and any plan change gives a new key.

**D2. Director calls and cache (`crates/server` services, `crates/worker` adapters).**
- New `services/director` that uses the existing `Http` port. It calls Workers AI REST, or the AI binding through a new port.
- Windows of about 600 chars, aligned to paragraph ends, with the previous window's characters passed in for consistency.
- Each result goes to R2 at `director/v1/<sha256>.json` with `usage` recorded.
- Where it runs:
  - at document import, for the first 20k chars, as a Queue job;
  - from the consumer, ahead of the playback position.
- A render request for a segment with no direction yet goes out at once on the undirected sentence path, so a direction miss never blocks playback.
- Daily neuron budget in D1, with a per-user share. When it runs out, fall back to undirected.
- Checks:
  - C4: a second import of the same document makes 0 Workers AI calls.
  - C5: when the budget is out, renders still succeed, keyed as undirected.
  - C6: neuron usage per 1k chars is logged, and stays ≤ 0.12 on a 20k-char test document (study: 0.10).
  - C7: time to first audio after Play is unchanged against today, because the first segment never waits for the director.

**D3. Space engine (`space/nolabs_engine`, `crates/core/src/proto/engine.rs` and `contracts.py`).**
- Contract v2: `EngineItem` gets an optional `sentences: Vec<KokoroSentencePlan>`. When absent, today's behaviour stays.
- `kokoro_engine.synthesize` takes `speed` and a voice mix. The blend is a weighted mean of the [510, 1, 256] packs, cached per mix.
- Each sentence is a separate pipeline call, trimmed, with `pause_after_ms` of silence appended. Word timings are shifted, and the item's trailing pause is the last sentence's `pause_after_ms`.
- Segment levelling stays per item (`audio/pipeline.py`). Bump `ADAPTER_VERSION`.
- Checks:
  - C8: with no `sentences`, output is byte-identical to v1 (fixture test).
  - C9: a 3-passage replay through the Space gives EI ≥ 0.60 for af_heart, measured with `prosody.py measure` and no `--calibrate` (study: 0.67 local).
  - C10: word timings stay monotonic and inside the clip. The player highlight test passes.
  - C11: the Space CPU RTF on per-sentence items stays within 20 % of paragraph items (open: not measured on the Space; note 08 has 2.5× realtime for paragraphs).
  - C12: 0 clipped samples, WER ≤ 1 % with `asr_check.py`.

**D4. Ming with direction (`crates/server` render plan, `space/nolabs_engine/engines/ming_plan.py`).**
- The direction's emotion and intensity map to `Mode::Emotion` on reference voices when intensity ≥ 0.4 (threshold assumed). Tags only go on reference voices.
- Every clip gets a silence check: peak < 0.02 means retry with another seed. This is the UK_George failure from note 32.
- Checks:
  - C13: no silent clip reaches R2.
  - C14: no extra ZeroGPU seconds per char against IP. Measure on Kaggle, not ZeroGPU.

**D5. Kaggle bulk job (new `tools/bulk/` or `crates/server` cron plus a driver; reuses `tools/expressiveness/ming/ming_kaggle.py` and `media/music`).**
- Flow:
  - The cron picks queued "expressive book" jobs.
  - It writes the items and director plans to a private dataset and pushes the private T4 kernel, running one worker per GPU.
  - It polls and pulls, uploads WebM and sidecars to R2 under the normal cache keys, and marks the segments ready.
- Admission:
  - hold ≤ 1 of the 2 GPU sessions for the product;
  - keep a weekly GPU-hour ledger in D1 (the database);
  - refuse a book that does not fit the remaining week, and show the ETA.
- Checks:
  - C15: one 20k-char test chapter renders end to end, and the app plays it from R2 with word timings.
  - C16: measured GPU-s per char is within 10 % of 0.143.
  - C17: a failed or timed-out kernel retries once, and never leaves segments half marked.
  - C18: Kaggle credentials stay in the server's secrets only.

## Open items
- Owner listens to the pack, ranks clips 01-05 per passage and judges the blends, the pitch breaks on sentence calls, and whether character voices help. This unblocks shipping character voices by default and settles whether the EI can stand in for the ear.
- The UK_George creak check (clip 05, suspense and reflective). Unblocks a possible "expressive George" Ming preset.
- The other Kaggle bakeoffs: Qwen3-TTS (25), Chatterbox (28), IndexTTS-2.5 (29), Fish (30) and Higgs (31) are still "pending run". One of them could beat Kokoro + director, and then the Standard recommendation above is revisited. Unblocked by those kernels finishing.
- Ming seed variance has not been measured, so Ming differences under ~0.2 EI are unreliable. Unblocked by 3 seeds × conditions, ~40 min on a T4.
- Director policy: the 20k-char import window and the per-user neuron share are proposals. Unblocked by an owner decision, and by whether to enable Workers Paid ($0.011/1k neurons).
- A rule-based fallback director (punctuation pauses, quote detection) has not been tried. It could recover part of the +0.08 pause gain at no neuron cost. Unblocked by one local run with `render_local.py`.
- The cheaper director model (`qwen3-30b-a3b-fp8`) is unmeasured for cost and quality. Unblocked by one run over the three passages.
- C11, the Space CPU speed of per-sentence calls. Unblocked by D3 on a staging Space.
