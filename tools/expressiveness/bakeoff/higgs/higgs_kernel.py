# Higgs TTS 3 4B bake-off on a private Kaggle GPU kernel (tools/expressiveness/bakeoff/higgs). Never touches ZeroGPU.
# The job spec is embedded below by higgs_bakeoff.py (JOBS). Flow:
#   1. pip transformers (pinned), snapshot the model, codec and Whisper at pinned revisions.
#   2. Pick two synthetic reference voices from zero-shot renders (F0 + Whisper WER); no real person is cloned.
#   3. One worker process per GPU (T4 x2: one voice per GPU), fp16 LM (bf16 fallback), codec fp32. RTF per item
#      excludes a warm-up; VRAM from torch peak allocation and an nvidia-smi poller.
#   4. Whisper ASR of every output for a word error rate (skips, repeats, babble).
# Outputs in /kaggle/working: raw/<label>/<passage>/NN.flac (24 kHz PCM 16), refs/*.wav, higgs_log.json.
import json, os, re, shutil, subprocess, sys, threading, time, traceback

JOBS = json.loads(r'''__JOBS__''')
OUT = "/kaggle/working"
HF = "/tmp/hf"
os.environ.setdefault("HF_HOME", HF)
os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")


def sh(cmd, capture=False):
    print("+", cmd, flush=True)
    r = subprocess.run(cmd, shell=True, capture_output=capture, text=True)
    return r if capture else r.returncode


def dump(path, obj):
    with open(path + ".tmp", "w") as f:
        json.dump(obj, f, indent=1)
    os.replace(path + ".tmp", path)


# ------------------------------------------------------------------------------------------------------------ worker
def worker(gpu: int, take_all: bool = False) -> None:
    import numpy as np
    import soundfile as sf
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer

    paths = json.load(open(f"{OUT}/paths.json"))
    refs = json.load(open(f"{OUT}/refs.json"))
    mine = [it for it in JOBS["items"] if take_all or it["gpu"] == gpu]
    wlog = {"gpu": gpu, "items": [], "errors": []}
    wpath = f"{OUT}/worker{gpu}.json"
    tok = AutoTokenizer.from_pretrained(paths["model"])

    def load(dtype):
        t = time.time()
        m = AutoModelForCausalLM.from_pretrained(paths["model"], trust_remote_code=True, dtype=dtype)
        m = m.to("cuda").eval()
        m.config.audio_tokenizer_id = paths["codec"]
        m.get_audio_codec()
        torch.cuda.synchronize()
        return m, round(time.time() - t, 1)

    def ref_codes(m, name):
        r = refs[name]
        w, sr = sf.read(r["wav"], dtype="float32")
        assert sr == m.config.sample_rate, sr
        codec = m.get_audio_codec()
        x = torch.from_numpy(w).view(1, 1, -1).to("cuda", torch.float32)
        with torch.no_grad():
            c = codec.encode(x).audio_codes
        return c.squeeze(0).transpose(0, 1).to(torch.long), r["text"]

    def gen(m, text, codes, rtext, seed, max_new):
        torch.manual_seed(seed)
        torch.cuda.manual_seed_all(seed)
        torch.cuda.synchronize()
        t = time.time()
        w = m.generate_speech(text, tok, reference_codes=codes, reference_text=rtext, max_new_tokens=max_new,
                              **JOBS["sampling"])
        torch.cuda.synchronize()
        return w.numpy().astype(np.float32), time.time() - t

    dtype_name, model = None, None
    for name in ("float16", "bfloat16"):
        try:
            model, load_s = load(getattr(torch, name))
            codes, rtext = ref_codes(model, mine[0]["ref"])
            w, dt = gen(model, "This short sentence warms up the model and is not part of the study.", codes, rtext,
                        7, 400)
            secs = w.size / model.config.sample_rate
            ok = bool(np.isfinite(w).all()) and 1.5 < secs < 12
            wlog[f"warmup_{name}"] = {"load_s": load_s, "gen_s": round(dt, 2), "audio_s": round(secs, 2), "ok": ok,
                                      "peak": float(np.abs(w).max()) if w.size else 0.0}
            dump(wpath, wlog)
            if ok:
                dtype_name = name
                break
        except Exception:  # noqa: BLE001 - fp16 overflow -> NaN -> multinomial error: try bf16
            wlog[f"warmup_{name}"] = {"error": traceback.format_exc()[-2500:]}
            dump(wpath, wlog)
        del model
        model = None
        torch.cuda.empty_cache()
    if dtype_name is None:
        raise SystemExit("no dtype produced sane warm-up audio")
    wlog["dtype"] = dtype_name
    torch.cuda.reset_peak_memory_stats()
    cache = {}
    for it in mine:
        try:
            if it["ref"] not in cache:
                cache[it["ref"]] = ref_codes(model, it["ref"])
            codes, rtext = cache[it["ref"]]
            w, dt = gen(model, it["spoken"], codes, rtext, it["seed"], it["max_new_tokens"])
            sr = model.config.sample_rate
            d = f"{OUT}/raw/{it['label']}/{it['passage']}"
            os.makedirs(d, exist_ok=True)
            sf.write(f"{d}/{it['file']}", w, sr, subtype="PCM_16")
            secs = w.size / sr
            frames = round(secs * 25)
            rec = {"label": it["label"], "passage": it["passage"], "file": it["file"], "gen_s": round(dt, 3),
                   "audio_s": round(secs, 3), "rtf": round(dt / secs, 3) if secs else None, "frames": frames,
                   "hit_cap": frames >= it["max_new_tokens"] - 10, "peak": round(float(np.abs(w).max()), 3),
                   "finite": bool(np.isfinite(w).all()),
                   "vram_peak_alloc_gb": round(torch.cuda.max_memory_allocated() / 2**30, 2)}
            wlog["items"].append(rec)
            print(json.dumps(rec), flush=True)
        except Exception:  # noqa: BLE001
            wlog["errors"].append({"item": [it["label"], it["passage"], it["file"]],
                                   "error": traceback.format_exc()[-2000:]})
        dump(wpath, wlog)
    wlog["vram_peak_alloc_gb"] = round(torch.cuda.max_memory_allocated() / 2**30, 2)
    wlog["vram_peak_reserved_gb"] = round(torch.cuda.max_memory_reserved() / 2**30, 2)
    dump(wpath, wlog)


# ------------------------------------------------------------------------------------------------------------ parent
def words(s: str) -> list[str]:
    s = re.sub(r"<\|[^|]*\|>", " ", s).lower().replace("’", "'")
    return re.sub(r"[^a-z0-9' ]+", " ", s).split()


def wer(ref: list[str], hyp: list[str]) -> float:
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / max(1, len(ref))


def parent() -> None:
    t0 = time.time()
    log = {"stages": {}, "errors": []}
    lpath = f"{OUT}/higgs_log.json"

    def stage(name, t):
        log["stages"][name] = round(time.time() - t, 1)
        log["elapsed_s"] = round(time.time() - t0, 1)
        dump(lpath, log)

    t = time.time()
    log["nvidia_smi"] = sh("nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader",
                           capture=True).stdout.strip()
    log["python"] = sys.version
    log["pip_rc"] = sh("pip install -q " + " ".join(JOBS["pins"]))
    stage("pip", t)
    import numpy as np
    import soundfile as sf
    import torch
    import transformers
    import huggingface_hub
    from huggingface_hub import snapshot_download

    log["versions"] = {"torch": torch.__version__, "transformers": transformers.__version__,
                       "huggingface_hub": huggingface_hub.__version__, "numpy": np.__version__,
                       "cuda": torch.version.cuda}
    ngpu = torch.cuda.device_count()
    log["ngpu"] = ngpu
    dump(lpath, log)

    t = time.time()
    paths = {}
    for key, (repo, rev) in JOBS["repos"].items():
        paths[key] = snapshot_download(repo, revision=rev, cache_dir=f"{HF}/hub")
    dump(f"{OUT}/paths.json", paths)
    stage("download", t)

    # References: no real person is cloned (licence IV(b)(ii)). Higgs speaks REF_TEXT zero-shot (no reference) with a
    # series of seeds; each candidate gets a median F0 (autocorrelation) and a Whisper WER. Per voice the clean
    # candidate (WER <= max_wer) whose F0 is nearest the target becomes the fixed reference for every item.
    t = time.time()
    os.makedirs(f"{OUT}/refs", exist_ok=True)
    from transformers import AutoModelForCausalLM, AutoTokenizer, pipeline
    asr = pipeline("automatic-speech-recognition", model=paths["whisper"], dtype=torch.float16, device=0)

    def transcribe(wave, sr):
        from scipy.signal import resample_poly
        x = resample_poly(wave, 2, 3).astype(np.float32) if sr == 24000 else wave
        return asr({"raw": x, "sampling_rate": 16000}, chunk_length_s=30, batch_size=4,
                   generate_kwargs={"language": "en", "task": "transcribe"})["text"].strip()

    def f0_median(w, sr):
        hop, win = int(0.01 * sr), int(0.04 * sr)
        lo, hi = int(sr / 400), int(sr / 60)
        fr = [w[i:i + win] for i in range(0, w.size - win, hop)]
        rms = np.array([np.sqrt(np.mean(x ** 2)) for x in fr])
        gate = np.percentile(rms, 95) * 10 ** (-25 / 20)
        f0 = []
        for x, r in zip(fr, rms):
            if r < gate:
                continue
            x = x - x.mean()
            ac = np.correlate(x, x, "full")[win - 1:]
            if ac[0] <= 0:
                continue
            k = lo + int(np.argmax(ac[lo:hi]))
            if ac[k] / ac[0] > 0.5:
                f0.append(sr / k)
        return float(np.median(f0)) if len(f0) > 20 else None

    def trim(w, sr):
        db = 20 * np.log10(np.maximum(1e-6, np.abs(w)))
        idx = np.where(db > db.max() - 40)[0]
        a, b = max(0, idx[0] - int(0.05 * sr)), min(w.size, idx[-1] + int(0.1 * sr))
        w = w[a:b] * (10 ** (-3 / 20) / max(1e-9, float(np.abs(w[a:b]).max())))
        return w.astype(np.float32)

    rs = JOBS["ref_search"]
    tok0 = AutoTokenizer.from_pretrained(paths["model"])
    m0, dtype0 = None, None
    for name in ("float16", "bfloat16"):
        try:
            m0 = AutoModelForCausalLM.from_pretrained(paths["model"], trust_remote_code=True,
                                                      dtype=getattr(torch, name)).to("cuda").eval()
            m0.config.audio_tokenizer_id = paths["codec"]
            torch.manual_seed(1)
            w = m0.generate_speech(rs["text"], tok0, max_new_tokens=900, **JOBS["sampling"]).numpy()
            if np.isfinite(w).all() and w.size > 24000 * 4:
                dtype0 = name
                break
        except Exception:  # noqa: BLE001
            log["errors"].append(f"ref search {name}: " + traceback.format_exc()[-1500:])
        m0 = None
        torch.cuda.empty_cache()
    log["ref_search_dtype"] = dtype0
    cands, refs = [], {}
    for seed in rs["seeds"]:
        if all(v in refs for v in rs["targets"]):
            break
        torch.manual_seed(seed)
        w = m0.generate_speech(rs["text"], tok0, max_new_tokens=900, **JOBS["sampling"]).numpy().astype(np.float32)
        sr = m0.config.sample_rate
        w = trim(w, sr)
        hyp = transcribe(w, sr)
        c = {"seed": seed, "seconds": round(w.size / sr, 2), "f0": f0_median(w, sr), "hyp": hyp,
             "wer": round(wer(words(rs["text"]), words(hyp)), 3)}
        path = f"{OUT}/refs/cand_{seed}.wav"
        sf.write(path, w, sr, subtype="PCM_16")
        c["wav"] = path
        cands.append(c)
        log["ref_candidates"] = cands
        dump(lpath, log)
        for v, (lo, hi, target) in rs["targets"].items():
            ok = [x for x in cands if x["f0"] and lo <= x["f0"] <= hi and x["wer"] <= rs["max_wer"]
                  and rs["min_s"] <= x["seconds"] <= rs["max_s"]]
            if len(ok) >= rs["per_voice"]:
                best = min(ok, key=lambda x: abs(x["f0"] - target))
                refs[v] = {"wav": f"{OUT}/refs/{v}.wav", "text": rs["text"], "from_seed": best["seed"],
                           "f0": best["f0"], "seconds": best["seconds"], "asr": best["hyp"]}
                shutil.copy(best["wav"], refs[v]["wav"])
    missing = [v for v in rs["targets"] if v not in refs]
    if missing:
        raise SystemExit(f"no clean zero-shot candidate for {missing}")
    del m0
    torch.cuda.empty_cache()
    dump(f"{OUT}/refs.json", refs)
    log["refs"] = refs
    del asr
    torch.cuda.empty_cache()
    stage("refs", t)

    # Workers: one process per GPU. nvidia-smi poller records peak memory.used per GPU.
    t = time.time()
    used = sorted({it["gpu"] for it in JOBS["items"]})
    me = globals().get("__file__")
    single = ngpu < 2 or not me or not os.path.exists(me)
    log["mode"] = "one process, all items on GPU 0" if single else f"one process per GPU {used}"
    peak = {}
    stop = threading.Event()

    def poll():
        while not stop.is_set():
            r = subprocess.run("nvidia-smi --query-gpu=index,memory.used --format=csv,noheader,nounits", shell=True,
                               capture_output=True, text=True).stdout
            for line in r.strip().splitlines():
                i, mb = (int(x) for x in line.split(","))
                peak[i] = max(peak.get(i, 0), mb)
            time.sleep(1)

    th = threading.Thread(target=poll, daemon=True)
    th.start()
    if single:
        try:
            worker(0, take_all=True)
            rcs = [0]
        except BaseException:  # noqa: BLE001
            log["errors"].append(traceback.format_exc()[-2000:])
            rcs = [1]
        used = [0]
    else:
        procs = []
        for g in used:
            env = dict(os.environ, CUDA_VISIBLE_DEVICES=str(g))
            procs.append(subprocess.Popen([sys.executable, me, "--worker", str(g)], env=env))
        rcs = [p.wait() for p in procs]
    stop.set()
    log["worker_rc"] = rcs
    log["nvidia_smi_peak_mb"] = peak
    log["workers"] = {}
    for g in used:
        try:
            log["workers"][g] = json.load(open(f"{OUT}/worker{g}.json"))
        except Exception as e:  # noqa: BLE001
            log["errors"].append(f"worker{g} log: {e!r}")
    stage("synth", t)

    # ASR of every output.
    t = time.time()
    try:
        asr = pipeline("automatic-speech-recognition", model=paths["whisper"], dtype=torch.float16, device=0)
        log["asr"] = []
        for it in JOBS["items"]:
            f = f"{OUT}/raw/{it['label']}/{it['passage']}/{it['file']}"
            if not os.path.exists(f):
                continue
            w, sr = sf.read(f, dtype="float32")
            hyp = transcribe(w, sr)
            ref = words(it["spoken"])
            log["asr"].append({"label": it["label"], "passage": it["passage"], "file": it["file"], "hyp": hyp,
                               "wer": round(wer(ref, words(hyp)), 3), "ref_words": len(ref)})
            dump(lpath, log)
    except Exception:  # noqa: BLE001
        log["errors"].append(traceback.format_exc()[-2000:])
    stage("asr", t)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--worker":
        worker(int(sys.argv[2]))
    else:
        try:
            parent()
        except Exception:  # noqa: BLE001
            print(traceback.format_exc(), flush=True)
            log = {}
            try:
                log = json.load(open(f"{OUT}/higgs_log.json"))
            except Exception:  # noqa: BLE001
                pass
            log.setdefault("errors", []).append(traceback.format_exc()[-3000:])
            dump(f"{OUT}/higgs_log.json", log)
