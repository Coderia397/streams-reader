# One GPU shard of the IndexTTS bake-off (tools/expressiveness/bakeoff/indextts25). Runs inside the index-tts uv
# environment (Python 3.11, torch 2.8 cu128 from the upstream uv.lock) that itts_kernel.py builds, with
# CUDA_VISIBLE_DEVICES set to one GPU. Writes raw/<label>/<passage>/NN.flac (22.05 kHz PCM 16) and log_<shard>.json.
import argparse
import gc
import json
import os
import random
import sys
import time
import traceback
import warnings

ap = argparse.ArgumentParser()
ap.add_argument("--repo", required=True)
ap.add_argument("--jobs", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--shard", type=int, default=0)
ap.add_argument("--nshard", type=int, default=1)
ap.add_argument("--version", default="2.5")
ap.add_argument("--smoke", action="store_true")
a = ap.parse_args()

T0 = time.time()
os.chdir(a.repo)  # infer_v2_5 sets HF_HUB_CACHE=./checkpoints/hf_cache
sys.path.insert(0, a.repo)
spec = json.load(open(a.jobs))
INP = os.path.dirname(a.jobs)
LOG = os.path.join(a.out, f"log_{'smoke' if a.smoke else a.shard}_{a.version}.json")
log = {"shard": a.shard, "nshard": a.nshard, "version": a.version, "stages": {}, "items": [], "errors": []}


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    json.dump(log, open(LOG, "w"), indent=1)


def stage(name, t):
    log["stages"][name] = round(time.time() - t, 1)
    save()


import numpy as np  # noqa: E402
import soundfile as sf  # noqa: E402
import torch  # noqa: E402

log["torch"] = torch.__version__
log["cuda"] = torch.version.cuda
log["gpu"] = torch.cuda.get_device_name(0)
log["bf16_supported"] = torch.cuda.is_bf16_supported()

t = time.time()
try:
    if a.version == "2.5":
        from indextts.infer_v2_5 import IndexTTS2
        tts = IndexTTS2(cfg_path="checkpoints/config.yaml", model_dir="checkpoints", use_bf16=False,
                        use_cuda_kernel=False, use_qwen_emo=True)
    else:
        from indextts.infer_v2 import IndexTTS2
        aux = {"w2v_bert": "checkpoints_v2/hf_cache/w2v-bert-2.0",
               "semantic_codec": "checkpoints_v2/hf_cache/semantic_codec_model.safetensors",
               "campplus": "checkpoints_v2/hf_cache/campplus_cn_common.bin",
               "bigvgan": "checkpoints_v2/hf_cache/bigvgan"}
        tts = IndexTTS2(cfg_path="checkpoints_v2/config.yaml", model_dir="checkpoints_v2", use_fp16=False,
                        use_cuda_kernel=False, use_qwen_emo=True, aux_paths=aux)
except Exception as e:  # noqa: BLE001
    log["load_error"] = traceback.format_exc()[-4000:]
    save()
    raise
log["load_mem_gb"] = round(torch.cuda.memory_allocated() / 2**30, 2)
stage("load", t)
SR = 22050
SEED = spec["seed"]


def seed():
    random.seed(SEED)
    np.random.seed(SEED)
    torch.manual_seed(SEED)
    torch.cuda.manual_seed_all(SEED)


def synth(ref, text, emo_vector=None, emo_alpha=1.0, duration_factor=1.0):
    kw = {"spk_audio_prompt": ref, "text": text, "output_path": None, "emo_alpha": emo_alpha, "emo_vector": emo_vector}
    if a.version == "2.5":
        kw.update(lang="EN", duration_factor=duration_factor)
    seed()
    res = tts.infer(**kw)
    if res is None:
        raise RuntimeError("infer returned None")
    sr, wav = res
    assert sr == SR, sr
    return np.asarray(wav, dtype=np.float32).reshape(-1) / 32768.0


def qwen_vector(text):
    d = tts.qwen_emo.inference(text)
    return [float(x) for x in d.values()]


# Items of this shard: grouped by voice so the speaker-prompt cache stays warm, split into contiguous chunks
# balanced by characters.
flat = []
for c in spec["conditions"]:
    for it in c["items"]:
        flat.append((c["voice"], c["label"], c["passage"], it))
flat.sort(key=lambda x: ("lvf", "lvm", "heart").index(x[0]))
if a.smoke:
    flat = [x for x in flat if x[3]["mode"] == "emotext"][:1]
    mine = flat
else:
    total = sum(len(x[3]["text"]) for x in flat)
    mine, acc = [], 0
    for x in flat:
        lo, hi = total * a.shard / a.nshard, total * (a.shard + 1) / a.nshard
        mid = acc + len(x[3]["text"]) / 2
        if lo <= mid < hi:
            mine.append(x)
        acc += len(x[3]["text"])
log["n_items"] = len(mine)
refs = {v: os.path.join(INP, f) for v, f in spec["refs"].items()}

# Warm-up per voice (CUDA kernels, speaker-prompt features): timed separately, not in the RTF.
t = time.time()
log["warmup"] = {}
for v in dict.fromkeys(x[0] for x in mine):
    torch.cuda.synchronize()
    s = time.time()
    w = synth(refs[v], "The warm-up sentence is not part of the study.")
    torch.cuda.synchronize()
    log["warmup"][v] = {"gpu_s": round(time.time() - s, 2), "audio_s": round(w.size / SR, 2)}
stage("warmup", t)
torch.cuda.reset_peak_memory_stats()

t = time.time()
for v, label, passage, it in mine:
    d = os.path.join(a.out, "raw", label, passage)
    os.makedirs(d, exist_ok=True)
    rec = {"label": label, "passage": passage, "file": it["file"], "voice": v, "mode": it["mode"],
           "chars": len(it["text"])}
    try:
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always", RuntimeWarning)
            torch.cuda.synchronize()
            s = time.time()
            vec, alpha, dur = None, 1.0, 1.0
            if it["mode"] == "emotext":
                vec, alpha = qwen_vector(it["text"]), it["emo_alpha"]
            elif it["mode"] == "directed":
                vec, dur = it["emo_vector"], it["duration_factor"]
            w = synth(refs[v], it["text"], vec, alpha, dur)
            torch.cuda.synchronize()
            dt = time.time() - s
        sf.write(os.path.join(d, it["file"]), w, SR, subtype="PCM_16")
        rec.update(gpu_s=round(dt, 3), audio_s=round(w.size / SR, 3), peak=round(float(np.abs(w).max()), 4),
                   emo_vector=vec, emo_alpha=alpha, duration_factor=dur,
                   warnings=[str(x.message)[:200] for x in caught if "max_mel_tokens" in str(x.message)])
    except Exception as e:  # noqa: BLE001 - one failing item must not lose the run
        rec["error"] = f"{type(e).__name__}: {e}"[:500]
        log["errors"].append(traceback.format_exc()[-2000:])
    log["items"].append(rec)
    print(json.dumps(rec), flush=True)
    if len(log["items"]) % 10 == 0:
        save()
log["peak_mem_alloc_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
log["peak_mem_reserved_gb"] = round(torch.cuda.max_memory_reserved() / 2**30, 2)
stage("synth", t)

# ASR check (Whisper small.en, greedy): content errors such as skipped, repeated or invented words.
t = time.time()
try:
    del tts
    gc.collect()
    torch.cuda.empty_cache()
    import whisper

    asr = whisper.load_model("small.en", device="cuda", download_root="/tmp/whisper")
    todo = [(r, os.path.join(a.out, "raw", r["label"], r["passage"], r["file"])) for r in log["items"]
            if "error" not in r]
    if a.shard == 0 and not a.smoke:
        log["ref_asr"] = {v: asr.transcribe(p, language="en", fp16=True, temperature=0.0,
                                            condition_on_previous_text=False)["text"].strip()
                          for v, p in refs.items()}
    for r, p in todo:
        r["asr"] = asr.transcribe(p, language="en", fp16=True, temperature=0.0,
                                  condition_on_previous_text=False)["text"].strip()
except Exception:  # noqa: BLE001
    log["asr_error"] = traceback.format_exc()[-2000:]
stage("asr", t)

ok = [i for i in log["items"] if "gpu_s" in i]
gpu = sum(i["gpu_s"] for i in ok)
aud = sum(i["audio_s"] for i in ok)
log["summary"] = {"items": len(ok), "failed": len(log["items"]) - len(ok), "gpu_s": round(gpu, 1),
                  "audio_s": round(aud, 1), "rtf": round(gpu / aud, 3) if aud else None}
save()
print(json.dumps(log["summary"]), flush=True)
