# Director prototype and Kokoro mapping

The LLM director (notes/06 v2 phase 2) annotates each sentence; `kokoro_map.py` turns that into what Kokoro-82M can do; `render_local.py` renders it with a local Kokoro that is driven like the Space; the harness (`../prosody.py`) and `artefacts.py` / `asr_check.py` measure the result. Findings: `notes/33-director-kokoro-mapping.md`.

| file | what |
|---|---|
| `prompt.md`, `schema.json` | director prompt (PROMPT_VERSION 1) and the JSON schema sent as `response_format` |
| `director.py` | builds the request from `passages/segments.json`, calls Workers AI `@cf/google/gemma-4-26b-a4b-it`, caches in `cache/`, then validates like the Rust side would: anchoring (±3 words), hysteresis, tag rules |
| `out/<passage>.json` | validated director output used by the renders |
| `kokoro_map.py` | the mapping (speed, pauses, blends, casting, emphasis, register, gain) and the conditions |
| `kokoro_ctrl.py` | Kokoro with per-word duration / gap / F0 hooks; identical to the stock forward when unused (`selfcheck`) |
| `render_local.py` | renders conditions in the harness layout using the Space's own trim and levelling code |
| `artefacts.py` | joins, clicks, pitch breaks, clipping |
| `asr_check.py` | WER with Parakeet TDT 0.6B v3 (ONNX, local) |
| `breaks_where.py` | where pitch breaks sit (near speech edges or inside speech): `uv run breaks_where.py <root> <cond>...` |
| `aggregate.py` | per-seed prosody + artefacts + WER into `results/director.{json,md}` |

```sh
uv run tools/expressiveness/director/director.py                     # cached; --refresh spends ~190 neurons
uv venv -p 3.12 <env> && uv pip install -p <env>/bin/python -r tools/expressiveness/director/requirements.txt
# Kokoro snapshot at hexgrad/Kokoro-82M@f3ff3571 (config.json, kokoro-v1_0.pth, voices/*.pt) in <weights>
<env>/bin/python tools/expressiveness/director/render_local.py --weights <weights> --out <scratch>/local
uv run tools/expressiveness/prosody.py measure <scratch>/local --out tools/expressiveness/results/director.json --md tools/expressiveness/results/director.md
uv run tools/expressiveness/director/artefacts.py <scratch>/local --out tools/expressiveness/results/director-artefacts.json
uv run tools/expressiveness/director/asr_check.py <scratch>/local --out tools/expressiveness/results/director-asr.json
# the reported table: 3 seeds (--seed 1..3), measured per seed, then
uv run tools/expressiveness/director/aggregate.py --prosody results/director-runs/p{1,2,3}.json ... --out results/director.json --md results/director.md
```

Notes: the espeak-ng fallback is disabled (it aborts on macOS x86_64); out-of-dictionary names go in `kokoro_ctrl.EXTRA_LEXICON`, and `synth` raises on any word left without phonemes. `asr_check.py` forces the CPU execution provider (CoreML returns garbage).
