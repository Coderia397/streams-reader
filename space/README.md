---
title: nolabs engine
emoji: 🎧
colorFrom: gray
colorTo: indigo
sdk: gradio
sdk_version: "6.17.3"
python_version: "3.10"
app_file: app.py
pinned: false
license: mit
short_description: Private speech engine for nolabs-reader
preload_from_hub:
  - hexgrad/Kokoro-82M config.json,kokoro-v1_0.pth,voices/af_alloy.pt,voices/af_aoede.pt,voices/af_bella.pt,voices/af_heart.pt,voices/af_jessica.pt,voices/af_kore.pt,voices/af_nicole.pt,voices/af_nova.pt,voices/af_river.pt,voices/af_sarah.pt,voices/af_sky.pt,voices/am_adam.pt,voices/am_echo.pt,voices/am_eric.pt,voices/am_fenrir.pt,voices/am_liam.pt,voices/am_michael.pt,voices/am_onyx.pt,voices/am_puck.pt,voices/am_santa.pt,voices/bf_alice.pt,voices/bf_emma.pt,voices/bf_isabella.pt,voices/bf_lily.pt,voices/bm_daniel.pt,voices/bm_fable.pt,voices/bm_george.pt,voices/bm_lewis.pt f3ff3571791e39611d31c381e3a41a3af07b4987
  - PeterReid/graphemes_to_phonemes_en_us config.json,generation_config.json,model.safetensors a5631b285d18d59483c32c0c3379cb9fac924f4b
  - PeterReid/graphemes_to_phonemes_en_gb config.json,generation_config.json,model.safetensors d8357d5067fa26a5c34134d6bbcf4bbf000c0ac8
  - inclusionAI/Ming-3.0-tts-0.5B added_tokens.json,campplus.onnx,config.json,configuration.json,merges.txt,model.safetensors,special_tokens_map.json,tokenizer.json,tokenizer_config.json,vocab.json 7247f4c94a5b2e987404ed28a5cfec41b1a4ca61
---

Private speech engine for [nolabs-reader](https://github.com/Coderia397/nolabs-reader). Kokoro-82M (Apache-2.0) runs on the
CPU for standard narration, with the Apache-2.0 BART G2P network (`PeterReid/graphemes_to_phonemes_en_us`/`_gb`, as in
misaki main) for words outside misaki's lexicon instead of espeak-ng; Ming-3.0-tts-0.5B (MIT) runs on ZeroGPU for cloned
and expressive voices. Ming inference code is
vendored in `vendor/ming3` from [inclusionAI/Ming-3.0-tts](https://github.com/inclusionAI/Ming-3.0-tts) (MIT) via the
torchaudio-free port in `mrfakename/ming-3.0-tts-0.5b`.
`nolabs_engine/engines/g2p_fallback.py` is adapted from hexgrad/misaki (Apache-2.0, see `nolabs_engine/engines/LICENSE.misaki`).

Contract v1 (`nolabs_engine/contracts.py`, mirror of `crates/core/src/proto/engine.rs`). Each endpoint takes one JSON
string and returns one JSON string as its result (the SSE `complete` event):

| Endpoint | Queue | Does |
|---|---|---|
| `render_cpu` | `cpu`, 1 at a time | Kokoro: `EngineRequest` → `EngineResponse`; writes `audio/kokoro/<key>.webm`, its `.json` sidecar, then `jobs/<id>.json` |
| `render_gpu` | `gpu`, 1 at a time | Ming in one `@spaces.GPU` generator call per batch; same writes under `audio/ming/` |
| `prepare_reference` | `cpu` | Trims `voices/<id>/raw.wav` to the best 10–18 s of whole script lines → `voices/<id>/ref.wav` |
| `health` | no limit | Engine status, R2 `HeadBucket`, pinned model revisions; never touches the GPU |

Streaming: `render_cpu` and `render_gpu` are generators. Before `complete`, the SSE stream carries one `generating` event
per item, in request order, whose data is `["<EngineItemResult JSON>"]` (the same JSON as the item's sidecar), sent once
that item's audio and sidecar are in R2. The last `generating` event repeats the `complete` payload (the full
`EngineResponse`, recognisable by its `job_id`). Segment N+1 is synthesised while N is encoded and uploaded. Loudness:
segment N is levelled against the programme loudness of segments 0..N, so the first one ships without waiting.

Errors: `bad request: …` (fix the request), `engine error: …` (our side), ZeroGPU quota and runs-limit errors unchanged.

Model weights are pinned by commit in `nolabs_engine/revisions.py`; change them together with the `preload_from_hub`
lines above and the Worker vars `KOKORO_REV` / `MING_REV` (`health` also reports the G2P fallback commits as
`kokoro_g2p`; a change there changes Kokoro output, so it needs a new `ADAPTER_VERSION` in the Worker). Environment: `R2_ENDPOINT`, `R2_ACCESS_KEY_ID`,
`R2_SECRET_ACCESS_KEY`, `R2_BUCKET` (Space secrets); optional `MING_BASE_SECONDS` (8), `MING_SECONDS_PER_CHAR` (0.06),
`MING_MAX_SECONDS` (180), `NOLABS_THREADS`.

Tests (no models needed): `uv run --python 3.12 --with pytest --with numpy --with pyloudnorm --with soundfile python -m
pytest -q tests`; add `--with imageio-ffmpeg` to also run the ffmpeg tests. Speed benchmarks (local Kokoro and the live
Space): `tools/bench/`.
