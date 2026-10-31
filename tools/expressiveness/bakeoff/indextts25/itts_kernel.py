# IndexTTS-2.5 bake-off on a private Kaggle GPU kernel (tools/expressiveness/bakeoff/indextts25). Inputs come from the
# private dataset made by itts_kaggle.py: jobs.json, itts_worker.py, ref_*.wav. The image's Python (3.13) is too new
# for index-tts (<3.12), so the pinned upstream commit is installed with its own uv.lock into a Python 3.11 venv.
# Models are downloaded at pinned revisions. One worker per GPU; IndexTTS-2 is the fallback if 2.5 does not load.
# Outputs in /kaggle/working: raw/<label>/<passage>/NN.flac, itts_log.json, worker logs.
import glob, json, os, shutil, subprocess, sys, time, traceback

T0 = time.time()
OUT = "/kaggle/working"
LOG_PATH = f"{OUT}/itts_log.json"
log = {"stages": {}, "items": [], "errors": []}


def save_log():
    log["elapsed_s"] = round(time.time() - T0, 1)
    with open(LOG_PATH, "w") as f:
        json.dump(log, f, indent=1)


def sh(cmd, env=None, out=None):
    print("+", cmd, flush=True)
    if out:
        with open(out, "w") as f:
            return subprocess.run(cmd, shell=True, env=env, stdout=f, stderr=subprocess.STDOUT).returncode
    return subprocess.run(cmd, shell=True, env=env).returncode


def stage(name, t):
    log["stages"][name] = round(time.time() - t, 1)
    save_log()


jobs_path = glob.glob("/kaggle/input/**/jobs.json", recursive=True)
if not jobs_path:
    raise SystemExit("jobs.json not found under /kaggle/input")
INP = os.path.dirname(jobs_path[0])
spec = json.load(open(jobs_path[0]))
log["input_dir"] = INP
log["nvidia_smi"] = subprocess.run("nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader",
                                   shell=True, capture_output=True, text=True).stdout.strip()
NGPU = max(1, len(log["nvidia_smi"].splitlines()))
log["python_image"] = sys.version
log["disk"] = subprocess.run("df -h /tmp /kaggle/working | tail -2", shell=True, capture_output=True,
                             text=True).stdout
save_log()

# ------------------------------------------------------------------------------------------------ code + environment
t = time.time()
W = "/tmp/itts"
REPO = f"{W}/index-tts"
os.makedirs(W, exist_ok=True)
env = dict(os.environ, UV_LINK_MODE="copy", UV_CACHE_DIR=f"{W}/uvcache", USE_MODELSCOPE="false",
           HF_HUB_DISABLE_PROGRESS_BARS="1", PYTHONUNBUFFERED="1")
log["pip_uv_rc"] = sh(f"pip install -q uv=={spec['uv_version']}")
url = f"https://codeload.github.com/{spec['code_repo']}/tar.gz/{spec['code_commit']}"
log["code_rc"] = sh(f"cd {W} && curl -sL {url} -o code.tgz && tar xzf code.tgz && rm code.tgz && "
                    f"mv index-tts-{spec['code_commit']} index-tts")
# Upstream install (README: uv sync --all-extras) without the webui/deepspeed/flash-attn extras: none is needed for
# inference and flash-attn/deepspeed would compile against the T4.
log["uv_sync_rc"] = sh(f"cd {REPO} && uv sync --frozen --python 3.11 --no-dev", env=env, out=f"{OUT}/uv_sync.txt")
PY = f"{REPO}/.venv/bin/python"
log["venv"] = subprocess.run(f"{PY} -c \"import sys,torch,transformers;print(sys.version.split()[0],torch.__version__,"
                             f"torch.version.cuda,transformers.__version__,torch.cuda.is_available())\"",
                             shell=True, capture_output=True, text=True, env=env).stdout.strip()
shutil.rmtree(f"{W}/uvcache", ignore_errors=True)
stage("install", t)

# --------------------------------------------------------------------------------------------------------- models
DL = r'''
import json, os, shutil, sys
from huggingface_hub import hf_hub_download, snapshot_download
m = json.loads(sys.argv[1]); version = sys.argv[2]
ck = "checkpoints" if version == "2.5" else "checkpoints_v2"
repo = "IndexTeam/IndexTTS-2.5" if version == "2.5" else "IndexTeam/IndexTTS-2"
snapshot_download(repo, revision=m[repo], local_dir=ck)
hc = os.path.join(ck, "hf_cache")
snapshot_download("facebook/w2v-bert-2.0", revision=m["facebook/w2v-bert-2.0"], local_dir=f"{hc}/w2v-bert-2.0",
                  allow_patterns=["config.json", "model.safetensors", "preprocessor_config.json"])
p = hf_hub_download("funasr/campplus", "campplus_cn_common.bin", revision=m["funasr/campplus"], local_dir=f"{hc}/_dl")
shutil.copy(p, f"{hc}/campplus_cn_common.bin")
bv = "nvidia/bigvgan_v2_22khz_80band_256x"
os.makedirs(f"{hc}/bigvgan", exist_ok=True)
for f in ("config.json", "bigvgan_generator.pt"):
    shutil.copy(hf_hub_download(bv, f, revision=m[bv], local_dir=f"{hc}/_dl"), f"{hc}/bigvgan/{f}")
if version != "2.5":
    p = hf_hub_download("amphion/MaskGCT", "semantic_codec/model.safetensors", revision=m["amphion/MaskGCT"],
                        local_dir=f"{hc}/_dl")
    shutil.copy(p, f"{hc}/semantic_codec_model.safetensors")
shutil.rmtree(f"{hc}/_dl", ignore_errors=True)
print("models ready", ck)
'''
open(f"{W}/dl.py", "w").write(DL)


def download(version):
    t = time.time()
    rc = sh(f"cd {REPO} && {PY} {W}/dl.py '{json.dumps(spec['models'])}' {version}", env=env,
            out=f"{OUT}/download_{version}.txt")
    log[f"download_{version}_rc"] = rc
    log["checkpoint_sizes"] = subprocess.run(f"cd {REPO} && du -sh checkpoints* checkpoints*/hf_cache/* 2>/dev/null",
                                             shell=True, capture_output=True, text=True).stdout
    stage(f"download_{version}", t)
    return rc


def smoke(version):
    t = time.time()
    rc = sh(f"CUDA_VISIBLE_DEVICES=0 {PY} {INP}/itts_worker.py --repo {REPO} --jobs {jobs_path[0]} "
            f"--out /tmp/smoke --version {version} --smoke", env=env, out=f"{OUT}/smoke_{version}.txt")
    log[f"smoke_{version}_rc"] = rc
    try:
        log[f"smoke_{version}"] = json.load(open(f"/tmp/smoke/log_smoke_{version}.json"))
    except Exception:  # noqa: BLE001
        log[f"smoke_{version}"] = None
    stage(f"smoke_{version}", t)
    return rc


os.makedirs("/tmp/smoke", exist_ok=True)
VERSION = "2.5"
if download("2.5") != 0 or smoke("2.5") != 0:
    VERSION = "2"
    log["fallback"] = "IndexTTS-2.5 did not load or synthesise; see smoke_2.5.txt"
    shutil.rmtree(f"{REPO}/checkpoints", ignore_errors=True)
    if download("2") != 0 or smoke("2") != 0:
        log["fatal"] = "neither IndexTTS-2.5 nor IndexTTS-2 ran; see smoke_*.txt"
        save_log()
        raise SystemExit(1)
log["model_version"] = VERSION
save_log()

# ------------------------------------------------------------------------------------------------------- workers
t = time.time()
procs = []
for g in range(NGPU):
    cmd = (f"CUDA_VISIBLE_DEVICES={g} {PY} {INP}/itts_worker.py --repo {REPO} --jobs {jobs_path[0]} --out {OUT} "
           f"--shard {g} --nshard {NGPU} --version {VERSION}")
    print("+", cmd, flush=True)
    procs.append(subprocess.Popen(cmd, shell=True, env=env, stdout=open(f"{OUT}/worker_{g}.txt", "w"),
                                  stderr=subprocess.STDOUT))
while any(p.poll() is None for p in procs):
    time.sleep(30)
    done = sum(sum(1 for _ in open(f"{OUT}/worker_{g}.txt") if _.startswith('{"label"')) for g in range(NGPU))
    print(time.strftime("%H:%M:%S"), "items done:", done, flush=True)
log["worker_rc"] = [p.returncode for p in procs]
stage("workers", t)

shards = []
for g in range(NGPU):
    try:
        shards.append(json.load(open(f"{OUT}/log_{g}_{VERSION}.json")))
    except Exception:  # noqa: BLE001
        log["errors"].append(f"no log for shard {g}")
for s in shards:
    log["items"] += s["items"]
    log["errors"] += s["errors"]
log["shards"] = [{k: v for k, v in s.items() if k not in ("items", "errors")} for s in shards]
ok = [i for i in log["items"] if "gpu_s" in i]
gpu = sum(i["gpu_s"] for i in ok)
aud = sum(i["audio_s"] for i in ok)
chars = sum(i["chars"] for i in ok)
log["summary"] = {"items": len(ok), "failed": len(log["items"]) - len(ok), "gpu_s": round(gpu, 1),
                  "audio_s": round(aud, 1), "rtf": round(gpu / aud, 3) if aud else None,
                  "gpu_s_per_char": round(gpu / chars, 4) if chars else None,
                  "peak_mem_alloc_gb": max((s.get("peak_mem_alloc_gb") or 0) for s in shards) if shards else None,
                  "peak_mem_reserved_gb": max((s.get("peak_mem_reserved_gb") or 0) for s in shards) if shards else None}
by_mode = {}
for i in ok:
    m = by_mode.setdefault(i["mode"], [0.0, 0.0])
    m[0] += i["gpu_s"]
    m[1] += i["audio_s"]
log["summary"]["rtf_by_mode"] = {k: round(v[0] / v[1], 3) for k, v in by_mode.items()}
save_log()
print(json.dumps(log["summary"], indent=1), flush=True)
