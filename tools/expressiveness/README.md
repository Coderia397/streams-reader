# Expressiveness harness

Measures how varied a narration is (pitch, energy, pauses, rate) and blends it into one expressiveness index (EI), so voice and engine changes can be compared. Findings and the baseline: `notes/20-expressiveness-baseline.md`. The index definition is in the `prosody.py` docstring.

```sh
OUT=<scratch dir>/audio
CARGO_TARGET_DIR=<scratch>/target cargo run -q --release --manifest-path tools/expressiveness/segment/Cargo.toml \
  -- tools/expressiveness/passages/*.txt > tools/expressiveness/passages/segments.json   # app segmenter
uv run tools/expressiveness/render.py --out $OUT --control        # Kokoro via the live API (refuses ming:*)
uv run tools/expressiveness/fetch_human.py --out $OUT             # LibriVox ceiling excerpts
uv run tools/expressiveness/prosody.py monotone $OUT              # F0-flattened control
uv run tools/expressiveness/prosody.py measure $OUT --out tools/expressiveness/results/<name>.json \
  --md tools/expressiveness/results/<name>.md                     # frozen calibration; add --calibrate only to re-baseline
```

The audio layout is `<root>/<label>/<passage>/NN.webm` plus `meta.json`. Any new source (Ming, a prompt experiment) that writes the same layout can be measured.

Ming on a private Kaggle T4 (no ZeroGPU; `notes/32-ming-kaggle-study.md`):

```sh
W=<scratch>/ming
uv run tools/expressiveness/ming/ming_kaggle.py ref|jobs|push --work $W     # af_heart reference, jobs.json, dataset + kernel
uv run tools/expressiveness/ming/ming_kaggle.py pull --work $W --wait       # outputs to $W/out
uv run tools/expressiveness/ming/ming_kaggle.py probe --work $W            # re-render silent items (seed fallback), then pull --probe
uv run tools/expressiveness/ming/ming_kaggle.py collect --work $W --audio <scratch>/audio_ming
uv run tools/expressiveness/ming/f0check.py <audio roots>                   # F0 jump / creak check per label
```

Listening pack (joins existing renders, no new measurement; `notes/35-expressiveness.md`):

```sh
uv run tools/expressiveness/make_pack.py --scratch <scratch>/expr --out <scratch>/expr/pack
```
