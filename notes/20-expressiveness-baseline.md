# Expressiveness harness and Kokoro baseline

Finding. The harness `tools/expressiveness/` measures prosody without a transcript, so TTS and human audio are measured the same way, and blends eight features into an expressiveness index (EI). Kokoro's overall F0 spread already equals the LibriVox readers (2.1-3.4 st vs 2.7-4.2 st). The gap is phrase-level variety: pause-length variety (ln sd 0.36-0.42 vs 0.58), pitch-level shifts between phrases (0.94-1.54 st vs 1.80 st) and, for the male voices, energy range. af_heart is the most expressive Kokoro voice (EI 0.32, 68 % of the human ceiling) and bm_george the least (EI -0.49, 21 %).

## 2026-10-31

### Setup (verified)
- Passages: `tools/expressiveness/passages/{1-suspense,2-dialogue,3-reflective}.txt`, 656 / 610 / 633 bytes, written for this study.
- Segments: `passages/segments.json` was produced by `tools/expressiveness/segment/`, a small Rust bin that calls `nolabs_core::text` (normaliser, Kokoro profile 120-320, sentences). The result was 3 / 6 / 3 segments, one per paragraph, all with boundary `paragraph`.
- Renders: `render.py` sent one POST /api/render per voice and passage to https://listen.streams.watch with priority interactive and emotion neutral.
  - The passphrase login still works on the live Worker (POST /api/session returned 204 with x-nolabs-api 1 and 2), although the repo code says the route moved to Supabase. The live deploy is older than the repo. Verified 2026-10-31.
  - Total: 15 sets (5 labels x 3 passages), 60 Kokoro clips, about 9.5 min of audio. Wall time was 11-87 s per set; the first two sets were cold.
  - No ming:* voice was requested. The script refuses any non-Kokoro voice.
- Human references: `fetch_human.py` cut 75 s excerpts from archive.org LibriVox, re-encoded at 32 kbps:
  - Poe, The Tell-Tale Heart, 9:00-10:15 (`12_creepytales_1206_librivox`).
  - Austen, Pride and Prejudice ch. 1, 0:35-1:50 (`solo_pride_librivox`).
  - Thoreau, Walden ch. 5, 1:00-2:15 (`walden_librivox`).
  - These texts and speakers differ from the test passages. They are a ceiling matched by genre only (assumed adequate). The content of the excerpts was not checked by ear or by ASR (open).
- Controls:
  - `control_sentence`: af_heart with every boundary set to `sentence`, as specified.
  - `control_monotone`: the same audio with F0 flattened to the passage median by Praat PSOLA (`prosody.py monotone`).
- Audio lives in `scratchpad/expr/audio/` (4.0 MB, 75 WebM files including the controls and human clips; not in the repo).

### Index definition (decision, by the assistant under the task's delegation)
- Pitch features (4):
  - F0 sd, in semitones relative to the passage median.
  - F0 range, p95-p5.
  - sd of the per-phrase F0 slopes (Theil-Sen).
  - sd of the per-phrase median F0.
- Energy features (2):
  - p90-p10 of the speech-frame RMS dB.
  - sd of the per-phrase mean dB.
- Timing features (2):
  - sd of the ln of the pause durations.
  - sd of the per-phrase syllables per second.
- How the index is computed:
  - Each feature becomes z = clip((x - mu) / sigma, -3, 3). mu and sigma are frozen in `tools/expressiveness/calibration.json` from the 21 baseline passages.
  - EI = 0.5 × pitch + 0.25 × energy + 0.25 × timing, where each group is the mean of its z values.
  - % of human = 100 × (EI - EI_monotone) / (EI_human - EI_monotone). The anchors are -0.861 and 0.889.
- Rejected:
  - Equal thirds per group. The monotone control (-0.48) scored above bm_george (-0.67), because the control keeps af_heart's energy and pauses.
  - Contour-shape sameness, the mean pairwise r of per-phrase F0 contours (Kokoro 0.20-0.57, human 0.10-0.34). The monotone control gives r ≈ 0, which would rank it as varied.
- Phrases are inter-pausal units: speech runs between silences of at least 150 ms. They stand in for sentences.
- Syllable nuclei follow De Jong & Wempe, tuned to a 1 dB dip, 50 Hz and a voicing check. They come to 0.86 × the text syllables, with sd 0.05 over 12 Kokoro passages. Three settings were compared: dips of 1.0, 1.5 and 2.0 dB. The first draft (2 dB, strict voicing) found only 0.70 of the text syllables.

### Validation (verified on this run)
- Order of EI by voice: monotone -0.86 < bm_george -0.49 < bf_emma -0.18 < am_michael 0.01 < control_sentence 0.28 ≈ af_heart 0.32 < human 0.89. The two anchors are in the expected order.
- `control_sentence` is not flat. Kokoro ignores the boundary during synthesis (`space/nolabs_engine/engines/kokoro_engine.py`), so only the pause the pipeline appends changes, from 600 to 250 ms. The EI moves by -0.04 and the ln-pause sd by 0.39 → 0.32. The flat F0 control is `control_monotone`.
- The phrase proxy only loosely tracks real sentences. Over 15 Kokoro passages, the correlation with the measures computed from word timings (`sentence_check` in baseline.json) is r = 0.25 for slope sd and r = 0.34 for rate sd.
- Before the per-phrase slopes were made robust (plain least squares, phrases from 0.3 s), one pause near the 150 ms threshold changed af_heart dialogue slope sd from 3.6 to 6.7 between two nearly identical renders. Theil-Sen and a 0.5 s minimum phrase length fixed this: 3.3 vs 3.5.

### Baseline (`tools/expressiveness/results/baseline.json`, `baseline.md`)
| voice (mean of 3 passages) | F0 sd st | F0 range st | slope sd | F0 level sd | energy dB | energy level sd | ln-pause sd | rate sd | EI | % of human |
|---|---|---|---|---|---|---|---|---|---|---|
| af_heart | 2.9 | 9.3 | 3.1 | 1.54 | 20.2 | 1.85 | 0.39 | 0.85 | 0.32 | 68 |
| bf_emma | 2.1 | 6.8 | 2.7 | 0.99 | 20.5 | 1.57 | 0.42 | 0.62 | -0.18 | 39 |
| am_michael | 3.3 | 10.7 | 3.4 | 1.44 | 18.8 | 1.31 | 0.38 | 0.66 | 0.01 | 50 |
| bm_george | 2.7 | 9.2 | 2.9 | 0.94 | 17.1 | 1.19 | 0.36 | 0.61 | -0.49 | 21 |
| control_sentence | 2.8 | 9.2 | 3.3 | 1.51 | 20.4 | 1.89 | 0.32 | 0.87 | 0.28 | 65 |
| control_monotone | 0.4 | 0.3 | 0.0 | 0.01 | 20.8 | 1.76 | 0.36 | 0.89 | -0.86 | 0 |
| human (LibriVox) | 3.3 | 10.4 | 3.9 | 1.80 | 21.8 | 1.99 | 0.58 | 0.80 | 0.89 | 100 |

- Kokoro pauses cluster between 250 and 700 ms (p10-p90 in every passage). The human readers range from 160 to 1,600 ms. The Walden reader's median pause is 700 ms.
- Kokoro's EI does not rise for the passages that need it. af_heart scores 0.60 / 0.39 / -0.02 across suspense, dialogue and reflective. bm_george stays at -0.54 to -0.45 in all three. The human readers score highest on the reflective excerpt (1.17) and on suspense (1.03).
- Cloudflare Gemma and Kaggle were not used. The baseline did not need them.

## Open items
- Check the human excerpts' content (listen, or run Workers AI whisper at about 46 neurons per minute). This unblocks trusting the ceiling.
- For a sentence-level human ceiling, either force-align the human clips (whisper word timestamps) or read the same three passages aloud. The latter needs the owner's recording.
- Recompute calibration.json only on purpose. Later runs (Ming, prompting experiments) must use `measure` without `--calibrate`, so EI stays comparable with this baseline.
