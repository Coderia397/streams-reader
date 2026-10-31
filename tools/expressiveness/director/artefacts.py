#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Artefact checks for renders made by render_local.py (they log every edit in meta.json).

    uv run tools/expressiveness/director/artefacts.py <root> [--out results/x.json]

Per condition (mean over passages):
- timbre_jump_db: log-mel spectral distance (RMS over 40 bands, dB) between the long-term spectrum of the last 0.4 s
  of speech before each call join and the first 0.4 s after it. A voice or blend change shows here. k_plain has no
  joins inside a segment, so the reference is the same distance across its natural sentence gaps (`natural_jump_db`).
- click_ratio: largest |sample step| within ±3 ms of an edit point (call edges, inserted silence), divided by the
  99.9th percentile step of the clip. Above 1 = a step sharper than anything in the speech.
- loud_cut: share of in-call silence insertions made where the signal was above -45 dBFS (not silent).
- abrupt_edges: share of call edges whose first/last 10 ms are within 15 dB of the call's RMS (cut mid-sound).
- pitch_breaks_per_min: consecutive voiced 10 ms frames whose F0 differs by more than 4 semitones (vocoder or F0
  manipulation glitches), per minute of voiced speech.
- clipped: model samples at |x| >= 0.999 before levelling; ceiling_db: gain the peak ceiling took away.
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import parselmouth

SR = 24_000


def decode(path: Path) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", "1",
           "-ar", str(SR), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32)


def mel_fb(n_fft=512, n_mels=40, sr=SR, fmin=60, fmax=8000) -> np.ndarray:
    def hz2mel(f):
        return 2595 * np.log10(1 + f / 700)
    mels = np.linspace(hz2mel(fmin), hz2mel(fmax), n_mels + 2)
    hz = 700 * (10 ** (mels / 2595) - 1)
    bins = np.floor((n_fft + 1) * hz / sr).astype(int)
    fb = np.zeros((n_mels, n_fft // 2 + 1))
    for m in range(1, n_mels + 1):
        a, b, c = bins[m - 1], bins[m], bins[m + 1]
        fb[m - 1, a:b] = (np.arange(a, b) - a) / max(1, b - a)
        fb[m - 1, b:c] = (c - np.arange(b, c)) / max(1, c - b)
    return fb


FB = mel_fb()


def ltas_db(x: np.ndarray) -> np.ndarray | None:
    """Long-term log-mel spectrum of the speech frames of x."""
    n, hop = 512, 240
    if x.size < n * 2:
        return None
    frames = np.lib.stride_tricks.sliding_window_view(x, n)[::hop] * np.hanning(n)
    e = np.sqrt(np.mean(frames ** 2, axis=1))
    keep = e > (e.max() * 10 ** (-30 / 20))
    if keep.sum() < 5:
        return None
    p = np.abs(np.fft.rfft(frames[keep], axis=1)) ** 2
    return 10 * np.log10(FB @ p.mean(axis=0) + 1e-10)


def speech_runs(x: np.ndarray, thr_db=-45.0) -> np.ndarray:
    hop = 240
    e = np.array([np.sqrt(np.mean(x[i:i + hop] ** 2)) for i in range(0, x.size - hop, hop)])
    return 20 * np.log10(e + 1e-9) > thr_db


def side(x: np.ndarray, pos: int, before: bool, span_s=0.4) -> np.ndarray:
    """Up to span_s of speech next to pos (skipping silence)."""
    hop = 240
    sp = speech_runs(x)
    want, idx = int(span_s * SR / hop), []
    i = pos // hop
    rng = range(i - 1, -1, -1) if before else range(i, sp.size)
    for j in rng:
        if sp[j]:
            idx.append(j)
            if len(idx) >= want:
                break
    if not idx:
        return np.zeros(0, np.float32)
    lo, hi = min(idx), max(idx) + 1
    return x[lo * hop:hi * hop]


def jump_db(x: np.ndarray, pos: int) -> float | None:
    a, b = ltas_db(side(x, pos, True)), ltas_db(side(x, pos, False))
    if a is None or b is None:
        return None
    d = (a - a.mean()) - (b - b.mean())    # spectral shape only, level removed
    return float(np.sqrt(np.mean(d ** 2)))


def pitch_breaks(x: np.ndarray) -> tuple[int, float]:
    snd = parselmouth.Sound(x.astype(np.float64), SR)
    p = snd.to_pitch_ac(time_step=0.01, pitch_floor=60, pitch_ceiling=600)
    f = p.selected_array["frequency"]
    v = f > 0
    st = 12 * np.log2(np.where(v, f, 1) / 100)
    both = v[1:] & v[:-1]
    jumps = int(np.sum(both & (np.abs(np.diff(st)) > 4)))
    return jumps, float(v.sum() * 0.01)


def check_passage(d: Path) -> dict:
    meta = json.loads((d / "meta.json").read_text())
    jumps, nat, clicks, loud, abrupt, edges, cuts = [], [], [], [], 0, 0, 0
    breaks, voiced = 0, 0.0
    for seg in meta["segments"]:
        x = decode(d / seg["file"])
        step = np.abs(np.diff(x))
        ref = np.percentile(step, 99.9) + 1e-9
        b, vs = pitch_breaks(x)
        breaks, voiced = breaks + b, voiced + vs
        ed = seg.get("edits", [])
        starts = [e for e in ed if e["kind"] == "call_start"]
        for e in ed:
            pos = int(e["pos"])
            w = x[max(0, pos - 72):pos + 72]
            if w.size > 1:
                clicks.append(float(np.abs(np.diff(w)).max() / ref))
            if e["kind"] in ("call_start", "call_end"):
                edges += 1
                abrupt += e["edge_db"] > -15
            if e["kind"] == "gap_insert":
                cuts += 1
                loud.append(e["cut_dbfs"] > -45)
                j = jump_db(x, pos)
                if j is not None:
                    nat.append(j)
        for e in starts[1:]:
            j = jump_db(x, int(e["pos"]))
            if j is not None:
                jumps.append(j)
    calls = meta.get("calls", [])
    return {"joins": len(jumps), "timbre_jump_db": float(np.mean(jumps)) if jumps else None,
            "timbre_jump_p90_db": float(np.percentile(jumps, 90)) if jumps else None,
            "gap_inserts": cuts, "gap_jump_db": float(np.mean(nat)) if nat else None,
            "click_ratio_max": float(max(clicks)) if clicks else None,
            "clicks_over_1": int(sum(c > 1 for c in clicks)),
            "loud_cut": float(np.mean(loud)) if loud else 0.0,
            "abrupt_edges": abrupt / edges if edges else 0.0,
            "pitch_breaks_per_min": 60 * breaks / voiced if voiced else None,
            "clipped": int(sum(c.get("raw_clipped", 0) for c in calls)),
            "raw_peak": max((c.get("raw_peak", 0) for c in calls), default=None),
            "ceiling_db": max((s.get("ceiling_limited_db", 0) for s in meta["segments"]), default=0.0)}


def natural_gaps(d: Path) -> list[float]:
    """Spectral distance across Kokoro's own sentence gaps inside one call (k_plain reference)."""
    meta = json.loads((d / "meta.json").read_text())
    out = []
    for seg in meta["segments"]:
        x = decode(d / seg["file"])
        sp = speech_runs(x)
        # silent runs of 150 ms or more inside the clip
        run = 0
        for j, s in enumerate(sp):
            if not s:
                run += 1
            else:
                if run >= 15 and j - run > 0:
                    v = jump_db(x, (j - run // 2) * 240)
                    if v is not None:
                        out.append(v)
                run = 0
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("root", type=Path)
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    rows = {}
    for cdir in sorted(p for p in a.root.iterdir() if p.is_dir()):
        per = {}
        for d in sorted(p for p in cdir.iterdir() if (p / "meta.json").exists()):
            per[d.name] = check_passage(d)
            ng = natural_gaps(d)
            per[d.name]["pause_jump_db"] = float(np.mean(ng)) if ng else None
        keys = per[next(iter(per))].keys()
        mean = {}
        for k in keys:
            vals = [v[k] for v in per.values() if v[k] is not None]
            mean[k] = round(float(np.mean(vals)), 3) if vals else None
        for k in ("clipped", "joins", "gap_inserts", "clicks_over_1"):
            mean[k] = int(sum(v[k] for v in per.values()))
        rows[cdir.name] = {"mean": mean, "passages": per}
        m = mean
        print(f"{cdir.name:14} joins {m['joins']:3} timbre {m['timbre_jump_db']} (p90 {m['timbre_jump_p90_db']}) "
              f"pause-jump {m['pause_jump_db']} clicks>1 {m['clicks_over_1']} max {m['click_ratio_max']} "
              f"loud_cut {m['loud_cut']} abrupt {m['abrupt_edges']} breaks/min {m['pitch_breaks_per_min']} "
              f"clipped {m['clipped']} peak {m['raw_peak']} ceiling {m['ceiling_db']}")
    if a.out:
        a.out.write_text(json.dumps(rows, indent=1))


if __name__ == "__main__":
    main()
