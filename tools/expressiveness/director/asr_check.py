#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["onnx-asr[cpu,hub]", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Intelligibility check: word error rate of each clip against its text, with NVIDIA Parakeet TDT 0.6B v3 (ONNX int8,
local CPU). A mapping that garbles words (vocoder artefacts from F0/duration edits, odd blends) shows up as WER.

    uv run tools/expressiveness/director/asr_check.py <root> [--out results/x.json]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

import imageio_ffmpeg
import numpy as np


def decode(path: Path) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", "1",
           "-ar", "16000", "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32).copy()


def norm(t: str) -> list[str]:
    t = t.lower().replace("’", "'")
    return [SPELLING.get(w, w) for w in re.sub(r"[^a-z0-9' ]+", " ", t).split()]


SPELLING = {"grey": "gray", "ok": "okay"}   # British/American spellings the ASR normalises


def edits(ref: list[str], hyp: list[str]) -> int:
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1]


def main() -> None:
    import onnx_asr
    ap = argparse.ArgumentParser()
    ap.add_argument("root", type=Path)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    model = onnx_asr.load_model("nemo-parakeet-tdt-0.6b-v3", quantization="int8", providers=["CPUExecutionProvider"])
    only = set(filter(None, a.only.split(",")))
    out = {}
    for cdir in sorted(p for p in a.root.iterdir() if p.is_dir() and (not only or p.name in only)):
        errs = words = 0
        bad = []
        for d in sorted(p for p in cdir.iterdir() if (p / "meta.json").exists()):
            meta = json.loads((d / "meta.json").read_text())
            for seg in meta["segments"]:
                hyp = model.recognize(decode(d / seg["file"]), sample_rate=16000)
                r, h = norm(seg["text"]), norm(hyp)
                e = edits(r, h)
                errs, words = errs + e, words + len(r)
                if e:
                    bad.append({"passage": d.name, "file": seg["file"], "errors": e, "hyp": hyp})
        out[cdir.name] = {"wer": round(errs / max(1, words), 4), "errors": errs, "words": words, "clips_with_errors": bad}
        print(f"{cdir.name:14} WER {100 * errs / max(1, words):.1f} % ({errs}/{words})", flush=True)
    if a.out:
        a.out.write_text(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
