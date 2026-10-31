# IndexTTS-2.5 expressiveness bakeoff

Plan and results for IndexTTS-2.5 (bilibili; IndexTTS-2 as fallback) on a private Kaggle T4 kernel, scored with `tools/expressiveness/prosody.py` against the Kokoro baseline (notes/20). Conclusion: IndexTTS-2.5 ran first time (no fallback), 423/423 items, WER about 2 %; the af_heart clone is the cleanest voice (EI 0.56 plain = 81 %, 1.19 directed = 117 %, no clipping); text emotion (QwenEmotion) inflates EI by raising F0 2-6 st and clips the male clone, so EI above 100 % here is over-acting, not human-like. Too slow for bulk on a T4: RTF 3.85 fp32.

## 2026-10-31

### Plan (done; checks A1-A5 verified 2026-10-31: 0 % failed, 15 labels scored, WER per label, RTF + VRAM logged, licences read today)
- Code: `tools/expressiveness/bakeoff/indextts25/` (`itts_kaggle.py` driver: refs, jobs, push, pull, collect; `itts_kernel.py` Kaggle script; `itts_worker.py` one worker per GPU; `artefacts.py` WER, HNR, F0 jumps, long gaps, HF ratio). Kernel `lbrahimali/streams-itts25-bakeoff`, private, T4 x2, internet on; inputs in private dataset `lbrahimali/streams-itts25-bakeoff-inputs`.
- Pins (verified on the HF and GitHub APIs 2026-10-31): code index-tts/index-tts@d9e41aac (main, 2026-09-29), installed with its own `uv.lock` (`uv sync --frozen --python 3.11 --no-dev`, uv 0.13.1); IndexTTS-2.5@c39ce5ba, IndexTTS-2@740dcaff, w2v-bert-2.0@da985ba0, funasr/campplus@e4b6ede7, BigVGAN v2 22 kHz@633ff708, MaskGCT@265c6cef (IndexTTS-2 only).
- Voices: zero-shot clones (the model has no presets). lvf = LibriVox Pride and Prejudice solo reader, 4:32.05-4:43.50 (female); lvm = LibriVox Walden ch. 5, 5:03.8-5:16.9 (male); heart = the 14 s Kokoro af_heart reference from the Ming study. Both LibriVox windows lie outside the 75 s human-ceiling excerpts.
- Conditions per voice: plain (per segment, as the app sends), plain_sent (per sentence, no control), emotext (per sentence QwenEmotion vector, emo_alpha 0.65, same as use_emo_text), directed (director emotion and intensity as an 8-dim emo_vector with the upstream bias; pace slow/fast -> duration_factor 1.10/0.92), directed_dp (same audio, director pauses). Seed 1234.
- Post-processing: Space pipeline with the Ming profile (−16 LUFS, pauses 250/600/1200 ms), WebM/Opus 48 kbps.
- Acceptance checks: (A1) kernel completes with < 5 % failed items; (A2) every condition scored with frozen calibration; (A3) WER per condition from Whisper small.en; (A4) RTF and peak VRAM; (A5) licences checked on the model cards on 2026-10-31.

### Licences (verified 2026-10-31)
- Code (GitHub index-tts/index-tts, LICENSE at d9e41aac and main): bilibili Model Use License Agreement; GitHub reports `other` / NOASSERTION.
- IndexTTS-2.5 weights: model card `license: other`, `license_name: bilibili-model-license`, LICENSE file = bilibili Model Use License Agreement. Not gated.
- IndexTTS-2 weights: model card has no license field; repo ships LICENSE.txt (bilibili Model Use License Agreement) and LICENSE_ZH.txt.
- Terms (verified on the LICENSE text): 2.1 worldwide, non-exclusive, royalty-free licence to Use (no non-commercial clause); 2.2 more than 100 million MAU or group revenue above RMB 1 billion in the prior year needs a separate written licence; 3.4 b keep notices and a copy of the agreement; 3.4 c model or outputs may not improve other AI models (except IndexTTS2 derivatives or non-commercial models); 4.1 a disclaimer text on any distributed derivative; 4.2 no high-risk uses; 5.1 revocable on breach; 6 PRC law, Shanghai arbitration; 9 Chinese text prevails.
- Auxiliary weights: w2v-bert-2.0 MIT, CAM++ Apache-2.0, BigVGAN v2 MIT (assumed from their model cards, not rechecked today); MaskGCT semantic codec CC-BY-NC-4.0, used only by IndexTTS-2.

### Push (verified)
- Session hit the usage limit at about 01:50 GMT before the push. At 05:44 GMT Higgs and Ming were COMPLETE; the dataset was re-versioned and one push at 05:45 GMT was accepted (kernel version 1); RUNNING by 05:47 GMT.

### Run (verified, kernel version 1, 05:45-07:05 GMT)
- Stages (itts_log.json): install 134 s, download 80 s (checkpoints 7.8 GB), smoke 166 s (load 101 s), workers 4381 s on 2 x T4. Image Python 3.13.15; venv Python 3.11.17, torch 2.8.0+cu128, transformers 4.52.1. 423 items, 0 failed, 0 max_mel_tokens warnings.
- RTF = GPU seconds / audio seconds, fp32 (use_bf16 False), no CUDA kernel, after one warm-up per voice: all 3.85 (7804 s / 2027 s); per segment (plain) 2.44, per sentence 3.97, emotext 4.77 (includes the QwenEmotion call), directed 3.86. Per voice: heart 4.53, lvf 3.84, lvm 3.41. 0.35 GPU-s/char. Ming on the same T4 (note 32): RTF 2.24.
- VRAM (torch max_memory_allocated / reserved, per worker): load 7.58 GB, peak 9.38 / 11.14 GB.
- Install problems: none blocking. Kaggle Python 3.13 is outside index-tts `requires-python` (<3.12): fixed with `uv sync --frozen --python 3.11 --no-dev`; webui, deepspeed and flash-attn extras skipped.

### Scores (verified; `prosody.py measure`, frozen calibration; mean of 3 passages; `bakeoff/indextts25/results/prosody.md`)
| voice | plain | plain_sent | emotext | directed | directed_dp |
|---|---|---|---|---|---|
| heart (af_heart clone) | 0.56 / 81 % | 0.77 / 93 % | 1.28 / 122 % | 1.19 / 117 % | 1.26 / 121 % |
| lvf (LibriVox female) | 0.71 / 90 % | 1.05 / 109 % | 1.55 / 138 % | 1.33 / 125 % | 1.41 / 130 % |
| lvm (LibriVox male) | 0.97 / 104 % | 1.34 / 126 % | 2.23 / 177 % | 1.47 / 133 % | 1.48 / 134 % |
Kokoro af_heart 0.32 / 68 % (notes/20), Kokoro + director 0.67 / 87 % (notes/33).

### Artefacts (verified; `artefacts.py`, Whisper small.en in the kernel; raw FLAC int16 for clipping)
- WER 1.7-3.7 % per condition; nearly all errors are normalisation (I'm -> I am, grey -> gray, watch -> watched). No skipped or repeated phrases found.
- Median F0 (Hz, three passages): heart plain 199/191/190, emotext 219/221/223 (+2 st); lvf plain 247/244/242, emotext 283/287/284 (+2.6 st); lvm plain 104/102/104, emotext 145/147/147 (+6 st). directed stays within 0-2 st of plain.
- Clipping (samples at int16 full scale): heart 0 in every condition; lvm emotext 0.77 per 1000 samples, 16 of 43 clips with more than 20 clipped samples; others below 0.06 per 1000.
- HF ratio (5-11 kHz vs 0.3-5 kHz): heart -23 dB (Kokoro af_heart -21), lvf -37 to -39 (muffled, inherited from the 64 kbps MP3 reference; human excerpts -29), lvm -27 to -31.
- F0 jumps over 8 st per minute: heart 1.2-3.5, lvf 10.5-13.6, lvm 0.5-8.1 (Kokoro 0-1.5, human 8.0).
- Speaking rate: per-sentence rendering slows all voices (heart 4.66 -> 3.7-4.1 syl/s, lvm 3.62 -> 2.65-2.78; human Walden reader 3.2). lvm has 3-4 pauses longer than 1.2 s inside clips in plain, emotext and directed.
- QwenEmotion argmax over the 43 sentences: calm 23, surprised 7, afraid 5, angry 4, sad 2, melancholic 2.

### Reading of the results (assumed; no listening)
- Per-sentence calls alone add 12-22 points (plain -> plain_sent); emotion control adds another 20-50. The EI rewards pitch range, so emotext's F0 rise scores high; with the F0 shift and clipping it is likely strained or shouted on the male clone.
- directed is the balanced setting: EI 117-133 % with F0 near the reference and clipping near zero.
- The af_heart clone is the cleanest timbre; the LibriVox clones copy the recording quality of their references (bandwidth, breathiness).

## Open items
- Listening test by the owner: heart_directed_dp, heart_plain, lvf_directed_dp, lvm_directed_dp, lvm_emotext (to confirm the over-acting reading). Unblocked by the owner listening to `tools/expressiveness/bakeoff/indextts25/audio/`.
- Speed: bf16 or the upstream CUDA kernel were not tried; RTF 3.85 rules out bulk on free T4s unless one of them gives about 2x. Unblocked by one more kernel run.
- Licence decision: bilibili terms (2.2 scale cap, 3.4 c no training other models on outputs) need the owner's decision before any product use.
