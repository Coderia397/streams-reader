#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Artefact and content checks for rendered passages (transcript-aware, unlike prosody.py), per label.

    uv run tools/expressiveness/bakeoff/indextts25/artefacts.py <audio root> [<root> ...] --out results/artefacts.json

Per label (mean over passages / clips):
- wer: word error rate of the Whisper small.en transcript in meta (segments[].asr) against the text, after lowercasing,
  dropping punctuation and spelling out 0-20. Needs `asr` in meta (the Kaggle worker adds it); else None.
- syl_per_s: text syllables / speech seconds (pauses >= 150 ms excluded), the speaking rate a listener hears.
- hnr_db: Praat harmonicity (cc, 10 ms, 75 Hz) mean over voiced frames: lower = breathier / noisier / buzzier voice.
- f0_jumps_per_min: voiced-frame steps of more than 8 st between 10 ms frames (octave errors, cracks, glitches).
- long_gaps: pauses inside one clip longer than 1.2 s (the model stalling mid-sentence).
- hf_ratio_db: energy 5-11 kHz minus energy 0.3-5 kHz (dB) on speech frames: very low = muffled, high = hiss.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import numpy as np
import parselmouth

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import prosody as P  # noqa: E402

NUM = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen " \
      "seventeen eighteen nineteen twenty".split()


def words(s: str) -> list[str]:
    s = s.lower().replace("’", "'")
    s = re.sub(r"\b(\d+)\b", lambda m: NUM[int(m.group(1))] if int(m.group(1)) <= 20 else m.group(1), s)
    return re.findall(r"[a-z0-9']+", s)


def wer(ref: str, hyp: str) -> tuple[int, int]:
    r, h = words(ref), words(hyp)
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
            prev, d[j] = d[j], cur
    return d[len(h)], len(r)


def clip_stats(wave: np.ndarray) -> dict:
    db = P.rms_db(wave)
    mask = P.speech_mask(db)
    ipus, pauses = P.segment_ipus(mask)
    speech_s = sum(b - a for a, b in ipus) * P.HOP_S
    snd = parselmouth.Sound(wave.astype(np.float64), sampling_frequency=P.SR)
    hnr = snd.to_harmonicity_cc(time_step=0.01, minimum_pitch=75).values[0]
    hnr = hnr[hnr > -100]
    _, hz = P.f0_track(wave)
    st = 12 * np.log2(np.where(hz > 0, hz, np.nan) / 100)
    steps = np.abs(np.diff(st))
    jumps = int(np.nansum(steps > 8))
    return {"speech_s": speech_s, "hnr": float(np.mean(hnr)) if hnr.size else None, "jumps": jumps,
            "gaps": sum(1 for a, b in pauses if (b - a) * P.HOP_S > 1.2), "dur_s": wave.size / P.SR}


def hf_ratio(path: Path) -> float | None:
    w = P.decode(path, 24_000)
    n = 1024
    if w.size < n:
        return None
    frames = np.lib.stride_tricks.sliding_window_view(w, n)[::256] * np.hanning(n)
    spec = np.abs(np.fft.rfft(frames, axis=1)) ** 2
    f = np.fft.rfftfreq(n, 1 / 24_000)
    lo = spec[:, (f >= 300) & (f < 5000)].sum(1)
    hi = spec[:, (f >= 5000) & (f < 11000)].sum(1)
    keep = lo > np.percentile(lo, 50)
    return float(10 * np.log10(hi[keep].sum() / lo[keep].sum()))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("roots", type=Path, nargs="+")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--only", default="", help="regex on labels")
    a = ap.parse_args()
    out = {}
    for root in a.roots:
        for d in sorted(p for p in root.glob("*/*") if (p / "meta.json").exists()):
            meta = json.loads((d / "meta.json").read_text())
            label = meta["label"]
            if a.only and not re.search(a.only, label):
                continue
            acc = out.setdefault(label, {"err": 0, "nref": 0, "asr": 0, "syl": 0, "speech_s": 0.0, "hnr": [],
                                         "jumps": 0, "gaps": 0, "dur_s": 0.0, "hf": [], "worst": []})
            for seg in meta["segments"]:
                w = P.decode(d / seg["file"])
                s = clip_stats(w)
                text = seg.get("text") or ""
                if text:
                    acc["syl"] += P.text_syllables(text)
                acc["speech_s"] += s["speech_s"]
                acc["dur_s"] += s["dur_s"]
                acc["jumps"] += s["jumps"]
                acc["gaps"] += s["gaps"]
                if s["hnr"] is not None:
                    acc["hnr"].append(s["hnr"])
                hf = hf_ratio(d / seg["file"])
                if hf is not None:
                    acc["hf"].append(hf)
                if seg.get("asr") is not None and text:
                    e, n = wer(text, seg["asr"])
                    acc["err"] += e
                    acc["nref"] += n
                    acc["asr"] += 1
                    if e:
                        acc["worst"].append({"clip": f"{meta['passage']}/{seg['file']}", "errors": e, "words": n,
                                             "text": text, "asr": seg["asr"]})
            print(label, meta["passage"], file=sys.stderr)
    res = {}
    for label, acc in out.items():
        res[label] = {
            "wer": round(acc["err"] / acc["nref"], 4) if acc["nref"] else None,
            "syl_per_s": round(acc["syl"] / acc["speech_s"], 2) if acc["speech_s"] and acc["syl"] else None,
            "hnr_db": round(float(np.mean(acc["hnr"])), 2) if acc["hnr"] else None,
            "f0_jumps_per_min": round(acc["jumps"] / (acc["speech_s"] / 60), 2) if acc["speech_s"] else None,
            "long_gaps": acc["gaps"], "audio_s": round(acc["dur_s"], 1),
            "hf_ratio_db": round(float(np.mean(acc["hf"])), 2) if acc["hf"] else None,
            "worst": sorted(acc["worst"], key=lambda x: -x["errors"])[:8]}
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(res, indent=1, ensure_ascii=False))
    print("| label | WER | syl/s | HNR dB | F0 jumps/min | gaps >1.2 s | HF ratio dB | audio s |")
    print("|---|---|---|---|---|---|---|---|")
    for k, v in res.items():
        print(f"| {k} | {v['wer']} | {v['syl_per_s']} | {v['hnr_db']} | {v['f0_jumps_per_min']} | {v['long_gaps']} | "
              f"{v['hf_ratio_db']} | {v['audio_s']} |")


if __name__ == "__main__":
    main()
