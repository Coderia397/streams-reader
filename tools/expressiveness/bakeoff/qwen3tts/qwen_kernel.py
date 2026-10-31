# Qwen3-TTS 1.7B bakeoff on a private Kaggle GPU kernel (tools/expressiveness/bakeoff/qwen3tts).
# The driver (qwen_kaggle.py push) replaces the JOBS_B64 line with the job spec, so no dataset is needed.
# Stages: pip (qwen-tts pinned) -> LibriVox reference intros (archive.org) -> two workers in parallel, one per T4:
#   gpu0 "cv":      CustomVoice 1.7B, dtype probe, all cv conditions
#   gpu1 "base_vd": Whisper transcribes the references, Base 1.7B clones, then VoiceDesign 1.7B
# then "fast" (faster-qwen3-tts in a venv, gpu0) and "analysis" (Whisper ASR + Base speaker embedding per clip, gpu1).
# Outputs in /kaggle/working: raw/<label>/<passage>/NN.flac, refs/, qwen_log.json, analysis.json, probe/.
import base64, json, os, subprocess, sys, time, traceback, zlib

JOBS_B64 = ""  # filled by qwen_kaggle.py
OUT = "/kaggle/working"
os.environ.setdefault("HF_HOME", "/tmp/hf")
T0 = time.time()
LOG = f"{OUT}/qwen_log.json"
log = {"stages": {}, "errors": []}


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    json.dump(log, open(LOG, "w"), indent=1)


def sh(cmd, **kw):
    print("+", cmd, flush=True)
    return subprocess.run(cmd, shell=True, **kw).returncode


WORKER = r'''
import gc, json, os, random, sys, time, traceback
import numpy as np, soundfile as sf, torch
role, spec_path, out = sys.argv[1], sys.argv[2], sys.argv[3]
spec = json.load(open(spec_path))
OUT = out
wlog = {"role": role, "items": [], "errors": [], "models": {}}
LOGP = f"{OUT}/worker_{role}.json"
def save():
    json.dump(wlog, open(LOGP, "w"), indent=1)
def seed(n):
    random.seed(n); np.random.seed(n % 2**32); torch.manual_seed(n); torch.cuda.manual_seed_all(n)
def sync():
    torch.cuda.synchronize()
def max_tokens(text):
    return int(min(2048, 60 + 2.0 * len(text)))
DT = {"bf16": torch.bfloat16, "fp16": torch.float16, "fp32": torch.float32}

def load(repo_key, dtype):
    from qwen_tts import Qwen3TTSModel
    from huggingface_hub import snapshot_download
    r = spec["models"][repo_key]
    path = snapshot_download(r["repo"], revision=r["revision"])
    torch.cuda.empty_cache(); torch.cuda.reset_peak_memory_stats()
    t = time.time()
    m = Qwen3TTSModel.from_pretrained(path, device_map="cuda:0", dtype=DT[dtype], attn_implementation="sdpa")
    sync()
    wlog["models"].setdefault(repo_key, {}).update({"load_s": round(time.time() - t, 1), "dtype": dtype,
        "vram_after_load_gb": round(torch.cuda.memory_allocated() / 2**30, 2)})
    return m

def free(m):
    del m; gc.collect(); torch.cuda.empty_cache()

def gen(model, engine, text, ctrl):
    kw = dict(language="English", max_new_tokens=max_tokens(text))
    if engine == "cv":
        return model.generate_custom_voice(text=text, speaker=ctrl["speaker"], instruct=ctrl.get("instruct") or None, **kw)
    if engine == "vd":
        return model.generate_voice_design(text=text, instruct=ctrl["instruct"], **kw)
    if engine == "clone":
        return model.generate_voice_clone(text=text, voice_clone_prompt=ctrl["_prompt"], **kw)
    raise ValueError(engine)

PROBE = ["The lighthouse keeper finished his supper at seven, as he had done every evening for eleven years.",
         "Then another.",
         "\"Don't you dare make a joke of this! I waited up until two in the morning, Tom. Two!\""]

def probe(repo_key, engine, ctrl):
    """Render PROBE in each dtype; keep the fastest one whose clips are finite, not silent and not capped."""
    res = {}
    os.makedirs(f"{OUT}/probe", exist_ok=True)
    for dt in ["bf16", "fp16", "fp32"]:
        try:
            m = load(repo_key, dt)
            c = dict(ctrl)
            if engine == "clone":
                c["_prompt"] = m.create_voice_clone_prompt(ref_audio=ctrl["ref_audio"], ref_text=ctrl["ref_text"])
            r = {"gpu_s": 0.0, "audio_s": 0.0, "ok": True, "durs": []}
            for i, t in enumerate(PROBE):
                seed(1234 + i); sync(); t0 = time.time()
                wavs, sr = gen(m, engine, t, c)
                sync(); dt_s = time.time() - t0
                w = np.asarray(wavs[0], dtype=np.float32)
                a = w.size / sr
                fin = bool(np.isfinite(w).all()); rms = float(np.sqrt(np.mean(w ** 2))) if w.size else 0.0
                capped = a * 12.5 >= max_tokens(t) - 1
                r["durs"].append(round(a, 2)); r["gpu_s"] += dt_s; r["audio_s"] += a
                r["ok"] = r["ok"] and fin and rms > 1e-3 and not capped and a > 0.4
                if fin:
                    sf.write(f"{OUT}/probe/{repo_key}_{dt}_{i}.flac", np.clip(w, -1, 1), sr, subtype="PCM_16")
            r["rtf"] = round(r["gpu_s"] / max(r["audio_s"], 1e-6), 3)
            r["peak_vram_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
            res[dt] = r
            free(m)
        except Exception:
            res[dt] = {"ok": False, "error": traceback.format_exc()[-1500:]}
            gc.collect(); torch.cuda.empty_cache()
        wlog.setdefault("probe", {})[repo_key] = res; save()
    good = [d for d in res if res[d].get("ok")]
    if not good:
        raise RuntimeError(f"no dtype works for {repo_key}: {res}")
    # durations must agree with fp32 (a broken half-precision run tends to babble or stop early)
    ref = res.get("fp32", {}).get("durs") if res.get("fp32", {}).get("ok") else None
    if ref:
        good = [d for d in good if all(0.6 <= a / max(b, 0.1) <= 1.6 for a, b in zip(res[d]["durs"], ref))] or ["fp32"]
    best = min(good, key=lambda d: res[d]["rtf"])
    wlog.setdefault("chosen_dtype", {})[repo_key] = best; save()
    return best

def run_conditions(model, engine, conds, ctrl_extra=None):
    for c in conds:
        d = f"{OUT}/raw/{c['label']}/{c['passage']}"
        os.makedirs(d, exist_ok=True)
        for k, it in enumerate(c["items"]):
            ctrl = dict(it["controls"]); ctrl.update(ctrl_extra or {})
            try:
                seed(spec["seed"] + k); sync(); t0 = time.time()
                wavs, sr = gen(model, engine, it["text"], ctrl)
                sync(); g = time.time() - t0
                w = np.clip(np.asarray(wavs[0], dtype=np.float32), -1, 1)
                sf.write(f"{d}/{it['file']}", w, sr, subtype="PCM_16")
                a = w.size / sr
                wlog["items"].append({"label": c["label"], "passage": c["passage"], "file": it["file"], "gpu_s": round(g, 3),
                    "audio_s": round(a, 3), "sr": sr, "capped": a * 12.5 >= max_tokens(it["text"]) - 1,
                    "peak_vram_gb": round(torch.cuda.max_memory_allocated() / 2**30, 2)})
            except Exception:
                wlog["errors"].append({"label": c["label"], "passage": c["passage"], "file": it["file"],
                                       "error": traceback.format_exc()[-1500:]})
            save()
        print(role, c["label"], c["passage"], "done", flush=True)

def conds_of(engine):
    return [c for c in spec["conditions"] if c["engine"] == engine]

def whisper():
    from transformers import pipeline
    r = spec["models"]["whisper"]
    return pipeline("automatic-speech-recognition", model=r["repo"], revision=r["revision"], torch_dtype=torch.float16,
                    device="cuda:0")

if role == "cv":
    first = conds_of("cv")[0]["items"][0]["controls"]
    dt = probe("customvoice", "cv", {"speaker": first["speaker"]})
    m = load("customvoice", dt)
    wlog["speakers"] = m.get_supported_speakers()
    run_conditions(m, "cv", conds_of("cv"))
    wlog["models"]["customvoice"]["peak_vram_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
    save()

elif role == "base_vd":
    asr = whisper()
    refs = {}
    for name, r in spec["refs"].items():
        w, sr = sf.read(f"{OUT}/refs/{name}_full.wav", dtype="float32")
        o = asr({"raw": w, "sampling_rate": sr}, return_timestamps=True, generate_kwargs={"language": "en"})
        chunks = [c for c in o["chunks"] if c["timestamp"][0] is not None and c["timestamp"][1] is not None]
        start = chunks[0]["timestamp"][0]
        sel = []
        for c in chunks:
            if c["timestamp"][1] - start > r["max_s"]:
                break
            sel.append(c)
        if not sel:
            sel = chunks[:1]
        end = sel[-1]["timestamp"][1]
        a, b = max(0, int((start - 0.05) * sr)), min(w.size, int((end + 0.25) * sr))
        clip = w[a:b]
        clip = clip / max(1e-6, float(np.abs(clip).max())) * 0.84
        sf.write(f"{OUT}/refs/{name}.wav", clip, sr, subtype="PCM_16")
        refs[name] = {"file": f"refs/{name}.wav", "start_s": round(start, 2), "end_s": round(end, 2),
                      "text": " ".join(c["text"].strip() for c in sel), "full_chunks": o["chunks"]}
    wlog["refs"] = refs; save()
    json.dump(refs, open(f"{OUT}/refs/refs.json", "w"), indent=1)
    del asr; gc.collect(); torch.cuda.empty_cache()
    clone = conds_of("clone")
    if clone:
        r0 = clone[0]["items"][0]["controls"]["ref"]
        dt = probe("base", "clone", {"ref_audio": f"{OUT}/{refs[r0]['file']}", "ref_text": refs[r0]["text"]})
        m = load("base", dt)
        for name in sorted({c["items"][0]["controls"]["ref"] for c in clone}):
            p = m.create_voice_clone_prompt(ref_audio=f"{OUT}/{refs[name]['file']}", ref_text=refs[name]["text"])
            run_conditions(m, "clone", [c for c in clone if c["items"][0]["controls"]["ref"] == name], {"_prompt": p})
        wlog["models"]["base"]["peak_vram_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
        free(m)
    vd = conds_of("vd")
    if vd:
        dt = wlog.get("chosen_dtype", {}).get("base", "fp32")
        m = load("voicedesign", dt)
        run_conditions(m, "vd", vd)
        wlog["models"]["voicedesign"]["peak_vram_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
        free(m)
    save()

elif role == "fast":
    import inspect
    from faster_qwen3_tts import FasterQwen3TTS
    from huggingface_hub import snapshot_download
    import importlib.metadata as md
    wlog["versions"] = {p: md.version(p) for p in ["faster-qwen3-tts", "transformers", "torch"]}
    r = spec["models"]["customvoice"]
    path = snapshot_download(r["repo"], revision=r["revision"])
    torch.cuda.reset_peak_memory_stats()
    sig = inspect.signature(FasterQwen3TTS.from_pretrained)
    wlog["from_pretrained_sig"] = str(sig)
    t = time.time()
    m = FasterQwen3TTS.from_pretrained(path)
    wlog["load_s"] = round(time.time() - t, 1); save()
    t = time.time(); m.warmup(prefill_len=100); sync(); wlog["warmup_s"] = round(time.time() - t, 1)
    wlog["generate_custom_voice_sig"] = str(inspect.signature(m.generate_custom_voice)); save()
    for c in conds_of("fast"):
        d = f"{OUT}/raw/{c['label']}/{c['passage']}"
        os.makedirs(d, exist_ok=True)
        for k, it in enumerate(c["items"]):
            ctrl = it["controls"]
            try:
                seed(spec["seed"] + k); sync(); t0 = time.time()
                kw = dict(text=it["text"], speaker=ctrl["speaker"], language="English",
                          instruct=ctrl.get("instruct") or None, max_new_tokens=max_tokens(it["text"]))
                params = inspect.signature(m.generate_custom_voice).parameters
                if not any(p.kind == p.VAR_KEYWORD for p in params.values()):
                    dropped = [k for k in kw if k not in params]
                    if dropped:
                        wlog["dropped_kwargs"] = dropped
                    kw = {k: v for k, v in kw.items() if k in params}
                wavs, sr = m.generate_custom_voice(**kw)
                sync(); g = time.time() - t0
                w = wavs[0]
                w = w.detach().float().cpu().numpy() if hasattr(w, "detach") else np.asarray(w, dtype=np.float32)
                w = np.clip(w.reshape(-1), -1, 1)
                sf.write(f"{d}/{it['file']}", w, sr, subtype="PCM_16")
                a = w.size / sr
                wlog["items"].append({"label": c["label"], "passage": c["passage"], "file": it["file"], "gpu_s": round(g, 3),
                    "audio_s": round(a, 3), "sr": sr, "capped": a * 12.5 >= max_tokens(it["text"]) - 1,
                    "peak_vram_gb": round(torch.cuda.max_memory_allocated() / 2**30, 2)})
            except Exception:
                wlog["errors"].append({"label": c["label"], "file": it["file"], "error": traceback.format_exc()[-1500:]})
            save()
        print(role, c["label"], c["passage"], "done", flush=True)

elif role == "analysis":
    import glob
    labels = sys.argv[4].split(",") if len(sys.argv) > 4 and sys.argv[4] else None
    files = sorted(glob.glob(f"{OUT}/raw/*/*/*.flac") + glob.glob(f"{OUT}/probe/*.flac"))
    if labels:
        files = [f for f in files if f.split("/")[-3] in labels]
    asr = whisper()
    res = {}
    import librosa
    for f in files:
        w, sr = sf.read(f, dtype="float32")
        w16 = librosa.resample(w, orig_sr=sr, target_sr=16000)
        try:
            res[f.replace(OUT + "/", "")] = {"asr": asr({"raw": w16, "sampling_rate": 16000},
                                                        generate_kwargs={"language": "en"})["text"]}
        except Exception:
            res[f.replace(OUT + "/", "")] = {"asr_error": traceback.format_exc()[-500:]}
    del asr; gc.collect(); torch.cuda.empty_cache()
    try:
        m = load("base", "fp32")
        tsr = m.model.speaker_encoder_sample_rate
        for f in files:
            w, sr = sf.read(f, dtype="float32")
            w = librosa.resample(w, orig_sr=sr, target_sr=tsr)
            with torch.inference_mode():
                e = m.model.extract_speaker_embedding(audio=np.ascontiguousarray(w, dtype=np.float32), sr=tsr)
            res[f.replace(OUT + "/", "")]["spk"] = [round(float(x), 5) for x in e.reshape(-1).float().cpu().tolist()]
        free(m)
    except Exception:
        wlog["errors"].append({"stage": "speaker_embedding", "error": traceback.format_exc()[-2000:]})
    tag = sys.argv[5] if len(sys.argv) > 5 else "main"
    json.dump(res, open(f"{OUT}/analysis_{tag}.json", "w"))
    save()
'''


def main():
    spec = json.loads(zlib.decompress(base64.b64decode(JOBS_B64)))
    os.makedirs(f"{OUT}/refs", exist_ok=True)
    open("/tmp/worker.py", "w").write(WORKER)
    json.dump(spec, open("/tmp/spec.json", "w"))
    log["nvidia_smi"] = subprocess.run("nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader",
                                       shell=True, capture_output=True, text=True).stdout.strip()
    log["python"] = sys.version
    t = time.time()
    log["pip_rc"] = sh("pip install -q " + " ".join(spec["pins"]))
    stage("pip", t)
    import torch, transformers, importlib.metadata as md
    log["versions"] = {p: md.version(p) for p in ["qwen-tts", "transformers", "accelerate", "torch", "torchaudio",
                                                    "huggingface_hub", "soundfile", "librosa"]}
    log["cuda"] = torch.version.cuda
    ngpu = torch.cuda.device_count()
    log["ngpu"] = ngpu
    save()
    # faster-qwen3-tts in its own venv (it needs transformers 5); installed on the CPU while the GPUs work
    fast_pip = None
    if any(c["engine"] == "fast" for c in spec["conditions"]):
        fast_pip = subprocess.Popen("python -m venv --system-site-packages /tmp/fenv && /tmp/fenv/bin/pip install -q "
                                    + spec["fast_pin"] + " > /tmp/fast_pip.log 2>&1", shell=True)
    t = time.time()
    import imageio_ffmpeg
    ff = imageio_ffmpeg.get_ffmpeg_exe()
    for name, r in spec["refs"].items():
        rc = subprocess.run([ff, "-hide_banner", "-loglevel", "error", "-y", "-ss", str(r["start_s"]), "-t",
                             str(r["length_s"]), "-i", r["url"], "-ac", "1", "-ar", "24000", f"{OUT}/refs/{name}_full.wav"]).returncode
        log.setdefault("ref_download", {})[name] = rc
    stage("refs", t)

    def launch(role, gpu, *extra):
        env = dict(os.environ, CUDA_VISIBLE_DEVICES=str(gpu))
        py = "/tmp/fenv/bin/python" if role == "fast" else sys.executable
        f = open(f"/tmp/{role}.log", "a")
        return subprocess.Popen([py, "/tmp/worker.py", role, "/tmp/spec.json", OUT, *extra], env=env, stdout=f,
                                stderr=subprocess.STDOUT)

    def wait(procs, name):
        t = time.time()
        for role, p in procs:
            log.setdefault("rc", {})[role] = p.wait()
        stage(name, t)

    gpu1 = 1 if ngpu > 1 else 0
    if ngpu > 1:
        wait([("cv", launch("cv", 0)), ("base_vd", launch("base_vd", 1))], "render")
    else:
        wait([("cv", launch("cv", 0))], "render_cv")
        wait([("base_vd", launch("base_vd", 0))], "render_base_vd")
    main_labels = sorted({c["label"] for c in spec["conditions"] if c["engine"] != "fast"})
    if fast_pip is not None:
        log["fast_pip_rc"] = fast_pip.wait()
        log["fast_pip_log"] = open("/tmp/fast_pip.log").read()[-3000:]
        save()
        if ngpu > 1:
            pa = launch("analysis", gpu1, ",".join(main_labels), "main")
            wait([("fast", launch("fast", 0))], "fast")
            wait([("analysis", pa)], "analysis_main")
        else:
            wait([("fast", launch("fast", 0))], "fast")
            wait([("analysis", launch("analysis", 0, ",".join(main_labels), "main"))], "analysis_main")
        fast_labels = sorted({c["label"] for c in spec["conditions"] if c["engine"] == "fast"})
        wait([("analysis_fast", launch("analysis", gpu1, ",".join(fast_labels), "fast"))], "analysis_fast")
    else:
        wait([("analysis", launch("analysis", gpu1, ",".join(main_labels), "main"))], "analysis_main")
    for role in ["cv", "base_vd", "fast", "analysis"]:
        try:
            log.setdefault("worker_tail", {})[role] = open(f"/tmp/{role}.log").read()[-4000:]
        except OSError:
            pass
    save()


def stage(name, t):
    log["stages"][name] = round(time.time() - t, 1)
    save()


try:
    main()
except Exception:
    log["errors"].append(traceback.format_exc()[-3000:])
    print(log["errors"][-1], flush=True)
save()
print(json.dumps({k: v for k, v in log.items() if k != "worker_tail"}, indent=1)[:5000])
