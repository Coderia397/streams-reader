# Chatterbox bake-off launcher on a private Kaggle GPU kernel (tools/expressiveness/bakeoff/chatterbox).
# Chatterbox pins torch 2.6.0, transformers 5.2.0 and numpy < 2, which clash with the Kaggle image, so everything runs
# in a fresh uv venv (Python 3.11, the version upstream tests on). The worker and its inputs come from the private
# dataset made by cb_kaggle.py.
import glob, json, os, subprocess, sys, time

T0 = time.time()
OUT = "/kaggle/working"
log = {"install": {}}


def sh(cmd):
    print("+", cmd, flush=True)
    return subprocess.run(cmd, shell=True).returncode


def save():
    log["elapsed_s"] = round(time.time() - T0, 1)
    json.dump(log, open(f"{OUT}/install_log.json", "w"), indent=1)


jobs = glob.glob("/kaggle/input/**/jobs.json", recursive=True)
if not jobs:
    raise SystemExit("jobs.json not found under /kaggle/input")
spec = json.load(open(jobs[0]))
inp = os.path.dirname(jobs[0])
log["input_dir"] = inp
log["image_python"] = sys.version
sh("nvidia-smi")

t = time.time()
log["install"]["uv"] = sh("pip install -q uv==0.9.5")
log["install"]["venv"] = sh("uv venv -q --python 3.11 /tmp/cb")
py = "/tmp/cb/bin/python"
pkgs = " ".join(f"'{p}'" for p in spec["pip"])
log["install"]["pip"] = sh(f"uv pip install -q --python {py} {pkgs}")
log["install"]["seconds"] = round(time.time() - t, 1)
log["install"]["freeze"] = subprocess.run(f"uv pip freeze --python {py}", shell=True, capture_output=True,
                                          text=True).stdout
save()
if log["install"]["pip"] != 0:
    raise SystemExit("install failed")

rc = sh(f"cd /tmp && {py} -u {os.path.join(inp, 'cb_worker.py')} {jobs[0]}")
log["worker_rc"] = rc
save()
