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
- Superseded by the 2026-10-31 outputs below (the 2026-10-30 renders were replaced).

## 2026-10-31

### Expressive voice-over (owner: "not expressive enough")
- One `POST /api/render` request with all six lines, voice `ming:US_Emma` (warm US narrator; UK_Charlotte would have
  sounded too close to the book's British voice), per-line emotion: late neutral, readto happy, voice neutral,
  anywhere happy, tagline happy, endcard happy. Result: all ready, no retry. GPU ledger after the run (GET /api/status):
  used 38 s of 280 s, 1 of 3 runs. Durations (API `duration_ms`): late 4.32 s, readto 1.66 s, voice 4.06 s,
  anywhere 3.59 s, tagline 5.01 s, endcard 1.59 s (Kokoro takes kept in `assets/vo/en/` as fallback; the Ming set is
  `assets/vo/en-ming/`, selected by `audio.vo_set` in the shot). Edit re-timed: lines at 1.5, 8.3, 14.9, 19.5, 31.2,
  37.3 s. Status: verified rendered and audible (ffmpeg volumedetect mean -17 to -20 dB); not judged by ear.

### Rig QA (owner: "sometimes the limbs are weird and the joints are weird")
Tool: `media/qa.py <shot> --out DIR` renders every 0.25 s at 960x540 into 3x3 sheets, joint close-ups (shoulders,
elbows, wrists on the phone, hips, knees, ankles; set hidden so the armchair does not block them) and a metrics file
(lowest sole height, IK hand error, elbow/knee hinge angles, cable points inside the torso). Passes run: 8 on the promo
(164 frames each). Defects found and fixed at the rig level (verified on the final pass: 0 metric issues, sheets read):
1. Elbow IK picked the elbow side away from the pole (upper arm rotated by -alpha), so elbows could flip up or inwards.
   Fix: candidate solver, elbow is a hinge that only flexes forward, pick the reaching candidate closest to the pole.
   Measured on the final pass: elbows -6 to -95 deg, never positive; knees 6 to 96 deg, never negative.
2. Hands floated beside the phone: IK placed the hand centre, then the wrist rotated about its pivot and moved the hand.
   Fix: with a grip orientation the solver aims the wrist pivot; hands clamp the phone's side edges (thumb in front,
   fingers behind). Hand-target error 0.000 m (qa metrics).
3. Visible gaps where boxes rotate at elbows, knees and hips. Fix: round joint caps at those pivots.
4. Shoe sole showed a seam under the shoe. Fix: sole overlaps the shoe.
5. Headphone cable ran through the arm and torso while walking (1-3 cable points inside the torso box on 69 of 164
   samples). Fix: cable routed over the collar, down the chest and belly front. Now 0.
6. Feet could float or sink while standing/walking. Fix: ground solve (lowest sole to y = 0) every frame; planted foot
   at 0.000 m, swing foot up to 0.028 m. Ground speed now derived from the stride (no foot sliding).
7. While "reading" the face looked at the camera, not the phone. Fix: look-at constraint on neck (35 %) and head
   (65 %), clamped, keyable weight (`look_w`).
8. The tired nod dropped the head 51 deg. Fix: 14 deg nod.
9. Falling leaves crossed the lens in tracking shots (big orange blur). Fix: leaf volume kept behind the camera paths.
10. Orbit cameras pivoted around the aim point, swinging through the foreground tree row. Fix: orbit around the subject,
    `aim` only shifts the look point; foreground trees moved to 5-6 m from the path.
11. Screenshot could be taken before the compositor presented the frame. Fix: wait two animation frames before
    capture. Determinism checked: frames 30-32 rendered twice in one session gave identical PNG md5 hashes.
Polish: glide from his face to the rainy window, dissolve into the park opening on the canopy and tilting down to him
(replaces the whip); calm park face; back path with four strolling people; grain lowered for compression; web MP4s
are two-pass x264 from a master (1080p 1450k video, 720p 850k).

### Finding about the app (for the player team)
- In the capture the app read the heading, then skipped the first paragraph ("Alice was beginning to get very
  tired...") and continued with "So she was considering..." both in audio and highlight (frames at 3-5 s of
  `assets/screen/app-alice.webm`; narration from 3.54 s). Status: verified once, cause unknown (fresh profile, voice
  switched to Emma just before Play, progress set to 0). Unblock: reproduce in the player harness.

### Outputs (2026-10-31, local in media/dist/, gitignored; sizes from ls, durations and loudness from ffmpeg/ebur128)
| File | Size | Notes |
|---|---|---|
| promo/promo.webm | 7.05 MB | 41.0 s, 1920x1080 AV1 + Opus |
| promo/promo.mp4 | 7.81 MB | 41.0 s, 1080p H.264 two-pass 1350k + AAC; -16.0 LUFS, true peak -1.5 dBTP |
| promo/promo-720p.mp4 | 5.15 MB | 41.0 s, 720p two-pass 850k |
| promo/promo.poster.{jpg,avif} | 135 KB / 35 KB | frame at 14 s (POV of the app) |
| promo/promo.en.vtt | 0.5 KB | voice-over captions |
| hero/hero-loop.{webm,mp4} | 0.49 / 0.99 MB | 10 s seamless loop, 1280x720, silent |
| hero/hero-loop-portrait.{webm,mp4} | 0.43 / 0.90 MB | 10 s loop, 720x1280 |
| hero/*.poster.{jpg,avif} | 56-59 KB / 11-12 KB | |
| spots/<name>{,-512}.{png,webp} | 6.7 MB total | 20 presets, transparent, 1024 and 512 px |
| preview/ | | first-look stills and 5 s room clip (2026-10-30, older rig) |
Render speed for the final promo: 685 s for 1230 frames (1.8 frames/s; the park now has five rigs).

### Handover
- Lead's decision (2026-10-31): the next film version moves to headless Blender on Kaggle (real armatures, IK
  constraints, path-traced light, motion blur), built by another agent. This three.js studio stays for quick cuts,
  spots and loops.

## Open items
- Other languages: only English Kokoro voices are in the app's voice catalogue; es/fr/pt/hi/zh Kokoro voices exist
  upstream, de/ar need another engine (`media/vo/languages.json`). Unblock: expose those voices in the API.
- Re-capture the screen after the Streams-blue app redesign ships (`media/capture/capture_app.py`), then re-render.
- Determinism across separate sessions not checked (within one session verified, 2026-10-31). Unblock: render a shot in two runs and compare frame hashes.
- Spots: owner review of the 20 presets (some poses are stiff). Unblock: owner picks the ones to redo.
- The film's look stays three.js-grade (no path tracing, no real motion blur); the Blender pipeline is the next step (lead, 2026-10-31).
- Music was chosen by analysis, not by ear. Unblock: owner listens to `assets/music/bed.ogg` vs `bed-alt.ogg`.
