---
title: nolabs engine
emoji: 🎧
colorFrom: gray
colorTo: indigo
sdk: gradio
sdk_version: "6.17.3"
python_version: "3.10"
app_file: app.py
pinned: false
license: mit
short_description: Private speech engine for nolabs-reader
preload_from_hub:
  - hexgrad/Kokoro-82M
  - inclusionAI/Ming-3.0-tts-0.5B
---

Private speech engine for [nolabs-reader](https://github.com/Coderia397/nolabs-reader). Kokoro-82M (Apache-2.0) runs on the
CPU for standard narration; Ming-3.0-tts-0.5B (MIT) runs on ZeroGPU for cloned and expressive voices. Ming inference code is
vendored in `vendor/ming3` from [inclusionAI/Ming-3.0-tts](https://github.com/inclusionAI/Ming-3.0-tts) (MIT) via the
torchaudio-free port in `mrfakename/ming-3.0-tts-0.5b`.

Endpoints (each takes and returns one JSON string): `render_cpu`, `render_gpu`, `design_voice`, `catalog`, `health`.
