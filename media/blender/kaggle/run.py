# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Push a film render job to a private Kaggle GPU kernel, wait for it, download the outputs.

    uv run --script media/blender/kaggle/run.py media/blender/kaggle/jobs/preview.json --out DIR [--slug NAME] [--no-wait]
    uv run --script media/blender/kaggle/run.py --fetch --slug NAME --out DIR      # only download a finished run

The job JSON lists tasks (segment, frames or anim/range, engine, samples, percent); see jobs/*.json. build_scene.py,
the shot JSON and the job are packed into the kernel source (base64 tar.gz), so no dataset is needed; the kernel
fetches Blender from download.blender.org and the app screen capture from the public GitHub repository.
Token: KAGGLE_API_TOKEN from ~/.config/streams/kaggle.env (never printed). Accelerator: NvidiaTeslaT4 (T4 x2).
"""
from __future__ import annotations

import argparse
import base64
import io
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BLENDER_DIR = HERE.parent
USER = "lbrahimali"


def env() -> dict:
    e = dict(os.environ)
    p = Path.home() / ".config/streams/kaggle.env"
    for line in p.read_text().splitlines():
        if "=" in line and not line.startswith("#"):
            k, v = line.split("=", 1)
            e[k.strip()] = v.strip().strip('"')
    return e


def kaggle(*args, check=True) -> str:
    r = subprocess.run(["uvx", "kaggle", *args], capture_output=True, text=True, env=env())
    if check and r.returncode:
        raise SystemExit(f"kaggle {' '.join(args)} failed:\n{r.stdout}\n{r.stderr}")
    return r.stdout + r.stderr


def payload(job: dict) -> str:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tf:
        tf.add(BLENDER_DIR / "build_scene.py", arcname="build_scene.py")
        for p in (BLENDER_DIR / "shots").glob("*.json"):
            tf.add(p, arcname=f"shots/{p.name}")
        data = json.dumps(job).encode()
        ti = tarfile.TarInfo("job.json")
        ti.size = len(data)
        tf.addfile(ti, io.BytesIO(data))
    return base64.b64encode(buf.getvalue()).decode()


def push(job: dict, slug: str) -> None:
    stage = Path(tempfile.mkdtemp(prefix="kaggle-film-"))
    src = (HERE / "kernel_template.py").read_text().replace("__PAYLOAD__", payload(job))
    (stage / "kernel.py").write_text(src)
    meta = {"id": f"{USER}/{slug}", "title": slug, "code_file": "kernel.py", "language": "python",
            "kernel_type": "script", "is_private": True, "enable_gpu": True, "enable_internet": True,
            "dataset_sources": [], "competition_sources": [], "kernel_sources": []}
    (stage / "kernel-metadata.json").write_text(json.dumps(meta))
    print(f"pushing {slug} ({len(src) // 1024} KB source)", flush=True)
    print(kaggle("kernels", "push", "-p", str(stage), "--accelerator", "NvidiaTeslaT4").strip()[-600:], flush=True)


def wait(slug: str, poll: int = 60) -> str:
    t0 = time.time()
    while True:
        out = kaggle("kernels", "status", f"{USER}/{slug}", check=False)
        m = re.search(r'status "?([A-Za-z_.]+)"?', out)
        st = (m.group(1) if m else out.strip()).lower()
        print(f"[{(time.time() - t0) / 60:5.1f} min] {st}", flush=True)
        if any(k in st for k in ("complete", "error", "cancel")):
            return st
        time.sleep(poll)


def fetch(slug: str, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    print(kaggle("kernels", "output", f"{USER}/{slug}", "-p", str(out), "--force", check=False)[-1500:], flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("job", nargs="?")
    ap.add_argument("--slug", default="streams-reader-film-v2")
    ap.add_argument("--out", required=True)
    ap.add_argument("--no-wait", action="store_true")
    ap.add_argument("--fetch", action="store_true")
    ap.add_argument("--poll", type=int, default=60)
    a = ap.parse_args()
    if not a.fetch:
        job = json.loads(Path(a.job).read_text())
        push(job, a.slug)
        if a.no_wait:
            return
        time.sleep(30)
        st = wait(a.slug, a.poll)
        print("final status:", st)
    fetch(a.slug, Path(a.out))


if __name__ == "__main__":
    main()
