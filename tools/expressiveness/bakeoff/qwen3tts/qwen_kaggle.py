#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg",
#                 "praat-parselmouth>=0.4.5"]
# ///
"""Qwen3-TTS 1.7B bakeoff on a private Kaggle GPU kernel (T4 x2). Never touches ZeroGPU or the live API.

    uv run tools/expressiveness/bakeoff/qwen3tts/qwen_kaggle.py jobs    --work W   # W/spec.json, W/conditions.json
    uv run tools/expressiveness/bakeoff/qwen3tts/qwen_kaggle.py push    --work W   # private GPU kernel, spec embedded
    uv run tools/expressiveness/bakeoff/qwen3tts/qwen_kaggle.py pull    --work W [--wait]   # status, download W/out
    uv run tools/expressiveness/bakeoff/qwen3tts/qwen_kaggle.py collect --work W   # bakeoff/qwen3tts/audio/<label>/...
    uv run tools/expressiveness/bakeoff/qwen3tts/qwen_kaggle.py check   --work W   # WER, speaker drift, artefacts

Conditions (label prefix qwen_):
- cv_<spk>_seg        CustomVoice preset, one item per segment (paragraph), no instruction (as the app would send).
- cv_<spk>_sent       the same, one item per sentence (control for the split).
- cv_<spk>_director   per sentence, instruction built from director/out (emotion, intensity, pace, emphasis).
- cv_<spk>_styled     per sentence, hand-written instruction (styles.json "sent").
- cv_<spk>_segstyled  per segment, hand-written paragraph instruction (styles.json "para").
- clone_<ref>_seg / _sent   Base ICL clone of a LibriVox intro (Whisper transcript), per segment / per sentence.
- vd_<g>_styled / _segstyled VoiceDesign: fixed persona + the hand-written style, per sentence / per segment.
- fast_cv_<spk>_styled faster-qwen3-tts (CUDA graphs) on CustomVoice with the styled instructions.
Post-processing as the Ming study (space/nolabs_engine/audio/pipeline.py, Ming profile), WebM/Opus 48 kbps.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import time
import zlib
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
EXPR = HERE.parent.parent
REPO = EXPR.parent.parent
sys.path.insert(0, str(REPO / "space"))
sys.path.insert(0, str(EXPR))

OWNER = "lbrahimali"
KERNEL = f"{OWNER}/streams-qwen3tts-bakeoff"
AUDIO = Path(os.environ.get("QWEN_AUDIO", HERE / "audio"))
SEED = 1234
BITRATE = 48
# Revisions verified on the Hugging Face API 2026-10-31.
MODELS = {
    "customvoice": {"repo": "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice", "revision": "0c0e3051f131929182e2c023b9537f8b1c68adfe"},
    "base": {"repo": "Qwen/Qwen3-TTS-12Hz-1.7B-Base", "revision": "fd4b254389122332181a7c3db7f27e918eec64e3"},
    "voicedesign": {"repo": "Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign", "revision": "5ecdb67327fd37bb2e042aab12ff7391903235d3"},
    "whisper": {"repo": "openai/whisper-large-v3-turbo", "revision": "41f01f3fe87f28c78e2fbf8b568835947dd65ed9"},
}
PINS = ["qwen-tts==0.1.1", "transformers==4.57.3", "accelerate==1.12.0", "imageio-ffmpeg==0.6.0"]
# run 2: unpinned, pip took transformers 5.22.1 (2026-10-30) and qwen-tts-hf crashed (Qwen3TTSTokenizerV2Encoder
# .upsample has no setter); 5.15.1 was the newest release when qwen-tts-hf 0.1.1.post1 shipped (2026-08-25)
FAST_PIN = "faster-qwen3-tts==0.5.4 qwen-tts-hf==0.1.1.post1 transformers==5.15.1"
IA = "https://archive.org/download"
# LibriVox intros ("This is a LibriVox recording ..."): neutral reading by the same public-domain readers as the
# human ceiling files. F0 median of the first 30 s (praat, 2026-10-31): poe 201 Hz, pp 223 Hz, walden 105 Hz.
REFS = {
    # run 4: "cut"/"text" fixed by hand from the energy envelope (scratchpad tools/pauses.py, 10 ms frames, max-35 dB):
    # Whisper word timestamps ran past the 30 s file for pp_f/walden_m and the auto cut fell back to one word.
    "poe_f": {"url": f"{IA}/12_creepytales_1206_librivox/creepytalesbypoe_01_poe_64kb.mp3", "start_s": 0,
              "length_s": 30, "max_s": 12, "min_s": 4, "cut": [0.63, 10.15],
              "text": "Chapter 1 of Creepy Tales by Edgar Allan Poe. This is a LibriVox recording."},
    "pp_f": {"url": f"{IA}/solo_pride_librivox/prideandprejudice_01-04_austen_apc_64kb.mp3", "start_s": 0,
             "length_s": 30, "max_s": 12, "min_s": 4},
    "walden_m": {"url": f"{IA}/walden_librivox/walden_c05_64kb.mp3", "start_s": 0, "length_s": 30, "max_s": 12, "min_s": 4,
                 "cut": [3.07, 13.33], "text": "This is a LibriVox recording. All LibriVox recordings are in the public "
                 "domain. For more information, please visit LibriVox.org."},
}
CV_MAIN = {"serena": "Serena", "aiden": "Aiden"}   # warm female (Chinese native), sunny/warm US male
CV_EXTRA = {"ryan": "Ryan", "vivian": "Vivian", "sohee": "Sohee"}
PERSONA = {
    "f": "A warm, mellow female audiobook narrator in her late thirties with a native English accent, low-medium "
         "pitch, soft and intimate, with clear diction.",
    "m": "A warm, resonant male audiobook narrator in his forties with a native American English accent, a gentle "
         "baritone, measured and kind, with clear diction.",
}
EMO_WORD = {"happy": "cheerful and amused", "sad": "sad and subdued", "angry": "angry", "fearful": "fearful and tense",
            "disgusted": "disgusted and contemptuous", "surprised": "surprised"}


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


# ----------------------------------------------------------------------------------------------------------- jobs
def director_instruction(s: dict) -> str:
    """Director plan sentence -> a short English instruction ('' = plain, as for a neutral normal sentence)."""
    parts = []
    if s["emotion"] != "neutral":
        x = s["intensity"]
        deg = "slightly " if x < 0.35 else ("very " if x > 0.6 else "")
        parts.append(f"{deg}{EMO_WORD[s['emotion']]}".capitalize())
    if s["pace"] == "slow":
        parts.append("slower than usual")
    elif s["pace"] == "fast":
        parts.append("faster, urgent")
    out = ", ".join(parts)
    emph = [re.sub(r"[^\w']", "", s["words"][i]) for i in s["emphasis"]]
    emph = [w for w in emph if w]
    if emph:
        out = (out + ". " if out else "") + "Stress the word" + ("s " if len(emph) > 1 else " ") + \
              " and ".join(f"'{w}'" for w in emph) + "."
    elif out:
        out += "."
    return out[:1].upper() + out[1:]


def build_conditions() -> list[dict]:
    segments = json.loads((EXPR / "passages/segments.json").read_text())
    styles = json.loads((HERE / "styles.json").read_text())
    plans = {p: json.loads((EXPR / f"director/out/{p}.json").read_text())["plan"] for p in segments}
    out = []

    def sentences(passage):
        """(segment index, sentence index in segment, text, boundary, director plan row) per sentence."""
        rows, k = [], 0
        for si, seg in enumerate(segments[passage]):
            n = len(seg["sentences"])
            for j, (a, b) in enumerate(seg["sentences"]):
                plan = plans[passage][k]
                assert plan["segment"] == si and plan["text"].strip() == seg["text"][a:b].strip(), (passage, k)
                rows.append((si, j, seg["text"][a:b], "sentence" if j + 1 < n else seg["boundary"], plan))
                k += 1
        return rows

    def per_seg(label, engine, voice, passage, ctrl_of):
        return {"label": label, "engine": engine, "voice": voice, "passage": passage, "items": [
            {"file": f"{i:02d}.flac", "text": s["text"], "boundary": s["boundary"], "controls": ctrl_of(i, s)}
            for i, s in enumerate(segments[passage])]}

    def per_sent(label, engine, voice, passage, ctrl_of):
        return {"label": label, "engine": engine, "voice": voice, "passage": passage, "items": [
            {"file": f"{k:02d}.flac", "text": t, "boundary": b, "controls": ctrl_of(si, j, t, plan)}
            for k, (si, j, t, b, plan) in enumerate(sentences(passage))]}

    for passage in segments:
        st = styles[passage]
        for short, spk in CV_MAIN.items():
            v = f"qwen3tts-cv:{spk}"
            out.append(per_seg(f"qwen_cv_{short}_seg", "cv", v, passage, lambda i, s, spk=spk: {"speaker": spk}))
            out.append(per_sent(f"qwen_cv_{short}_sent", "cv", v, passage,
                                lambda si, j, t, p, spk=spk: {"speaker": spk}))
            out.append(per_sent(f"qwen_cv_{short}_director", "cv", v, passage,
                                lambda si, j, t, p, spk=spk: {"speaker": spk, "instruct": director_instruction(p)}))
            out.append(per_sent(f"qwen_cv_{short}_styled", "cv", v, passage,
                                lambda si, j, t, p, spk=spk, st=st: {"speaker": spk, "instruct": st[si]["sent"][j]}))
            out.append(per_seg(f"qwen_cv_{short}_segstyled", "cv", v, passage,
                               lambda i, s, spk=spk, st=st: {"speaker": spk, "instruct": st[i]["para"]}))
            out.append(per_sent(f"qwen_fast_cv_{short}_styled", "fast", f"faster-qwen3-tts-cv:{spk}", passage,
                                lambda si, j, t, p, spk=spk, st=st: {"speaker": spk, "instruct": st[si]["sent"][j]}))
        for short, spk in CV_EXTRA.items():
            out.append(per_seg(f"qwen_cv_{short}_seg", "cv", f"qwen3tts-cv:{spk}", passage,
                               lambda i, s, spk=spk: {"speaker": spk}))
        for ref in REFS:
            v = f"qwen3tts-base:clone(librivox:{ref})"
            out.append(per_seg(f"qwen_clone_{ref}_seg", "clone", v, passage, lambda i, s, ref=ref: {"ref": ref}))
            out.append(per_sent(f"qwen_clone_{ref}_sent", "clone", v, passage,
                                lambda si, j, t, p, ref=ref: {"ref": ref}))
        for g, persona in PERSONA.items():
            v = f"qwen3tts-vd:{g}"
            out.append(per_sent(f"qwen_vd_{g}_styled", "vd", v, passage,
                                lambda si, j, t, p, persona=persona, st=st:
                                {"instruct": f"{persona} Delivery: {st[si]['sent'][j]}"}))
            out.append(per_seg(f"qwen_vd_{g}_segstyled", "vd", v, passage,
                               lambda i, s, persona=persona, st=st: {"instruct": f"{persona} Delivery: {st[i]['para']}"}))
    return out


def cmd_jobs(work: Path, only: str | None = None, skip_engines: str = "") -> None:
    conds = build_conditions()
    if only:
        conds = [c for c in conds if re.search(only, c["label"])]
    spec = {"models": MODELS, "pins": PINS, "fast_pin": FAST_PIN, "seed": SEED, "refs": REFS, "conditions": conds,
            "skip_engines": [e for e in skip_engines.split(",") if e], "retry_engines": ["clone", "vd"]}
    work.mkdir(parents=True, exist_ok=True)
    (work / "spec.json").write_text(json.dumps(spec, ensure_ascii=False, indent=1))
    n, eng = 0, {}
    for c in conds:
        eng[c["engine"]] = eng.get(c["engine"], 0) + len(c["items"])
        n += len(c["items"])
    print(f"{len(conds)} condition sets, {n} items, by engine {eng}")
    for c in conds[:12]:
        if c["passage"] == "1-suspense" and "director" in c["label"]:
            for it in c["items"]:
                print(" ", repr(it["controls"].get("instruct")))


# ----------------------------------------------------------------------------------------------------------- push
DATASET = f"{OWNER}/streams-qwen3tts-prior-raw"


def push_prior(work: Path) -> list[str]:
    """Run 2+: W/prior/prior_raw.tar (raw clips of engines already rendered) goes up as a private dataset."""
    ds = work / "prior"
    if not (ds / "prior_raw.tar").exists():
        return []
    if (ds / ".uploaded").exists():
        return [DATASET]
    (ds / "dataset-metadata.json").write_text(json.dumps(
        {"title": "streams-qwen3tts-prior-raw", "id": DATASET, "licenses": [{"name": "other"}]}))
    st = kaggle("datasets", "status", DATASET, capture=True)
    if st.returncode == 0 and "ready" in st.stdout.lower():
        r = kaggle("datasets", "version", "-p", str(ds), "-r", "skip", "-m", time.strftime("raw %Y-%m-%d %H:%M"))
    else:
        r = kaggle("datasets", "create", "-p", str(ds), "-r", "skip")
    if r.returncode != 0:
        raise SystemExit("dataset upload failed")
    for _ in range(60):
        s_ = kaggle("datasets", "status", DATASET, capture=True).stdout.strip().lower()
        print("dataset:", s_, flush=True)
        if "ready" in s_:
            (ds / ".uploaded").write_text(time.strftime("%Y-%m-%d %H:%M"))
            break
        time.sleep(10)
    return [DATASET]


def cmd_push(work: Path) -> None:
    sources = push_prior(work)
    spec = (work / "spec.json").read_bytes()
    src = (HERE / "qwen_kernel.py").read_text()
    blob = base64.b64encode(zlib.compress(spec, 9)).decode()
    assert 'JOBS_B64 = ""' in src
    kd = work / "kernel"
    kd.mkdir(parents=True, exist_ok=True)
    (kd / "qwen_kernel.py").write_text(src.replace('JOBS_B64 = ""', f'JOBS_B64 = "{blob}"', 1))
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-qwen3tts-bakeoff", "code_file": "qwen_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": sources, "competition_sources": [], "kernel_sources": []}))
    if kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4").returncode != 0:
        raise SystemExit("kernel push failed")


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
    out.mkdir(parents=True, exist_ok=True)
    kaggle("kernels", "output", KERNEL, "-p", str(out), "-o")


# -------------------------------------------------------------------------------------------------------- collect
def cmd_collect(work: Path) -> None:
    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    from nolabs_engine.audio import pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    profile = AudioProfile(id=2, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    spec = json.loads((work / "spec.json").read_text())
    timing = {}
    for wl in (work / "out").glob("worker_*.json"):
        for i in json.loads(wl.read_text()).get("items", []):
            timing[(i["label"], i["passage"], i["file"])] = i
    for c in spec["conditions"]:
        raw = work / "out/raw" / c["label"] / c["passage"]
        if not raw.exists():
            print(f"skip {c['label']}/{c['passage']}: no output")
            continue
        clips = []
        for it in c["items"]:
            f = raw / it["file"]
            if not f.exists():
                clips.append(RuntimeError(f"missing {f}"))
                continue
            w, sr = sf.read(f, dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        if any(isinstance(x, BaseException) for x in clips):
            print(f"skip {c['label']}/{c['passage']}: {sum(isinstance(x, BaseException) for x in clips)} missing")
            continue
        done = pipeline.finish_batch(clips, profile)
        bad = [f"{it['file']}: {fin}" for it, fin in zip(c["items"], done) if isinstance(fin, BaseException)]
        if bad:
            print(f"skip {c['label']}/{c['passage']}: {'; '.join(bad)[:200]}")
            continue
        d = AUDIO / c["label"] / c["passage"]
        d.mkdir(parents=True, exist_ok=True)
        meta = []
        for k, (it, fin) in enumerate(zip(c["items"], done, strict=True)):
            if isinstance(fin, BaseException):
                raise SystemExit(f"{c['label']}/{c['passage']}/{it['file']}: {fin}")
            name = f"{k:02d}.webm"
            (d / name).write_bytes(fin.data)
            t = timing.get((c["label"], c["passage"], it["file"]), {})
            meta.append({"file": name, "text": it["text"], "boundary": it["boundary"],
                         "instruct": it["controls"].get("instruct"), "duration_ms": fin.duration_ms,
                         "gain_db": round(fin.gain_db, 2), "gpu_s": t.get("gpu_s"), "raw_audio_s": t.get("audio_s"),
                         "capped": t.get("capped")})
        (d / "meta.json").write_text(json.dumps({"voice": c["voice"], "label": c["label"], "passage": c["passage"],
                                                 "engine": c["engine"], "segments": meta}, ensure_ascii=False, indent=1))
        print(f"{c['label']}/{c['passage']}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")


# ---------------------------------------------------------------------------------------------------------- check
def _words(s: str) -> list[str]:
    s = s.lower().replace("’", "'")
    s = re.sub(r"[^a-z0-9' ]+", " ", s)
    return [w.strip("'") for w in s.split() if w.strip("'")]


def wer(ref: str, hyp: str) -> tuple[int, int]:
    r, h = _words(ref), _words(hyp)
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
            prev, d[j] = d[j], cur
    return d[len(h)], len(r)


def cmd_check(work: Path) -> None:
    """Per condition: WER (Whisper large-v3-turbo on the raw clip), speaker drift (cosine of each clip's Base x-vector
    to the condition mean), raw clip timing (lead/trail silence, longest inner silence, chars/s), capped clips, and
    a crude spectral artefact proxy (share of speech-frame energy above 8 kHz)."""
    spec = json.loads((work / "spec.json").read_text())
    an = {}
    for f in (work / "out").glob("analysis_*.json"):
        an.update(json.loads(f.read_text()))
    timing = {}
    for wl in (work / "out").glob("worker_*.json"):
        for i in json.loads(wl.read_text()).get("items", []):
            timing[(i["label"], i["passage"], i["file"])] = i
    rows = {}
    for c in spec["conditions"]:
        r = rows.setdefault(c["label"], {"err": 0, "n": 0, "gpu_s": 0.0, "audio_s": 0.0, "capped": 0, "clips": 0,
                                         "embs": [], "lead": [], "trail": [], "gap": [], "cps": [], "hf": [],
                                         "worst": []})
        for it in c["items"]:
            key = f"raw/{c['label']}/{c['passage']}/{it['file']}"
            p = work / "out" / key
            if not p.exists():
                continue
            r["clips"] += 1
            a = an.get(key, {})
            if "asr" in a:
                e, n = wer(it["text"], a["asr"])
                r["err"] += e
                r["n"] += n
                if n and e / n > 0.2:
                    r["worst"].append(f"{c['passage']}/{it['file']} wer {e / n:.2f}: {a['asr'][:90]!r}")
            if a.get("spk"):
                r["embs"].append(np.array(a["spk"]))
            t = timing.get((c["label"], c["passage"], it["file"]), {})
            r["gpu_s"] += t.get("gpu_s") or 0
            r["audio_s"] += t.get("audio_s") or 0
            r["capped"] += bool(t.get("capped"))
            w, sr = sf.read(p, dtype="float32")
            hop = sr // 100
            fr = w[: w.size // hop * hop].reshape(-1, hop)
            db = 20 * np.log10(np.sqrt((fr ** 2).mean(1)) + 1e-9)
            sp = db > db.max() - 35
            idx = np.flatnonzero(sp)
            if idx.size:
                r["lead"].append(idx[0] / 100)
                r["trail"].append((sp.size - idx[-1] - 1) / 100)
                inner = sp[idx[0]: idx[-1] + 1]
                runs, cur = [], 0
                for v in inner:
                    cur = 0 if v else cur + 1
                    runs.append(cur)
                r["gap"].append(max(runs) / 100 if runs else 0)
                r["cps"].append(len(it["text"]) / max(0.1, (idx[-1] - idx[0] + 1) / 100))
                spec_ = np.abs(np.fft.rfft(fr[sp] * np.hanning(hop), axis=1)) ** 2 if sp.sum() else None
                if spec_ is not None:
                    fq = np.fft.rfftfreq(hop, 1 / sr)
                    r["hf"].append(float(spec_[:, fq >= 8000].sum() / spec_.sum()))
    out = {}
    print("| label | clips | WER % | spk cos min / mean | RTF (gpu s / audio s) | capped | lead/trail s | max inner gap s "
          "| chars/s | HF>8k % |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for label, r in rows.items():
        if not r["clips"]:
            continue
        cos = None
        if len(r["embs"]) > 1:
            E = np.stack(r["embs"])
            E = E / np.linalg.norm(E, axis=1, keepdims=True)
            m = E.mean(0)
            m /= np.linalg.norm(m)
            cs = E @ m
            cos = (round(float(cs.min()), 3), round(float(cs.mean()), 3))
        o = {"clips": r["clips"], "wer_pct": round(100 * r["err"] / r["n"], 1) if r["n"] else None, "spk_cos": cos,
             "rtf": round(r["gpu_s"] / max(1e-6, r["audio_s"]), 3) if r["audio_s"] else None, "capped": r["capped"],
             "lead_s": round(float(np.mean(r["lead"])), 2), "trail_s": round(float(np.mean(r["trail"])), 2),
             "max_gap_s": round(float(np.max(r["gap"])), 2), "cps": round(float(np.mean(r["cps"])), 1),
             "hf_pct": round(100 * float(np.mean(r["hf"])), 2) if r["hf"] else None, "worst": r["worst"][:6],
             "audio_s": round(r["audio_s"], 1), "gpu_s": round(r["gpu_s"], 1)}
        out[label] = o
        print(f"| {label} | {o['clips']} | {o['wer_pct']} | {cos} | {o['rtf']} | {o['capped']} | {o['lead_s']}/"
              f"{o['trail_s']} | {o['max_gap_s']} | {o['cps']} | {o['hf_pct']} |")
    (HERE / "results").mkdir(exist_ok=True)
    (HERE / "results/check.json").write_text(json.dumps(out, indent=1))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["jobs", "push", "pull", "collect", "check"])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--wait", action="store_true")
    ap.add_argument("--only", help="jobs: keep condition labels matching this regex")
    ap.add_argument("--skip-engines", default="", help="jobs: engines the kernel does not render (comma list)")
    a = ap.parse_args()
    {"jobs": lambda: cmd_jobs(a.work, a.only, a.skip_engines), "push": lambda: cmd_push(a.work), "pull": lambda: cmd_pull(a.work, a.wait),
     "collect": lambda: cmd_collect(a.work), "check": lambda: cmd_check(a.work)}[a.cmd]()


if __name__ == "__main__":
    main()
