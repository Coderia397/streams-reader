#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "soundfile>=0.12", "pyloudnorm==0.1.1", "imageio-ffmpeg"]
# ///
"""Pick the two clone references for the Chatterbox bake-off from the LibriVox readings that fetch_human.py uses.

    uv run tools/expressiveness/bakeoff/chatterbox/select_ref.py --out <work>/dataset

The source MP3 (64 kbps) is decoded straight to 24 kHz (not the 32 kbps Opus excerpt). Inside the same 75 s window
that fetch_human.py cuts, the calmest stretch is chosen: 10-12.5 s, starting and ending in a pause of >= 250 ms,
no interior pause over 900 ms, lowest F0 sd (st). Then the speech span is trimmed, set to -20 LUFS and peak <= -1.5
dBFS (as nolabs_engine/reference.py prepares a user reference). Writes ref_<voice>.wav and refs.json.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import pyloudnorm
import soundfile as sf

EXPR = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(EXPR))
import fetch_human  # noqa: E402
import prosody  # noqa: E402

SR = 24_000
# voice -> fetch_human passage key (reader): F0 medians 240 Hz (Austen reader) and 105 Hz (Walden reader).
VOICES = {"female": "2-dialogue", "male": "3-reflective"}
MIN_S, MAX_S = 10.0, 12.5


def load(url: str, start: float, length: float) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-ss", str(start), "-t",
           str(length), "-i", url, "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32).copy()


def pick(wave24: np.ndarray) -> tuple[float, float, float]:
    n16 = int(wave24.size * prosody.SR / SR)
    w16 = np.interp(np.linspace(0, wave24.size - 1, n16), np.arange(wave24.size), wave24).astype(np.float32)
    db = prosody.rms_db(w16)
    ipus, pauses = prosody.segment_ipus(prosody.speech_mask(db))
    t, hz = prosody.f0_track(w16)
    h = prosody.HOP_S
    long_p = [(s * h, e * h) for s, e in pauses if (e - s) * h >= 0.25]
    best = None
    for i, (_, a) in enumerate(long_p):  # start at the end of a pause
        for b, _ in long_p[i + 1:]:      # end at the start of a later pause
            dur = b - a
            if dur < MIN_S:
                continue
            if dur > MAX_S:
                break
            inner = [(e - s) * h for s, e in pauses if s * h > a and e * h < b]
            if inner and max(inner) > 0.9:
                continue
            sel = (t >= a) & (t <= b) & np.isfinite(hz)
            if sel.sum() < 200:
                continue
            st = 12 * np.log2(hz[sel] / np.median(hz[sel]))
            sd = float(np.std(st))
            if best is None or sd < best[2]:
                best = (a, b, sd)
    if best is None:
        raise SystemExit("no window found")
    return best


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    refs = {}
    for voice, passage in VOICES.items():
        item, name, start, what = fetch_human.REFERENCES[passage]
        url = f"{fetch_human.IA}/{item}/{name}"
        w = load(url, start, fetch_human.LENGTH_S)
        s0, s1, sd = pick(w)
        seg = w[int((s0 - 0.08) * SR):int((s1 + 0.15) * SR)]
        gain = -20.0 - pyloudnorm.Meter(SR).integrated_loudness(seg.astype(np.float64))
        seg = seg * 10 ** (gain / 20)
        seg *= min(1.0, 10 ** (-1.5 / 20) / max(1e-9, float(np.abs(seg).max())))
        f = f"ref_{voice}.wav"
        sf.write(a.out / f, seg.astype(np.float32), SR, subtype="PCM_16")
        hz = prosody.f0_track(np.interp(np.linspace(0, seg.size - 1, int(seg.size * prosody.SR / SR)),
                                        np.arange(seg.size), seg).astype(np.float32))[1]
        refs[voice] = {"file": f, "source": url, "reader_of": what, "start_s": round(start + s0 - 0.08, 2),
                       "length_s": round(seg.size / SR, 2), "f0_sd_st": round(sd, 2),
                       "f0_median_hz": round(float(np.nanmedian(hz)), 1), "gain_db": round(gain, 1)}
        print(voice, refs[voice])
    (a.out / "refs.json").write_text(json.dumps(refs, indent=1))


if __name__ == "__main__":
    main()
