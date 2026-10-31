# Film studio: reusable voxel scene, promo, hero loop, spots

Finding + reference + progress. A deterministic Three.js scene (`media/scene/`) driven by JSON timelines renders
Streams Reader videos with one command (`uv run --script media/render.py <shot.json>`). Conclusion: the pipeline works
end to end on this Mac's GPU without Node or frames on disk; the first promo (41 s, 1080p, voice-over by our own
Kokoro API, ACE-Step music from Kaggle) and both hero loops are rendered; the 20 spot illustrations render from presets.
How to use it: `media/README.md`. Sources and licences: `media/CREDITS.md`.

## 2026-10-30

### What exists (verified on rendered output)
- Scene: voxel mascot (bevelled boxes, pixel textures, FK rig, two-bone arm IK, blinks, breathing, gaze, 14 expressions,
  16 poses, walk cycle), Streams-blue over-ear headphones with ember rings (ears / neck / earbuds, optional cable to the
  phone), phone with the captured app as its screen and a screen-sized area light on face and hands, room at night
  (armchair, lamp, shelf, plant, rug, side table with steaming mug, fairy lights, rainy window on a voxel city, moon
  shafts, dust motes), park (golden / day / dusk, or street; trees, path, benches, lamp posts, falling leaves, birds,
  god rays, sky dome), neutral studio for spots. Post: bokeh DOF with rack focus, bloom, ACES, grade with vignette,
  grain and slight aberration; transitions cut / dissolve / whip; optional sub-frame motion blur.
- Determinism: time only advances through `studio.frame(i)`; every random value is seeded; DPR fixed at 1; loops snap
  all periodic motion to the loop length. Status: assumed identical across runs (same code, same GPU); not diffed.
- Renderer: `media/render.py` serves the repo on 127.0.0.1, drives headless Chrome over CDP, pipes JPEG screenshots
  into one ffmpeg process that writes AV1 WebM (SVT-AV1 + Opus) and H.264 MP4 (+ AAC), limited-range BT.709.
- App capture: `media/capture/capture_app.py` (re-runnable). Voice-over: `media/vo/render_vo.py`. Mix:
  `media/tools/audio.py` (sidechain ducking, two-pass loudnorm to -16 LUFS / -1.5 dBTP), captions as WebVTT.

### Measurements
- Render speed, AMD Radeon Pro 5500M via ANGLE/Metal (`GL:` line printed by render.py): 1080p promo 3.8 frames/s;
  720p hero 2.2-2.7 frames/s alone, 1.3 frames/s while another render shared the GPU. First frame takes 3-12 s
  (shader compile). Measured from render.py's progress lines.
- App capture: polled `Page.captureScreenshot` at DPR 2 gave 198 frames in about 27 s (7.3 frames/s) while the app
  played; `Page.startScreencast` delivers only 1x (390x844) frames, so it is kept as an option. Clip 26 s, 780x1688
  VP9, 1.35 MB. Narration recorded from the app's media element (MediaRecorder): starts 0.84 s after the first frame,
  heading read 0.85-2.27 s, "Alice was beginning..." from 3.54 s (ffmpeg silencedetect -40 dB / 0.25 s).
- Voice-over (Kokoro af_heart through POST /api/render): 6 lines, 18.7 s of speech in total (durations from the API's
  `duration_ms`): late 3.76 s, readto 1.75 s, voice 3.74 s, anywhere 3.25 s, tagline 4.71 s, endcard 1.49 s.
- Music on Kaggle (Tesla T4, bf16, CPU offload, cuDNN off): 60 s clips took 196 s and 155 s to generate (from
  `bed.params.json` / `music_log.json`), 431 s for the whole notebook. Bed 0 (used) is quiet for 10 s (about -24 dB RMS),
  then -11 to -15 dB RMS from 10 to 50 s, spectral centroid 2.6-4.2 kHz (numpy analysis in 5 s windows); bed 1 is
  brighter (centroid up to 5 kHz) and was kept as the alternate.

### Decisions (assistant, delegated by the owner's brief)
- Voice-over voice: `kokoro:af_heart` (warm, the catalogue's default and highest-graded Kokoro voice; listening was not
  possible). The app reads the book with `kokoro:bf_emma` so the book and the narrator sound different.
- Hero loop from the room (cosiest image); the phone screen is not legible there because a held phone faces the reader.
  The app itself is shown in the promo's eye-line insert (POV shot with the head hidden).
- Story: night, tired eyes on a paper book -> picks up the phone -> POV of the app reading Alice with word highlight ->
  looks up -> whip to the park, walking with wired over-ear headphones -> tagline -> end card with the SVG wordmark.
- Character restyled after owner feedback ("a bit goofy"): calmer face (narrower eyes, small mouth, faint blush),
  no topknot, head scaled 0.86, relaxed lean-back pose with one ankle over the knee, one-handed phone hold.

### Beliefs that turned out wrong (corrected)
- `pip install` of ACE-Step on Kaggle failed: it pins `spacy==3.8.4`, which has no Python 3.13 wheel. Fix: install with
  `--no-deps` and unpinned deps. Then `py3langid` 0.3 lacks `from_pickled_model`: pin 0.2.2. Then bf16 conv2d had no
  cuDNN engine on the T4: disable cuDNN. Version 4 of the notebook works.
- Uploading a `<video>` frame straight to a WebGL texture gave a black screen in headless GPU Chrome; copying the frame
  through a 2D canvas after `seeked` works.
- A shell `&` inside a backgrounded command was killed when the wrapper exited (the first hero render produced a
  0-byte WebM); run renders in the foreground of a background task instead.

### Outputs
- See the final table in the progress section below (filled after the renders).

## Open items
- Other languages: only English Kokoro voices are in the app's voice catalogue; es/fr/pt/hi/zh Kokoro voices exist
  upstream, de/ar need another engine (`media/vo/languages.json`). Unblock: expose those voices in the API.
- Re-capture the screen after the Streams-blue app redesign ships (`media/capture/capture_app.py`), then re-render.
- Determinism was not checked by rendering twice and diffing. Unblock: render a shot twice and compare frame hashes.
- Music was chosen by analysis, not by ear. Unblock: owner listens to `assets/music/bed.ogg` vs `bed-alt.ogg`.
