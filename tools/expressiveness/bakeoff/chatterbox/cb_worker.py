# Chatterbox bake-off worker (tools/expressiveness/bakeoff/chatterbox). Runs inside the Kaggle kernel's own uv venv
# (cb_kernel.py), never locally. Inputs from the private dataset: jobs.json, ref_<voice>.wav, score/ (Kokoro and human
# WebM for the same ASR / MOS tools). Outputs in /kaggle/working:
#   raw/<label>/<passage>/NN.flac   24 kHz PCM 16, as the model returns it (Perth watermark included)
#   cb_log.json                     per item GPU seconds, audio seconds, per model load time and peak VRAM
#   asr.json, mos.json              Whisper large-v3-turbo transcripts and UTMOS22 strong per clip
import gc, glob, json, os, subprocess, sys, time, traceback

import numpy as np
import soundfile as sf
import torch

OUT = "/kaggle/working"
T0 = time.time()
spec = json.load(open(sys.argv[1]))
INP = os.path.dirname(sys.argv[1])
log = {"stages": {}, "models": {}, "items": [], "errors": []}
LOG = f"{OUT}/cb_log.json"


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    json.dump(log, open(LOG, "w"), indent=1)


def smi():
    return subprocess.run("nvidia-smi --query-gpu=index,name,memory.used,memory.total --format=csv,noheader",
                          shell=True, capture_output=True, text=True).stdout.strip()


log["gpu"] = smi()
log["torch"] = torch.__version__
log["cuda"] = torch.version.cuda
for mod in ("transformers", "diffusers", "numpy", "librosa", "chatterbox"):
    try:
        m = __import__(mod)
        log.setdefault("versions", {})[mod] = getattr(m, "__version__", "?")
    except Exception as e:  # noqa: BLE001
        log.setdefault("versions", {})[mod] = repr(e)[:200]
save()

from huggingface_hub import snapshot_download  # noqa: E402

SEED = spec["seed"]
DEV = "cuda"


def load_model(name):
    m = spec["models"][name]
    t = time.time()
    root = snapshot_download(m["repo"], revision=m["revision"], allow_patterns=m["files"])
    dl = time.time() - t
    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()
    t = time.time()
    if name == "orig":
        from chatterbox.tts import ChatterboxTTS
        model = ChatterboxTTS.from_local(root, DEV)
    elif name == "turbo":
        from chatterbox.tts_turbo import ChatterboxTurboTTS
        model = ChatterboxTurboTTS.from_local(root, DEV)
    elif name == "mtl":
        from chatterbox.mtl_tts import ChatterboxMultilingualTTS
        model = ChatterboxMultilingualTTS.from_local(root, DEV, t3_model=m["t3_model"])
    else:
        raise ValueError(name)
    torch.cuda.synchronize()
    log["models"][name] = {"download_s": round(dl, 1), "load_s": round(time.time() - t, 1),
                           "weights_gb": round(torch.cuda.memory_allocated() / 2**30, 2)}
    return model


def gen(model, name, text, p):
    kw = {"temperature": p.get("temperature", 0.8)}
    if name in ("orig", "mtl"):
        kw.update(exaggeration=p["exaggeration"], cfg_weight=p["cfg_weight"])
    if name == "mtl":
        kw["language_id"] = "en"
    torch.manual_seed(SEED)
    torch.cuda.manual_seed_all(SEED)
    np.random.seed(SEED)
    torch.cuda.synchronize()
    s = time.time()
    with torch.inference_mode():
        wav = model.generate(text, **kw)
    torch.cuda.synchronize()
    return wav.detach().float().cpu().reshape(-1).numpy(), time.time() - s


for name in spec["model_order"]:
    conds = [c for c in spec["conditions"] if c["model"] == name]
    if not conds:
        continue
    t_stage = time.time()
    try:
        model = load_model(name)
    except Exception as e:  # noqa: BLE001
        log["errors"].append(f"load {name}: " + traceback.format_exc()[-3000:])
        log["models"][name] = {"error": f"{type(e).__name__}: {e}"[:500]}
        save()
        continue
    sr = model.sr
    log["models"][name]["sample_rate"] = sr
    # Warm-up on the first voice (CUDA kernels, first allocations): timed apart, not in the RTF.
    first_voice = conds[0]["voice"]
    t = time.time()
    model.prepare_conditionals(os.path.join(INP, spec["refs"][first_voice]["file"]),
                               **({"exaggeration": 0.5} if name != "turbo" else {}))
    log["models"][name]["prepare_s"] = round(time.time() - t, 2)
    w, dt = gen(model, name, "The warm-up sentence is not part of the study.",
                {"exaggeration": 0.5, "cfg_weight": 0.5})
    log["models"][name]["warmup"] = {"gpu_s": round(dt, 2), "audio_s": round(w.size / sr, 2)}
    save()
    current_voice = first_voice
    for c in conds:
        if c["voice"] != current_voice:
            model.prepare_conditionals(os.path.join(INP, spec["refs"][c["voice"]]["file"]),
                                       **({"exaggeration": 0.5} if name != "turbo" else {}))
            current_voice = c["voice"]
        d = f"{OUT}/raw/{c['label']}/{c['passage']}"
        os.makedirs(d, exist_ok=True)
        for it in c["items"]:
            rec = {"label": c["label"], "model": name, "passage": c["passage"], "file": it["file"],
                   "chars": len(it["spoken"])}
            try:
                w, dt = gen(model, name, it["spoken"], it["params"])
                sf.write(f"{d}/{it['file']}", w, sr, subtype="PCM_16")
                rec.update(gpu_s=round(dt, 3), audio_s=round(w.size / sr, 3), peak=round(float(np.abs(w).max()), 4))
            except Exception as e:  # noqa: BLE001 - one failing item must not lose the run
                rec["error"] = f"{type(e).__name__}: {e}"[:500]
                log["errors"].append(traceback.format_exc()[-2000:])
            log["items"].append(rec)
            print(json.dumps(rec), flush=True)
        save()
    ok = [i for i in log["items"] if i["model"] == name and "gpu_s" in i]
    g, a = sum(i["gpu_s"] for i in ok), sum(i["audio_s"] for i in ok)
    log["models"][name].update(items=len(ok), gpu_s=round(g, 1), audio_s=round(a, 1),
                               rtf=round(g / a, 3) if a else None,
                               peak_vram_gb=round(torch.cuda.max_memory_allocated() / 2**30, 2),
                               peak_reserved_gb=round(torch.cuda.max_memory_reserved() / 2**30, 2),
                               smi_during=smi())
    log["stages"][name] = round(time.time() - t_stage, 1)
    save()
    del model
    gc.collect()
    torch.cuda.empty_cache()

# ------------------------------------------------------------------------------------------------ ASR and MOS
import imageio_ffmpeg  # noqa: E402

FF = imageio_ffmpeg.get_ffmpeg_exe()


def decode16(path):
    cmd = [FF, "-hide_banner", "-loglevel", "error", "-i", path, "-ac", "1", "-ar", "16000", "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32).copy()


clips = sorted(glob.glob(f"{OUT}/raw/**/*.flac", recursive=True))
clips += sorted(glob.glob(f"{INP}/score__*.webm"))
clips += [os.path.join(INP, r["file"]) for r in spec["refs"].values()]


def key(p):
    if p.startswith(f"{OUT}/raw/"):
        return p[len(f"{OUT}/raw/"):]
    b = os.path.basename(p)
    return b[len("score__"):].replace("__", "/") if b.startswith("score__") else b


t = time.time()
asr = {}
try:
    from transformers import WhisperForConditionalGeneration, WhisperProcessor
    a = spec["asr"]
    proc = WhisperProcessor.from_pretrained(a["repo"], revision=a["revision"])
    wm = WhisperForConditionalGeneration.from_pretrained(a["repo"], revision=a["revision"],
                                                         dtype=torch.float16).to(DEV).eval()
    for p in clips:
        try:
            x = decode16(p)
            if x.size > 16000 * 29:  # long form (human excerpts): 28 s windows
                parts = [x[i:i + 16000 * 28] for i in range(0, x.size, 16000 * 28)]
            else:
                parts = [x]
            txt = []
            for part in parts:
                f = proc(part, sampling_rate=16000, return_tensors="pt").input_features.to(DEV, torch.float16)
                with torch.inference_mode():
                    ids = wm.generate(f, language="en", task="transcribe")
                txt.append(proc.batch_decode(ids, skip_special_tokens=True)[0].strip())
            asr[key(p)] = " ".join(txt)
        except Exception as e:  # noqa: BLE001
            asr[key(p)] = f"ERROR {type(e).__name__}: {e}"[:300]
    del wm
    gc.collect()
    torch.cuda.empty_cache()
except Exception:  # noqa: BLE001
    log["errors"].append("asr: " + traceback.format_exc()[-3000:])
json.dump(asr, open(f"{OUT}/asr.json", "w"), indent=1, ensure_ascii=False)
log["stages"]["asr"] = round(time.time() - t, 1)
save()

t = time.time()
mos = {}
try:
    pred = torch.hub.load(spec["mos"]["hub"], spec["mos"]["model"], trust_repo=True).to(DEV).eval()
    for p in clips:
        try:
            x = torch.from_numpy(decode16(p))[None].to(DEV)
            with torch.inference_mode():
                mos[key(p)] = round(float(pred(x, 16000).reshape(-1)[0]), 3)
        except Exception as e:  # noqa: BLE001
            mos[key(p)] = f"ERROR {type(e).__name__}: {e}"[:300]
except Exception:  # noqa: BLE001
    log["errors"].append("mos: " + traceback.format_exc()[-3000:])
json.dump(mos, open(f"{OUT}/mos.json", "w"), indent=1)
log["stages"]["mos"] = round(time.time() - t, 1)
save()
print(json.dumps({k: v for k, v in log["models"].items()}, indent=1), flush=True)
