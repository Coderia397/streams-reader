# Qwen3-TTS 1.7B expressiveness bakeoff

Plan and results for Qwen3-TTS 1.7B (CustomVoice with per-sentence style instructions, Base cloning, VoiceDesign, faster-qwen3-tts) on a private Kaggle T4 kernel, scored with `tools/expressiveness/prosody.py` against the Kokoro baseline (notes/20). Conclusion: pending the kernel run.

## 2026-10-31

### Plan (open)
- Code: `tools/expressiveness/bakeoff/qwen3tts/` (`qwen_kaggle.py` driver, `qwen_kernel.py` Kaggle script, `styles.json` hand-written per-sentence and per-paragraph instructions). Kernel `lbrahimali/streams-qwen3tts-bakeoff`, private, T4, internet on, job spec embedded in the script (no dataset).
- Pins: qwen-tts 0.1.1 (transformers 4.57.3, accelerate 1.12.0), faster-qwen3-tts 0.5.4 in a venv (needs transformers 5). Weights at HF revisions CustomVoice 0c0e3051, Base fd4b2543, VoiceDesign 5ecdb673, Whisper large-v3-turbo 41f01f3f (verified on the HF API).
- 75 condition sets, 703 items: CustomVoice Serena (female) and Aiden (male) x {seg, sent, director, styled, segstyled}; Ryan, Vivian, Sohee plain; Base clones of three LibriVox intros (poe_f 201 Hz, pp_f 223 Hz, walden_m 105 Hz); VoiceDesign female/male persona + style; faster-qwen3-tts on the styled condition.
- Post-processing as the Ming study (Space pipeline, Ming profile, pauses 250/600 ms), WebM/Opus 48 kbps.
- Acceptance checks: (A1) kernel completes with < 5 % failed items; (A2) every condition scored with frozen calibration; (A3) WER per condition from Whisper; (A4) RTF and peak VRAM per model; (A5) licences checked on the model cards on 2026-10-31.

### Kaggle limits (verified 2026-10-31)
- Push refused with "Maximum batch GPU session count of 2 reached" while the Ming and Higgs kernels ran. The push waits for a free slot (`scratchpad/qwen/wait_push.sh`, 5 min poll).

### Push (verified)
- The wait loop stopped at 02:03 when the session hit its usage limit (5 polls, running=2 each time). At 05:44 GMT Ming and Higgs were COMPLETE; one push at 05:45 GMT was accepted (kernel version 1).

### Run 1 (verified, kernel version 1, 05:45-06:54 GMT, 4102 s)
- Image: Python 3.13.15, torch 2.11.0+cu128, CUDA 12.8, 2 x Tesla T4 15 GB, driver 580.178.04. qwen-tts 0.1.1, transformers 4.57.3, accelerate 1.12.0, huggingface_hub 0.36.2 (from `qwen_log.json`).
- CustomVoice: all 342 items of the 39 cv condition sets rendered, 0 errors, 0 capped. dtype probe (3 sentences): bf16 RTF 2.59, fp16 2.57, fp32 2.12, so fp32 was kept (T4 has no fast bf16). Run RTF 2.07-2.10 per condition (GPU seconds / audio seconds, `worker_cv.json`), so slower than real time on one T4. VRAM 7.79 GB after load, 8.76 GB peak.
- Wrong belief: the static imageio ffmpeg could read the archive.org https URLs. It segfaulted (rc -11) for all three references, so Base cloning and VoiceDesign never ran. Fix: download with urllib, decode the local file, librosa fallback.
- Wrong belief: `python -m venv --system-site-packages` works on the Kaggle image. ensurepip failed, so faster-qwen3-tts was not installed, and reading the missing pip log raised in main() before the ASR/speaker analysis stage. Fix: `venv --without-pip` + `pip --python`, uv fallback, guarded log read.
- Scores (frozen calibration, `results/cv.md`, mean of 3 passages, % of human): Serena seg 86, sent 76, director 85, styled 75, segstyled 103; Aiden seg 127, sent 127, director 158, styled 142, segstyled 117; Ryan 139, Vivian 126, Sohee 119 (plain per segment). Kokoro af_heart 68, am_michael 50.
- Artefact evidence (`scratchpad qual.py`, Praat 10 ms, joined passages): pitch breaks > 4 st per voiced minute: Serena 1.3-5.3, Aiden 3.8-10.9, Kokoro af_heart 17.5, human 22.3. Programme pace 12-13 chars/s (Serena, Aiden) vs Kokoro 15-18. F0 p5/p50/p95: Serena styled 170/226/312 Hz, Aiden director 88/139/246 Hz.

### Run 2 (open)
- Only clone, VoiceDesign, faster-qwen3-tts and the analysis (Whisper WER, speaker cosine) for all clips. The run 1 raw clips go in as private dataset `lbrahimali/streams-qwen3tts-prior-raw` (`prior_raw.tar`, 43 MB); `spec.json` has `skip_engines: ["cv"]`. Push waits for a GPU slot (`scratchpad/qwen/wait_push2.sh`; chatterbox and IndexTTS kernels held both slots at 07:02 GMT).

## Open items
- Run 2: unblocked when a Kaggle GPU slot frees.
