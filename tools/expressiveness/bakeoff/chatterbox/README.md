# Chatterbox bakeoff (2026-10-31)

Chatterbox original 0.5B, Turbo 350M and Multilingual V3 0.5B, cloned from two LibriVox readers, rendered on a private Kaggle T4 (kernel `lbrahimali/streams-chatterbox-bakeoff` v1, 36 min) and scored with `../../prosody.py` (frozen calibration). Findings and decisions: `notes/37-chatterbox-bakeoff.md`.

Layout
- `audio/<label>/<passage>/NN.webm` + `meta.json`: app pipeline (levelled to -16 LUFS, pauses 250/600 ms), WebM/Opus 48 kbps. `_dp` labels use the director's pause_after_ms.
- `refs/ref_{female,male}.webm`: the clone references (Opus 64 kbps copies of the 24 kHz WAV sent to Kaggle). Source in `results/kaggle/refs.json`.
- `results/chatterbox.{json,md}`: prosody.py measure. `results/summary.{json,md}`: EI + Whisper WER + UTMOS + flags (analyze.py). `results/pitch.json`: pitch breaks / F0 median / 6-8 kHz energy. `results/kaggle/`: kernel logs (cb_log.json per-item GPU time, install_log.json with the full `uv pip freeze`), asr.json, mos.json, conditions.json.

Re-run
```sh
W=<scratch>/cb
uv run select_ref.py --out $W/dataset
uv run cb_kaggle.py jobs --work $W --score <baseline audio root>
uv run cb_kaggle.py push --work $W          # waits for a free GPU slot (2 per account)
uv run cb_kaggle.py pull --work $W --wait
uv run cb_kaggle.py collect --work $W --audio audio
uv run ../../prosody.py measure audio --out results/chatterbox.json --md results/chatterbox.md
uv run analyze.py --work $W --audio audio --base <baseline audio root> --prosody results/chatterbox.json --md results/summary.md --out results/summary.json
```
The raw 24 kHz FLAC outputs were deleted locally after scoring; `pull` downloads them again from the kernel output.
