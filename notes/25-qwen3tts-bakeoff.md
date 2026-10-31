# Qwen3-TTS 1.7B expressiveness bakeoff

Plan and results for Qwen3-TTS 1.7B (CustomVoice with per-sentence style instructions, Base cloning, VoiceDesign, faster-qwen3-tts) on a private Kaggle T4 kernel, scored with `tools/expressiveness/prosody.py` against the Kokoro baseline (notes/20). Conclusion so far: CustomVoice Aiden reaches 117-158 % of the human prosody index and Serena 75-103 %, at RTF about 2.1 on one T4 (slower than real time); faster-qwen3-tts runs the same model at RTF 0.83 on a T4 with the same scores; VoiceDesign 111-132 %; clones pending run 4.

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

### Run 2 (verified, kernel version 2, pushed 07:48 GMT, 3473 s)
- Scope: clone, VoiceDesign, faster-qwen3-tts and the analysis (Whisper WER, speaker cosine). The run 1 raw clips went in as private dataset `lbrahimali/streams-qwen3tts-prior-raw` (`prior_raw.tar`, 43 MB); `spec.json` had `skip_engines: ["cv"]`.
- Base clone: 165 items, 0 errors, fp32 chosen (probe RTF bf16 2.62, fp16 2.62, fp32 2.23), VRAM 7.83 GB after load. RTF 2.18-2.43 per condition (`worker_base_vd.json`).
- Correction (same day, after the WER check): the poe_f clone is broken too. WER seg 19.5 %, sent 55.6 % (`results/check_run2.json`; e.g. "I'll, I'll, I'll ...", reference words leaking as "Creepy Tay"). All three reference clips end 0.25 s into the next word (last 100 ms frames -16 to -20 dB against a -52 dB floor; pp_f and walden_m also start with 3 s of silence that Whisper stamped as 0.0). The poe_f scores below are superseded (files renamed `clone_poe_INVALID.*`, audio deleted); run 3 re-renders pp_f (female) and walden_m (male) with the fixed cut.
- poe_f clone (reference 0.32-7.0 s, transcript "Chapter 1 of Creepy Tales by Edgar Allan Poe."): 1 capped of 55 (superseded, see the correction above). Scores (frozen calibration, `results/clone_poe_INVALID.md`): seg 156 %, sent 114 %. Artefact check (`scratchpad/qwen/tools/qual.py`): seg F0 p5/p50/p95 156/245/490 Hz and 28.8 pitch breaks per voiced minute (human 22.3), 10.1 chars/s; sent 143/215/342 Hz, 12.6 breaks/min, 11.9 chars/s. Assumed: the seg score is inflated by falsetto jumps or octave errors.
- pp_f and walden_m clones failed: 16 + 12 capped items, sub-second clips at 30-260 chars/s, silent clips. Wrong belief: Whisper segment chunks give a transcript that matches the cut reference. pp_f got 0-15.0 s for one 13-word sentence and walden_m was cut mid-sentence ("... public domain. For more"), so the ICL prompt text did not match the audio. Fix in run 3: word timestamps, cut at the latest sentence end within 4-12 s followed by a >= 120 ms pause, end refined on the energy envelope; one seed retry for capped, silent or > 28 chars/s clips (clone, vd only; gpu_s counts every attempt).
- VoiceDesign: CUDA OOM at load. Wrong belief: `free(m)` frees the model. It only deletes the function's local name, so Base (7.8 GB) stayed resident on the module-level `m`. Fix: inline `del m` at module level (also in the probe loop, where the previous dtype's model was alive during the next load, so run 1/2 probe peak VRAM figures for fp16/fp32 include a leftover model).
- faster-qwen3-tts 0.5.4: venv install ok, but unpinned pip took transformers 5.22.1 (released 2026-10-30) and the qwen-tts-hf fork crashed at load (`property 'upsample' of 'Qwen3TTSTokenizerV2Encoder' object has no setter`). Fix: pin `qwen-tts-hf==0.1.1.post1 transformers==5.15.1` (newest transformers when the fork shipped on 2026-08-25); load fp16 first (T4 has no native bf16), bf16 fallback.
- Wrong belief: an uploaded tar stays a tar on Kaggle. Kaggle unpacked `prior_raw.tar` into a `raw/` tree (with macOS `._*` files), the kernel glob for `prior_raw.tar` found nothing, and the CustomVoice clips got no WER. Fix: the kernel also copies unpacked `raw/qwen_*` trees and deletes them before the output is saved.

### Run 3 (verified, kernel version 3, 10:49-12:40 GMT, 6528 s)
- pp_f and walden_m clones (fixed reference cut, retry), VoiceDesign f/m styled + segstyled, faster-qwen3-tts Serena/Aiden styled; spec `scratchpad/qwen/work3/spec.json` (`jobs --only '^qwen_(clone_(pp_f|walden_m)_|vd_|fast_)' --skip-engines cv`), 306 items, 0 errors (`worker_base_vd.json`, `worker_fast.json`).
- faster-qwen3-tts 0.5.4 (qwen-tts-hf 0.1.1.post1, transformers 5.15.1, fp16, CUDA graphs) works on a T4: 86 items, 0 errors, RTF 0.83 (Aiden 0.827, Serena 0.835; GPU s / audio s, `results/check_run3.json`), 2.5x faster than qwen-tts 0.1.1 fp32 (2.1). Same prosody: Serena styled 76 % (75 % on qwen-tts), Aiden styled 142 % (142 %). WER Serena 3.1 %, Aiden 1.1 % (Whisper large-v3-turbo, 43 clips each). Peak VRAM not logged by this worker (open).
- VoiceDesign (fp32, 7.79 GB after load, 8.62 GB peak, RTF 2.13-2.17): WER 0.6-1.7 %, speaker cosine min 0.94-0.98. Prosody (frozen calibration, `results/vd_fast.md`, mean of 3 passages): female styled 111 %, segstyled 132 %; male styled 113 %, segstyled 115 %.
- Artefact check (`scratchpad/qwen/tools/qual.py`, `scratchpad/qwen/qual_vd_fast.txt`): VoiceDesign female segstyled F0 p5/p50/p95 113/181/273 Hz, voiced share 0.46, 9.3 pitch breaks per voiced minute; male 72-74/101-103/182-185 Hz. Assumed: the low p5 is creak (vocal fry) on hushed lines, not octave errors. faster Serena 0.7 breaks/min, Aiden 6.5.
- pp_f and walden_m clones failed again: WER 374-680 %, 2-12 capped per condition. Wrong belief: Whisper word timestamps are accurate on these intros. They ran to 45 s on a 30 s file, and "This" was stamped 0-5 s over 3 s of leading silence, so no sentence end fell within 12 s of start 0 and the cut fell back to the single word "This" (5 s of mostly silence). Fix in run 4: cut and transcript fixed by hand from the energy envelope (`scratchpad/qwen/tools/pauses.py`): poe_f 0.63-10.15 s "Chapter 1 of Creepy Tales by Edgar Allan Poe. This is a LibriVox recording." (agrees with Whisper there), walden_m 3.07-13.33 s, three intro sentences, each block bounded by pauses of 0.34-1.5 s (assumed mapping of text to blocks). pp_f dropped (block-to-text mapping ambiguous).

### Run 4 (open)
- poe_f and walden_m clones with the fixed cuts, plus analysis (WER, speaker cosine) of the 342 CustomVoice clips from the prior dataset; spec `scratchpad/qwen/work5` (`jobs --only '^qwen_(cv_|clone_(poe_f|walden_m)_)' --skip-engines cv`, 110 clone items). Push waits for a free GPU slot (`scratchpad/qwen/wait_push5.sh`); the first attempt at 12:43 GMT was refused ("Maximum batch GPU session count of 2 reached") by other workflows' kernels.

### Licences (verified 2026-10-31)
- Weights: Qwen3-TTS-12Hz-1.7B-CustomVoice, -Base, -VoiceDesign and Qwen3-TTS-Tokenizer-12Hz: `license: apache-2.0` in the HF card metadata (HF API, revisions above); no LICENSE file in the HF repos; not gated.
- Code: QwenLM/Qwen3-TTS Apache-2.0 (GitHub API); qwen-tts 0.1.1 Apache-2.0 (PyPI); qwen-tts-hf fork andimarafioti/Qwen3-TTS Apache-2.0 (GitHub); faster-qwen3-tts MIT (GitHub, wheel metadata). Whisper large-v3-turbo (analysis only) MIT.

## Open items
- Run 3 results: unblocked when the kernel completes.
- Run 4: unblocked when run 3 completes and a GPU slot is free.
