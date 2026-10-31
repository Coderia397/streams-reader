#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["numpy>=1.26", "scipy>=1.11", "imageio-ffmpeg"]
# ///
"""Artefact evidence for the Higgs bake-off clips (no listening): per set the speech rate against text syllables, the
longest internal silence, Whisper WER, clips that hit the token cap, clipping, and two spectral checks on speech
frames (spectral flatness: hiss/buzz raises it; energy above 8 kHz relative to 0-8 kHz: codec band limit or noise).

    uv run tools/expressiveness/bakeoff/higgs/analyze.py <audio root> [--json out.json]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

import imageio_ffmpeg
import numpy as np

SR = 48_000


def decode(p: Path) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(p), "-ac", "1", "-ar",
           str(SR), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32)


def syllables(text: str) -> int:
    text = re.sub(r"<\|[^|]*\|>", " ", text).lower()
    n = 0
    for w in re.findall(r"[a-z']+", text):
        k = len(re.findall(r"[aeiouy]+", w))
        if w.endswith("e") and k > 1 and not w.endswith("le"):
            k -= 1
        n += max(1, k)
    return n


def clip_stats(w: np.ndarray) -> dict:
    hop, win = SR // 100, int(0.025 * SR)
    fr = np.lib.stride_tricks.sliding_window_view(w, win)[::hop]
    db = 10 * np.log10(np.mean(fr ** 2, axis=1) + 1e-12)
    th = max(np.percentile(db, 95) - 30, np.percentile(db, 5) + 6)
    sp = db > th
    idx = np.where(sp)[0]
    inner = sp[idx[0]:idx[-1] + 1] if idx.size else sp
    longest, run = 0, 0
    for v in inner:
        run = 0 if v else run + 1
        longest = max(longest, run)
    spec = np.abs(np.fft.rfft(fr[sp][:, :1024] * np.hanning(1024), axis=1)) ** 2 + 1e-12
    f = np.fft.rfftfreq(1024, 1 / SR)
    flat = np.exp(np.mean(np.log(spec), axis=1)) / np.mean(spec, axis=1)
    hf = 10 * np.log10(spec[:, f >= 8000].sum(1) / spec[:, (f > 80) & (f < 8000)].sum(1))
    return {"speech_s": float(sp.sum()) / 100, "longest_gap_s": longest / 100, "flatness": float(np.median(flat)),
            "hf_db": float(np.median(hf)), "clip_frac": float(np.mean(np.abs(w) > 0.99))}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("root", type=Path)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    rows = []
    for d in sorted(p for p in a.root.glob("*/*") if (p / "meta.json").exists()):
        meta = json.loads((d / "meta.json").read_text())
        st, wers, caps, gaps = [], [], 0, []
        syl = 0
        for s in meta["segments"]:
            c = clip_stats(decode(d / s["file"]))
            st.append(c)
            gaps.append(c["longest_gap_s"])
            syl += syllables(s.get("spoken") or s.get("text") or "")
            if s.get("wer") is not None:
                wers.append(s["wer"])
            caps += bool(s.get("hit_cap"))
        speech = sum(c["speech_s"] for c in st)
        rtfs = [s["rtf"] for s in meta["segments"] if s.get("rtf")]
        rows.append({"label": meta["label"], "passage": meta["passage"], "clips": len(st),
                     "dur_s": round(sum(s.get("duration_ms", 0) for s in meta["segments"]) / 1000, 1),
                     "syl_per_speech_s": round(syl / speech, 2) if speech else None,
                     "max_gap_s": round(max(gaps), 2), "wer_mean": round(float(np.mean(wers)), 3) if wers else None,
                     "wer_max": round(max(wers), 3) if wers else None, "hit_cap": caps,
                     "flatness": round(float(np.median([c["flatness"] for c in st])), 4),
                     "hf_db": round(float(np.median([c["hf_db"] for c in st])), 1),
                     "clip_frac": round(max(c["clip_frac"] for c in st), 5),
                     "rtf_mean": round(float(np.mean(rtfs)), 2) if rtfs else None})
    h = list(rows[0])
    print("| " + " | ".join(h) + " |\n|" + "---|" * len(h))
    for r in rows:
        print("| " + " | ".join(str(r[k]) for k in h) + " |")
    if a.json:
        a.json.write_text(json.dumps(rows, indent=1))
    sys.exit(0)


if __name__ == "__main__":
    main()
