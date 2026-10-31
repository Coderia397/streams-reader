#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""IndexTTS-2.5 (bilibili, falls back to IndexTTS-2) on a private Kaggle T4 kernel for the engine bake-off (notes/20).
Never touches ZeroGPU or the live API.

    uv run tools/expressiveness/bakeoff/indextts25/itts_kaggle.py refs    --work W   # LibriVox refs + Ming's af_heart ref
    uv run tools/expressiveness/bakeoff/indextts25/itts_kaggle.py jobs    --work W   # W/dataset/jobs.json
    uv run tools/expressiveness/bakeoff/indextts25/itts_kaggle.py push    --work W   # private dataset + private GPU kernel
    uv run tools/expressiveness/bakeoff/indextts25/itts_kaggle.py pull    --work W --wait
    uv run tools/expressiveness/bakeoff/indextts25/itts_kaggle.py collect --work W --audio A

Voices (zero-shot clones, the model has no presets):
- lvf   LibriVox solo reader of Pride and Prejudice (female, F0 about 245 Hz), 11.5 s of narration from 4:32.
- lvm   LibriVox reader of Walden ch. 5 (male, F0 about 100 Hz), 13.1 s from 5:04.
        Both windows lie outside the 75 s excerpts that fetch_human.py uses as the human ceiling.
- heart the 14 s Kokoro af_heart reference that the Ming study made (W_ming/dataset/ref_af_heart.wav).
Conditions (labels itts25_<voice>_<cond>):
- plain       one item per segment (paragraph), emotion from the reference, as the app would send it.
- plain_sent  one item per sentence, no emotion control: control for the per-sentence split.
- emotext     per sentence, QwenEmotion text-to-emotion on the sentence (use_emo_text), emo_alpha 0.65 (webui default).
- directed    per sentence, the director's emotion and intensity as an 8-dim emo_vector (normalize_emo_vec with the
              upstream bias), neutral -> no vector; pace slow/fast -> duration_factor 1.10 / 0.92.
- directed_dp the same audio with the director's pause_after_ms instead of the 250/600 ms profile pauses (no GPU).
Post-processing: the Space pipeline (trim, programme loudness with the Ming profile, boundary pauses), WebM/Opus 48 kbps.
"""
from __future__ import annotations

import argparse
import dataclasses
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
EXPR = HERE.parent.parent
REPO = EXPR.parent.parent
SPACE = REPO / "space"
sys.path.insert(0, str(SPACE))

OWNER = "lbrahimali"
DATASET = f"{OWNER}/streams-itts25-bakeoff-inputs"
KERNEL = f"{OWNER}/streams-itts25-bakeoff"
CODE_REPO = "index-tts/index-tts"
CODE_COMMIT = "d9e41aac89fd00b3d71497fddb287b7f24613712"  # main, 2026-09-29
MODELS = {  # repo -> revision, resolved 2026-10-31 from the HF API
    "IndexTeam/IndexTTS-2.5": "c39ce5ba981572cb187443877ff559dfb246ce63",
    "IndexTeam/IndexTTS-2": "740dcaff396282ffb241903d150ac011cd4b1ede",
    "facebook/w2v-bert-2.0": "da985ba0987f70aaeb84a80f2851cfac8c697a7b",
    "funasr/campplus": "e4b6ede7ce16997aff4ae69fbca1f0175e2afede",
    "nvidia/bigvgan_v2_22khz_80band_256x": "633ff708ed5b74903e86ff1298cf4a98e921c513",
    "amphion/MaskGCT": "265c6cef07625665d0c28d2faafb1415562379dc",  # only for the IndexTTS-2 fallback (CC-BY-NC-4.0)
}
UV_VERSION = "0.13.1"
IA = "https://archive.org/download"
REFS = {  # id -> (archive item/file, start s, length s, description)
    "lvf": ("solo_pride_librivox/prideandprejudice_01-04_austen_apc_64kb.mp3", 272.05, 11.45,
            "LibriVox Pride and Prejudice v2 solo reader, 4:32.05-4:43.50"),
    "lvm": ("walden_librivox/walden_c05_64kb.mp3", 303.8, 13.1, "LibriVox Walden ch. 5 reader, 5:03.8-5:16.9"),
}
EMO_INDEX = {"happy": 0, "angry": 1, "sad": 2, "fearful": 3, "disgusted": 4, "surprised": 6}
PACE = {"slow": 1.10, "fast": 0.92, "normal": 1.0}
EMO_ALPHA_TEXT = 0.65
BITRATE = 48
SEED = 1234


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


def decode(path: Path | str, sr: int, ss: float | None = None, t: float | None = None) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error"]
    if ss is not None:
        cmd += ["-ss", str(ss), "-t", str(t)]
    cmd += ["-i", str(path), "-ac", "1", "-ar", str(sr), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32)


# ------------------------------------------------------------------------------------------------------------ refs
def cmd_refs(work: Path, ming_ref: Path) -> None:
    import pyloudnorm

    from nolabs_engine.audio import silence

    ds = work / "dataset"
    ds.mkdir(parents=True, exist_ok=True)
    sr = 24_000
    info = {}
    for rid, (path, ss, t, what) in REFS.items():
        wave = decode(f"{IA}/{path}", sr, ss, t).copy()
        # As nolabs_engine/reference.py prepares a user reference: speech span, -20 LUFS, peak under -1.5 dBFS.
        wave = silence.trim(wave, sr, guard_ms=100, fade_ms=10).wave
        gain = -20.0 - pyloudnorm.Meter(sr).integrated_loudness(wave.astype(np.float64))
        wave = wave * 10 ** (gain / 20)
        wave *= min(1.0, 10 ** (-1.5 / 20) / max(1e-9, float(np.abs(wave).max())))
        sf.write(ds / f"ref_{rid}.wav", wave.astype(np.float32), sr, subtype="PCM_16")
        info[rid] = {"source": f"{IA}/{path}", "start_s": ss, "length_s": t, "description": what,
                     "seconds": round(wave.size / sr, 2), "gain_db": round(gain, 1)}
        print(rid, info[rid])
    shutil.copy(ming_ref, ds / "ref_heart.wav")
    w, r = sf.read(ds / "ref_heart.wav")
    info["heart"] = {"source": str(ming_ref), "description": "Kokoro af_heart, neutral library text (Ming study ref)",
                     "seconds": round(w.size / r, 2)}
    (work / "refs.json").write_text(json.dumps(info, indent=1))


# ------------------------------------------------------------------------------------------------------------ jobs
def emo_vector(emotion: str, intensity: float) -> list[float] | None:
    if emotion not in EMO_INDEX:
        return None
    v = [0.0] * 8
    v[EMO_INDEX[emotion]] = round(min(1.0, 1.25 * float(intensity)), 3)
    # IndexTTS2.normalize_emo_vec(apply_bias=True), as the webui applies it to vector input.
    bias = [0.9375, 0.875, 1.0, 1.0, 0.9375, 0.9375, 0.6875, 0.5625]
    v = [a * b for a, b in zip(v, bias)]
    s = sum(v)
    if s > 0.8:
        v = [a * 0.8 / s for a in v]
    return [round(a, 4) for a in v]


def build_conditions(segments: dict, plans: dict) -> list[dict]:
    out = []
    for voice in ("lvf", "lvm", "heart"):
        for passage, segs in segments.items():
            plan = plans[passage]
            items = [{"file": f"{i:02d}.flac", "text": s["text"], "boundary": s["boundary"], "mode": "plain"}
                     for i, s in enumerate(segs)]
            out.append({"label": f"itts25_{voice}_plain", "voice": voice, "passage": passage, "items": items})
            sents = []
            for si, s in enumerate(segs):
                spans = s["sentences"]
                for k, (a, b) in enumerate(spans):
                    sents.append({"text": s["text"][a:b], "segment": si,
                                  "boundary": "sentence" if k + 1 < len(spans) else s["boundary"]})
            assert len(sents) == len(plan), (passage, len(sents), len(plan))
            for p, s in zip(plan, sents):
                assert p["text"].strip() == s["text"].strip(), (passage, p["text"], s["text"])
            for cond in ("plain_sent", "emotext", "directed"):
                items = []
                for n, (p, s) in enumerate(zip(plan, sents)):
                    it = {"file": f"{n:02d}.flac", "text": s["text"], "boundary": s["boundary"], "mode": cond,
                          "emotion": p["emotion"], "intensity": p["intensity"], "pace": p["pace"],
                          "pause_after_ms": p["pause_after_ms"]}
                    if cond == "emotext":
                        it["emo_alpha"] = EMO_ALPHA_TEXT
                    if cond == "directed":
                        it["emo_vector"] = emo_vector(p["emotion"], p["intensity"])
                        it["duration_factor"] = PACE.get(p["pace"], 1.0)
                    items.append(it)
                out.append({"label": f"itts25_{voice}_{cond}", "voice": voice, "passage": passage, "items": items})
    return out


def cmd_jobs(work: Path) -> None:
    segments = json.loads((EXPR / "passages/segments.json").read_text())
    plans = {p: json.loads((EXPR / f"director/out/{p}.json").read_text())["plan"] for p in segments}
    conds = build_conditions(segments, plans)
    spec = {"code_repo": CODE_REPO, "code_commit": CODE_COMMIT, "models": MODELS, "uv_version": UV_VERSION,
            "seed": SEED, "refs": {v: f"ref_{v}.wav" for v in ("lvf", "lvm", "heart")}, "conditions": conds}
    ds = work / "dataset"
    ds.mkdir(parents=True, exist_ok=True)
    (ds / "jobs.json").write_text(json.dumps(spec, ensure_ascii=False, indent=1))
    shutil.copy(HERE / "itts_worker.py", ds / "itts_worker.py")
    (work / "conditions.json").write_text(json.dumps(conds, ensure_ascii=False, indent=1))
    n = sum(len(c["items"]) for c in conds)
    chars = sum(len(i["text"]) for c in conds for i in c["items"])
    print(f"{len(conds)} condition sets, {n} items, {chars} chars")


# ------------------------------------------------------------------------------------------------------------ push
def running_gpu_kernels() -> list[str]:
    import csv
    import datetime as dt

    rows = list(csv.DictReader(kaggle("kernels", "list", "--mine", "--page-size", "40", "--csv",
                                      capture=True).stdout.splitlines()))
    since = (dt.datetime.utcnow() - dt.timedelta(hours=14)).strftime("%Y-%m-%d %H:%M")
    busy = []
    for row in rows:
        ref = row.get("ref") or ""
        if not ref or (row.get("lastRunTime") or "") < since:
            continue
        st = kaggle("kernels", "status", ref, capture=True).stdout.lower()
        if "running" in st or "queued" in st:
            busy.append(ref)
    return busy


def cmd_push(work: Path, max_busy: int) -> None:
    ds = work / "dataset"
    for f in ("jobs.json", "itts_worker.py", "ref_lvf.wav", "ref_lvm.wav", "ref_heart.wav"):
        if not (ds / f).exists():
            raise SystemExit(f"missing {ds / f}: run refs and jobs first")
    (ds / "dataset-metadata.json").write_text(json.dumps(
        {"title": "streams-itts25-bakeoff-inputs", "id": DATASET, "licenses": [{"name": "other"}]}))
    exists = kaggle("datasets", "status", DATASET, capture=True)
    if exists.returncode == 0 and "ready" in exists.stdout.lower():
        r = kaggle("datasets", "version", "-p", str(ds), "-m", f"inputs {time.strftime('%Y-%m-%d %H:%M')}")
    else:
        r = kaggle("datasets", "create", "-p", str(ds))
    if r.returncode != 0:
        raise SystemExit("dataset upload failed")
    for _ in range(60):
        st = kaggle("datasets", "status", DATASET, capture=True).stdout.strip().lower()
        print("dataset:", st, flush=True)
        if "ready" in st:
            break
        time.sleep(10)
    kd = work / "kernel"
    kd.mkdir(exist_ok=True)
    shutil.copy(HERE / "itts_kernel.py", kd / "itts_kernel.py")
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-itts25-bakeoff", "code_file": "itts_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [DATASET], "competition_sources": [],
        "kernel_sources": []}))
    # Queue politely: wait while the account already runs max_busy kernels, one push attempt per wait.
    for attempt in range(48):
        busy = running_gpu_kernels()
        print(time.strftime("%H:%M:%S"), "busy kernels:", busy, flush=True)
        if len(busy) < max_busy:
            r = kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4", capture=True)
            print(r.stdout, r.stderr, flush=True)
            if r.returncode == 0 and "error" not in (r.stdout + r.stderr).lower():
                return
        time.sleep(300)
    raise SystemExit("kernel push did not go through")


def cmd_pull(work: Path, wait: bool) -> None:
    while True:
        st = kaggle("kernels", "status", KERNEL, capture=True)
        line = (st.stdout + st.stderr).strip()
        print(time.strftime("%H:%M:%S"), line, flush=True)
        low = line.lower()
        if "complete" in low or "error" in low or "cancel" in low or not wait:
            break
        time.sleep(120)
    out = work / "out"
    out.mkdir(exist_ok=True)
    kaggle("kernels", "output", KERNEL, "-p", str(out), "-o")


# --------------------------------------------------------------------------------------------------------- collect
def cmd_collect(work: Path, audio: Path) -> None:
    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    from nolabs_engine.audio import pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    # crates/server/src/config.rs MING_AUDIO at 48 kbps, the profile the Ming study used, so the clips are comparable.
    profile = AudioProfile(id=2, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    conds = json.loads((work / "conditions.json").read_text())
    log = json.loads((work / "out/itts_log.json").read_text())
    timing = {(i["label"], i["passage"], i["file"]): i for i in log["items"]}
    model = log.get("model_version", "2.5")
    for c in conds:
        variants = [(c["label"], None)]
        if c["label"].endswith("_directed"):
            variants.append((c["label"] + "_dp", "director"))
        raw = work / "out/raw" / c["label"] / c["passage"]
        clips = []
        for it in c["items"]:
            f = raw / it["file"]
            if not f.exists():
                clips.append(RuntimeError(f"missing {f}"))
                continue
            w, sr = sf.read(f, dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        for label, pauses in variants:
            d = audio / label / c["passage"]
            d.mkdir(parents=True, exist_ok=True)
            stream = pipeline.Stream(profile)
            levelled = [stream.level(cl) for cl in clips]
            meta = []
            for k, (it, lv) in enumerate(zip(c["items"], levelled, strict=True)):
                if isinstance(lv, BaseException):
                    raise SystemExit(f"{c['label']}/{c['passage']}/{it['file']}: {lv}")
                prof = profile
                if pauses == "director":
                    ms = int(it["pause_after_ms"])
                    prof = dataclasses.replace(profile, pauses=PauseProfile(sentence_ms=ms, paragraph_ms=ms,
                                                                            section_ms=ms))
                fin = pipeline._finish(lv.prepared, lv.boundary, lv.gain_db, prof, codec_encode())
                name = f"{k:02d}.webm"
                (d / name).write_bytes(fin.data)
                t = timing.get((c["label"], c["passage"], it["file"]), {})
                meta.append({"file": name, "text": it["text"], "boundary": it["boundary"], "mode": it["mode"],
                             "emotion": it.get("emotion"), "emo_vector": t.get("emo_vector"),
                             "duration_factor": it.get("duration_factor"), "duration_ms": fin.duration_ms,
                             "gain_db": round(fin.gain_db, 2), "gpu_s": t.get("gpu_s"),
                             "raw_audio_s": t.get("audio_s"), "asr": t.get("asr")})
            (d / "meta.json").write_text(json.dumps({"voice": f"indextts-{model}:{c['voice']}", "label": label,
                                                     "passage": c["passage"], "engine": f"indextts-{model}",
                                                     "segments": meta}, ensure_ascii=False, indent=1))
            print(f"{label}/{c['passage']}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")


def codec_encode():
    from nolabs_engine.audio import codec
    return codec.encode_webm


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["refs", "jobs", "push", "pull", "collect"])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--audio", type=Path)
    ap.add_argument("--ming-ref", type=Path)
    ap.add_argument("--wait", action="store_true")
    ap.add_argument("--max-busy", type=int, default=2)
    a = ap.parse_args()
    if a.cmd == "refs":
        cmd_refs(a.work, a.ming_ref)
    elif a.cmd == "jobs":
        cmd_jobs(a.work)
    elif a.cmd == "push":
        cmd_push(a.work, a.max_busy)
    elif a.cmd == "pull":
        cmd_pull(a.work, a.wait)
    else:
        cmd_collect(a.work, a.audio)


if __name__ == "__main__":
    main()
