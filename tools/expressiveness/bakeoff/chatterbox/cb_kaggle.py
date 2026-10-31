#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""Chatterbox bake-off (original 0.5B, Turbo 350M, Multilingual V3 0.5B) on a private Kaggle T4 kernel.
Never touches ZeroGPU or the live API.

    uv run tools/expressiveness/bakeoff/chatterbox/select_ref.py --out W/dataset    # clone references
    uv run tools/expressiveness/bakeoff/chatterbox/cb_kaggle.py jobs    --work W --score A   # jobs.json (+ score/)
    uv run tools/expressiveness/bakeoff/chatterbox/cb_kaggle.py push    --work W
    uv run tools/expressiveness/bakeoff/chatterbox/cb_kaggle.py pull    --work W [--wait]
    uv run tools/expressiveness/bakeoff/chatterbox/cb_kaggle.py collect --work W --audio A2  # A2/<label>/<passage>/NN.webm

A is the baseline audio root (Kokoro voices and LibriVox human excerpts), copied into the dataset so the kernel scores
them with the same ASR and MOS models. Conditions, per voice (female = Austen LibriVox reader, male = Walden reader),
labels cb_<model><variant>_<voice>:
- orig_e50 / orig_e70 / orig_e90   original, one item per segment, exaggeration/cfg 0.5/0.5, 0.7/0.4, 0.9/0.3
- orig_dir                         original, per sentence, exaggeration and cfg from the LLM director plan
- turbo                            Turbo, per segment (Turbo ignores exaggeration and cfg)
- turbo_tags                       Turbo, per sentence, director emotion as a Turbo style tag ([fear], [angry], ...)
                                   plus the hand paralinguistic tags of passages/directions.json mapped to Turbo tags
- mtl3 / mtl3_dir                  Multilingual V3 (language en), per segment 0.5/0.5, and per sentence as orig_dir
Per-sentence conditions are collected twice: with the app's pipeline pauses (sentence 250, paragraph 600 ms) and,
as <label>_dp, with the director's pause_after_ms.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import imageio_ffmpeg
import soundfile as sf

HERE = Path(__file__).resolve().parent
EXPR = HERE.parents[1]
REPO = EXPR.parent.parent
SPACE = REPO / "space"
sys.path.insert(0, str(SPACE))

OWNER = "lbrahimali"
DATASET = f"{OWNER}/streams-chatterbox-bakeoff-inputs"
KERNEL = f"{OWNER}/streams-chatterbox-bakeoff"
SEED = 1234
BITRATE = 48
CB_COMMIT = "5de7a54aa4e5e2baadb0182dde554908b48b85c2"   # resemble-ai/chatterbox master, 2026-07-21
MODELS = {
    "orig": {"repo": "ResembleAI/chatterbox", "revision": "5bb1f6ee58e50c3b8d408bc82a6d3740c2db6e18",
             "files": ["ve.safetensors", "t3_cfg.safetensors", "s3gen.safetensors", "tokenizer.json", "conds.pt"]},
    "turbo": {"repo": "ResembleAI/chatterbox-turbo", "revision": "749d1c1a46eb10492095d68fbcf55691ccf137cd",
              "files": ["*.safetensors", "*.json", "*.txt", "*.pt", "*.model"]},
    "mtl": {"repo": "ResembleAI/chatterbox", "revision": "5bb1f6ee58e50c3b8d408bc82a6d3740c2db6e18",
            "t3_model": "v3",
            "files": ["ve.pt", "t3_mtl23ls_v3.safetensors", "s3gen.pt", "grapheme_mtl_merged_expanded_v1.json",
                      "conds.pt", "Cangjie5_TC.json"]},
}
PIP = [f"chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox@{CB_COMMIT}", "imageio-ffmpeg==0.6.0",
       "soundfile==0.13.1"]
ASR = {"repo": "openai/whisper-large-v3-turbo", "revision": "41f01f3fe87f28c78e2fbf8b568835947dd65ed9"}
MOS = {"hub": "tarepan/SpeechMOS:v1.2.0", "model": "utmos22_strong"}
SCORE_LABELS = ["af_heart", "am_michael", "bf_emma", "bm_george", "human"]
VOICES = ["female", "male"]
# Ming tags in directions.json -> Turbo tokens (added_tokens.json of ResembleAI/chatterbox-turbo).
TAG_MAP = {"[Laughter]": "[chuckle]", "[Sigh]": "[sigh]", "[Breathing]": "[gasp]", "[Uhm]": "uh,"}
STYLE = {"fearful": "[fear]", "angry": "[angry]", "disgusted": "[angry]", "happy": "[happy]",
         "surprised": "[surprised]"}
PAUSE_RANGE_MS = (120, 1800)


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


# ----------------------------------------------------------------------------------------------------------- jobs
def directed_params(s: dict) -> dict:
    """Director sentence -> Chatterbox exaggeration / cfg. Neutral stays at the defaults; an emotion raises
    exaggeration with its intensity; slow pace or high exaggeration lowers cfg (upstream tip: lower cfg slows the
    pacing that high exaggeration speeds up)."""
    e, i, pace = s["emotion"], float(s["intensity"]), s["pace"]
    ex = 0.5 if e == "neutral" else min(1.0, 0.5 + 0.6 * i)
    cfg = 0.35 if pace == "slow" else 0.5
    if ex >= 0.85:
        cfg = 0.3
    elif ex >= 0.7:
        cfg = min(cfg, 0.4)
    return {"exaggeration": round(ex, 2), "cfg_weight": cfg, "temperature": 0.8}


def turbo_text(text: str, s: dict, hand) -> str:
    if hand is not None and not isinstance(hand, str):
        text = hand[1]
        for k, v in TAG_MAP.items():
            text = text.replace(k, v)
    style = STYLE.get(s["emotion"]) if float(s["intensity"]) >= 0.4 else None
    return f"{style} {text}" if style else text


def build(segments: dict, plans: dict, directions: dict) -> list[dict]:
    conds = []

    def seg_items(segs, params):
        return [{"file": f"{k:02d}.flac", "text": s["text"], "spoken": s["text"], "boundary": s["boundary"],
                 "params": params, "pause_ms": None} for k, s in enumerate(segs)]

    def sent_items(passage, segs, how):
        items, flat = [], plans[passage]
        n = 0
        for si, s in enumerate(segs):
            sents = [s["text"][a:b] for a, b in s["sentences"]]
            hands = directions[passage][si]
            assert len(hands) == len(sents)
            for k, text in enumerate(sents):
                p = flat[n]
                assert p["segment"] == si, (passage, n, p["segment"], si)
                n += 1
                spoken, params = how(text, p, hands[k])
                lo, hi = PAUSE_RANGE_MS
                items.append({"file": f"{len(items):02d}.flac", "text": text, "spoken": spoken,
                              "boundary": "sentence" if k + 1 < len(sents) else s["boundary"], "params": params,
                              "emotion": p["emotion"], "intensity": p["intensity"], "pace": p["pace"],
                              "pause_ms": int(min(hi, max(lo, p["pause_after_ms"])))})
        assert n == len(flat), (passage, n, len(flat))
        return items

    def directed(text, p, hand):
        return text, directed_params(p)

    def tagged(text, p, hand):
        return turbo_text(text, p, hand), {"temperature": 0.8}

    for v in VOICES:
        for passage, segs in segments.items():
            def add(label, model, items, mode):
                conds.append({"label": f"cb_{label}_{v}", "model": model, "voice": v, "passage": passage,
                              "mode": mode, "items": items})
            add("orig_e50", "orig", seg_items(segs, {"exaggeration": 0.5, "cfg_weight": 0.5}), "segment")
            add("orig_e70", "orig", seg_items(segs, {"exaggeration": 0.7, "cfg_weight": 0.4}), "segment")
            add("orig_e90", "orig", seg_items(segs, {"exaggeration": 0.9, "cfg_weight": 0.3}), "segment")
            add("orig_dir", "orig", sent_items(passage, segs, directed), "sentence")
            add("turbo", "turbo", seg_items(segs, {"temperature": 0.8}), "segment")
            add("turbo_tags", "turbo", sent_items(passage, segs, tagged), "sentence")
            add("mtl3", "mtl", seg_items(segs, {"exaggeration": 0.5, "cfg_weight": 0.5}), "segment")
            add("mtl3_dir", "mtl", sent_items(passage, segs, directed), "sentence")
    order = {"orig": 0, "turbo": 1, "mtl": 2}
    conds.sort(key=lambda c: (order[c["model"]], c["voice"]))
    return conds


def cmd_jobs(work: Path, score: Path | None) -> None:
    segments = json.loads((EXPR / "passages/segments.json").read_text())
    directions = json.loads((EXPR / "passages/directions.json").read_text())
    plans = {p: json.loads((EXPR / f"director/out/{p}.json").read_text())["plan"] for p in segments}
    conds = build(segments, plans, directions)
    ds = work / "dataset"
    refs = json.loads((ds / "refs.json").read_text())
    spec = {"seed": SEED, "models": MODELS, "model_order": ["orig", "turbo", "mtl"], "pip": PIP, "asr": ASR,
            "mos": MOS, "refs": refs, "conditions": conds}
    (ds / "jobs.json").write_text(json.dumps(spec, ensure_ascii=False, indent=1))
    (work / "conditions.json").write_text(json.dumps(conds, ensure_ascii=False, indent=1))
    shutil.copy(HERE / "cb_worker.py", ds / "cb_worker.py")
    if score:
        for lab in SCORE_LABELS:
            for f in (score / lab).rglob("*.webm"):
                # Kaggle datasets keep no sub-folders: score__<label>__<passage>__NN.webm
                rel = f.relative_to(score).parts
                shutil.copy(f, ds / ("score__" + "__".join(rel)))
    n = sum(len(c["items"]) for c in conds)
    chars = sum(len(i["spoken"]) for c in conds for i in c["items"])
    print(f"{len(conds)} condition sets, {n} items, {chars} chars")


# ----------------------------------------------------------------------------------------------------------- push
def cmd_push(work: Path, kernel_only: bool = False) -> None:
    if not kernel_only:
        push_dataset(work)
    push_kernel(work)


def push_dataset(work: Path) -> None:
    ds = work / "dataset"
    (ds / "dataset-metadata.json").write_text(json.dumps(
        {"title": "streams-chatterbox-bakeoff-inputs", "id": DATASET, "licenses": [{"name": "other"}]}))
    st = kaggle("datasets", "status", DATASET, capture=True)
    if st.returncode == 0 and "ready" in st.stdout.lower():
        r = kaggle("datasets", "version", "-p", str(ds), "-r", "skip", "-m", f"inputs {time.strftime('%Y-%m-%d %H:%M')}")
    else:
        r = kaggle("datasets", "create", "-p", str(ds), "-r", "skip")
    if r.returncode != 0:
        raise SystemExit("dataset upload failed")
    for _ in range(60):
        s = kaggle("datasets", "status", DATASET, capture=True).stdout.strip().lower()
        print("dataset:", s, flush=True)
        if "ready" in s:
            break
        time.sleep(10)


def push_kernel(work: Path) -> None:
    """Kaggle allows 2 batch GPU sessions per account; when both are taken, retry every 10 min (at most 6 h)."""
    kd = work / "kernel"
    kd.mkdir(exist_ok=True)
    shutil.copy(HERE / "cb_kernel.py", kd / "cb_kernel.py")
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-chatterbox-bakeoff", "code_file": "cb_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [DATASET], "competition_sources": [],
        "kernel_sources": []}))
    for _ in range(36):
        r = kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4", capture=True)
        msg = (r.stdout + r.stderr).strip()
        print(time.strftime("%H:%M:%S"), msg, flush=True)
        if "session count" not in msg.lower():
            if r.returncode != 0 or "error" in msg.lower():
                raise SystemExit("kernel push failed")
            return
        time.sleep(600)
    raise SystemExit("no free GPU session after 6 h")


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


# -------------------------------------------------------------------------------------------------------- collect
def cmd_collect(work: Path, audio: Path) -> None:
    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    from dataclasses import replace

    from nolabs_engine.audio import codec, pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    profile = AudioProfile(id=3, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    conds = json.loads((work / "conditions.json").read_text())
    log = json.loads((work / "out/cb_log.json").read_text())
    timing = {(i["label"], i["passage"], i["file"]): i for i in log["items"]}
    for c in conds:
        raw = work / "out/raw" / c["label"] / c["passage"]
        clips = []
        for it in c["items"]:
            f = raw / it["file"]
            if not f.exists():
                clips.append(RuntimeError(f"missing {f}"))
                continue
            w, sr = sf.read(f, dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        stream = pipeline.Stream(profile)
        levelled = [stream.level(x) for x in clips]
        variants = [(c["label"], None)]
        if c["mode"] == "sentence":
            variants.append((c["label"] + "_dp", "director"))
        for label, pauses in variants:
            d = audio / label / c["passage"]
            d.mkdir(parents=True, exist_ok=True)
            meta = []
            for k, (it, lv) in enumerate(zip(c["items"], levelled, strict=True)):
                if isinstance(lv, BaseException):
                    print(f"SKIP {label}/{c['passage']}/{it['file']}: {lv}")
                    continue
                prof = profile
                if pauses:
                    ms = it["pause_ms"]
                    prof = replace(profile, pauses=PauseProfile(sentence_ms=ms, paragraph_ms=ms, section_ms=ms))
                fin = pipeline._finish(lv.prepared, lv.boundary, lv.gain_db, prof, codec.encode_webm)
                name = f"{k:02d}.webm"
                (d / name).write_bytes(fin.data)
                t = timing.get((c["label"], c["passage"], it["file"]), {})
                meta.append({"file": name, "text": it["text"], "spoken": it["spoken"], "boundary": it["boundary"],
                             "params": it["params"], "emotion": it.get("emotion"), "pause_ms": it.get("pause_ms"),
                             "duration_ms": fin.duration_ms, "speech_ms": fin.speech_ms,
                             "gain_db": round(fin.gain_db, 2), "gpu_s": t.get("gpu_s"),
                             "raw_audio_s": t.get("audio_s"), "src": f"{c['label']}/{c['passage']}/{it['file']}"})
            (d / "meta.json").write_text(json.dumps(
                {"voice": f"chatterbox-{c['model']}:{c['voice']}", "label": label, "passage": c["passage"],
                 "engine": "chatterbox", "model": c["model"], "segments": meta}, ensure_ascii=False, indent=1))
            print(f"{label}/{c['passage']}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["jobs", "push", "pull", "collect"])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--audio", type=Path)
    ap.add_argument("--score", type=Path)
    ap.add_argument("--wait", action="store_true")
    ap.add_argument("--kernel-only", action="store_true")
    a = ap.parse_args()
    if a.cmd == "jobs":
        cmd_jobs(a.work, a.score)
    elif a.cmd == "push":
        cmd_push(a.work, a.kernel_only)
    elif a.cmd == "pull":
        cmd_pull(a.work, a.wait)
    else:
        cmd_collect(a.work, a.audio)


if __name__ == "__main__":
    main()
