# Streams Reader film studio

A reusable 3D scene for Streams Reader videos and illustrations: a voxel character (the Streams Reader mascot) with
Streams-blue over-ear headphones, a phone whose screen is a real capture of the app, two sets (a reading room at night,
a park at golden hour / day / dusk, or a street), cinematic cameras, depth of field, bloom and grading. A video is a
JSON timeline; one command renders it to AV1 WebM + H.264 MP4 + poster, with the voice-over and music mixed in.

Requirements: `uv`, Google Chrome (any channel in /Applications), and a GPU (ANGLE/Metal is used; `--swiftshader`
works without one, slowly). No Node. ffmpeg is fetched by `uvx --from imageio-ffmpeg` on first use (or set `FFMPEG`).
Frames are never written to disk: Chrome renders each frame on demand and the screenshot is piped into ffmpeg.

## Make a video

    uv run --script media/render.py media/shots/promo.json            # -> media/dist/promo/promo.{webm,mp4,poster.jpg,en.vtt}
    uv run --script media/render.py media/shots/promo.json --stills 3,15.5   # PNG stills to check framing
    uv run --script media/render.py media/shots/promo.json --range 20:26 --scale 0.5   # a quick rough section

Copy a shot from `media/shots/` and edit it. Times inside a segment are relative to the segment start.

```jsonc
{
  "name": "my-video", "fps": 30, "duration": 12, "resolution": [1920, 1080], "seed": 11,
  "loop": false,                 // true: every motion snaps to the duration so the clip loops seamlessly
  "poster_t": 4.0, "motion_blur": 1, "encode": {"webm": {"codec": "av1", "crf": 34}, "mp4": {"crf": 21}},
  "segments": [{
    "start": 0, "end": 12,
    "environment": {"preset": "room"},          // or {"preset": "park", "options": {"time": "golden|day|dusk", "setting": "park|street"}}
    "transition_in": {"type": "whip", "duration": 0.8, "direction": [1, 0]},   // cut (default), dissolve, whip/slide
    "lighting": {"screen_glow": 0.3, "screen_area": 5, "exposure": 1.0, "lights": {"lamp": {"intensity": 3}}},
    "look": {"dof": {"aperture": 0.006}, "bloom": {"strength": 0.5}, "grade": {"gain": [1.03, 1, 0.95], "vignette": 0.4}},
    "fade": [{"t": 0, "v": 1}, {"t": 1.5, "v": 0}],          // fade from/to black
    "character": {
      "anchor": "seat",                          // room: seat; park: path_start, bench
      "keys": [{"t": 0, "pos": [0, 0, 0], "rotY": 0}],     // offset from the anchor, facing (deg)
      "headphones": "ears",                      // ears | neck | earbuds | none
      "cable": true,                             // wired: cable from the left cup to the phone
      "walk": {"speed": 0.85, "heading": 90, "cadence": 0.85, "arms": 0.45},
      "poses": [{"t": 0, "pose": "sit_relaxed"}, {"t": 3, "pose": "sit_relaxed+look_up", "blend": 0.8}],
      "expressions": [{"t": 0, "expr": "absorbed"}, {"t": 4, "expr": "soft_smile"}],
      "hold": [{"t": 0, "item": "phone", "hold": "read_one"}],   // item: phone | book | none
      "hold_yaw": 20, "phone_scale": 1.6, "pov": false           // pov hides the head for an eye-line insert
    },
    "screen": {"clip": "assets/screen/app-alice.webm", "in": 3.0, "on": [{"t": 0, "v": [1]}], "loop_xfade": 0.8},
    "camera": {
      "handheld": 0.3, "follow": false,          // follow: keys are relative to the walking character
      "focus": [{"t": 0, "on": "face"}, {"t": 2, "on": "phone"}],   // rack focus: face | phone | chest | eye | metres
      "keys": [
        {"t": 0, "pos": [1.3, 1.3, 0.45], "target": [-0.12, 1.0, -1.3], "fov": 33},
        {"t": 6, "orbit": [30, 4, 1.6], "target": "face", "aim": [0, -0.1, 0], "fov": 30, "ease": "inout"},
        {"t": 9, "pos": "eye", "offset": [0, 0.06, 0.05], "target": "phone", "fov": 30, "ease": "step"}
      ]
    },
    "props": [{"type": "mug", "attach": "handR", "pos": [0, 0, 0], "rot": [0, 0, 0], "scale": 1}]
  }],
  "overlays": [{"start": 8, "end": 12, "type": "title", "text_id": "title", "sub_id": "sub", "position": "upper"},
               {"start": 10, "end": 12, "type": "endcard", "sub_id": "url"}],
  "audio": {
    "music": {"file": "assets/music/bed.ogg", "gain_db": -2, "fade_out": 3},
    "vo": [{"id": "late", "at": 1.6}],          // line ids from media/vo/script.<lang>.json
    "narration": [{"file": "assets/screen/app-alice.narration.webm", "at": 5, "in": 3.0, "dur": 4, "gain_db": 0}],
    "loudness": -16, "true_peak": -1.5
  }
}
```

Vocabulary: poses `stand, sit, sit_relaxed, sit_slouch, sit_cross, sit_floor, read, look_up, look_down, wave,
arms_rest, arms_lap, arms_up, thumbs_up, jump, scratch_head` (combine with `+`, add `"joints": {"head": [x, y, z]}`);
expressions `neutral, calm, absorbed, soft_smile, smile, happy, sleepy, tired, surprised, curious, focused, content,
speak, look_up_calm`; holds `read, read_one, read_low, read_high, show, walk, side, lap, pocket, book_read, book_lap`;
camera easing `inout, linear, in, out, smooth, step`. Camera `orbit` is `[yaw, pitch, distance]` around the target,
yaw 0 in front of the character, 90 on its left.

Mix: voice-over and narration form one voice bus; the music is ducked under it with `sidechaincompress`, then the mix
is normalised with two-pass `loudnorm` to -16 LUFS integrated, -1.5 dBTP. Captions (WebVTT) are written from the
voice-over lines. Delivery: AV1 (SVT-AV1) + Opus in WebM, H.264 + AAC in MP4, limited-range BT.709 yuv420p.

## Spot illustrations

    uv run --script media/render.py --spots                      # every media/spots/*.json
    uv run --script media/render.py --spot media/spots/welcome.json

Each preset is a pose, expression, props and camera in a neutral studio; output is transparent PNG + WebP at 1024 and
512 px with a soft contact shadow, in `media/dist/spots/`.

## Re-capture the app screen

The phone shows a real capture of listen.streams.watch. When the app changes (the Streams-blue redesign), re-run:

    uv run --script media/capture/capture_app.py                 # settings in media/capture/app-screen.json

It logs in with the passphrase from `~/.config/nolabs/app.env` (never printed), opens the document at 390x844 (DPR 2,
mobile emulation, throwaway Chrome profile), picks the voice, presses Play and records ~26 s: frames from polled
screenshots (about 7 fps; the app only changes per word) resampled to 30 fps VP9, plus the app's own audio recorded
from its media element (`*.narration.webm`). The document's saved reading position is restored afterwards. Shots
reference the clip by path and in-point, so after a re-capture check the `in` points with `--stills`.

## Other languages

1. Copy `media/vo/script.en.json` to `script.<lang>.json`, translate `text` and `cards`, keep the ids, set `voice`.
2. `uv run --script media/vo/render_vo.py --lang <lang>` (Kokoro voices only; the GPU voices are reserved).
3. `uv run --script media/render.py media/shots/promo.json --lang <lang> --out media/dist/promo-<lang>`.
Line timing in the shot is by id; if a translation runs long, move the next `at` in a copy of the shot.
`media/vo/languages.json` lists which Kokoro voice fits each UI language; only English voices are in the app's
catalogue today, so the other languages wait for those voices to be exposed by the API.

## Music

`media/music/music.py` is the Kaggle notebook that generated the bed with ACE-Step (see CREDITS.md). To make a new
bed: edit `PROMPT`/seeds, then `uvx kaggle kernels push -p media/music` (token in `~/.config/streams/kaggle.env`),
wait for COMPLETE, `uvx kaggle kernels output lbrahimali/streams-reader-music-bed -p <dir>`.

## Layout

- `scene/` the Three.js page (`index.html`, `js/main.js` timeline + renderer, `character.js`, `props.js`,
  `props_spot.js`, `env_room.js`, `env_park.js`, `env_studio.js`, `post.js`, `overlay.js`), vendored three.js.
- `render.py` renderer; `tools/` (CDP driver, ffmpeg locator, audio mix, API client); `capture/`; `vo/`; `music/`.
- `shots/` timelines; `spots/` illustration presets; `assets/` inputs (screen clip, VO, music); `dist/` outputs.
