#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""Ming-3.0-tts-0.5B on a private Kaggle T4 kernel for the expressiveness study (notes/32). Never touches ZeroGPU.

    uv run tools/expressiveness/ming/ming_kaggle.py ref     --work W   # Kokoro af_heart reference via the live API
    uv run tools/expressiveness/ming/ming_kaggle.py jobs    --work W   # W/dataset/jobs.json from segments + directions
    uv run tools/expressiveness/ming/ming_kaggle.py push    --work W   # private dataset + private GPU kernel (T4)
    uv run tools/expressiveness/ming/ming_kaggle.py pull    --work W   # wait for the kernel, download W/out
    uv run tools/expressiveness/ming/ming_kaggle.py probe   --work W   # re-render items that came back silent
    uv run tools/expressiveness/ming/ming_kaggle.py pull    --work W --probe   # its output to W/probe/out
    uv run tools/expressiveness/ming/ming_kaggle.py collect --work W --audio A   # A/<label>/<passage>/NN.webm + meta

Kaggle auth: KAGGLE_API_TOKEN from ~/.config/streams/kaggle.env, passed to the CLI (uvx kaggle) through the
environment only. Conditions (labels):
- ming_<voice>_ip           IP preset, one item per segment (paragraph), as the app sends a neutral Ming preset.
- ming_<voice>_ip_sent      IP preset, one item per sentence (sentence pause 250 ms inside a paragraph): control for
                            the per-sentence split that the directed condition needs.
- ming_<voice>_directed     per sentence: neutral -> IP, emotion -> EMOTION on the preset's IP anchor, tagged ->
                            TTS on the anchor + ANCHOR_TEXT (passages/directions.json).
- ming_clone_heart          TTS on the Kokoro af_heart reference + transcript, one item per segment.
- ming_clone_heart_sent     the same per sentence.
- ming_clone_heart_directed per sentence: neutral -> TTS, emotion -> EMOTION on the reference, tagged -> TTS.
The audio is then run through the Space's own post-processing (space/nolabs_engine/audio/pipeline.py: trim, programme
loudness with the Ming profile, boundary pauses) and encoded to WebM/Opus 48 kbps, 48 kHz.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tarfile
import time
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
EXPR = HERE.parent
REPO = EXPR.parent.parent
SPACE = REPO / "space"
sys.path.insert(0, str(SPACE))
sys.path.insert(0, str(EXPR))

OWNER = "lbrahimali"
DATASET = f"{OWNER}/streams-ming-study-inputs"
KERNEL = f"{OWNER}/streams-ming-expressiveness"
PROBE_KERNEL = f"{OWNER}/streams-ming-probe"
PRESETS = {"emma": "US_Emma", "george": "UK_George"}
# Neutral, unrelated to the test passages, about 14 s in af_heart: inside the app's 10-18 s reference window.
REF_TEXT = ("The library opens at nine on weekdays and at ten on Saturdays. Books may be borrowed for three weeks, and "
            "most of them can be renewed online. Please return them to the desk by the main entrance, or use the drop "
            "box outside.")
BITRATE = 48
SEED = 1234


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


def decode(path: Path, sr: int) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", "1",
           "-ar", str(sr), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32)


# ------------------------------------------------------------------------------------------------------------ ref
def cmd_ref(work: Path) -> None:
    import pyloudnorm

    from nolabs_engine.audio import silence
    from render import Api

    ds = work / "dataset"
    ds.mkdir(parents=True, exist_ok=True)
    api = Api().login()
    st = api.render("kokoro:af_heart", [{"text": REF_TEXT, "boundary": "sentence", "emotion": "neutral"}])
    webm = work / "ref_af_heart.webm"
    webm.write_bytes(api.req("GET", f"/api/audio/{st[0]['key']}", raw=True))
    sr = 24_000
    wave = decode(webm, sr)
    # As nolabs_engine/reference.py prepares a user reference: speech span only, -20 LUFS, peak under -1.5 dBFS.
    wave = silence.trim(wave, sr, guard_ms=100, fade_ms=10).wave
    gain = -20.0 - pyloudnorm.Meter(sr).integrated_loudness(wave.astype(np.float64))
    wave = wave * 10 ** (gain / 20)
    wave *= min(1.0, 10 ** (-1.5 / 20) / max(1e-9, float(np.abs(wave).max())))
    sf.write(ds / "ref_af_heart.wav", wave.astype(np.float32), sr, subtype="PCM_16")
    print(f"reference: {wave.size / sr:.2f} s, gain {gain:+.1f} dB, key {st[0]['key'][:12]}")


# ----------------------------------------------------------------------------------------------------------- jobs
def _sentences(seg: dict) -> list[str]:
    return [seg["text"][a:b] for a, b in seg["sentences"]]


def build_conditions(segments: dict, directions: dict) -> list[dict]:
    from nolabs_engine.engines.ming_plan import EMOTION_LABELS

    def per_segment(label, voice, passage, segs, task, controls, prompt):
        return {"label": label, "voice": voice, "passage": passage, "items": [
            {"file": f"{i:02d}.flac", "text": s["text"], "spoken": s["text"], "boundary": s["boundary"],
             "task": task, "controls": controls, "prompt": prompt, "emotion": "neutral"} for i, s in enumerate(segs)]}

    def per_sentence(label, voice, passage, segs, plan):
        items = []
        for si, s in enumerate(segs):
            sents = _sentences(s)
            dirs = directions[passage][si]
            assert len(dirs) == len(sents), (passage, si, len(dirs), len(sents))
            for k, (text, d) in enumerate(zip(sents, dirs)):
                emotion, tagged = (d, None) if isinstance(d, str) else (d[0], d[1])
                task, controls, prompt, spoken = plan(emotion, tagged, text)
                items.append({"file": f"{len(items):02d}.flac", "text": text, "spoken": spoken,
                              "boundary": "sentence" if k + 1 < len(sents) else s["boundary"], "task": task,
                              "controls": controls, "prompt": prompt, "emotion": emotion, "tagged": tagged is not None})
        return {"label": label, "voice": voice, "passage": passage, "items": items}

    out = []
    for short, ip in PRESETS.items():
        anchor = f"anchor:{ip}"

        def neutral_ip(emotion, tagged, text, ip=ip):
            return "IP", {"ip": ip}, "", text

        def directed_ip(emotion, tagged, text, ip=ip, anchor=anchor):
            if tagged:
                return "TTS", {}, anchor, tagged
            if emotion == "neutral":
                return "IP", {"ip": ip}, "", text
            return "EMOTION", {"emotion": EMOTION_LABELS[emotion]}, anchor, text

        for passage, segs in segments.items():
            out.append(per_segment(f"ming_{short}_ip", f"ming:{ip}", passage, segs, "IP", {"ip": ip}, ""))
            out.append(per_sentence(f"ming_{short}_ip_sent", f"ming:{ip}", passage, segs, neutral_ip))
            out.append(per_sentence(f"ming_{short}_directed", f"ming:{ip}", passage, segs, directed_ip))

    ref = "ref:af_heart"

    def neutral_ref(emotion, tagged, text):
        return "TTS", {}, ref, text

    def directed_ref(emotion, tagged, text):
        if tagged:
            return "TTS", {}, ref, tagged
        if emotion == "neutral":
            return "TTS", {}, ref, text
        return "EMOTION", {"emotion": EMOTION_LABELS[emotion]}, ref, text

    for passage, segs in segments.items():
        out.append(per_segment("ming_clone_heart", "ming:clone(kokoro:af_heart)", passage, segs, "TTS", {}, ref))
        out.append(per_sentence("ming_clone_heart_sent", "ming:clone(kokoro:af_heart)", passage, segs, neutral_ref))
        out.append(per_sentence("ming_clone_heart_directed", "ming:clone(kokoro:af_heart)", passage, segs,
                                directed_ref))
    return out


def cmd_jobs(work: Path) -> None:
    from nolabs_engine import revisions
    from nolabs_engine.engines.ming_plan import ANCHOR_TEXT

    segments = json.loads((EXPR / "passages/segments.json").read_text())
    directions = json.loads((EXPR / "passages/directions.json").read_text())
    conds = build_conditions(segments, directions)
    # The kernel synthesises `spoken` (tags included); `text` stays the plain sentence for syllable counts.
    kernel_conds = [{**c, "items": [{**i, "text": i["spoken"]} for i in c["items"]]} for c in conds]
    spec = {"repo": revisions.MING_REPO, "commit": revisions.MING_COMMIT, "files": list(revisions.MING_FILES),
            "seed": SEED, "anchor_text": ANCHOR_TEXT, "anchors": list(PRESETS.values()),
            "refs": {"af_heart": {"file": "ref_af_heart.wav", "transcript": REF_TEXT}}, "conditions": kernel_conds}
    ds = work / "dataset"
    ds.mkdir(parents=True, exist_ok=True)
    (ds / "jobs.json").write_text(json.dumps(spec, ensure_ascii=False, indent=1))
    (work / "conditions.json").write_text(json.dumps(conds, ensure_ascii=False, indent=1))
    n = sum(len(c["items"]) for c in conds)
    chars = sum(len(i["spoken"]) for c in conds for i in c["items"])
    tasks = {}
    for c in conds:
        for i in c["items"]:
            tasks[i["task"]] = tasks.get(i["task"], 0) + 1
    print(f"{len(conds)} condition sets, {n} items, {chars} chars, tasks {tasks}")


# ----------------------------------------------------------------------------------------------------------- push
def cmd_push(work: Path) -> None:
    ds = work / "dataset"
    if not (ds / "jobs.json").exists() or not (ds / "ref_af_heart.wav").exists():
        raise SystemExit("run ref and jobs first")
    with tarfile.open(ds / "ming3.tgz", "w:gz") as tf:
        tf.add(SPACE / "vendor/ming3", arcname="ming3",
               filter=lambda ti: None if "__pycache__" in ti.name else ti)
    (ds / "dataset-metadata.json").write_text(json.dumps(
        {"title": "streams-ming-study-inputs", "id": DATASET, "licenses": [{"name": "other"}]}))
    exists = kaggle("datasets", "status", DATASET, capture=True)
    if exists.returncode == 0 and "ready" in exists.stdout.lower():
        r = kaggle("datasets", "version", "-p", str(ds), "-m", f"inputs {time.strftime('%Y-%m-%d %H:%M')}")
    else:
        r = kaggle("datasets", "create", "-p", str(ds))
    if r.returncode != 0:
        raise SystemExit("dataset upload failed")
    for _ in range(60):
        st = kaggle("datasets", "status", DATASET, capture=True).stdout.strip().lower()
        print("dataset:", st)
        if "ready" in st:
            break
        time.sleep(10)
    kd = work / "kernel"
    kd.mkdir(exist_ok=True)
    shutil.copy(HERE / "ming_kernel.py", kd / "ming_kernel.py")
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-ming-expressiveness", "code_file": "ming_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [DATASET], "competition_sources": [],
        "kernel_sources": []}))
    if kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4").returncode != 0:
        raise SystemExit("kernel push failed")


def cmd_probe(work: Path) -> None:
    """Silent items of the first run (peak < 0.02) -> a second private T4 kernel with seed fallbacks."""
    log = json.loads((work / "out/ming_log.json").read_text())
    conds = {(c["label"], c["passage"]): c for c in json.loads((work / "conditions.json").read_text())}
    items = []
    for e in log["items"]:
        if e["label"] != "anchor" and e.get("peak", 1.0) < 0.02:
            it = next(i for i in conds[(e["label"], e["passage"])]["items"] if i["file"] == e["file"])
            if it["prompt"] != "anchor:UK_George" or it["task"] != "TTS":
                raise SystemExit(f"probe only handles TTS on the UK_George anchor: {e}")
            items.append({"label": e["label"], "passage": e["passage"], "file": e["file"], "text": it["spoken"]})
    print(f"{len(items)} silent items")
    kd = work / "probe/kernel"
    kd.mkdir(parents=True, exist_ok=True)
    src = (HERE / "ming_probe_kernel.py").read_text()
    (kd / "ming_probe_kernel.py").write_text(src.replace("__PROBE__", repr({"items": items}), 1))
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": PROBE_KERNEL, "title": "streams-ming-probe", "code_file": "ming_probe_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [DATASET], "competition_sources": [],
        "kernel_sources": []}))
    if kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4").returncode != 0:
        raise SystemExit("kernel push failed")


def cmd_pull(work: Path, wait: bool, probe: bool = False) -> None:
    kernel = PROBE_KERNEL if probe else KERNEL
    while True:
        st = kaggle("kernels", "status", kernel, capture=True)
        line = (st.stdout + st.stderr).strip()
        print(time.strftime("%H:%M:%S"), line, flush=True)
        low = line.lower()
        if "complete" in low or "error" in low or "cancel" in low or not wait:
            break
        time.sleep(60)
    out = work / ("probe/out" if probe else "out")
    out.mkdir(parents=True, exist_ok=True)
    kaggle("kernels", "output", kernel, "-p", str(out), "-o")


# -------------------------------------------------------------------------------------------------------- collect
def cmd_collect(work: Path, audio: Path) -> None:
    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    from nolabs_engine.audio import pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    # crates/server/src/config.rs MING_AUDIO, at the study's 48 kbps.
    profile = AudioProfile(id=2, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    conds = json.loads((work / "conditions.json").read_text())
    log = json.loads((work / "out/ming_log.json").read_text())
    timing = {(i["label"], i.get("passage"), i.get("file")): i for i in log["items"]}
    probe_log = work / "probe/out/probe_log.json"
    replaced = set()
    if probe_log.exists():  # re-rendered items replace the silent ones; timing of the try that was kept
        for e in json.loads(probe_log.read_text())["items"]:
            ok = [t for t in e["tries"] if t.get("peak", 0) > 0.02]
            if ok:
                key = (e["label"], e["passage"], e["file"])
                timing[key] = {**e, **ok[-1]}
                replaced.add(key)
    for c in conds:
        raw = work / "out/raw" / c["label"] / c["passage"]
        d = audio / c["label"] / c["passage"]
        d.mkdir(parents=True, exist_ok=True)
        clips = []
        for it in c["items"]:
            f = raw / it["file"]
            if (c["label"], c["passage"], it["file"]) in replaced:
                f = work / "probe/out/raw" / c["label"] / c["passage"] / it["file"]
            if not f.exists():
                clips.append(RuntimeError(f"missing {f}"))
                continue
            w, sr = sf.read(f, dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        done = pipeline.finish_batch(clips, profile)
        bad = [(it["file"], fin) for it, fin in zip(c["items"], done, strict=True) if isinstance(fin, BaseException)]
        if bad:  # a set with a failed clip is not measured; stale clips of an earlier collect are removed
            print(f"SKIP {c['label']}/{c['passage']}: " + "; ".join(f"{f}: {e}" for f, e in bad))
            shutil.rmtree(d)
            continue
        meta = []
        for k, (it, fin) in enumerate(zip(c["items"], done, strict=True)):
            name = f"{k:02d}.webm"
            (d / name).write_bytes(fin.data)
            t = timing.get((c["label"], c["passage"], it["file"]), {})
            meta.append({"file": name, "text": it["text"], "spoken": it["spoken"], "boundary": it["boundary"],
                         "task": it["task"], "emotion": it["emotion"], "duration_ms": fin.duration_ms,
                         "gain_db": round(fin.gain_db, 2), "gpu_s": t.get("gpu_s"), "raw_audio_s": t.get("audio_s"),
                         "seed": t.get("seed", SEED)})
        (d / "meta.json").write_text(json.dumps({"voice": c["voice"], "label": c["label"], "passage": c["passage"],
                                                 "engine": "ming", "segments": meta}, ensure_ascii=False, indent=1))
        print(f"{c['label']}/{c['passage']}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["ref", "jobs", "push", "probe", "pull", "collect"])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--audio", type=Path)
    ap.add_argument("--wait", action="store_true")
    ap.add_argument("--probe", action="store_true", help="pull: the probe kernel")
    a = ap.parse_args()
    if a.cmd == "ref":
        cmd_ref(a.work)
    elif a.cmd == "jobs":
        cmd_jobs(a.work)
    elif a.cmd == "push":
        cmd_push(a.work)
    elif a.cmd == "probe":
        cmd_probe(a.work)
    elif a.cmd == "pull":
        cmd_pull(a.work, a.wait, a.probe)
    else:
        cmd_collect(a.work, a.audio)


if __name__ == "__main__":
    main()
