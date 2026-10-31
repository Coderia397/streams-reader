# On-device Standard voice (Kokoro-82M) in the native apps

Finding + plan. Kokoro-82M can run inside the Tauri apps with `ort` and ONNX Runtime, and the output can match the server's segments, word timings and player. The model is `onnx-community/Kokoro-82M-v1.0-ONNX-timestamped`, which reports per-token durations. The phonemizer is `misaki-rs`, a byte-parity port of misaki 0.9.4, with the Apache-2.0 BART fallback in place of espeak-ng, so there is no GPL. Output is the same WebM/Opus profile 1 the server writes.

Speed on this i7-9750H (verified): fp32 runs at 2.1-2.3x real time on 4-6 threads. int8 is slower than fp32 on this x86 CPU. Desktop can render live. A Pixel 8 is expected near 1x real time on CPU (assumed from Tensor G5/G3 reports), so Android renders ahead and uses the server for anything it cannot keep up with, until it is measured on the phone.

## 2026-10-30

### Measured on this Mac (prototype, Python)
Setup: Intel Core i7-9750H (6 cores / 12 threads, 32 GB), macOS 26 (Darwin 25.6), on AC power, `pmset -g therm` showed no speed limit. Python 3.12 in a `uv venv` with onnxruntime 1.23.2 (CPUExecutionProvider, `intra_op_num_threads` = N, `inter_op_num_threads` = 1, graph optimisation ALL). Phonemes came from misaki 0.9.4 (`en.G2P(trf=False, british=False, fallback=None)`, spaCy `en_core_web_sm` 3.8.0). Voice: `af_heart`, speed 1. Input: 4 segments of 942 chars in total (222, 300, 306 and 171 tokens), the same size as the KOKORO segment profile (120-320 chars). Model call time only; G2P took 41-59 ms for all 4 segments. Other jobs were running on the machine (an ffmpeg at about 650 % CPU during the int8 runs), so the load average is recorded with each run. Process CPU time (`time.process_time`) is reported because it is less sensitive to that load. Scripts and full logs: [raw/2026-10-30-tts-local/](raw/2026-10-30-tts-local/).

| Model file (sha256 prefix) | Threads | Audio / wall | Real time | CPU-s per audio-s | Peak RSS | Load avg | Status |
|---|---|---|---|---|---|---|---|
| int8 `model_quantized.onnx` ts (c0c02b32) | 1 | 59.1 s / 124.8 s | 0.47x | 1.85 | 717 MB | 32 → 6 | verified |
| int8 ts | 2 | 59.1 s / 96.8 s | 0.61x | n/a | 1,438 MB | 10-33 | verified (contended) |
| int8 ts | 4 | 59.1 s / 118.1 s | 0.50x | n/a | 1,475 MB | 10-33 | verified (contended) |
| int8 ts | 6 | 59.1 s / 55.1 s | 1.07x | 2.41 | 1,252 MB | 32 → 6 | verified (contended) |
| fp32 `model.onnx` ts (651ea829) | 1 | 59.1 s / 83.0 s | 0.71x | 1.28 | 978 MB | 4.6 | verified |
| fp32 ts | 2 | 59.1 s / 56.4 s | 1.05x | 1.53 | 1,373 MB | 4.6 | verified |
| fp32 ts | 4 | 59.1 s / 28.3 s | **2.09x** | 1.53 | 1,427 MB | 4.6 | verified |
| fp32 ts | 6 | 59.1 s / 26.0 s | **2.27x** | 2.08 | 1,502 MB | 4.6 | verified |

Notes on the numbers (the fp32 run started 2026-10-31 00:46 local, after the int8 runs):
- On one thread, int8 uses 45 % more CPU per audio second than fp32 (1.85 vs 1.28). This is consistent with ONNX Runtime having no optimised `ConvInteger` kernel on CPU. Kokoro's decoder is mostly convolutions, which dynamic quantisation turns into `ConvInteger`. The cause is assumed; the measurement is verified. **Correction to note 11**, which assumed int8 (about 90 MB) was the desktop choice and expected "several times realtime". On x86, fp32 is faster, and it gives 2.1-2.3x, not several times.
- First audio: the first 222-token segment (12.9 s of audio) took 5.2 s with fp32 on 6 threads. A 120-char first segment is about 2.5 s (derived, assumed).
- Peak RSS of 0.7-1.5 GB includes Python, numpy and spaCy (about 60-150 MB). The ORT arena grows with thread count. On Android it must be measured in Rust, with the arena reduced.
- Determinism (verified): two int8 runs of the same input gave identical waveforms and durations (max abs diff 0.0). The export's noise source is therefore fixed, so a given device and file always produces the same audio.
- Durations output (verified): `durations` is unrounded float per token, including the two pad tokens. `sum(max(1, round(d))) × 600` equals the waveform length exactly for 4/4 segments (310,800, 437,400, 421,800 and 249,000 samples). One frame is 600 samples, which is 25 ms at 24 kHz. This matches kokoro 0.9.4 `KModel`: `pred_dur = round(duration).clamp(min=1)`. torch rounds half to even, so Rust must use `f32::round_ties_even`, not `round`.
- int8 changes timing (verified): int8 and fp32 durations differ, with total frames 521/520, 734/732, 708/705 and 416/418. Over 22 words, word starts differ by a mean of 77 ms and a max of 225 ms. The server runs PyTorch fp32, so the fp32 ONNX export is the one expected to reproduce the server's timings. That parity is assumed; acceptance check C3 measures it.
- Voice files (verified): onnx-community `voices/af_heart.bin` (522,240 bytes, float32 [510, 1, 256]) is byte-identical to the tensor inside the server's pinned `hexgrad/Kokoro-82M@f3ff357/voices/af_heart.pt` (sha256 d583ccff… for both).
- Style row (verified in kokoro source): the server uses `pack[len(ps) - 1]`, where `ps` is the phoneme string. The onnx-community README uses `voices[len(tokens)]`, which is one row off. Use `len(ps) - 1`.
- Out-of-lexicon words: with no fallback, misaki gave `❓` for one token in the test sentence (in `7:30 p.m.`). The server uses misaki with an espeak-ng fallback (`unk=''`), so the device needs a fallback too. See the phonemizer section.
- Tried and abandoned: `kokoro-onnx` 0.6.1 with `espeakng-loader` 0.2.4 on macOS x86_64. Its `libespeak-ng.dylib` ignores the data path it is given and looks in its CI build folder (`/Users/runner/work/...`). Neither `ESPEAK_DATA_PATH` nor `set_data_path` helps, and 0.2.1 ships no x86_64 dylib. Not worth fixing, because espeak is not in the design.
- Disk: the prototype peaked at 494 MB in the scratchpad (venv plus two int8 files, then venv without G2P plus fp32). The model files and venv were deleted afterwards.

### Model files (verified via HF API, 2026-10-30)
| Repo @ commit | File | Bytes | Outputs |
|---|---|---|---|
| `onnx-community/Kokoro-82M-v1.0-ONNX-timestamped@dd4401a9add81ac692d20e240d22ec9dda82cc29` (Apache-2.0) | `onnx/model.onnx` fp32 | 325,532,171 | `waveform`, `durations` |
| same | `onnx/model_quantized.onnx` int8 | 92,361,055 | same |
| same | `onnx/model_fp16.onnx` / `model_q8f16.onnx` / `model_uint8f16.onnx` | 163,232,776 / 86,072,942 / 114,182,909 | same; not measured |
| `onnx-community/Kokoro-82M-v1.0-ONNX@1939ad2a…` | `onnx/model_quantized.onnx` | 92,361,116 | `waveform` only (no timings) |
| either repo | `voices/<id>.bin` | 522,240 each (raw f32 [510,1,256]); 55 voices | — |
| `thewh1teagle/kokoro-onnx` release `model-files-v1.1` | `kokoro-v1.0.int8.onnx` 114 MB, `voices-v1.0.bin` 28 MB (npz) | — | used by kokoro-onnx/Kokoros |

Inputs: `input_ids` int64 [1, n] with pad 0 at both ends (n ≤ 512), `style` f32 [1, 256], `speed` f32 [1]. The vocab comes from `hexgrad/Kokoro-82M` `config.json` (178 symbols).

### Rust runtime (verified 2026-10-30)
- `ort` 2.0.0-rc.15 (2026-10-19, MIT OR Apache-2.0) wraps ONNX Runtime 1.31. Still a release candidate, so pin it exactly (`=2.0.0-rc.15`). Its `download-binaries` list (`ort-sys` `build/download/dist.tsv`) has prebuilt ORT 1.31.0 for `aarch64-linux-android`+nnapi, `x86_64-linux-android`+nnapi, `aarch64-apple-darwin`+coreml, `x86_64-pc-windows-msvc` (+directml / webgpu / openvino), `aarch64-pc-windows-msvc`+directml and `x86_64-unknown-linux-gnu`. **There is no `x86_64-apple-darwin`.**
- Microsoft stopped shipping Intel macOS builds after ONNX Runtime **1.23.2** (2025-10-25, assets `onnxruntime-osx-x86_64-1.23.2.tgz` and `universal2`). 1.24.1 onwards ships only `osx-arm64`, and PyPI wheels follow the same pattern. The owner's Mac is Intel, so the macOS x86_64 build uses `ort` with `load-dynamic` and bundles the 1.23.2 dylib. `ort` defaults to API level 17, so a 1.23 library loads (assumed; acceptance check C9).
- Execution providers: use CPU everywhere at first. NNAPI is deprecated in Android 15. CoreML, DirectML and NNAPI do not run the int8 dynamic-quant ops and fall back to CPU (assumed). GPU EPs are a later experiment with the fp16 file.
- Thread advice from sherpa-onnx discussion #3968: never use more threads than cores, because going from 2 to 3 threads on 2 cores cost 28-33 %. Fewer threads in more processes beats more threads in one.

### Existing Rust Kokoro work (checked 2026-10-30)
| Project | Licence | State | Use |
|---|---|---|---|
| `lucasjinreal/Kokoros` | Apache-2.0 | 833 stars, last push 2026-10-05; espeak-ng phonemizer; word timestamps via the timestamped export; streaming; macOS/Linux, no Android | Reference only (espeak, CLI/server shape) |
| `kokoro-tts` crate 0.3.3 (mzdk100) | Apache-2.0 | 2026-06-01; ort rc.12, claims Android/iOS cross-compile; G2P via cmudict-fast + jieba | Reference for the Android build |
| `kokorox` 0.1.5, `kokoro-tiny` 0.1.0 | Apache-2.0 | 252 and 1,742 downloads, stale since 2025 | No |
| `misaki-rs` 0.8.0 (MicheleYin) | MIT; dictionary data from misaki (Apache-2.0) | published 2026-10-30, 10 stars, 1 maintainer; crate 10.4 MB with embedded us/gb gold+silver lexicons (about 12 MB of JSON) and a 6.3 MB POS tagger; `espeak` is a default feature and can be turned off | **Use** with `default-features = false` |
| `sherpa-onnx` | Apache-2.0, but bundles espeak-ng (GPL-3.0) for Kokoro | active | No (GPL path, C++) |
| `piper-rs`/`espeak-rs` 0.2.0 | MIT bindings to GPL espeak-ng | 2026-05-21 | No |

misaki-rs parity claim (from its PARITY.md, not re-run here): byte-identical US English phoneme strings **and** per-token `(text, phonemes)` attribution against misaki 0.9.4 fixtures. That covers 71/71 adversarial, 700/700 prose and 900/900 held-out cases, plus tokenizer and tagger on 20,623 tokens. The tests run with `--no-default-features`, i.e. with no fallback, which is the mode we would use. British English parity is not claimed.

### Phonemizer and licence reasoning
- espeak-ng is GPL-3.0 (GitHub licence field, verified). Linking it into the app makes every distributed binary a GPL-3.0 combined work. Our MIT code may be combined with it, but each copy given to another person must come with the corresponding source of the whole app under GPL-3.0. The repo is private and the app is meant for invited users, so we would have to publish or hand over source. GPL also conflicts with App Store terms (iOS later). Running espeak as a separate program is not practical on Android. Decision: no espeak-ng in the apps.
- Out-of-lexicon fallback: `PeterReid/graphemes_to_phonemes_en_us` (and `_gb`) is a 1-layer BART with d_model 128. The weights are 3,011,692 bytes and Apache-2.0, and it outputs misaki phonemes directly. misaki HEAD made it the **default** English fallback (`FallbackNetwork`, commit 232a5e32, 2025-04-14), but it is not in a PyPI release, because misaki 0.9.4 is the last. Export it to ONNX (encoder plus a greedy decoder loop, about 3-4 MB) and run it with the same `ort` session pool.
- Rejected fallbacks: OpenPhonemizer (BSD-3-Clause-Clear, about 61 MB ONNX plus 9-10 MB dictionary, outputs espeak IPA that would need mapping; scores 93.5 % similarity to espeak); CMUdict ARPAbet mapping (licence fine, but it needs a hand mapping to misaki's set and still has no answer for OOV words); letter spell-out (what the old misaki-rs did; sounds wrong).
- Server parity: the Space uses misaki 0.9.4 with the **espeak** fallback (kokoro 0.9.4 `KPipeline`, `unk=''`). For device and server to sound the same on rare words, the Space should switch to the same BART fallback. The Space already has transformers 4.46.3, which provides BartForConditionalGeneration. That vendors misaki HEAD's `FallbackNetwork` (Apache-2.0) and bumps the Kokoro adapter version, which re-keys the Kokoro cache. Owner decision (open).

### Word timings
The server pipeline, which the device must copy line for line, is in `space/nolabs_engine/engines/timing.py` and kokoro 0.9.4 `KPipeline.join_timestamps`:
1. `pred_dur[i] = max(1, round_ties_even(durations[i]))`, with index 0 and the last index being the pad tokens.
2. Join tokens in half-frames: `left = right = 2*max(0, pred_dur[0]-3)`. For each misaki token with phonemes, start = left/80 s. Then `left = right + 2*sum(pred_dur[i..j]) + space_dur`, end = left/80, `right = left + space_dur`. A token without phonemes but with whitespace consumes one id.
3. `place_tokens`: skip tokens without a letter or digit, find the token text in the item text from a cursor, and emit `[start_ms, end_ms, char_start, char_end]` in code-point offsets.
4. `shift_words`: subtract the trimmed lead, then clamp inside the speech part.

The Python port in `raw/2026-10-30-tts-local/bench_ids.py` reproduces step 2 on the test data. Example: `('In', 0.275, 0.375), ('2026', 0.375, 1.413), …` for fp32.

### Newer small TTS models (checked 2026-10-30)
None replaces Kokoro on the device, because the device has to sound like the server's Standard voice, and that voice is Kokoro v1.0.
- Kokoro v1.1-zh (Apache-2.0, 2025-03) adds Chinese. There is no English v1.1, so stay on v1.0 to match the server.
- KittenTTS: the small 0.8 models are Apache-2.0 and use espeak via phonemizer, which is the same GPL problem. `kitten-tts-2` is a 1.7-2B model under the "Stellon Labs Community License", which ends the commercial grant above USD 1 M revenue. Not suitable.
- Pocket TTS (Kyutai, CC-BY-4.0, 100M): about 6x real time on an M4 CPU with 2 cores, has cloning, 6 languages. It has no official word timestamps (a community fork adds them). It is a candidate for later on-device cloning, not for the Standard voice.
- Supertonic 2/3 (OpenRAIL, use restrictions): not OSI.
- Piper: the maintained `OHF-Voice/piper1-gpl` is GPL-3.0, and `rhasspy/piper` (MIT) is archived.
- MOSS-TTS-Nano (Apache-2.0): already rejected in note 03 for having no emotion control.
- litert-community/Kokoro-82M (Apache-2.0) is the same Kokoro as LiteRT `.litertlm` files (295 MB) with its own G2P and word timestamps. Its Pixel 10 Pro (Tensor G5) results: CPU 4 threads RTF 0.93, GPU RTF 0.49-0.50, peak memory 400-945 MB. On a Galaxy S26: CPU RTF 0.26, GPU 0.12. Pixel 8a (Tensor G3) fp32 CPU with 4 threads: RTF about 1.8. This is the best evidence for the Pixel 8. G3 cores are older than G5, so expect CPU RTF of about 1.0 or worse with int8 (assumed). The LiteRT GPU path is the Android speed option if ORT CPU is too slow.

### Output format decision (assistant, 2026-10-30)
Decision: the device writes the **same WebM/Opus profile 1** as the server (24 kHz mono, 32 kbps, VBR, 20 ms frames, `-application audio`). It applies the same trim (−45 dBFS, keep 30 ms, 5 ms fades), loudness (toward −16 LUFS, ±4 dB, peak ≤ −1.5 dBFS) and pauses (250/600/1200 ms).
Reason: the webview player (MediaSource `audio/webm; codecs="opus"`), `Track`, the cache, offline downloads and the planned Media3/cpal native players (note 11) all stay as they are. One hour is about 14 MB at 32 kbps; i16 PCM would be 172 MB. libopus encodes at 24 kHz natively, so there is no resampling. Encoding cost is assumed to be under 2 % of synthesis; check C5.
Rejected: PCM straight to the native engine. It would add a second player path, have no cache or offline story, and Media3 needs files anyway. Keep a PCM debug output only.
Loudness difference: the server plans gain per batch. The device renders one segment at a time, so it uses a per-document gain taken from the first 3 segments and stored per (document, voice), with a per-segment correction of ±4 dB. Expected difference is under 1 LU (assumed; check C4).

### Recommended design: crate `crates/tts-local`
Native only (never wasm). It depends on `nolabs-core` for `WordTiming`, `Boundary`, `cache_key` and `AudioProfile`.

```rust
pub struct LocalTts { /* handle to the worker thread */ }
pub struct Options { pub model_dir: PathBuf, pub precision: Precision, pub threads: usize, pub spin: bool }
pub enum Precision { Fp32, Int8 }
pub enum Priority { Now, Next, Ahead, Background }        // Now = segment the listener is waiting for
pub struct Job { pub key: String, pub text: String, pub voice: String, pub boundary: Boundary,
                 pub priority: Priority, pub doc: String }
pub struct Rendered { pub key: String, pub webm: Vec<u8>, pub duration_ms: u32, pub speech_ms: u32,
                      pub words: Vec<WordTiming>, pub synth_ms: u32 }
pub struct Stats { pub rtf_ewma: f32, pub queued: usize, pub model_rev: String }
impl LocalTts {
    pub fn start(opts: Options) -> Result<LocalTts, TtsError>;          // spawns "tts-local" thread, loads session
    pub fn submit(&self, job: Job) -> oneshot::Receiver<Result<Rendered, TtsError>>;
    pub fn cancel_doc(&self, doc: &str);                                // drops queued jobs of a document
    pub fn stats(&self) -> Stats;
}
pub mod models {   // install, verify, remove
    pub struct Manifest { pub model_rev: String, pub files: Vec<FileEntry> }   // path, url, mirror_url, sha256, bytes
    pub async fn ensure(dir: &Path, m: &Manifest, progress: impl Fn(u64, u64)) -> Result<(), TtsError>;
    pub fn installed(dir: &Path, m: &Manifest) -> bool;
}
pub fn local_model_rev(p: Precision) -> &'static str;  // "onnx-community/Kokoro-82M-v1.0-ONNX-timestamped@dd4401a9:fp32"
```
Modules:
- `g2p`: misaki-rs (no default features) plus the BART fallback on ort. A token rewrite cache.
- `tokens`: vocab and the 510 limit, with the kokoro `waterfall_last` split ported for long expansions.
- `synth`: the ort session and the style row `len(ps)-1`.
- `timing`: ports of `join_timestamps`, `place_tokens` and `shift_words`, with golden tests against the Space.
- `post`: trim, `ebur128` loudness, gain and pauses.
- `encode`: libopus via the `opus` crate (BSD-3) plus a small WebM muxer (EBML header, Info with Duration, a Tracks entry `A_OPUS` with OpusHead, CodecDelay and SeekPreRoll, SimpleBlock clusters, Cues), tested with ffprobe and the player harness.
- `worker`: one OS thread and a priority queue.

Cache keys use `cache_key` with `model_rev = local_model_rev(..)` and a separate `LOCAL_ADAPTER_VERSION`, so device audio never collides with server audio, and a model or precision change re-keys it.

Model management:
- Download on first use; do not bundle. Installers stay small, and the model updates independently of the app.
- Files are mirrored from the pinned HF commit to R2 `nolabs-media/models/kokoro-onnx/<rev>/…`. The Worker serves them at `listen.streams.watch/models/…`; R2 has free egress, and the files are under 0.5 GB of the 10 GB R2 allowance. The pinned HF `resolve/<commit>` URL is the second source.
- The manifest holds sha256 and byte counts. Downloads resume with HTTP Range into `*.part`, are checked by sha256, then renamed atomically into `app_data_dir/models/kokoro/<rev>/`.
- Sizes: desktop fp32 is 325.5 MB. Android gets int8 at 92.4 MB unless the Pixel measurement favours fp32. Voices are 522 KB each and downloaded when chosen (all 28 English voices are 14.6 MB). The BART fallback is about 3-4 MB.
- On Android, downloads are Wi-Fi only by default and show progress in Settings → Voices → "Use on this device".
- Binary size: misaki-rs embeds about 18 MB of lexicon and tagger, and ORT static adds about 15-25 MB per ABI (assumed). Measure under C10.

Threading:
- One dedicated thread owns the session; G2P runs on the same thread (about 10-15 ms per segment, measured in Python).
- `intra_op_num_threads`: desktop = min(physical cores, 4). On this CPU, 4 threads gave 92 % of the 6-thread speed for 74 % of the CPU time. Android starts at 4 (X3 + 3 × A715 on Tensor G3) and is tuned by measurement.
- `inter_op` = 1, spinning off (`session.intra_op.allow_spinning=0`) to save battery.
- One job at a time, highest priority first. `Ahead` and `Background` jobs give way between segments when a `Now` job arrives (no pre-emption inside a model call).
- `Background` (render a whole document ahead) runs only on AC power on desktop, and only while charging and unmetered on Android, through a foreground worker with a notification. It pauses at Android thermal status ≥ SEVERE.

How the Tauri app calls it (aligned with [18-native-apps-architecture.md](18-native-apps-architecture.md), where the Rust core owns playback and the render-ahead feeder):
- The feeder (`apps/tauri/src-tauri/src/feed.rs`) is the only caller. It holds a `LocalTts`, gives it the window of segments it would otherwise `POST /api/render`, and writes each `Rendered` to `audio/kokoro-local/{key}.webm` plus a `.json` sidecar (same shape as the server's `EngineItemResult`). It then sends `Append` to the desktop engine or the Media3 playlist and `SegmentReady` to the UI, exactly as for server clips. The webview never handles audio bytes from local rendering.
- `NativeBridge` additions for the UI (Settings → Voices): `tts_local_status` → {installed, model_rev, precision, rtf_ewma, bytes}; `tts_local_install` / `tts_local_remove`; progress arrives through `subscribe` as `TtsLocalProgress {done, total}`. Default methods return `Unsupported` on the web.
- Android runs the same Rust in-process. Media3 plays the local `file://` WebM as it does server clips.

Fallback to the server (render router inside the feeder, not in the crate):
- Lookup order per segment: local cache (local or server key) → local engine, if the model is installed, the voice is Kokoro, and `rtf_ewma × playback_rate ≤ 0.8` → server `POST /api/render`.
- During playback, if local buffered-ahead falls under 90 s, the next segments go to the server as well. The first result to arrive is used, and local keeps working further ahead.
- A local error retries once locally, then goes to the server.
- Ming voices always use the server.
- Offline with no model, the user is told to "download the Standard voice for offline listening".
- The web app does not change.

### Implementation plan (phase 5 of note 14) with acceptance checks
| Step | Work | Owner dir | Acceptance check |
|---|---|---|---|
| 5.1 | Golden fixtures from the pinned Space. A CPU endpoint or offline script dumps, for 60 segments (prose, numbers, abbreviations, quotes, names, a 300-char item): misaki tokens, phoneme strings, `pred_dur`, words and the raw wave, at both en-us and en-gb. Export BART G2P to ONNX. | `space/` (script), `crates/tts-local/tests/fixtures` | C1: fixtures committed with the Space commit and model revs recorded |
| 5.2 | `g2p` + `tokens` + `synth` + `timing` (host build) | `crates/tts-local` | C2: phoneme strings and token attribution equal to fixtures 60/60 (US); GB differences listed. C3: fp32 ONNX `pred_dur` equal to PyTorch on ≥ 99 % of tokens, and word starts within 25 ms (1 frame) for ≥ 99 % of words |
| 5.3 | `post` + `encode` (libopus, WebM muxer) | `crates/tts-local` | C4: integrated loudness within 1 LU of the server clip for the same text, peak ≤ −1.5 dBFS. C5: ffprobe reports opus, 24 kHz, mono, and the file plays in `crates/player/harness` with MediaSource; encode < 2 % of synth time |
| 5.4 | `worker` + `models` | `crates/tts-local` | C6: download killed at 50 % resumes; a corrupted byte fails the sha256 and is re-fetched. C7: on this i7-9750H, fp32 with 4 threads ≥ 2.0x real time over 10 min of a book, first audio ≤ 3 s for a 120-char first segment, peak RSS ≤ 900 MB in Rust (Python measured 1.43 GB) |
| 5.5 | Router in the feeder + `NativeBridge` additions + Settings UI | `apps/tauri`, `apps/bridge`, `crates/web` | C8: with networking off, a downloaded-model document plays start to finish with word highlighting. Forced local error → server audio within the usual latency. No gap or click at segment joins (listening plus a 5 ms boundary check) |
| 5.6 | Intel macOS (`load-dynamic`, ORT 1.23.2 universal2) and Windows/Linux builds | `apps/` | C9: the Intel build loads ORT 1.23.2 and renders. C10: installer size increase recorded per platform |
| 5.7 | Android arm64 (ort prebuilt 1.31 Android) on the Pixel 8 | `apps/android` | C11: int8 vs fp32 RTF at 2/3/4 threads over 10 min, RSS, °C and battery % per hour recorded. Decision rule: render live only if sustained RTF ≤ 0.7, otherwise only ahead of time while charging plus the server for live playback. If CPU RTF > 1, try the LiteRT GPU build (separate runtime) |
| 5.8 | Space switches to the BART fallback (if the owner agrees) | `space/` | C12: the Space and device give the same phonemes on 100 OOV words (names, coinages); Kokoro adapter version bumped |

Note 14's gate "measured RTF before/after" is covered by C7 and C11.

## Open items
- Pixel 8 speed, memory and battery (C11). Unblock: Android build of 5.7 on the owner's phone.
- Owner decision: switch the Space's out-of-lexicon fallback from espeak to the BART network (5.8). This changes how rare words sound and re-keys the Kokoro cache. Unblock: owner yes/no.
- misaki-rs risk: 10 stars, one maintainer, released the day of this note, and the provenance of its POS tagger weights is not stated (assumed derived from spaCy `en_core_web_sm`, MIT). Unblock: pin by commit, run its parity suite in CI, read its tagger build script; vendor it if it goes stale.
- fp32 vs PyTorch timing parity (C3) is assumed until 5.1 fixtures exist.
- Re-measure fp32 on an idle machine (load average was 4.6 during the fp32 run). Unblock: a quiet window, or the Rust benchmark in C7.
- WebM/Opus MediaSource support in the Tauri webviews (WKWebView on macOS, WebKitGTK on Linux, WebView2 on Windows) if the webview player is used before the native players. Unblock: phase 4 smoke test.
- `ort` is still a release candidate (rc.15). Unblock: pin `=2.0.0-rc.15`, re-check at each rc.

## Sources
- HF API trees: [Kokoro-82M-v1.0-ONNX-timestamped](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX-timestamped), [Kokoro-82M-v1.0-ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX), [hexgrad/Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M), [PeterReid/graphemes_to_phonemes_en_us](https://huggingface.co/PeterReid/graphemes_to_phonemes_en_us), [litert-community/Kokoro-82M](https://huggingface.co/litert-community/Kokoro-82M), [kyutai/pocket-tts](https://huggingface.co/kyutai/pocket-tts), [KittenML/kitten-tts-2](https://huggingface.co/KittenML/kitten-tts-2), [openphonemizer/ckpt](https://huggingface.co/openphonemizer/ckpt)
- kokoro source: [pipeline.py](https://github.com/hexgrad/kokoro/blob/main/kokoro/pipeline.py) (`join_timestamps`, `pack[len(ps)-1]`, espeak fallback), [model.py](https://github.com/hexgrad/kokoro/blob/main/kokoro/model.py) (`round().clamp(min=1)`); misaki [en.py HEAD](https://github.com/hexgrad/misaki/blob/main/misaki/en.py) (`FallbackNetwork`)
- [misaki-rs](https://github.com/MicheleYin/misaki-rs) and its PARITY.md; crates.io API for `ort`, `misaki-rs`, `kokoro-tts`, `kokorox`, `espeak-rs`, `piper-rs`, `tract-onnx`
- `ort-sys` 2.0.0-rc.15 `build/download/dist.tsv`; [ort docs](https://ort.pyke.io/perf/execution-providers); [ONNX Runtime releases](https://github.com/microsoft/onnxruntime/releases) (osx-x86_64 last in v1.23.2)
- [Kokoros](https://github.com/lucasjinreal/Kokoros), [mzdk100/kokoro](https://github.com/mzdk100/kokoro), [sherpa-onnx discussion #3968](https://github.com/k2-fsa/sherpa-onnx/discussions/3968), [thewh1teagle/kokoro-onnx releases](https://github.com/thewh1teagle/kokoro-onnx/releases)
- Licences from the GitHub API: espeak-ng GPL-3.0, OHF-Voice/piper1-gpl GPL-3.0, rhasspy/piper MIT (archived), NeuralVox/OpenPhonemizer BSD-3-Clause-Clear, misaki Apache-2.0
- ONNX Runtime quantisation guidance (dynamic for RNN/transformer, static for CNN): [onnxruntime.ai](https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html)
