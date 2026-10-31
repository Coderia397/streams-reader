# Ming-3.0-tts-0.5B expressiveness study on a private Kaggle GPU kernel (tools/expressiveness/ming, notes/32).
# Inputs come from the private dataset made by ming_kaggle.py: ming3.tgz (space/vendor/ming3, MIT), jobs.json, the
# clone reference wav. Synthesis mirrors space/nolabs_engine/engines/ming_gpu.py: same tasks and kwargs, seed 1234
# before every call, cfg 2.0, temperature 0, IP anchor of ANCHOR_TEXT per preset for EMOTION, prompt cache on.
# Outputs in /kaggle/working: raw/<label>/<passage>/NN.flac (native rate, PCM 16), anchors/, ming_log.json.
import glob, json, os, subprocess, sys, tarfile, time, traceback

T0 = time.time()
OUT = "/kaggle/working"
LOG_PATH = f"{OUT}/ming_log.json"
log = {"stages": {}, "items": [], "errors": []}


def save_log():
    log["elapsed_s"] = round(time.time() - T0, 1)
    with open(LOG_PATH, "w") as f:
        json.dump(log, f, indent=1)


def sh(cmd):
    print("+", cmd, flush=True)
    return subprocess.run(cmd, shell=True).returncode


def stage(name, t):
    log["stages"][name] = round(time.time() - t, 1)
    save_log()


jobs_path = glob.glob("/kaggle/input/**/jobs.json", recursive=True)
if not jobs_path:
    raise SystemExit("jobs.json not found under /kaggle/input")
INP = os.path.dirname(jobs_path[0])
spec = json.load(open(jobs_path[0]))
log["input_dir"] = INP

t = time.time()
log["nvidia_smi"] = subprocess.run("nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader",
                                   shell=True, capture_output=True, text=True).stdout.strip()
log["python"] = sys.version
PINS = ["transformers==4.46.3", "tokenizers==0.20.3", "huggingface_hub==0.36.2", "safetensors==0.8.0",
        "accelerate==1.14.0", "diffusers==0.33.0", "x-transformers==2.9.2", "einx==0.4.3", "einops==0.8.2",
        "loguru==0.7.3", "inflect==7.5.0", "onnxruntime==1.23.2", "soundfile==0.14.0"]
rc = sh("pip install -q " + " ".join(PINS))
log["pip_rc"] = rc
if rc != 0:  # one pin without a wheel for this Python: install the rest one by one, unpinned as a last resort
    log["pip_fallback"] = {}
    for p in PINS:
        if sh(f"pip install -q {p}") != 0:
            name = p.split("==")[0]
            log["pip_fallback"][p] = sh(f"pip install -q {name}")
log["pynini_rc"] = sh("pip install -q pynini==2.1.6.post1")
stage("pip", t)

t = time.time()
with tarfile.open(os.path.join(INP, "ming3.tgz")) as tf:
    tf.extractall("/tmp")
sys.path.insert(0, "/tmp/ming3")
import numpy as np
import soundfile as sf
import torch
from huggingface_hub import snapshot_download

log["torch"] = torch.__version__
log["versions"] = {}
for mod in ("transformers", "tokenizers", "diffusers", "x_transformers", "onnxruntime", "numpy"):
    try:
        log["versions"][mod] = __import__(mod).__version__
    except Exception as e:  # noqa: BLE001
        log["versions"][mod] = repr(e)
root = snapshot_download(spec["repo"], revision=spec["commit"], allow_patterns=spec["files"])
stage("download", t)

t = time.time()
from ming_tts import MingTTS  # noqa: E402

tts = MingTTS(root, device="cuda", text_normalization=True, cache_prompts=True)
try:
    tts.normalizer  # noqa: B018
    log["text_normalization"] = True
except Exception as e:  # noqa: BLE001 - same fallback as the Space
    log["text_normalization"] = f"disabled: {e!r}"
    tts.text_normalization = False
SR = tts.sample_rate
log["sample_rate"] = SR
log["bf16_supported"] = torch.cuda.is_bf16_supported()
stage("load", t)

SEED = spec["seed"]


def synth(task, text, controls, prompt, prompt_text):
    kwargs = {"task": task, "text": text}
    if task == "IP":
        kwargs.update(controls=controls, use_zero_spk_emb=True)
    elif task == "EMOTION":
        kwargs.update(controls=controls, prompt_wav_path=prompt, use_spk_emb=True)
    elif task == "TTS":
        kwargs.update(language_id="en", prompt_wav_path=prompt, prompt_text=prompt_text, use_spk_emb=True)
    else:
        raise ValueError(task)
    torch.manual_seed(SEED)
    wave = tts.synthesize(cfg=2.0, temperature=0.0, **kwargs)
    return wave.detach().float().cpu().reshape(-1).numpy().astype(np.float32)


def timed(*a):
    torch.cuda.synchronize()
    s = time.time()
    w = synth(*a)
    torch.cuda.synchronize()
    return w, time.time() - s


# Warm-up (CUDA context, kernels, first allocations): timed separately, not counted in the RTF.
t = time.time()
cudnn_note = None
try:
    w, dt = timed("IP", "The warm-up sentence is not part of the study.", {"ip": "US_Emma"}, "", "")
except Exception as e:  # noqa: BLE001 - bf16 conv without a cuDNN engine on the T4 (notes/16): retry without cuDNN
    cudnn_note = f"cuDNN disabled after: {type(e).__name__}: {e}"[:500]
    torch.backends.cudnn.enabled = False
    w, dt = timed("IP", "The warm-up sentence is not part of the study.", {"ip": "US_Emma"}, "", "")
log["cudnn"] = cudnn_note or "enabled"
log["warmup"] = {"gpu_s": round(dt, 2), "audio_s": round(w.size / SR, 2)}
stage("warmup", t)

# Prompts: anchors for presets (IP render of ANCHOR_TEXT, as ming_gpu.run does once per preset) and the clone refs.
t = time.time()
os.makedirs(f"{OUT}/anchors", exist_ok=True)
prompts = {}
for voice in spec["anchors"]:
    w, dt = timed("IP", spec["anchor_text"], {"ip": voice}, "", "")
    path = f"/tmp/anchor-{voice}.wav"
    sf.write(path, w, SR)
    sf.write(f"{OUT}/anchors/{voice}.flac", w, SR, subtype="PCM_16")
    prompts[f"anchor:{voice}"] = (path, spec["anchor_text"])
    log["items"].append({"label": "anchor", "voice": voice, "task": "IP", "chars": len(spec["anchor_text"]),
                         "gpu_s": round(dt, 3), "audio_s": round(w.size / SR, 3)})
for rid, ref in spec["refs"].items():
    prompts[f"ref:{rid}"] = (os.path.join(INP, ref["file"]), ref["transcript"])
stage("anchors", t)

t = time.time()
for cond in spec["conditions"]:
    d = f"{OUT}/raw/{cond['label']}/{cond['passage']}"
    os.makedirs(d, exist_ok=True)
    for it in cond["items"]:
        path, ptext = prompts.get(it["prompt"], ("", "")) if it["prompt"] else ("", "")
        rec = {"label": cond["label"], "passage": cond["passage"], "file": it["file"], "task": it["task"],
               "chars": len(it["text"])}
        try:
            w, dt = timed(it["task"], it["text"], it["controls"], path, ptext if it["task"] == "TTS" else "")
            sf.write(f"{d}/{it['file']}", w, SR, subtype="PCM_16")
            rec.update(gpu_s=round(dt, 3), audio_s=round(w.size / SR, 3), peak=round(float(np.abs(w).max()), 4))
        except Exception as e:  # noqa: BLE001 - one failing item must not lose the run
            rec["error"] = f"{type(e).__name__}: {e}"[:500]
            log["errors"].append(traceback.format_exc()[-2000:])
        log["items"].append(rec)
        print(json.dumps(rec), flush=True)
    save_log()
stage("conditions", t)

ok = [i for i in log["items"] if "gpu_s" in i and i["label"] != "anchor"]
gpu = sum(i["gpu_s"] for i in ok)
aud = sum(i["audio_s"] for i in ok)
chars = sum(i["chars"] for i in ok)
log["summary"] = {"items": len(ok), "failed": sum(1 for i in log["items"] if "error" in i),
                  "gpu_s": round(gpu, 1), "audio_s": round(aud, 1), "rtf": round(gpu / aud, 3) if aud else None,
                  "gpu_s_per_char": round(gpu / chars, 4) if chars else None,
                  "peak_mem_gb": round(torch.cuda.max_memory_allocated() / 2**30, 2)}
save_log()
print(json.dumps(log["summary"], indent=1), flush=True)
