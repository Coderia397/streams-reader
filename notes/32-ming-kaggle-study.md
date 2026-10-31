# Ming-3.0-tts-0.5B on a Kaggle T4: expressiveness and speed

Finding. Ming's US_Emma preset scores the same as Kokoro af_heart (EI 0.28 vs 0.32). Ming UK_George scores far above Kokoro bm_george (1.00 vs -0.49), but part of that comes from creaky phrase endings that inflate the slope measure (assumed, not checked by ear). Hand-placed emotion labels and tags add +0.06 EI on US_Emma and +0.37 on the af_heart clone. Cloning af_heart without direction adds nothing (0.31 vs 0.32). On a T4 Ming runs at RTF 2.24 (0.143 GPU-s per char), so Kaggle can do bulk offline renders but not live streaming.

## 2026-10-31

### Setup (verified)
- Tooling: `tools/expressiveness/ming/`.
  - `ming_kaggle.py` has the subcommands ref, jobs, push, probe, pull and collect.
  - `ming_kernel.py` is the main kernel. `ming_probe_kernel.py` re-renders the silent items.
  - Kaggle runs as the private kernel `lbrahimali/streams-ming-expressiveness`, which reads the private dataset `lbrahimali/streams-ming-study-inputs`. No ZeroGPU was used, and no `ming:*` voice was requested from the live API.
- Code and weights:
  - `space/vendor/ming3` (MIT) was shipped to the kernel as `ming3.tgz`.
  - Weights: `inclusionAI/Ming-3.0-tts-0.5B@7247f4c94a5b2e987404ed28a5cfec41b1a4ca61`, from `space/nolabs_engine/revisions.py`.
  - The kernel environment was Python 3.13.15 and torch 2.11.0+cu128. Pins: transformers 4.46.3, tokenizers 0.20.3, diffusers 0.33.0, onnxruntime 1.23.2 and pynini 2.1.6.post1. pip worked on the first try. Text normalisation was on, and cuDNN worked in bf16 on the T4 with no fallback needed.
- Synthesis matches `ming_gpu.py`:
  - seed 1234 before every call, cfg 2.0, temperature 0, prompt cache on.
  - Each preset gets one IP anchor of `ANCHOR_TEXT`. EMOTION calls use the anchor as their reference.
- Clone reference: Kokoro af_heart from the live API reading a neutral library notice (`REF_TEXT` in `ming_kaggle.py`). It is 14 s long and was trimmed and set to -20 LUFS as `reference.py` does.
- Post-processing used the Space pipeline (`finish_batch`) with the Ming profile: -16 LUFS, max 2 dB correction, pauses of 250 / 600 / 1200 ms. Output is WebM/Opus 48 kbps at 48 kHz, 3.1 MB for 280 clips, kept in scratchpad only.
- Conditions, each run on the 3 passages (294 items):
  - `ip`: one IP call per paragraph segment, which is what the app sends.
  - `ip_sent`: IP split per sentence, as a control for the split that the directed condition needs.
  - `directed`: per sentence, from `passages/directions.json`. Neutral sentences use IP. Emotion sentences use EMOTION on the anchor. Tagged sentences use TTS on the anchor with ANCHOR_TEXT as transcript.
  - `clone_heart`, `clone_heart_sent` and `clone_heart_directed` are the same three, with TTS or EMOTION on the af_heart reference.
- Measured with `prosody.py measure` and the frozen calibration from note 20.
  - Results: `tools/expressiveness/results/ming.json` and `ming.md`. The kernel log, with per-item GPU-s, is `results/ming_raw/ming_log.json`.

### Results (verified, EI with frozen calibration, single seed)
| label | suspense | dialogue | reflective | mean EI | % of human |
|---|---|---|---|---|---|
| Kokoro af_heart (note 20) | 0.60 | 0.39 | -0.02 | 0.32 | 68 |
| Kokoro bf_emma | -0.09 | -0.29 | -0.14 | -0.18 | 39 |
| Kokoro bm_george | -0.54 | -0.47 | -0.45 | -0.49 | 21 |
| Ming US_Emma ip | 0.32 | 0.28 | 0.24 | 0.28 | 65 |
| Ming US_Emma ip_sent | 0.29 | 0.46 | 0.16 | 0.30 | 66 |
| Ming US_Emma directed | 0.42 | 0.42 | 0.26 | 0.36 | 70 |
| Ming UK_George ip | 0.81 | 1.38 | 0.81 | 1.00 | 106 |
| Ming UK_George ip_sent | 0.80 | 1.18 | 0.55 | 0.84 | 97 |
| Ming UK_George directed | not measured: 7 tagged items silent, probe blocked | | | | |
| Ming clone af_heart | 0.52 | 0.40 | 0.01 | 0.31 | 67 |
| Ming clone af_heart sent | 0.24 | 0.42 | -0.23 | 0.14 | 57 |
| Ming clone af_heart directed | 0.08 | 1.14 | 0.31 | 0.51 | 78 |
| human (LibriVox) | 1.03 | 0.47 | 1.17 | 0.89 | 100 |

- Effect of direction, against the per-sentence control:
  - US_Emma: +0.06 overall (+0.13 / -0.04 / +0.10 per passage).
  - af_heart clone: +0.37 overall. Most of it is dialogue (+0.72), where per-phrase loudness sd rises from 1.93 to 2.91 dB. Suspense drops by 0.16.
  - With one seed and three passages, a difference of 0.1 is probably within run-to-run noise (assumed; Ming's seed variance was not measured).
- Pauses do not close the human gap. Ming's ln-pause sd is 0.32-0.39 against 0.58 for the readers, and its p10-p90 pauses fall in 150-670 ms. Most of the pause length comes from the pipeline's fixed boundary pauses, so a model swap alone cannot fix this.
- The clone keeps af_heart's pitch: median F0 196-207 Hz against 197-199 Hz for Kokoro. Its energy range is 20.7 dB against 20.2 dB.
- Why UK_George is high (assumed): it has 8.2 F0 jumps over 4 st per 1,000 voiced frames (human 6.8, Kokoro bm_george 0.3). It also has 78 (ip) and 204 (ip_sent) frames more than 7 st below the median, against 20 for bm_george. This fits creak at phrase ends, which pushes slope sd to 6.5-7.7 st/s against 3.9 for the readers. Check: `uv run tools/expressiveness/ming/f0check.py <audio roots>`. A listening check is open.
- `ip` vs `ip_sent`: splitting per sentence costs 0.16 EI on the clone and 0.16 on George. It is neutral on US_Emma. Per-sentence calls lose the paragraph-level prosody.

### T4 speed (verified, `ming_log.json`, 294 items)
- Setup took 104 s: pip 30 s, weight download 32 s, model load 43 s. Warm-up took 8.8 s for 3.2 s of audio.
- Synthesis took 2,421 GPU-s for 1,081 s of audio, so RTF 2.24 and 0.143 GPU-s per input char. ZeroGPU does 0.052 (note 08), so the T4 is 2.75× slower.
  - By task: IP 2.17-2.25, EMOTION 2.28-2.29 and TTS 2.19-2.28. Whole paragraphs run at RTF 2.17-2.19, single sentences at 2.25-2.29.
- Peak memory was 2.91 GB. The kernel ran on 2 T4s with 15 GB each but used one, and wall time was 2,575 s.
- Bulk-render capacity (assumed from Kaggle's 30 GPU-h/week quota): about 13 h of audio per week on one T4, or about 27 h with one worker per GPU. That fits offline catalogue renders. A live request cannot be served, since 1 s of audio takes 2.2 s.

### Problems
- UK_George + TTS on its IP anchor came back silent. All 7 tagged sentences were 0.3 s long with peak 0.0, while EMOTION on the same anchor and TTS on the US_Emma anchor worked. The app never plans TTS on a preset (`ming_plan.plan` raises), so this is a study-path failure and not a live bug. It does show that tags cannot be applied to a preset through its anchor without a check for silent output. Probe: see below.
- Kaggle allows at most 2 batch GPU sessions per account, and they were shared with the other bakeoffs (qwen3tts and indextts2.5 were running). The probe push was refused with "Maximum batch GPU session count of 2 reached" from 05:51 on.

### George probe (open, blocked)
- `ming_probe_kernel.py` is ready, filled with the 7 silent items, in `scratchpad/expr/ming/probe/kernel/` (kernel `lbrahimali/streams-ming-probe`). It does three things:
  - re-renders the UK_George anchor, plus a seed-99 variant;
  - runs 8 one-item diagnostics: the tagged text, untagged short and long text, raw_text, seeds 1 and 42, the seed-99 anchor, and the US_Emma anchor;
  - renders the 7 items with seed 1234, falling back to seeds 1, 42 and 7 until the peak is over 0.02.
- Pushes were refused every minute from 05:51 to 06:43 ("Maximum batch GPU session count of 2 reached"). streams-qwen3tts-bakeoff and streams-itts25-bakeoff held the 2 slots, both running since 05:45.
- Unblock: a free slot. Then run `ming_kaggle.py probe --work W` (it regenerates the kernel and pushes), `pull --work W --probe --wait`, `collect`, and `prosody.py measure`. `collect` swaps in the probe clips and logs the seed each one used.

## Open items
- Listen to Ming UK_George and confirm whether the creak is audible. That decides whether the 1.00 EI is real. Unblocked by 2 minutes of listening.
- Run 3 seeds per condition to measure Ming's run-to-run EI noise before trusting differences under 0.2. This costs about 40 min on a T4.
- Pause variety: change the pipeline's fixed pauses (250/600/1200 ms) to depend on the text. The Kokoro director work in `tools/expressiveness/director/` is the place to try it.
