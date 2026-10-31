# 22. Engine speed: per-segment streaming, pipelining, BART fallback

Correction 2026-10-31: the `Programme` claim below ("matches pyloudnorm within 0.01 LU") is superseded by [31](31-engine-review-fixes.md). `Programme` follows BS.1770 using complete blocks only and differs from pyloudnorm by up to about 0.2 LU when pyloudnorm adds a partial trailing block. The gain plan now uses that meter for both terms. Note 31 also covers the explicit close of the ZeroGPU generator on early stop and the Apache-2.0 text for the misaki-derived code.

Finding + decision + reference (engine-speed track, 2026-10-31). The Space now streams one item result per finished segment and synthesises segment N+1 while N is encoded and uploaded; contract v1 is unchanged for a client that reads only `complete`. The Kokoro out-of-lexicon fallback is now the Apache-2.0 BART G2P model (no new packages). Not deployed; the Worker needs `ADAPTER_VERSION` 2 before or with the deploy.

## 2026-10-31

### What changed in `space/` (verified by tests, not deployed)
- `nolabs_engine/streaming.py`: `ordered_pipeline(source, stage, pool, max_ahead)`. The source (synthesis) runs on its own thread inside a copy of the caller's `contextvars` context (so the Gradio request reaches ZeroGPU); the stage (encode + R2 upload) runs on the existing 4-thread pool; results come out in source order. At most `max_ahead` (Service default 2) items are synthesised ahead of the consumer. A source error is raised after the items before it, as the same object (ZeroGPU texts survive). Closing early stops the source and closes it on its own thread.
- `audio/loudness.py`: `Programme`, incremental BS.1770 integrated loudness of clips appended in order (K-weighting state carried across clips, blocks span joins). ~~Matches pyloudnorm on the concatenation within 0.01 LU (test with 4 noise-burst clips).~~ Superseded by 31: it uses complete blocks only, equals `measure_blocks` on the concatenation to 1e-9, and differs from pyloudnorm by up to about 0.2 LU. `measure_blocks` gives the per-clip term.
- `audio/pipeline.py`: `Stream.level` (ordered: trim, measure, add to programme, plan gain) and `Stream.finish` (any thread: gain, pause, encode). `finish_batch` is the sequential form of the same.
- `service.py`: `stream_cpu` / `stream_gpu` yield one `EngineItemResult` per item after its audio and sidecar are in R2, then the `EngineResponse` after the marker. `render_cpu` / `render_gpu` drain them. Log line gains `first_ms`.
- `engines/ming.py`, `engines/ming_gpu.py`: the `@spaces.GPU` function is a generator yielding `GpuAnchor`, `GpuWave(index, wave)`, `GpuDone(gpu_s)`. spaces 0.51.3 supports generator functions (`spaces/zero/decorator.py` picks `generator_function_wrapper` when `inspect.isgeneratorfunction`; verified in the cached wheel source). Still one GPU call per batch; encode and upload now overlap it.
- `handlers.py`, `app.py`: render endpoints are generator functions, so Gradio streams each yield.
- `engines/g2p_fallback.py`: `BartFallback`, adapted from misaki main `FallbackNetwork` (commit fba12365, Apache-2.0); `KokoroEngine` sets it as `pipeline.g2p.fallback` for lang codes a and b. Pinned in `revisions.G2P_REPOS`, preloaded in README `preload_from_hub`, reported by `health` as `model_revs.kokoro_g2p`.

### Decision: loudness is levelled against the programme so far (D35)
- Decision: segment N's batch gain comes from the integrated loudness of segments 0..N of the batch, not of the whole batch. The per-segment ±`max_correction_db` clamp and the peak ceiling are unchanged.
- Reason: per-segment streaming cannot wait for later segments. The first segment is levelled on its own (straight to the target); later ones converge on the old whole-batch value. For a two-clip batch (loud then 20 dB quieter) the old test's numbers still hold to 0.01 dB.
- Rejected: keeping whole-batch loudness (no streaming); a fixed per-segment target with no batch term (loses the whisper-versus-shout spread the batch gain protects).
- Effect: Kokoro and Ming audio for the same items can differ slightly from v1 output, so the Kokoro and Ming cache keys must move. Decided by the assistant under the track delegation; owner may veto.

### Wire format for the Worker owner (contract v1 unchanged)
- `render_cpu` and `render_gpu` SSE: `generating` events, one per item in request order, data `["<EngineItemResult JSON>"]`, the same JSON as the sidecar `<object_key>.json`, sent only after that item's audio and sidecar are written. Then one more `generating` with the full `EngineResponse` (it has `job_id`), then `complete` with the same `EngineResponse`. Gradio's `/call` path sends full values, not diffs (`simple_format=True`; checked in gradio 6.17.3 `routes.py` and `blocks.py`).
- The current Worker (`crates/server/src/gradio/mod.rs`) ignores `generating` and keeps working unchanged.
- To use streaming: in `await_result`, parse each `generating` value; an object without `job_id` is an item result that is already in R2 and can be marked ready and played; the one with `job_id` can be ignored in favour of `complete`.
- An `error` event can now arrive after some items were streamed (for example a ZeroGPU abort mid-batch). Those items are in R2 with sidecars; the marker is not written.
- Cost: the full response is sent twice (last `generating` and `complete`); a 64-item batch is tens of KB.

### BART fallback install check
- No new packages: `transformers==4.46.3` (already pinned for Ming) provides `BartForConditionalGeneration`. `uv pip compile` of `requirements.txt` plus `gradio==6.17.3` for `--python-version 3.10 --python-platform x86_64-manylinux_2_28` resolves to the same pins (gradio 6.17.3, spaces 0.51.3, torch 2.11.0, kokoro 0.9.4, misaki 0.9.4, transformers 4.46.3, huggingface-hub 0.36.2, safetensors 0.8.0). Verified 2026-10-31.
- Models: `PeterReid/graphemes_to_phonemes_en_us` @ `a5631b285d18d59483c32c0c3379cb9fac924f4b`, `_en_gb` @ `d8357d5067fa26a5c34134d6bbcf4bbf000c0ac8` (HF API, both Apache-2.0, `model.safetensors` 3,011,692 bytes each, saved by transformers 4.51.2). 1-layer BART, d_model 128, 64 positions; words longer than 62 characters are left to misaki's `unk`.
- Generation length: misaki main calls `generate` with no length, so the transformers default `max_length=20` applies (at most 18 phonemes). Kept for parity with misaki and the apps (`MAX_LENGTH = 20`); long words can be cut short. Open: decide whether the apps and the Space both raise it to 64.
- espeak-ng stays in `packages.txt` because kokoro 0.9.4 still builds its espeak fallback before we replace it; it is no longer used for English.

### Measurements (verified locally, noisy)
Setup: this Mac (i7-9750H, macOS 26, x86_64), Python 3.12 `uv venv`, kokoro 0.9.4, misaki 0.9.4, torch 2.2.2 (newest macOS x86_64 wheel; the Space has 2.11.0), numpy 1.26.4, transformers 4.46.3, model `hexgrad/Kokoro-82M@f3ff3571`, voice af_heart, 2 torch threads (Space allowance), real ffmpeg (imageio-ffmpeg) encode, R2 replaced by a 150 ms sleep per PUT. Text: `tools/bench/workload.py`, 994 chars in 4 segments (302/289/167/236), 56.45 s of audio. Script: `tools/bench/local_kokoro.py`; before = a copy of `space/nolabs_engine` taken before this change. Interleaved before/after, 2 rounds of 2 runs; other workflows were loading the machine (load average 102 → 7 during the runs, logged per round). Log: [raw/2026-10-31-engine-speed/local-kokoro-bench.log](raw/2026-10-31-engine-speed/local-kokoro-bench.log).

| Round (load avg 1 min at start) | Engine | Time to first item (median of 2) | Total (median) | RTF (total / audio) |
|---|---|---|---|---|
| 1 (9.7 / 11.2) | before | 74.9 s (= total; nothing until `complete`) | 74.9 s | 1.33 |
| 1 | after | 25.4 s | 72.5 s | 1.28 |
| 2 (11.8 / 11.8) | before | 60.5 s | 60.5 s | 1.07 |
| 2 | after | 15.9 s | 52.8 s | 0.94 |

- Time to the first playable segment drops by 66-74 % (it becomes one segment's synthesis plus its encode and upload). Total time drops 3-13 % (encode and the two PUTs per item overlap synthesis). Single runs ranged 41-97 s (after) and 55-81 s (before), so the totals are indicative only; re-measure on the Space.
- Absolute speed here (RTF about 1) is far below the Space (2.5x realtime, note 08) because of the load and the old torch build; the relative change is the result.
- BART: 20 out-of-lexicon words (names, coinages) give byte-identical phonemes from `BartFallback` and from misaki main's `FallbackNetwork` logic on transformers 4.46.3, for both en_us and en_gb (20/20 each). Uncached cost 193-232 ms per word under load (one `generate` per word); results are cached per word. End to end, `KokoroEngine` on both pipelines reports `BartFallback` and phonemises "Zorblax met Daenerys in Ouagadougou." as `zˈɔɹblæks mˈɛt dˈAnəɹiz ɪn wˌɑɡədˈuɡu.` (US) with five word timings. Script and output: [raw/2026-10-31-engine-speed/](raw/2026-10-31-engine-speed/).
- "antidisestablishmentarianism" comes out as `ˌæntidˌɪsəstˌæblɪ`: the 20-token limit cuts it (see the open item).

### Tried and abandoned
- Loading kokoro's espeak fallback on this Mac: `espeakng-loader` aborts the process looking for `phontab` (the same defect as in note 19). The local bench replaces `misaki.espeak` with a stub (`--espeak` turns that off on Linux).

## Requests to other owners
- Worker (`crates/server/src/config.rs`): `ADAPTER_VERSION` 1 → 2, deployed together with this Space build. Reason: BART pronunciations for out-of-lexicon words and programme-so-far loudness change the audio behind existing keys.
- Worker (`crates/server/src/gradio/`): optional streaming consumer as described above.

## Open items
- Before deploying, run `HF_TOKEN=… uv run tools/bench/space_bench.py --runs 3 --label before` on the current build, then `--label after` once the new build is up; add both to this note. Unblock: lead.
- Deploy the Space (not done by this track). Unblock: lead, together with `ADAPTER_VERSION` 2.
- BART `max_length` 20 versus 64 (see above). Unblock: owner or apps track decision, applied to both sides.
- Ming streaming is verified with fakes only; the first live `render_gpu` after deploy confirms ZeroGPU forwards the generator's items. Unblock: deploy plus one GPU run.
