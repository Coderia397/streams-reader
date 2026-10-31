# 31. Engine review fixes: misaki licence, one loudness meter, explicit generator close

Finding + decision (engine-speed track, 2026-10-31). Five reviewer defects in `space/` are fixed and covered by tests (213 pass). Correction to note 22: `Programme` does not match pyloudnorm within 0.01 LU in general; the gain plan now uses the same complete-block meter on both sides. Not deployed; `ADAPTER_VERSION` 2 (note 22) already covers the audio change.

## 2026-10-31

### 1. Apache-2.0 text for the misaki derivative (verified)
- `space/nolabs_engine/engines/g2p_fallback.py` is adapted from misaki `en.py` (Apache-2.0). The repo had no copy of the Apache License, which section 4(a) requires.
- Added `space/nolabs_engine/engines/LICENSE.misaki`: `Copyright hexgrad` plus the full Apache-2.0 text from hexgrad/misaki `LICENSE` at commit `fba1236595f2d2bf21d414ba6e57d25256afada3` (206 lines). The g2p_fallback.py docstring points to it, and the licence paragraph in `space/README.md` names the file.

### 2. One loudness meter on both sides of the gain plan (verified)
- Wrong belief (note 22): `Programme` matches pyloudnorm on the concatenation within 0.01 LU. The test passed only because of its clip lengths.
- Cause: pyloudnorm 0.1.1 counts round((n/fs - 0.4)/0.1) + 1 blocks. When more than 50 ms is left after the last full hop, that adds a partial trailing block, summed over fewer samples but divided by the full block length. `Programme` counts complete blocks only (BS.1770).
- Measured: 40 random multi-clip programmes at 24 kHz (white noise, 1-5 clips of 0.3-6 s, gain 0.02-0.3, numpy seed 0), pyloudnorm 0.1.1. All 40 were measurable. 13 of 40 differ from pyloudnorm by more than 0.01 LU, with a maximum of 0.218 LU. `Programme` equals `measure_blocks(np.concatenate(clips))` to below 1e-9 in all 40. Script: a one-off in the track scratchpad, deleted afterwards. The reviewer's independent run got 12 of 39 and a maximum of 0.157 LU.
- Effect before the fix: `plan_gains` took the batch term from `Programme` and the per-clip term from pyloudnorm, so segments 1 and later were off by up to about 0.2 dB. Segment 0 was unaffected because the two terms cancel.
- Fix: `audio/loudness.measure_blocks(wave, sr)` (a one-clip `Programme`), used by `audio/pipeline._prepare` for the per-clip term. `reference.py` keeps pyloudnorm `measure`. The fixed-length test was replaced by a randomized-length test (concatenation versus programme, to 1e-9), a test that the two meters agree when no partial block is left, and `test_per_clip_and_batch_terms_use_the_same_meter` on a length where they disagree.

### 3. Early close reaches the ZeroGPU generator (verified)
- Defect: `ordered_pipeline` closed only `levelled()`. The inner generators (`synthesise()`, `MingEngine.render_stream`, the `@spaces.GPU` generator) closed only when CPython dropped the last reference. That could happen on any thread, or only at cyclic GC if a traceback still held the frames. spaces 0.51.3 sets `worker.stop_event` only when GeneratorExit reaches its `yield res.value`, so after a disconnect the GPU could keep rendering the batch and spend the daily quota.
- Fix: `levelled()` closes `clips` in a `finally`. `synthesise()` wraps `ming.render_stream(...)` in `contextlib.closing`, and `render_stream` wraps `self._gpu_run(jobs)` the same way.
- Tests: `HeldMing` keeps every generator it hands out alive, so only an explicit close can end them. With it, closing `stream_gpu` after the first item runs the GPU generator's `finally` on the `synth` thread, and both held generators are finished. A second test gives `_stream` a held clip source and checks that it is closed on `synth`.
- Mutation check (scratch copy, each link removed on its own): without the close in `levelled()`, the `_stream` test fails. Without the `closing` in `synthesise()` or in `render_stream`, the `stream_gpu` test fails. A first version of the test without held references passed with both service closes removed, because refcounting closed the chain anyway. That version was abandoned.

### 4. `ordered_pipeline` no longer hangs when `iter(source)` raises (verified)
- `items = iter(source)` moved inside the producer's `try`, and the `finally` closes `items` only when it exists. Test: a source whose `__iter__` raises `TypeError`; the consumer gets the same object. Mutation check: with the old order the test fails, because the consumer would block on `handoff.get()` and hold the Gradio `cpu` slot.

### 5. ZeroGPU abort mid-batch (verified)
- `test_zerogpu_abort_mid_batch_keeps_finished_items_and_writes_no_marker`: the GPU generator yields item 0 and then raises a ZeroGPU quota `RuntimeError`. Exactly one `EngineItemResult` is yielded, the raised error is the same object, the R2 writes are item 0's audio and then its sidecar, and the marker is not written.

### How tests were run
`cd space && uv run --no-project --python 3.12 --with pytest --with numpy --with pyloudnorm --with soundfile --with imageio-ffmpeg python -m pytest -q tests`: 213 passed (2026-10-31). `uv run --with-requirements requirements.txt` cannot resolve on this Mac (torch 2.11.0 has no macOS x86_64 wheel).

## Open items
- Deploy together with `ADAPTER_VERSION` 2 (note 22). Unblock: lead.
