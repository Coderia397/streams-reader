# Credits and licences (media/)

Everything in the Streams Reader films and spot illustrations is either made here or comes from the sources below.

## Music
- `assets/music/bed.ogg` (used in `shots/promo.json`) and `assets/music/bed-alt.ogg` (second take, unused).
- Model: ACE-Step v1 3.5B (`ACE-Step/ACE-Step-v1-3.5B`), licence Apache-2.0, code https://github.com/ace-step/ACE-Step.
- Where it ran: a private Kaggle notebook (`lbrahimali/streams-reader-music-bed`, version 4) on a free Tesla T4, bfloat16 with CPU offload and cuDNN off. No Hugging Face ZeroGPU quota was used. The notebook source is `music/music.py`; generation parameters are in `assets/music/bed.params.json`.
- Prompt: "ambient, lo-fi, cinematic, calm, hopeful, warm felt piano, soft analog pads, gentle sub bass, light brushed drums, vinyl crackle, night, dreamy, instrumental, 78 bpm"; lyrics `[instrumental]`; 60 s; 60 steps; Euler; APG guidance 15; seeds 20261030 (bed) and 7741 (bed-alt).
- An earlier run fell back to MusicGen medium (CC-BY-NC weights) when ACE-Step failed to install; that output was not used and is not kept.

## Voices
- Voice-over and the book narration are rendered by Streams Reader itself (`POST /api/render` on listen.streams.watch).
  Promo voice-over: the expressive engine, inclusionAI Ming-3.0-tts-0.5B on the app's ZeroGPU Space (preset `ming:US_Emma`,
  per-line emotion; `assets/vo/en-ming/`), MIT licence (notes/03).
  Fallback take: Kokoro-82M (Apache-2.0) `kokoro:af_heart` (`assets/vo/en/`). The app reads Alice with `kokoro:bf_emma`.

## Text
- *Alice's Adventures in Wonderland*, Lewis Carroll (1865), public domain.

## Code and type
- three.js r179 (0.179.1), MIT, vendored in `scene/vendor/three/` (core, postprocessing, RoomEnvironment, RoundedBoxGeometry, RectAreaLight uniforms).
- Inter and Literata fonts (SIL Open Font License 1.1), copied from `crates/web/assets/fonts/`.
- Streams Reader wordmark and mark from `brand/`.

## Character, sets and props
- The voxel character, rooms, park, props and every texture are original, built in code in `scene/js/` (no third-party models or likenesses).
