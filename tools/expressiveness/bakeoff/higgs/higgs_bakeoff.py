#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""Higgs TTS 3 4B (bosonai/higgs-tts-3-4b via the transformers port multimodalart/higgs-audio-v3-tts-4b-transformers)
on a private Kaggle GPU kernel for the expressiveness bake-off. Never touches ZeroGPU or the live API.

    uv run tools/expressiveness/bakeoff/higgs/higgs_bakeoff.py push    --work W   # build jobs, push the private kernel
    uv run tools/expressiveness/bakeoff/higgs/higgs_bakeoff.py status
    uv run tools/expressiveness/bakeoff/higgs/higgs_bakeoff.py pull    --work W [--wait]   # download W/out
    uv run tools/expressiveness/bakeoff/higgs/higgs_bakeoff.py collect --work W --audio A  # A/<label>/<passage>/NN.webm

Voices: two synthetic narrators, a female and a male, picked from Higgs zero-shot renders of a neutral 14 s text (no
real person is cloned); the chosen render is the fixed clone reference for every item of that voice. Conditions per voice (labels higgs_<voice>_<cond>):
- plain      untagged, one call per segment (paragraph), as the app sends text. Seed 1234.
- plain_s2   the same with seed 99: sampling spread.
- expr       <|prosody:expressive_high|> in front of each segment, otherwise untagged.
- tagged     per segment, every sentence with its own hand-picked tags (higgs_directions.json).
- tagged_sent the same tags, one call per sentence (sentence pause 250 ms inside a paragraph).
- director   per segment, tags mapped mechanically from the LLM director (director/out/<passage>.json, Gemma 4,
             PROMPT_VERSION 1): DIRECTOR_EMOTION for a non-neutral emotion with confidence >= 0.5, pace slow/fast ->
             prosody:speed_slow/speed_fast, intensity >= 0.7 -> prosody:expressive_high. Pauses stay with the pipeline.
Sampling: temperature 0.8, top_k 50 (the model card's recommendation for cloning). The audio goes through the Space's
post-processing (trim, programme loudness, boundary pauses) and is encoded to WebM/Opus 48 kbps, 48 kHz.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXPR = HERE.parent.parent
REPO = EXPR.parent.parent
sys.path.insert(0, str(REPO / "space"))

KERNEL = "lbrahimali/streams-higgs-bakeoff"
REPOS = {  # pinned revisions, checked on the Hub 2026-10-31
    "model": ["multimodalart/higgs-audio-v3-tts-4b-transformers", "30f01593ee6a12efa586c92455afe4b76e45095d"],
    "codec": ["bosonai/higgs-audio-v2-tokenizer", "815a03f1b57dcb5ae918db415f22cd0c8a6f0f54"],
    "whisper": ["openai/whisper-large-v3-turbo", "41f01f3fe87f28c78e2fbf8b568835947dd65ed9"],
}
PINS = ["transformers==5.5.4"]
SAMPLING = {"temperature": 0.8, "top_k": 50}
# Reference voices: synthetic, made by Higgs itself (the licence, IV(b)(ii), forbids cloning a real person without
# consent, so LibriVox readers are not used). Zero-shot REF_TEXT over these seeds; per voice the WER-clean candidate
# nearest the target F0 (lo, hi, target Hz) is kept once at least per_voice clean candidates exist.
REF_TEXT = ("The library opens at nine on weekdays and at ten on Saturdays. Books may be borrowed for three weeks, and "
            "most of them can be renewed online. Please return them to the desk by the main entrance, or use the drop "
            "box outside.")
REF_SEARCH = {"text": REF_TEXT, "seeds": list(range(1, 41)), "max_wer": 0.1, "min_s": 8.0, "max_s": 20.0,
              "per_voice": 2, "targets": {"female": [160, 280, 200], "male": [80, 150, 115]}}
GPU = {"female": 0, "male": 1}
DIRECTOR_EMOTION = {"happy": "amusement", "sad": "sadness", "angry": "anger", "fearful": "fear",
                    "disgusted": "disgust", "surprised": "surprise"}


def director_tags(passage: str) -> list[str]:
    d = json.loads((EXPR / "director/out" / f"{passage}.json").read_text())
    out = []
    for s in sorted(d["raw"]["sentences"], key=lambda x: x["sentence_idx"]):
        t = ""
        if s["emotion"] in DIRECTOR_EMOTION and s["confidence"] >= 0.5:
            t += f"<|emotion:{DIRECTOR_EMOTION[s['emotion']]}|>"
        if s["pace"] in ("slow", "fast"):
            t += f"<|prosody:speed_{s['pace']}|>"
        if s["intensity"] >= 0.7:
            t += "<|prosody:expressive_high|>"
        out.append(t)
    return out
BITRATE = 48


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


def plain_len(s: str) -> int:
    return len(re.sub(r"<\|[^|]*\|>", "", s))


def max_tokens(s: str) -> int:
    # 25 codec frames/s; 0.1 s per character is slower than any reading of these passages, x1.8 headroom.
    return min(2048, int(25 * max(3.0, 0.1 * plain_len(s)) * 1.8) + 60)


def build_items() -> tuple[list[dict], list[dict]]:
    segments = json.loads((EXPR / "passages/segments.json").read_text())
    tags = json.loads((HERE / "higgs_directions.json").read_text())
    items, conds = [], []

    def add(label, voice, passage, entries):
        cond = {"label": label, "voice": voice, "passage": passage, "items": []}
        for i, (text, spoken, boundary, seed) in enumerate(entries):
            it = {"label": label, "passage": passage, "file": f"{i:02d}.flac", "text": text, "spoken": spoken,
                  "boundary": boundary, "seed": seed, "ref": voice, "gpu": GPU[voice],
                  "max_new_tokens": max_tokens(spoken)}
            cond["items"].append(it)
            items.append(it)
        conds.append(cond)

    for voice in REF_SEARCH["targets"]:
        for passage, segs in segments.items():
            def sents(si, s):
                out = []
                for k, (a, b) in enumerate(s["sentences"]):
                    t = tags[passage][si][k]
                    out.append((s["text"][a:b], t if t else s["text"][a:b]))
                return out

            pre = f"higgs_{voice}"
            add(f"{pre}_plain", voice, passage, [(s["text"], s["text"], s["boundary"], 1234) for s in segs])
            add(f"{pre}_plain_s2", voice, passage, [(s["text"], s["text"], s["boundary"], 99) for s in segs])
            add(f"{pre}_expr", voice, passage,
                [(s["text"], "<|prosody:expressive_high|>" + s["text"], s["boundary"], 1234) for s in segs])
            add(f"{pre}_tagged", voice, passage,
                [(s["text"], " ".join(sp for _, sp in sents(si, s)), s["boundary"], 1234)
                 for si, s in enumerate(segs)])
            dt = director_tags(passage)
            assert len(dt) == sum(len(s["sentences"]) for s in segs), passage
            ent, g = [], 0
            for s in segs:
                parts = []
                for a, b in s["sentences"]:
                    parts.append(dt[g] + s["text"][a:b])
                    g += 1
                ent.append((s["text"], " ".join(parts), s["boundary"], 1234))
            add(f"{pre}_director", voice, passage, ent)
            ent = []
            for si, s in enumerate(segs):
                ss = sents(si, s)
                for k, (t, sp) in enumerate(ss):
                    ent.append((t, sp, "sentence" if k + 1 < len(ss) else s["boundary"], 1234))
            add(f"{pre}_tagged_sent", voice, passage, ent)
    return items, conds


def cmd_push(work: Path) -> None:
    items, conds = build_items()
    work.mkdir(parents=True, exist_ok=True)
    (work / "conditions.json").write_text(json.dumps(conds, ensure_ascii=False, indent=1))
    jobs = {"repos": REPOS, "pins": PINS, "sampling": SAMPLING, "ref_search": REF_SEARCH, "items": items}
    blob = json.dumps(jobs, ensure_ascii=False)
    assert "'''" not in blob
    src = (HERE / "higgs_kernel.py").read_text().replace("__JOBS__", blob)
    kd = work / "kernel"
    kd.mkdir(exist_ok=True)
    (kd / "higgs_kernel.py").write_text(src)
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-higgs-bakeoff", "code_file": "higgs_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [], "competition_sources": [], "kernel_sources": []}))
    audio_s = sum(plain_len(i["spoken"]) for i in items) * 0.07
    print(f"{len(items)} items in {len(conds)} sets, about {audio_s / 60:.0f} min of audio")
    if kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4").returncode != 0:
        raise SystemExit("kernel push failed")


def status() -> str:
    st = kaggle("kernels", "status", KERNEL, capture=True)
    return (st.stdout + st.stderr).strip()


def cmd_pull(work: Path, wait: bool) -> None:
    while True:
        line = status()
        print(time.strftime("%H:%M:%S"), line, flush=True)
        low = line.lower()
        if "complete" in low or "error" in low or "cancel" in low or not wait:
            break
        time.sleep(120)
    out = work / "out"
    out.mkdir(exist_ok=True)
    kaggle("kernels", "output", KERNEL, "-p", str(out), "-o")


def cmd_collect(work: Path, audio: Path) -> None:
    import imageio_ffmpeg
    import soundfile as sf

    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    from nolabs_engine.audio import pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    # crates/server/src/config.rs MING_AUDIO (the 48 kHz profile) at the study's 48 kbps.
    profile = AudioProfile(id=2, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    conds = json.loads((work / "conditions.json").read_text())
    log = json.loads((work / "out/higgs_log.json").read_text())
    timing = {}
    for w in log.get("workers", {}).values():
        for i in w["items"]:
            timing[(i["label"], i["passage"], i["file"])] = i
    asr = {(a["label"], a["passage"], a["file"]): a for a in log.get("asr", [])}
    for c in conds:
        raw = work / "out/raw" / c["label"] / c["passage"]
        d = audio / c["label"] / c["passage"]
        d.mkdir(parents=True, exist_ok=True)
        clips = []
        for it in c["items"]:
            f = raw / it["file"]
            if not f.exists():
                clips.append(RuntimeError(f"missing {f}"))
                continue
            w, sr = sf.read(f, dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        if any(isinstance(x, BaseException) for x in clips):
            print(f"skip {c['label']}/{c['passage']}: missing clips", file=sys.stderr)
            continue
        done = pipeline.finish_batch(clips, profile)
        meta = []
        for k, (it, fin) in enumerate(zip(c["items"], done, strict=True)):
            if isinstance(fin, BaseException):
                raise SystemExit(f"{c['label']}/{c['passage']}/{it['file']}: {fin}")
            name = f"{k:02d}.webm"
            (d / name).write_bytes(fin.data)
            key = (c["label"], c["passage"], it["file"])
            t, a = timing.get(key, {}), asr.get(key, {})
            meta.append({"file": name, "text": it["text"], "spoken": it["spoken"], "boundary": it["boundary"],
                         "seed": it["seed"], "duration_ms": fin.duration_ms, "gain_db": round(fin.gain_db, 2),
                         "gen_s": t.get("gen_s"), "raw_audio_s": t.get("audio_s"), "rtf": t.get("rtf"),
                         "hit_cap": t.get("hit_cap"), "asr": a.get("hyp"), "wer": a.get("wer")})
        (d / "meta.json").write_text(json.dumps({"voice": f"higgs:clone({c['voice']})", "label": c["label"],
                                                 "passage": c["passage"], "engine": "higgs-tts-3-4b",
                                                 "segments": meta}, ensure_ascii=False, indent=1))
        print(f"{c['label']}/{c['passage']}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")
    refs = audio / "refs"
    refs.mkdir(parents=True, exist_ok=True)
    ff = imageio_ffmpeg.get_ffmpeg_exe()
    for wav in sorted((work / "out/refs").glob("*.wav")):
        subprocess.run([ff, "-hide_banner", "-loglevel", "error", "-y", "-i", str(wav), "-c:a", "libopus", "-b:a",
                        f"{BITRATE}k", str(refs / (wav.stem + ".webm"))], check=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["push", "status", "pull", "collect"])
    ap.add_argument("--work", type=Path)
    ap.add_argument("--audio", type=Path)
    ap.add_argument("--wait", action="store_true")
    a = ap.parse_args()
    if a.cmd == "push":
        cmd_push(a.work)
    elif a.cmd == "status":
        print(status())
    elif a.cmd == "pull":
        cmd_pull(a.work, a.wait)
    else:
        cmd_collect(a.work, a.audio)


if __name__ == "__main__":
    main()
