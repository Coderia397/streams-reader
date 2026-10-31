# Fish Audio bakeoff (Drama 3 int4, S2 Pro)

Plan and results for Fish Audio Drama 3 (int4 build `fishaudio/drama-3-int4`) across 2x T4 and S2 Pro on one T4, on a private Kaggle kernel, scored with `tools/expressiveness/prosody.py` against the Kokoro baseline (notes/20). Conclusion: pending the kernel run.

## 2026-10-31

### Plan (open)
- Code: `tools/expressiveness/bakeoff/fish/` (`fish_bakeoff.py` driver: jobs, push, pull, collect; `fish_kernel.py` Kaggle script; `fish_stage.py` per-stage worker; `analyze.py` summary). Kernel `lbrahimali/streams-fish-bakeoff`, private, T4 x2, internet on; dataset `lbrahimali/streams-fish-bakeoff-inputs` (two clone references + nothing else).
- Pins: fish-speech commit 0f8a12e8 (main, 2026-10-29); drama-3-int4 revision a6f58869; s2-pro revision 1de9996b; Whisper large-v3-turbo 41f01f3f; UTMOS22 strong via `tarepan/SpeechMOS:v1.2.0`. Pip pins in `fish_bakeoff.py`.
- Drama 3 int4 is an RTN int4 checkpoint (by akhaliq, mirrored by fishaudio) meant for SGLang Omni Marlin kernels on SM80+, tested only on A100-80GB and H200 (model card). T4 is SM75, so the kernel loads it into fish-speech's own PyTorch Drama 3 code (`fish_omni`): routed experts stay int4 on the GPU and are unpacked per forward for the selected experts; dense int4 projections are unpacked to fp16 at load; backbone split over both T4s by bytes, codebook tower and codec on cuda:1. Packing verified against the FP8 checkpoint by range-fetching two expert matrices and one dense matrix (`scratchpad/fish/scripts/check_pack.py`).
- Size estimate (assumed, from config and index): 22.1 GB on disk (index metadata); on GPU about 16.6 GB int4 experts + scales, 2.6 GB fp16 dense projections, 2.1 GB embedding + lm_head, about 1 GB tower, plus codec; under the 2 x 15 GB of two T4s.
- Voices: Fish models have no presets; both clone the Chatterbox bakeoff references (LibriVox Austen reader, female, 12.5 s, F0 229 Hz; LibriVox Walden reader, male, 11.3 s, F0 106 Hz). Transcripts from Whisper inside the kernel.
- Conditions: plain segment text, and directed (inline Fish tags per sentence from `director/out` plans: emotion + intensity + pace tag, `[emphasis]` before emphasis words, hand nonverbal tags mapped to `[inhale]`, `[chuckle]`, `[sigh]`, `[hesitating]`). 48 items per model (3 passages x segments x 2 voices x 2 conditions), 9406 characters.
- Sampling: fish-speech defaults temperature 0.7, top_p 0.7, top_k 30, seed 1234 + segment index.
- Post-processing: Space pipeline (`nolabs_engine.audio.pipeline`), -16 LUFS, pauses 250/600 ms, WebM/Opus 48 kbps, under `tools/expressiveness/bakeoff/fish/audio/<label>/<passage>/NN.webm`.
- Acceptance checks: (A1) Drama 3 loads on 2x T4 or the failure is logged; (A2) S2 Pro renders on one T4; (A3) every complete label scored with the frozen calibration; (A4) RTF and peak VRAM per model from the kernel logs; (A5) WER and UTMOS per label; (A6) licences read on the model cards on 2026-10-31.

### Licences (verified 2026-10-31, files fetched from the pinned revisions)
- fish-speech code at 0f8a12e8: `LICENSE` is the Fish Audio Community License v1.0 (last updated 2026-10-13); `pyproject.toml` says the same.
- fishaudio/drama-3 and fishaudio/drama-3-int4 weights: `LICENSE` is the Fish Audio Community License v1.0 (identical files). The drama-3 card front matter still says `license_name: fish-audio-research-license` while its body and LICENSE say Community License; the int4 card says `fish-audio-community-license`.
- Community License terms: research and non-commercial use free; commercial use free only while the licensee's annual revenue (with affiliates) is under USD 500,000, with notice to business@fish.audio within 30 days of crossing it and a 90-day transition; no licence for a Hosted Service (third parties supplying text, audio or voice samples to a feature whose primary purpose is TTS or voice cloning) without a separate agreement; generating speech for content the licensee creates, selects or controls is not a Hosted Service. The int4 card adds: no cloning real people's voices without consent, keep watermarks, do not train other speech models on outputs.
- fishaudio/s2-pro weights: Fish Audio Research License (last updated 2026-03-07): research and non-commercial only; any commercial use needs a separate written licence.

### Kaggle limits (verified 2026-10-31)
- Two concurrent GPU sessions per account (note 25). At 05:47 GMT the IndexTTS-2.5 and Qwen3-TTS kernels held both; the push waits for a free slot (`scratchpad/fish/scripts/wait_slot.sh`, 5 min poll, one push).

### Run 1 (verified, kernel version 1, pushed 07:06 GMT, 10.5 min)
- Setup 126 s, all pins installed, import probe passed; torch 2.11.0+cu128, Python 3.13.15, 2x Tesla T4 (sm 7.5, 15360 MiB), 31 GB RAM. int4 unpack self test max abs error 0.0.
- Drama 3: failed in the download. Anonymous snapshot of fishaudio/drama-3-int4 got 5 of 20 files in 4 min, then `429 Too Many Requests` from the Xet CDN (us.gcp.cdn.hf.co). Fix: each attempt in a subprocess, alternating Xet and plain HTTP (`HF_HUB_DISABLE_XET=1`), 2 workers, back-off 60 to 300 s, 10 attempts.
- S2 Pro: downloaded 10.25 GiB in 38 s, loaded in 86.7 s (10.41 GiB peak on cuda:0); with 6144-frame caches cuda:0 reached 14.4 GiB and the codec OOMed encoding the reference. Fix: codec on cuda:1, cache 4096 frames.
- Whisper transcripts of the references: female Austen ch. 1 opening (12.52 s), male Walden ch. 5 (11.34 s).

### Run 2 (open, kernel version 2, pushed 07:36 GMT)

## Open items
- Push the kernel when a GPU slot frees (unblocks: one of the two running kernels finishing).
- Results, scores and qualitative notes (unblocks: kernel run).
