# Chatterbox-Nano as the light candidate

Finding. Measures Chatterbox-Nano (ResembleAI, 110M GPT-2-small T3 + one-step meanflow S3Gen, MIT) as a light replacement for Kokoro: locally through the community ONNX port, and on a private Kaggle T4 in PyTorch. Conclusion: open; the local ONNX results are below and the T4 results follow.

## 2026-10-31

### Plan (open)
- Code (scratchpad, not in the repo, by the task's rule): `scratchpad/voice-search/measure/1-chatterbox-nano/code/`: `nano_jobs.py` (jobs from `tools/expressiveness/passages`, tag mapping from the Chatterbox bake-off), `nano_onnx.py` (ONNX port runner), `bench_torch.py` and `bench_kokoro.py` (fresh-process cold and warm latency), `nano_worker.py` + `nano_kernel.py` (Kaggle), `nano_kaggle.py`, `push_loop.py`, `make_clips.py`.
- Pins: chatterbox-tts git 5de7a54a (as note 37; its `ChatterboxTurboTTS.from_local(..., nano=True)` loads `t3_nano_v1.safetensors` with `GPT2_small` and `s3gen_meanflow.safetensors`, verified in the source on 2026-10-31). Weights ResembleAI/chatterbox-nano 71ccd1d0 (card `license: mit`, lastModified 2026-07-21). ONNX port owensong/chatterbox-nano-ONNX 4a66d7da (MIT; four graphs: fp16 embeddings, q4f16 speech encoder, q4f16 language model, q4 decoder; 547 MiB; the port publishes no quality or speed numbers). onnxruntime 1.23.2.
- Voices: the two LibriVox references of note 37 (female Pride and Prejudice 12.5 s, male Walden 11.3 s) and the 14 s Kokoro af_heart reference of note 32 (clone of af_heart for a fair comparison).
- Settings: Nano ignores exaggeration and cfg (the upstream `generate` warns and drops them), so the expressive controls are the reference clip and the paralinguistic tags. Conditions, seed 1234, temperature 0.8: plain per app segment (`cbn_<voice>`, torch; `cbnx_<voice>`, ONNX) for female, male, heart; per sentence with the bake-off's style and tag mapping (`cbn_tags_<voice>`, torch) for female and male.
- Latency texts (`bench_texts.json`): one sentence (98 chars) and the first three-sentence segment of 1-suspense (293 chars). Cold = fresh process, weights on local disk, process start to full waveform (no streaming). Warm = loaded model, after one warm-up.
- Acceptance checks: (A1) < 5 % failed items; (A2) RTF on T4 and CPU; (A3) cold and warm first audio for 1 and 3 sentences; (A4) WER and UTMOS per condition (Whisper large-v3-turbo 41f01f3f, UTMOS22 strong SpeechMOS v1.2.0, as notes 37 and 38); (A5) prosody index with the frozen calibration, as a filter; (A6) Opus 32 kbps clips per passage.

### Local ONNX on this Mac (verified)
- Machine: Intel Core i7-9750H, 6 cores / 12 threads, 32 GB (this Mac is Intel, not Apple silicon, so no MPS). Other agents were running at the same time: 1-min load average 4-15 during the runs (recorded per run), so these are pessimistic.
- Renders: 36/36 items, 0 errors, all ended on the stop token; RTF 0.96-1.19 per voice at the default thread count (sum of compute s / audio s, `onnx-out/onnx_log.json`).
- Latency (`bench/local_onnx.jsonl` and `bench/local_kokoro.jsonl`, same texts, same machine; Kokoro 0.9.4 af_heart in PyTorch CPU; n = 2 cold, 3 warm; medians):

| engine | threads | cold 1 sent. (process to audio) | cold 3 sent. | warm 1 sent. | warm 3 sent. | warm RTF |
|---|---|---|---|---|---|---|
| Nano ONNX | 2 | 17.8 s | 30.6 s | 6.65 s | 21.6 s | 1.36-1.47 |
| Nano ONNX | 6 | 17.1 s | 26.2 s | 4.60 s | 15.3 s | 0.97-1.05 |
| Nano ONNX | 12 | 16.1 s | 29.4 s | 5.45 s | 15.9 s | 1.04-1.20 |
| Kokoro af_heart | 2 | 10.5 s | 19.0 s | 3.50 s | 10.7 s | 0.62-0.63 |
| Kokoro af_heart | 6 | 10.3 s | 16.4 s | 2.27 s | 6.9 s | 0.39-0.40 |

  The ONNX cold time is about 8 s of session load plus 1 s of reference encoding; the warm time splits about 45 % language model and 55 % decoder (`lm_s`, `dec_s`). The port's decoder synthesises the whole segment in one call, so time to first audio equals the full segment time.
- Prosody index (filter only; `prosody.py measure`, frozen calibration; `results/prosody_onnx.md`), % of human, mean of 3 passages (suspense/dialogue/reflective): af_heart clone 69 % (50/92/65), female 91 % (95/115/64), male 115 % (102/143/99). Kokoro af_heart plain is 68 % (note 20). So the af_heart clone keeps Kokoro's level of variation; the LibriVox clones are near the Turbo numbers of note 37 (91 / 104 %).

## Open items
- T4 PyTorch run, CPU PyTorch bench, WER and UTMOS of all clips: unblocked when the Kaggle kernel `lbrahimali/streams-cbnano-measure` gets a GPU session (both held by the Qwen3-TTS and Fish kernels at 11:43 UTC; push retried every 10 min).
