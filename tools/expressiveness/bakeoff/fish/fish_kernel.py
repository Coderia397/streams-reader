# Fish Audio bake-off (Drama 3 int4, S2 Pro) on a private Kaggle GPU kernel (T4 x2). Built by fish_bakeoff.py push,
# which fills JOBS and STAGE_SRC below. Never touches ZeroGPU or the live API.
# Stages, each in its own process so GPU memory is freed between them: refs (Whisper transcripts of the two clone
# references), drama3, s2pro, asr (Whisper transcripts of every render). Outputs in /kaggle/working:
# raw/<label>/<passage>/NN.flac, logs/<stage>.json, refs.json, fish_log.json.
import base64, glob, json, os, subprocess, sys, time

JOBS = None  # @@JOBS@@
STAGE_SRC = None  # @@STAGE_SRC@@
T0 = time.time()
OUT = "/kaggle/working"
WORK = "/tmp/fish"
os.makedirs(WORK, exist_ok=True)
os.makedirs(f"{OUT}/logs", exist_ok=True)
log = {"stages": {}, "rc": {}}


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    json.dump(log, open(f"{OUT}/fish_log.json", "w"), indent=1)


def sh(cmd, timeout=None):
    print("+", cmd, flush=True)
    try:
        return subprocess.run(cmd, shell=True, timeout=timeout).returncode
    except subprocess.TimeoutExpired:
        return -9


def out(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip()


log["nvidia_smi"] = out("nvidia-smi --query-gpu=name,memory.total,driver_version,compute_cap --format=csv,noheader")
log["python"] = sys.version
log["disk_tmp"] = out("df -h /tmp | tail -1")
log["ram"] = out("free -g | head -2")
save()

jobs_path = glob.glob("/kaggle/input/**/ref_female.wav", recursive=True)
if not jobs_path:
    raise SystemExit("dataset not found under /kaggle/input")
JOBS["input_dir"] = os.path.dirname(jobs_path[0])
json.dump(JOBS, open(f"{WORK}/jobs.json", "w"))
open(f"{WORK}/fish_stage.py", "wb").write(base64.b64decode(STAGE_SRC))

t = time.time()
fs = JOBS["fish_speech"]
log["rc"]["clone"] = sh(f"git clone -q {fs['repo']} {WORK}/fish-speech && cd {WORK}/fish-speech && "
                        f"git checkout -q {fs['commit']} && git log -1 --format=%H")
def install(key, pins, flags=""):
    log["rc"][key] = sh(f"pip install -q {flags} " + " ".join(f"'{p}'" for p in pins))
    if log["rc"][key] != 0:  # a pin without a wheel for this Python: one by one, unpinned as a last resort
        fb = log.setdefault("pip_fallback", {})
        for p in pins:
            if sh(f"pip install -q {flags} '{p}'") != 0:
                fb[p] = sh(f"pip install -q {flags} '{p.split('==')[0]}'")


install("pip", JOBS["pins"])
install("pip_nodeps", JOBS["pins_nodeps"], "--no-deps")
install("pip_light", JOBS["pins_light"])
# Import probe: install whatever module the inference path still misses (logged), up to 10 rounds.
probe = ("import sys; sys.path.insert(0, '/tmp/fish/fish-speech'); "
         "import fish_speech.models.text2semantic.inference, fish_speech.models.dac.modded_dac")
log["probe_installs"] = []
for _ in range(10):
    r = subprocess.run([sys.executable, "-c", probe], capture_output=True, text=True)
    if r.returncode == 0:
        break
    tail = r.stderr.strip().splitlines()[-1] if r.stderr.strip() else "?"
    print(tail, flush=True)
    if "No module named" not in tail:
        log["probe_error"] = r.stderr[-3000:]
        break
    mod = tail.split("No module named")[-1].strip().strip("'\"").split(".")[0]
    pkg = JOBS["module_to_pip"].get(mod, mod)
    log["probe_installs"].append([mod, pkg, sh(f"pip install -q {pkg}")])
log["probe_ok"] = subprocess.run([sys.executable, "-c", probe]).returncode == 0
log["versions"] = out(f"{sys.executable} -m pip list 2>/dev/null | grep -iE "
                      "'^(torch|torchaudio|transformers|tokenizers|huggingface.hub|safetensors|numpy|librosa|"
                      "descript|einx|einops|hydra|omegaconf|soundfile|loguru|ormsgpack|tiktoken|triton) '")
log["stages"]["setup"] = round(time.time() - t, 1)
save()

env = dict(os.environ, HF_HOME="/tmp/hf/home", HF_HUB_ENABLE_HF_TRANSFER="0", PYTHONUNBUFFERED="1",
           TOKENIZERS_PARALLELISM="false",
           PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True")
for stage in JOBS["stages"]:
    t = time.time()
    try:
        rc = subprocess.run([sys.executable, f"{WORK}/fish_stage.py", stage], env=env,
                            timeout=JOBS["stage_timeout_s"][stage]).returncode
    except subprocess.TimeoutExpired:
        rc = -9
    log["rc"][stage] = rc
    log["stages"][stage] = round(time.time() - t, 1)
    log["disk_after_" + stage] = out("df -h /tmp | tail -1")
    save()
    if stage == "refs" and rc != 0:
        break
save()
print(json.dumps(log, indent=1))
