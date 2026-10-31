#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""Fish Audio bake-off: Drama 3 (int4 build, fishaudio/drama-3-int4) and S2 Pro on one private Kaggle kernel (T4 x2).
Never touches ZeroGPU or the live API.

    uv run tools/expressiveness/bakeoff/fish/fish_bakeoff.py jobs    --work W   # W/dataset (refs + jobs.json)
    uv run tools/expressiveness/bakeoff/fish/fish_bakeoff.py push    --work W   # private dataset + private GPU kernel
    uv run tools/expressiveness/bakeoff/fish/fish_bakeoff.py pull    --work W [--wait]
    uv run tools/expressiveness/bakeoff/fish/fish_bakeoff.py collect --work W --audio A   # A/<label>/<passage>/NN.webm

Clone references: the two clips of the Chatterbox bake-off (bakeoff/chatterbox/select_ref.py: Austen LibriVox reader,
female, and Walden reader, male; 12.5 / 11.3 s, -20 LUFS), copied into W/dataset; the kernel transcribes them with
Whisper large-v3-turbo because both Fish models need the reference transcript. Fish models have no preset voices.
Labels fish_<model>_<voice>_<cond>, one item per segment (paragraph), as the app sends text:
- plain     the segment text as is
- directed  the same segment with inline Fish tags per sentence from the LLM director plan (director/out): a sentence
            tag "[<intensity> <emotion>, slowly|quickly]" when the emotion is not neutral or the pace not normal,
            "[emphasis]" before the director's emphasis words, and the hand nonverbal tags of passages/directions.json
            mapped to Fish tags ([Breathing] -> [inhale], [Laughter] -> [chuckle], [Sigh] -> [sigh],
            [Uhm] -> [hesitating]).
"""
from __future__ import annotations

import argparse
import base64
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
DATASET = f"{OWNER}/streams-fish-bakeoff-inputs"
KERNEL = f"{OWNER}/streams-fish-bakeoff"
REFS_FROM = Path("/private/tmp/claude-501/-Users-ibrahim-code-nolabs-reader/a97fc270-2dc8-4cdf-895f-e925bee528d9/"
                 "scratchpad/cb/dataset")
BITRATE = 48
SEED = 1234
FISH_SPEECH = {"repo": "https://github.com/fishaudio/fish-speech.git",
               "commit": "0f8a12e8a651062fdee9c33a894bdccd3a8513f1"}  # main, 2026-10-29 "drama-3 small updates"
COMMON_FILES = ["model.safetensors.index.json", "config.json", "tokenizer.json", "tokenizer_config.json",
                "special_tokens_map.json", "chat_template.jinja", "codec.pth"]
SAMPLING = {"temperature": 0.7, "top_p": 0.7, "top_k": 30}  # fish-speech defaults for both models
MODELS = {
    "drama3": {"repo": "fishaudio/drama-3-int4", "revision": "a6f588691a13b6607cb09f9800556487ab7aa0fe",
               "files": ["model-*.safetensors", "LICENSE", *COMMON_FILES],
               # max_new_tokens 0 = fish-speech's 600 frames per ~300-byte chunk
               "gen": {**SAMPLING, "max_new_tokens": 0}},
    "s2pro": {"repo": "fishaudio/s2-pro", "revision": "1de9996b6be38b745688de084d87a5633f714e4e",
              "files": ["model-*.safetensors", "LICENSE.md", *COMMON_FILES],
              "cache_len": 6144,
              "gen": {**SAMPLING, "max_new_tokens": 1024, "chunk_length": 400}},
}
ASR = {"repo": "openai/whisper-large-v3-turbo", "revision": "41f01f3fe87f28c78e2fbf8b568835947dd65ed9"}
MOS = {"hub": "tarepan/SpeechMOS:v1.2.0", "model": "utmos22_strong"}   # as the Chatterbox bake-off
PINS = ["transformers==5.19.0", "tokenizers==0.23.2", "hydra-core==1.3.2", "omegaconf==2.3.0", "einx[torch]==0.2.2",
        "einops==0.8.1", "loguru==0.7.3", "natsort==8.4.0", "pyrootutils==1.0.4", "soundfile==0.13.1",
        "imageio-ffmpeg==0.6.0"]
PINS_NODEPS = ["descript-audio-codec==1.0.0", "descript-audiotools==0.7.2"]
PINS_LIGHT = ["argbind==0.3.9", "julius==0.2.7", "ffmpy==0.3.0", "flatten-dict==0.4.2", "pyloudnorm==0.1.1",
              "markdown2==2.4.10", "randomname==0.2.1", "pystoi==0.4.1", "torch-stoi==0.2.1"]
MODULE_TO_PIP = {"dac": "descript-audio-codec==1.0.0", "audiotools": "descript-audiotools==0.7.2",
                 "hydra": "hydra-core==1.3.2", "flatten_dict": "flatten-dict", "importlib_resources":
                 "importlib-resources", "torch_stoi": "torch-stoi", "silero_vad": "silero-vad",
                 "opencc": "opencc-python-reimplemented", "yaml": "pyyaml", "sklearn": "scikit-learn",
                 "google": "protobuf"}
NONVERBAL = {"[Breathing]": "[inhale]", "[Laughter]": "[chuckle]", "[Sigh]": "[sigh]", "[Uhm]": "[hesitating]"}
EMOTION_WORD = {"happy": "cheerful", "sad": "sad", "angry": "angry", "fearful": "fearful", "disgusted": "disgusted",
                "surprised": "surprised"}


def kaggle(*args: str, capture: bool = False) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    for line in (Path.home() / ".config/streams/kaggle.env").read_text().splitlines():
        if line.startswith("KAGGLE_API_TOKEN="):
            env["KAGGLE_API_TOKEN"] = line.split("=", 1)[1].strip().strip('"').strip("'")
    return subprocess.run(["uvx", "--from", "kaggle==2.2.5", "kaggle", *args], env=env, text=True,
                          capture_output=capture, check=False)


# ----------------------------------------------------------------------------------------------------------- jobs
def sentence_tag(p: dict) -> str:
    parts = []
    emo = p["emotion"]
    if emo != "neutral":
        x = p["intensity"]
        adv = "slightly " if x < 0.35 else "very " if x >= 0.65 else ""
        parts.append(adv + EMOTION_WORD[emo])
    if p["pace"] != "normal":
        parts.append("slowly" if p["pace"] == "slow" else "quickly")
    return f"[{', '.join(parts)}]" if parts else ""


def directed_sentence(text: str, p: dict, hand) -> str:
    words = text.split(" ")
    assert words == p["words"], (words, p["words"])
    nonverbal_at = None
    if not isinstance(hand, str):          # [emotion, tagged text] from passages/directions.json
        tw = hand[1].split(" ")
        k = next(i for i, w in enumerate(tw) if w in NONVERBAL)
        rest = tw[:k] + tw[k + 1:]
        assert rest == words, (rest, words)
        nonverbal_at = (k, NONVERBAL[tw[k]])
    out = []
    tag = sentence_tag(p)
    if tag:
        out.append(tag)
    for i, w in enumerate(words):
        if nonverbal_at and nonverbal_at[0] == i:
            out.append(nonverbal_at[1])
        if i in p["emphasis"]:
            out.append("[emphasis]")
        out.append(w)
    return " ".join(out)


def build_items() -> list[dict]:
    segments = json.loads((EXPR / "passages/segments.json").read_text())
    directions = json.loads((EXPR / "passages/directions.json").read_text())
    items = []
    for passage, segs in segments.items():
        plan = json.loads((EXPR / f"director/out/{passage}.json").read_text())["plan"]
        k = 0
        for si, s in enumerate(segs):
            sents = [s["text"][a:b] for a, b in s["sentences"]]
            directed = []
            for j, text in enumerate(sents):
                p = plan[k]
                assert p["text"] == text and p["segment"] == si
                directed.append(directed_sentence(text, p, directions[passage][si][j]))
                k += 1
            for voice in ("female", "male"):
                for cond, spoken in (("plain", s["text"]), ("directed", " ".join(directed))):
                    items.append({"voice": voice, "cond": cond, "passage": passage, "file": f"{si:02d}.flac",
                                  "segment": si, "text": s["text"], "spoken": spoken, "boundary": s["boundary"],
                                  "seed": SEED + si})
    return items


def cmd_jobs(work: Path) -> None:
    ds = work / "dataset"
    ds.mkdir(parents=True, exist_ok=True)
    refs = json.loads((REFS_FROM / "refs.json").read_text())
    for voice, r in refs.items():
        shutil.copy(REFS_FROM / r["file"], ds / r["file"])
    items = build_items()
    jobs = {"fish_speech": FISH_SPEECH, "models": MODELS, "asr": ASR, "mos": MOS, "pins": PINS, "pins_nodeps": PINS_NODEPS,
            "pins_light": PINS_LIGHT, "module_to_pip": MODULE_TO_PIP, "refs": refs, "items": items,
            "stages": ["refs", "drama3", "s2pro", "asr"],
            "stage_timeout_s": {"refs": 1800, "drama3": 16200, "s2pro": 14400, "asr": 3600},
            "budget_s": {"drama3": 15000, "s2pro": 13500}}
    (work / "jobs.json").write_text(json.dumps(jobs, ensure_ascii=False, indent=1))
    for it in items:
        if it["voice"] == "female" and it["cond"] == "directed":
            print(f"{it['passage']}/{it['file']}: {it['spoken']}")
    print(f"{len(items)} items, {sum(len(i['spoken'].encode()) for i in items)} bytes")


# ----------------------------------------------------------------------------------------------------------- push
def kernel_running() -> str:
    st = kaggle("kernels", "status", KERNEL, capture=True)
    return (st.stdout + st.stderr).strip()


def cmd_push(work: Path) -> None:
    status = kernel_running()
    if "RUNNING" in status or "QUEUED" in status:
        raise SystemExit(f"kernel busy: {status}")
    ds = work / "dataset"
    (ds / "dataset-metadata.json").write_text(json.dumps(
        {"title": "streams-fish-bakeoff-inputs", "id": DATASET, "licenses": [{"name": "other"}]}))
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
    jobs = json.loads((work / "jobs.json").read_text())
    src = (HERE / "fish_kernel.py").read_text()
    src = src.replace("JOBS = None  # @@JOBS@@", "JOBS = json.loads(" + repr(json.dumps(jobs, ensure_ascii=True)) + ")")
    stage = base64.b64encode((HERE / "fish_stage.py").read_bytes()).decode()
    src = src.replace("STAGE_SRC = None  # @@STAGE_SRC@@", f"STAGE_SRC = {stage!r}")
    (kd / "fish_kernel.py").write_text(src)
    (kd / "kernel-metadata.json").write_text(json.dumps({
        "id": KERNEL, "title": "streams-fish-bakeoff", "code_file": "fish_kernel.py", "language": "python",
        "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
        "machine_shape": "NvidiaTeslaT4", "dataset_sources": [DATASET], "competition_sources": [],
        "kernel_sources": []}))
    if kaggle("kernels", "push", "-p", str(kd), "--accelerator", "NvidiaTeslaT4").returncode != 0:
        raise SystemExit("kernel push failed")


def cmd_pull(work: Path, wait: bool) -> None:
    while True:
        line = kernel_running()
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
    from nolabs_engine.audio import pipeline
    from nolabs_engine.contracts import AudioProfile, PauseProfile

    profile = AudioProfile(id=4, bitrate_kbps=BITRATE, sample_rate=48_000, target_lufs=-16.0, max_correction_db=2.0,
                           peak_ceiling_dbfs=-1.5,
                           pauses=PauseProfile(sentence_ms=250, paragraph_ms=600, section_ms=1200))
    jobs = json.loads((work / "jobs.json").read_text())
    timing = {}
    for stage in ("drama3", "s2pro"):
        p = work / "out/logs" / f"{stage}.json"
        if p.exists():
            for i in json.loads(p.read_text())["items"]:
                timing[(i["label"], i["passage"], i["file"])] = i
    groups: dict[tuple[str, str], list[dict]] = {}
    for model in MODELS:
        for it in jobs["items"]:
            label = f"fish_{model}_{it['voice']}_{it['cond']}"
            groups.setdefault((label, it["passage"]), []).append({**it, "model": model, "label": label})
    for (label, passage), its in groups.items():
        raw = work / "out/raw" / label / passage
        if not all((raw / it["file"]).exists() for it in its):
            print(f"{label}/{passage}: incomplete, skipped")
            continue
        clips = []
        for it in its:
            w, sr = sf.read(raw / it["file"], dtype="float32")
            clips.append(pipeline.RawClip(wave=w, sample_rate=sr, boundary=it["boundary"]))
        done = pipeline.finish_batch(clips, profile)
        d = audio / label / passage
        d.mkdir(parents=True, exist_ok=True)
        meta = []
        for k, (it, fin) in enumerate(zip(its, done, strict=True)):
            if isinstance(fin, BaseException):
                raise SystemExit(f"{label}/{passage}/{it['file']}: {fin}")
            name = f"{k:02d}.webm"
            (d / name).write_bytes(fin.data)
            t = timing.get((label, passage, it["file"]), {})
            meta.append({"file": name, "text": it["text"], "spoken": it["spoken"], "boundary": it["boundary"],
                         "duration_ms": fin.duration_ms, "gain_db": round(fin.gain_db, 2),
                         **{k2: t.get(k2) for k2 in ("gen_s", "decode_s", "audio_s", "rtf_total", "frames")}})
        (d / "meta.json").write_text(json.dumps({"voice": label, "label": label, "passage": passage,
                                                 "engine": its[0]["model"], "segments": meta},
                                                ensure_ascii=False, indent=1))
        print(f"{label}/{passage}: {len(meta)} clips, {sum(m['duration_ms'] for m in meta) / 1000:.1f} s")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["jobs", "push", "pull", "collect"])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--audio", type=Path)
    ap.add_argument("--wait", action="store_true")
    a = ap.parse_args()
    if a.cmd == "jobs":
        cmd_jobs(a.work)
    elif a.cmd == "push":
        cmd_push(a.work)
    elif a.cmd == "pull":
        cmd_pull(a.work, a.wait)
    else:
        cmd_collect(a.work, a.audio)


if __name__ == "__main__":
    main()
