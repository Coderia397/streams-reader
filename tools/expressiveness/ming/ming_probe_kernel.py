# Probe for the silent UK_George tagged items of the Ming study (notes/32). In the first run all 7 tagged sentences
# rendered with the TTS task on the UK_George IP anchor came back as 0.3 s of silence (peak 0), while the same task
# on the US_Emma anchor and on the Kokoro reference worked. This kernel:
# 1. re-renders the UK_George anchor (IP, seed 1234) and, as a variant, with seed 99;
# 2. runs one-item diagnostics on "[Breathing] Then another.";
# 3. renders the 7 tagged items with seed 1234 and falls back to seeds 1, 42, 7 until the clip has sound
#    (peak > 0.02), as a production retry would; the seed used is logged.
# Inputs: the study dataset (ming3.tgz, jobs.json). PROBE below is filled in by ming_kaggle.py probe.
import glob, json, os, subprocess, sys, tarfile, time, traceback

PROBE = __PROBE__
T0 = time.time()
OUT = "/kaggle/working"
log = {"stages": {}, "diag": [], "items": [], "errors": []}


def save_log():
    log["elapsed_s"] = round(time.time() - T0, 1)
    with open(f"{OUT}/probe_log.json", "w") as f:
        json.dump(log, f, indent=1)


def sh(cmd):
    print("+", cmd, flush=True)
    return subprocess.run(cmd, shell=True).returncode


jobs_path = glob.glob("/kaggle/input/**/jobs.json", recursive=True)
INP = os.path.dirname(jobs_path[0])
spec = json.load(open(jobs_path[0]))
t = time.time()
PINS = ["transformers==4.46.3", "tokenizers==0.20.3", "huggingface_hub==0.36.2", "safetensors==0.8.0",
        "accelerate==1.14.0", "diffusers==0.33.0", "x-transformers==2.9.2", "einx==0.4.3", "einops==0.8.2",
        "loguru==0.7.3", "inflect==7.5.0", "onnxruntime==1.23.2", "soundfile==0.14.0"]
log["pip_rc"] = sh("pip install -q " + " ".join(PINS))
log["pynini_rc"] = sh("pip install -q pynini==2.1.6.post1")
with tarfile.open(os.path.join(INP, "ming3.tgz")) as tf:
    tf.extractall("/tmp")
sys.path.insert(0, "/tmp/ming3")
import numpy as np
import soundfile as sf
import torch
from huggingface_hub import snapshot_download

root = snapshot_download(spec["repo"], revision=spec["commit"], allow_patterns=spec["files"])
from ming_tts import MingTTS  # noqa: E402

tts = MingTTS(root, device="cuda", text_normalization=True, cache_prompts=True)
try:
    tts.normalizer  # noqa: B018
except Exception:  # noqa: BLE001
    tts.text_normalization = False
SR = tts.sample_rate
log["stages"]["setup"] = round(time.time() - t, 1)


def synth(task, text, controls, prompt, prompt_text, seed=1234, raw_text=False):
    kwargs = {"task": task, "text": text, "raw_text": raw_text}
    if task == "IP":
        kwargs.update(controls=controls, use_zero_spk_emb=True)
    elif task == "TTS":
        kwargs.update(language_id="en", prompt_wav_path=prompt, prompt_text=prompt_text, use_spk_emb=True)
    torch.manual_seed(seed)
    torch.cuda.synchronize()
    s = time.time()
    w = tts.synthesize(cfg=2.0, temperature=0.0, **kwargs)
    torch.cuda.synchronize()
    w = w.detach().float().cpu().reshape(-1).numpy().astype(np.float32)
    return w, time.time() - s


def rec(w, dt):
    return {"gpu_s": round(dt, 3), "audio_s": round(w.size / SR, 3), "peak": round(float(np.abs(w).max()), 4)}


synth("IP", "The warm-up sentence is not part of the study.", {"ip": "US_Emma"}, "", "")
os.makedirs(f"{OUT}/anchors", exist_ok=True)
anchors = {}
for name, voice, seed in (("george", "UK_George", 1234), ("george_s99", "UK_George", 99), ("emma", "US_Emma", 1234)):
    w, dt = synth("IP", spec["anchor_text"], {"ip": voice}, "", "", seed=seed)
    anchors[name] = f"/tmp/anchor-{name}.wav"
    sf.write(anchors[name], w, SR)
    sf.write(f"{OUT}/anchors/{name}.flac", w, SR, subtype="PCM_16")
    log["diag"].append({"case": f"anchor {name}", **rec(w, dt)})
save_log()

AT = spec["anchor_text"]
TAG = "[Breathing] Then another."
os.makedirs(f"{OUT}/diag", exist_ok=True)
cases = [
    ("george tagged s1234", TAG, "george", 1234, False),
    ("george untagged s1234", "Then another.", "george", 1234, False),
    ("george long untagged s1234", "I'm not sure I can explain it, not properly.", "george", 1234, False),
    ("george tagged raw_text s1234", TAG, "george", 1234, True),
    ("george tagged s1", TAG, "george", 1, False),
    ("george tagged s42", TAG, "george", 42, False),
    ("george_s99 anchor tagged s1234", TAG, "george_s99", 1234, False),
    ("emma tagged s1234", TAG, "emma", 1234, False),
]
for k, (case, text, anchor, seed, raw) in enumerate(cases):
    try:
        w, dt = synth("TTS", text, {}, anchors[anchor], AT, seed=seed, raw_text=raw)
        sf.write(f"{OUT}/diag/{k:02d}.flac", w, SR, subtype="PCM_16")
        r = {"case": case, "file": f"{k:02d}.flac", **rec(w, dt)}
    except Exception as e:  # noqa: BLE001
        r = {"case": case, "error": f"{type(e).__name__}: {e}"[:400]}
        log["errors"].append(traceback.format_exc()[-1500:])
    log["diag"].append(r)
    print(json.dumps(r), flush=True)
    save_log()

t = time.time()
for it in PROBE["items"]:
    d = f"{OUT}/raw/{it['label']}/{it['passage']}"
    os.makedirs(d, exist_ok=True)
    tries = []
    for seed in (1234, 1, 42, 7):
        try:
            w, dt = synth("TTS", it["text"], {}, anchors["george"], AT, seed=seed)
        except Exception as e:  # noqa: BLE001
            tries.append({"seed": seed, "error": f"{type(e).__name__}: {e}"[:300]})
            continue
        r = {"seed": seed, **rec(w, dt)}
        tries.append(r)
        if r["peak"] > 0.02:
            sf.write(f"{d}/{it['file']}", w, SR, subtype="PCM_16")
            break
    log["items"].append({"label": it["label"], "passage": it["passage"], "file": it["file"], "task": "TTS",
                         "chars": len(it["text"]), "tries": tries})
    print(json.dumps(log["items"][-1]), flush=True)
    save_log()
log["stages"]["items"] = round(time.time() - t, 1)
save_log()
