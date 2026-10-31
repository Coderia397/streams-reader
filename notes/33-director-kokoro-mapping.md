# LLM director mapped onto Kokoro-82M

Finding + decision. Code: `tools/expressiveness/director/`. Results: `tools/expressiveness/results/director.{json,md}`, raw per-seed files in `results/director-runs/`.
Conclusion: the director (gemma-4-26b on Workers AI) moves af_heart from EI 0.31 to 0.67 on its own voice, and bm_george from -0.57 to 0.27. The mapping is one Kokoro call per sentence, a 4-12 % speed change, the director's pause after each sentence and a partner-voice blend. Character voices add a further +0.23 on dialogue (0.90 overall), but the index overstates that part. Emphasis edits, in-call duration/F0 hooks and gain added nothing measurable.

## 2026-10-31

### Director (verified on real output)
- The prompt and schema are in `director/prompt.md` (PROMPT_VERSION 1) and `director/schema.json`, sent as `response_format` JSON schema. Each sentence gets {sentence_idx, mode, emotion, confidence, intensity, pace, pause_after_ms, emphasis_words, tags}. The passage also gets genre, characters (name, gender, age) and quote → speaker.
- `director.py` validates the output the way the Rust side would: words are anchored within ±3 indices, emotion changes on the narrator track need 2 sentences of hysteresis, and tags are dropped in nonfiction. Responses are cached in `director/cache/` by sha256 of the request.
- Cost per passage: suspense 52.1 neurons and 26.8 s; dialogue 86.0 neurons and 64.9 s; reflective 52.1 neurons and 24.6 s. That is 190 neurons for 1,899 characters, about 0.10 neuron per character. Taken from `usage` in `director/out/*.json`, one call each.
- At that rate a 600k-character book costs about 60k neurons, 6 days of the shared 10k/day free tier (assumed linear). Directing whole books on Workers AI free is not viable. It fits only the first chapters or a cheaper model (open).
- Speaker attribution is 8 of 8 quotes correct in the dialogue passage, checked by reading. The speech tags "she said" and "he began" are split off and read by the narrator.
- Emotions read plausibly. The suspense passage goes from neutral to fearful at 0.3-0.6. Martha is disgusted, then angry at 0.6-0.8. Tom is mock-happy at 0.3-0.4, then sad, then fearful.
- Hysteresis held one narrator flip (s7, fearful → angry). Six emphasis indices were off by one and were re-anchored.
- No tags were produced in any passage. The reflective passage was classed "fiction", although the prompt's nonfiction rule names reflection. With no tags, this had no effect here (open for real essays).

### Local Kokoro (verified)
- kokoro 0.9.4 on CPU, misaki[en], en_core_web_sm, weights `hexgrad/Kokoro-82M@f3ff3571791e39611d31c381e3a41a3af07b4987` (notes/13). The env was made with `uv venv -p 3.12` from `director/requirements.txt`.
- `kokoro_ctrl.Engine._forward` without controls is bit-identical to the stock forward. `selfcheck()` returned max abs difference 0.0 on 2026-10-31.
- Blending is a weighted mean of the voice packs ([510, 1, 256] tensors).
- Kokoro's source excitation is random. Each call is seeded by (seed, text), so conditions differ only by the mapping. af_heart was run with 3 seeds; the sd over seeds is in the table.
- Post-processing is the Space's own trim, segment levelling (-16 LUFS, ±4 dB) and -1.5 dBFS ceiling. The output is WebM/Opus at 32 kbps.

### Conditions
- k_plain: today's app, one call per paragraph segment.
- k_sent: one call per sentence, boundary pauses.
- a_pause: k_sent plus the director's pause_after_ms.
- a_speed: a_pause plus Kokoro `speed` per sentence. |Δ| = 4 % + 8 % × intensity, with the direction from pace, or from emotion when pace is normal and intensity ≥ 0.4. Used range 0.912-1.104.
- a_emph: a_speed with the call split before the first emphasis word (at least 3 words in, not the last word), 90 ms gap.
- a_blend: a_speed plus a partner voice at weight 0.45 × intensity, when intensity ≥ 0.3:
  - af_heart: af_bella for happy, surprised and angry; af_nicole for fearful, sad and disgusted.
  - bm_george: bm_fable and bm_lewis.
  - 22 of 43 calls were blended.
- a_chars: a_blend plus character voices by gender from the same accent family, never the narrator's voice. With af_heart narrating, Martha was af_bella and Tom am_michael (blended with am_puck or am_echo).
- b_*: paragraph calls with in-call hooks (`kokoro_ctrl`): per-word duration for speed (b_speed), +25 % duration and +50 ms before emphasis words (b_emph), sentence register shift and +1.5 st accent (b_f0), character calls (b_chars), and emotion gain (b_full).

### Results (verified; `results/director.md`; EI is the mean of 3 passages; af_heart rows are the mean of seeds 1-3, george rows are seed 1)
| condition | EI (sd) | Δ | % human | suspense | dialogue | reflective | F0 level sd | ln-pause sd | rate sd | pitch breaks/min | WER % |
|---|---|---|---|---|---|---|---|---|---|---|---|
| k_plain | 0.31 (0.03) | 0 | 67 | 0.43 | 0.51 | 0.00 | 1.51 | 0.38 | 0.86 | 17.7 | 0.1 |
| k_sent | 0.42 (0.03) | +0.11 | 73 | 0.55 | 0.37 | 0.35 | 1.29 | 0.38 | 0.93 | 28.9 | 0.0 |
| a_pause | 0.50 (0.08) | +0.19 | 78 | 0.63 | 0.42 | 0.46 | 1.29 | 0.47 | 0.90 | 28.9 | 0.0 |
| a_speed | 0.56 (0.02) | +0.24 | 81 | 0.51 | 0.64 | 0.52 | 1.30 | 0.45 | 0.94 | 28.2 | 0.1 |
| a_emph | 0.53 (0.01) | +0.22 | 80 | 0.52 | 0.72 | 0.36 | 1.22 | 0.52 | 0.97 | 28.4 | 0.7 |
| a_blend | 0.67 (0.05) | +0.35 | 87 | 0.67 | 0.81 | 0.52 | 1.44 | 0.46 | 1.10 | 30.5 | 0.7 |
| a_chars | 0.90 (0.02) | +0.59 | 101 | 0.67 | 1.52 | 0.52 | 2.65 | 0.47 | 1.00 | 27.6 | 0.2 |
| b_speed | 0.51 (0.03) | +0.19 | 78 | 0.58 | 0.52 | 0.42 | 1.36 | 0.52 | 0.88 | 12.5 | 0.2 |
| b_emph | 0.51 (0.01) | +0.20 | 78 | 0.51 | 0.79 | 0.23 | 1.37 | 0.55 | 0.86 | 17.9 | 0.3 |
| b_f0 | 0.56 (0.04) | +0.24 | 81 | 0.48 | 0.91 | 0.28 | 1.40 | 0.54 | 0.87 | 18.0 | 0.2 |
| b_chars | 0.86 (0.02) | +0.54 | 98 | 0.48 | 1.80 | 0.28 | 2.58 | 0.56 | 0.79 | 18.2 | 0.0 |
| b_full | 0.91 (0.03) | +0.60 | 101 | 0.44 | 2.01 | 0.28 | 2.64 | 0.58 | 0.85 | 19.4 | 0.0 |
| k_plain_george | -0.57 | 0 | 16 | -0.60 | -0.56 | -0.55 | 0.94 | 0.33 | 0.57 | 5.6 | 0.8 |
| a_speed_george | 0.14 | +0.71 | 57 | -0.08 | 0.32 | 0.18 | 1.12 | 0.49 | 0.74 | 1.4 | 0.6 |
| a_blend_george | 0.27 | +0.84 | 65 | 0.16 | 0.48 | 0.18 | 1.17 | 0.49 | 0.72 | 2.9 | 0.6 |
| a_chars_george | 0.77 | +1.34 | 93 | 0.16 | 1.97 | 0.18 | 1.72 | 0.47 | 0.80 | 4.0 | 0.3 |

Human reference (note 20): F0 level sd 1.80, ln-pause sd 0.58, rate sd 0.80. Per passage it is 1.03 / 0.47 / 1.17.

### What each mapping does
- Sentence calls (+0.11) and director pauses (+0.08) are the cheapest gains. The ln-pause sd goes from 0.38 to 0.47 (human 0.58). Reflective prose rises from 0.00 to 0.46. The a_pause sd over seeds is 0.08, so this step is less certain than the others.
- Speed (+0.06 on af_heart) lifts dialogue (0.42 → 0.64) and reflective (0.46 → 0.52). It lowers suspense (0.63 → 0.51): the slow sentences shrink the per-phrase pitch-level spread there. On bm_george, sentence calls plus pauses plus speed give +0.71, slope sd 2.9 → 5.4 (above the human 3.9; assumed to come from the per-call sentence-final falls).
- The blend (+0.11) is the largest gain on narration without dialogue. Suspense goes from 0.51 to 0.67, mostly through F0 level sd (1.30 → 1.44) and rate sd (0.94 → 1.10). Neutral sentences are never blended, so reflective is unchanged.
- Character voices (+0.23 overall, dialogue 0.81 → 1.52): a male character voice shifts the median F0 from 198 to 155 Hz, so dialogue F0 level sd goes from 1.52 to 5.14 st, against 1.67 for the human dialogue reader. The z-score is clipped at 3, but the index still rewards this as "expressive". The real gain for listeners is speaker clarity, which the index does not measure. The "101 % of human" is therefore inflated. On narration-only passages a_chars equals a_blend.
- Emphasis by splitting (-0.03): energy level sd drops from 1.89 to 1.46, and it caused the only content WER error ("flat and" → "flattened", seed 3). Emphasis by duration (b_emph, +0.01) changed nothing measurable. Dropped.
- In-call hooks (b_*): with the same director input they score lower than the sentence-call path on suspense (0.44-0.58 vs 0.67) and reflective (0.23-0.42 vs 0.52). They need a custom Kokoro forward in the Space. Not worth it.
- The F0 register shift (b_f0, +0.05 over b_emph) and gain (b_full, +0.05 over b_chars) only help dialogue. Gain on sentence calls is untested (open).

### Artefacts (verified with `artefacts.py`, `breaks_where.py`, `asr_check.py` Parakeet TDT 0.6B v3)
- Clipping: 0 model samples at |x| ≥ 0.999 in any condition. The highest raw peak is 0.726 (a_chars_george, dialogue). The peak ceiling took 5.0-6.3 dB off the planned gain in every condition, k_plain included.
- Joins: the log-mel distance across call joins is 7.3-8.7 dB, the same as across Kokoro's own sentence gaps in k_plain (8.0-8.1 dB). Clicks: the largest sample step within 3 ms of an edit is 0.003 × the clip's p99.9 step on the sentence path and 0.22 × on the hook path; none is above 1.
- Weakness of that metric: deliberate voice changes (a_chars) only reach 8.2 dB. The metric shows that no join is worse than a natural sentence gap. It cannot show that a blend change sounds natural (open: listening).
- Hook path: 12-17 % of inserted in-call silences were cut where the signal was above -45 dBFS (risk of a clipped word tail). The sentence path appends silence after a trimmed call: 0 %.
- Pitch breaks (>4 st jumps between voiced 10 ms frames): 28-31/min on sentence calls vs 12-19/min on paragraph calls (k_plain varies 14-20/min across seeds). The counts are small, 19-29 per condition per seed. Only 11-25 % fall within 120 ms of a speech edge. The excess comes from k_sent itself (28.9) and does not grow with speed or blend. It was not checked by ear (open).
- WER is 0.0-0.8 % everywhere. Every miss but one is a normalisation difference ("11" vs "eleven", "any more" vs "anymore", "slow" vs "slowly", "ask" vs "asked"). The exception is the a_emph error above.
- Duration: sentence calls make af_heart passages 1-3 s longer (director pauses 6.2 s vs 1.8 s of boundary pauses, while trimmed in-call gaps save about 3 s). bm_george comes out 6 s longer per passage (his per-call speech is 6 % longer).

### Decision (2026-10-31, by the assistant under the task's delegation)
- Recommended Kokoro mapping: `kokoro_map.RECOMMENDED` = a_chars.
  - One Kokoro call per sentence. Quotes and speech tags are separate calls; the tag is read neutral by the narrator, with 150 ms between them.
  - `speed` = 1 ± (4 % + 8 % × intensity). The direction comes from pace slow/fast, or else from the emotion (angry, surprised +1; happy, fearful +0.5; disgusted -0.5; sad -1) when intensity ≥ 0.4.
  - The director's pause_after_ms is appended as silence, replacing the fixed 250/600 ms boundary pauses.
  - Partner blend = 0.45 × intensity when intensity ≥ 0.3. The partner is animated (bella/isabella/puck/fable) for happy, surprised and angry, and soft (nicole/echo/lewis) for fearful, sad and disgusted.
  - Character voices by gender, from the narrator's accent family. Unknown gender stays on the narrator voice.
- Reason: it is the largest gain that uses only Kokoro's public API (speed, voice tensor). It adds no joins worse than a natural gap, no clipping and no WER cost.
- Rejected:
  - Emphasis split: it lowers EI and caused the one content error.
  - In-call duration/F0 hooks: lower EI on narration, and a custom forward plus loud gap cuts.
  - Gain: untested on the sentence path.
  - Blend weights above 0.45: not tried. Nicole at full weight is whispery (assumed), so they are not proposed.

### Open
- Listening check of a_blend / a_chars against k_plain for blend shifts, pitch breaks and the bm_george slope overshoot. Unblocked by the owner listening to `scratchpad/expr/s1/{k_plain,a_blend,a_chars}` (not in the repo) or a fresh render.
- Director cost: 0.10 neuron/char makes whole books infeasible on the shared 10k/day. Unblocked by a decision: direct only the first N chapters, a smaller Workers AI model, or batch the job on Kaggle.
- Nonfiction classification of reflective essays: needs a real essay test passage.
- Speaker-clarity metric: the EI rewards character voices for the pitch gap. A per-turn speaker-change check would measure the real effect.
- Space work: sentence-level calls with `speed` and the blended pack. The Rust side needs the director's anchoring and hysteresis (`director.py validate` is the reference). Owners of space/ and crates/core decide.
