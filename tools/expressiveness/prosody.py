#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Prosody measurements and the expressiveness index (EI) for rendered passages.

Audio layout (made by render.py / fetch_human.py / `monotone`): <root>/<label>/<passage>/NN.webm + meta.json. The
clips of a passage are decoded and joined in order, as the player plays them (each clip already carries its boundary
pause), and measured as one programme; every clip is also measured on its own.

    uv run tools/expressiveness/prosody.py monotone <root>                  # make <root>/control_monotone from control_sentence
    uv run tools/expressiveness/prosody.py measure <root> --out results/baseline.json --md results/baseline.md --calibrate
    uv run tools/expressiveness/prosody.py measure <root> --out results/x.json --md results/x.md   # later runs: frozen calibration

Method (transcript-free, so TTS and human audio are measured identically):
- F0: Praat autocorrelation, 10 ms step, two passes (De Looze & Hirst): pass 1 at 60-700 Hz, pass 2 at
  [0.75 x q25, 1.5 x q75] of pass 1. Values in semitones relative to the passage median.
- Speech/silence: 25 ms RMS frames, 10 ms hop; speech when the frame is above max(p95 - 30 dB, p5 + 6 dB). Silent runs
  of at least 150 ms are pauses; speech islands under 60 ms inside silence count as silence. Speech runs between pauses
  are phrases (inter-pausal units, IPUs) and stand in for sentences. Leading and trailing silence is not a pause.
- Syllable nuclei (De Jong & Wempe 2009): Praat intensity peaks (min pitch 50 Hz) above q99 - 25 dB, a dip of at least
  1 dB on both sides between neighbouring peaks, voiced within 20 ms. Articulation rate of a phrase (>= 0.5 s) =
  nuclei / phrase duration. On the Kokoro baseline nuclei = 0.86 x text syllables (sd 0.05 over 12 passages).
- Per phrase (>= 0.5 s): F0 slope (Theil-Sen, st/s), F0 level (median st), loudness (mean speech-frame dB).
- With word timings (Kokoro sidecars) the same F0-slope, rate and pause measures are also computed per real sentence
  (`sentence_check`), to show how well the IPU proxy tracks sentences. Not used by the index.

Expressiveness index (EI), per passage: z_f = clip((x_f - mu_f) / sigma_f, -3, 3) for eight features, mu and sigma
frozen in calibration.json from the calibration pool (all 21 passages of the baseline run: 4 Kokoro voices x 3, the
two flat controls x 3, the 3 human references). Features, all oriented so that higher = more varied:
  pitch:  f0_std_st, f0_range_st (p95 - p5), f0_slope_std (std of per-phrase slopes), f0_level_std (std of per-phrase
          median F0, st)
  energy: energy_range_db (p90 - p10 of speech-frame RMS dB), energy_level_std (std of per-phrase mean dB)
  timing: pause_log_std (std of ln pause durations), rate_std (std of per-phrase syllables/s)
EI = 0.50 x pitch + 0.25 x energy + 0.25 x timing, each group the mean of its z values. 0 = the pool average; one
unit = one pool standard deviation. Pitch weighs half because F0 variation is the main cue listeners use for
liveliness; with equal thirds the F0-flattened control (-0.48) outscored bm_george (-0.67), which no listener would
agree with, because the control keeps af_heart's energy and pauses.
`ceiling_pct` = 100 x (EI - EI_monotone) / (EI_human - EI_monotone) with the means from the calibration run:
0 = the monotone control, 100 = the LibriVox human references.
"""
from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path

import imageio_ffmpeg
import numpy as np
import parselmouth

SR = 16_000
HOP_S = 0.010
WIN_S = 0.025
MIN_PAUSE_S = 0.150
MIN_ISLAND_S = 0.060
# Phrases shorter than this get no slope or rate (too few frames for a stable fit).
MIN_PHRASE_S = 0.5
# Tuned on the 12 Kokoro baseline passages against text syllable counts (dip 1.0/1.5/2.0 dB x 50/100 Hz x voicing
# check): 1.0 dB, 50 Hz, voiced gave nuclei = 0.86 x text syllables with the lowest spread (sd 0.05).
NUCLEUS_DIP_DB = 1.0
GROUPS = {"pitch": ["f0_std_st", "f0_range_st", "f0_slope_std", "f0_level_std"],
          "energy": ["energy_range_db", "energy_level_std"],
          "timing": ["pause_log_std", "rate_std"]}
WEIGHTS = {"pitch": 0.50, "energy": 0.25, "timing": 0.25}
FEATURES = [f for fs in GROUPS.values() for f in fs]
HERE = Path(__file__).resolve().parent
CALIBRATION = HERE / "calibration.json"
PAUSE_BINS_MS = [150, 300, 500, 800, 1200, 10_000]


# ------------------------------------------------------------------------------------------------------------ audio
def decode(path: Path, sr: int = SR) -> np.ndarray:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", "1",
           "-ar", str(sr), "-f", "f32le", "-"]
    return np.frombuffer(subprocess.run(cmd, capture_output=True, check=True).stdout, dtype=np.float32).copy()


def encode_webm(wave: np.ndarray, sr: int, path: Path, kbps: int = 32) -> None:
    cmd = [imageio_ffmpeg.get_ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-y", "-f", "f32le", "-ar", str(sr),
           "-ac", "1", "-i", "-", "-c:a", "libopus", "-b:a", f"{kbps}k", "-ar", "24000", str(path)]
    subprocess.run(cmd, input=wave.astype(np.float32).tobytes(), check=True)


def load_passage(d: Path) -> tuple[np.ndarray, list[tuple[int, int]], dict]:
    """Joined wave, (start, end) sample span of each clip, meta."""
    meta = json.loads((d / "meta.json").read_text())
    waves, spans, at = [], [], 0
    for seg in meta["segments"]:
        w = decode(d / seg["file"])
        waves.append(w)
        spans.append((at, at + w.size))
        at += w.size
    return np.concatenate(waves), spans, meta


# ---------------------------------------------------------------------------------------------------------- tracks
def f0_track(wave: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Times and F0 in Hz (NaN when unvoiced), two-pass range."""
    snd = parselmouth.Sound(wave.astype(np.float64), SR)
    p1 = snd.to_pitch_ac(time_step=HOP_S, pitch_floor=60.0, pitch_ceiling=700.0)
    f = p1.selected_array["frequency"]
    f = f[f > 0]
    if f.size < 20:
        floor, ceil = 60.0, 700.0
    else:
        q25, q75 = np.percentile(f, [25, 75])
        floor, ceil = max(40.0, 0.75 * q25), min(900.0, 1.5 * q75)
    p2 = snd.to_pitch_ac(time_step=HOP_S, pitch_floor=floor, pitch_ceiling=ceil)
    hz = p2.selected_array["frequency"].astype(float)
    hz[hz <= 0] = np.nan
    return p2.xs(), hz


def rms_db(wave: np.ndarray) -> np.ndarray:
    hop, win = int(HOP_S * SR), int(WIN_S * SR)
    n = max(1, 1 + (wave.size - win) // hop)
    idx = np.arange(win)[None, :] + hop * np.arange(n)[:, None]
    frames = wave[np.clip(idx, 0, wave.size - 1)]
    return 20 * np.log10(np.sqrt(np.mean(frames ** 2, axis=1)) + 1e-10)


def speech_mask(db: np.ndarray) -> np.ndarray:
    thr = max(np.percentile(db, 95) - 30.0, np.percentile(db, 5) + 6.0)
    m = db > thr
    m = _fill_runs(m, value=True, max_len=int(MIN_ISLAND_S / HOP_S))  # short speech islands -> silence
    return m


def _fill_runs(m: np.ndarray, value: bool, max_len: int) -> np.ndarray:
    """Flip interior runs of `value` no longer than max_len."""
    m = m.copy()
    runs = _runs(m == value)
    for s, e in runs:
        if e - s <= max_len and s > 0 and e < m.size:
            m[s:e] = not value
    return m


def _runs(b: np.ndarray) -> list[tuple[int, int]]:
    """[start, end) frame runs where b is True."""
    if b.size == 0:
        return []
    d = np.diff(np.concatenate([[0], b.astype(np.int8), [0]]))
    return list(zip(np.flatnonzero(d == 1), np.flatnonzero(d == -1)))


def segment_ipus(mask: np.ndarray) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    """IPUs and pauses as frame spans. Pauses: interior silent runs >= MIN_PAUSE_S; shorter gaps stay inside an IPU."""
    speech = _runs(mask)
    if not speech:
        return [], []
    min_pause = int(round(MIN_PAUSE_S / HOP_S))
    ipus, pauses = [list(speech[0])], []
    for s, e in speech[1:]:
        gap = s - ipus[-1][1]
        if gap >= min_pause:
            pauses.append((ipus[-1][1], s))
            ipus.append([s, e])
        else:
            ipus[-1][1] = e
    return [tuple(x) for x in ipus], pauses


def syllable_nuclei(wave: np.ndarray, hz_times: np.ndarray, hz: np.ndarray) -> np.ndarray:
    """Times (s) of syllable nuclei, De Jong & Wempe style."""
    snd = parselmouth.Sound(wave.astype(np.float64), SR)
    inten = snd.to_intensity(minimum_pitch=50.0, time_step=HOP_S)
    t, v = inten.xs(), inten.values[0]
    if v.size < 3:
        return np.array([])
    thr = np.percentile(v, 99) - 25.0
    peaks = [i for i in range(1, v.size - 1) if v[i] > v[i - 1] and v[i] >= v[i + 1] and v[i] > thr]
    kept: list[int] = []
    for i in peaks:
        if kept:
            dip = v[kept[-1]:i + 1].min()
            if v[i] - dip < NUCLEUS_DIP_DB or v[kept[-1]] - dip < NUCLEUS_DIP_DB:
                if v[i] > v[kept[-1]]:
                    kept[-1] = i  # same nucleus, keep the louder peak
                continue
        kept.append(i)
    out = []
    for i in kept:
        j = int(np.argmin(np.abs(hz_times - t[i]))) if hz_times.size else -1
        if j >= 0 and abs(hz_times[j] - t[i]) < 0.03 and np.isfinite(hz[max(0, j - 2):j + 3]).any():
            out.append(t[i])
    return np.array(out)


# --------------------------------------------------------------------------------------------------------- metrics
def _slope(ts: np.ndarray, st: np.ndarray) -> float | None:
    """Theil-Sen slope (median of pairwise slopes, st/s): robust to the odd octave error at a phrase edge."""
    ok = np.isfinite(st)
    if ok.sum() < 20 or ts[ok][-1] - ts[ok][0] < MIN_PHRASE_S * 0.6:
        return None
    x, y = ts[ok], st[ok]
    if x.size > 150:
        keep = np.linspace(0, x.size - 1, 150).astype(int)
        x, y = x[keep], y[keep]
    i, j = np.triu_indices(x.size, k=1)
    dx = x[j] - x[i]
    return float(np.median((y[j] - y[i])[dx > 0] / dx[dx > 0]))


def _std(xs) -> float | None:
    xs = [x for x in xs if x is not None and math.isfinite(x)]
    return float(np.std(xs, ddof=1)) if len(xs) >= 3 else None


def measure_wave(wave: np.ndarray, words: list | None = None, sentences: list | None = None) -> dict:
    times, hz = f0_track(wave)
    voiced = hz[np.isfinite(hz)]
    med = float(np.median(voiced)) if voiced.size else float("nan")
    st = 12 * np.log2(hz / med)
    stv = st[np.isfinite(st)]
    db = rms_db(wave)
    mask = speech_mask(db)
    ipus, pauses = segment_ipus(mask)
    nuclei = syllable_nuclei(wave, times, hz)
    speech_db = db[mask]
    pause_ms = np.array([(e - s) * HOP_S * 1000 for s, e in pauses])

    slopes, rates, levels, louds = [], [], [], []
    for s, e in ipus:
        t0, t1 = s * HOP_S, e * HOP_S
        dur = t1 - t0
        if dur < MIN_PHRASE_S:
            continue
        sel = (times >= t0) & (times < t1)
        slopes.append(_slope(times[sel], st[sel]))
        rates.append(float(((nuclei >= t0) & (nuclei < t1)).sum() / dur))
        v = st[sel][np.isfinite(st[sel])]
        if v.size >= 10:
            levels.append(float(np.median(v)))
        louds.append(float(np.mean(db[s:e][mask[s:e]])))
    speech_s = sum((e - s) for s, e in ipus) * HOP_S
    total_s = wave.size / SR
    m = {
        "duration_s": round(total_s, 3),
        "speech_s": round(speech_s, 3),
        "f0_median_hz": round(med, 2),
        "f0_std_st": _r(np.std(stv)) if stv.size > 20 else None,
        "f0_range_st": _r(np.percentile(stv, 95) - np.percentile(stv, 5)) if stv.size > 20 else None,
        "f0_slope_std": _r(_std(slopes)),
        "f0_slopes": [_r(x) for x in slopes],
        "energy_range_db": _r(np.percentile(speech_db, 90) - np.percentile(speech_db, 10)) if speech_db.size else None,
        "f0_level_std": _r(_std(levels)),
        "energy_level_std": _r(_std(louds)),
        "ipu_count": len(ipus),
        "pause_count": int(pause_ms.size),
        "pauses_per_min": _r(pause_ms.size / total_s * 60) if total_s else None,
        "pause_ms": {k: _r(v) for k, v in _dist(pause_ms).items()},
        "pause_hist": _hist(pause_ms),
        "pause_log_std": _r(np.std(np.log(pause_ms / 1000), ddof=1)) if pause_ms.size >= 3 else None,
        "syllable_nuclei": int(nuclei.size),
        "artic_rate": _r(nuclei.size / speech_s) if speech_s else None,
        "rate_std": _r(_std(rates)),
        "rates": [_r(x) for x in rates],
    }
    return m


def sentence_check(meta: dict, spans: list[tuple[int, int]], wave: np.ndarray) -> dict | None:
    """Same slope/rate/pause measures over real sentences, from word timings ([start_ms, end_ms, c0, c1] per clip)."""
    sents = []  # (t0, t1, syllables) in joined-programme seconds
    for seg, (a, _) in zip(meta["segments"], spans):
        words = seg.get("words") or []
        if not words or not seg.get("sentences"):
            return None
        off = a / SR
        for c0, c1 in seg["sentences"]:
            ws = [w for w in words if w[2] >= c0 and w[3] <= c1]
            if not ws:
                continue
            sents.append((off + ws[0][0] / 1000, off + ws[-1][1] / 1000, text_syllables(seg["text"][c0:c1])))
    if len(sents) < 3:
        return None
    times, hz = f0_track(wave)
    st = 12 * np.log2(hz / np.nanmedian(hz))
    mask = speech_mask(rms_db(wave))
    slopes, rates, gaps = [], [], []
    for i, (t0, t1, syl) in enumerate(sents):
        sel = (times >= t0) & (times < t1)
        slopes.append(_slope(times[sel], st[sel]))
        f0, f1 = int(t0 / HOP_S), int(t1 / HOP_S)
        spoken = mask[f0:f1].sum() * HOP_S  # exclude internal silences
        if spoken > 0.3:
            rates.append(syl / spoken)
        if i + 1 < len(sents):
            gaps.append((sents[i + 1][0] - t1) * 1000)
    gaps = np.array(gaps)
    return {"sentences": len(sents), "f0_slope_std": _r(_std(slopes)), "rate_std": _r(_std(rates)),
            "rate_mean": _r(np.mean(rates)) if rates else None,
            "gap_ms": {k: _r(v) for k, v in _dist(gaps).items()}}


VOWELS = re.compile(r"[aeiouy]+")


def text_syllables(text: str) -> int:
    n = 0
    for w in re.findall(r"[a-z']+", text.lower()):
        w = w.replace("'", "")
        k = len(VOWELS.findall(w))
        if w.endswith("e") and not w.endswith(("le", "ee", "ye")) and k > 1:
            k -= 1
        if w.endswith("ed") and len(w) > 3 and w[-3] not in "td" and k > 1:
            k -= 1
        n += max(1, k)
    return n


def _dist(x: np.ndarray) -> dict:
    if x.size == 0:
        return {"n": 0}
    return {"n": int(x.size), "mean": float(x.mean()), "median": float(np.median(x)), "std": float(x.std()),
            "p10": float(np.percentile(x, 10)), "p90": float(np.percentile(x, 90)), "min": float(x.min()),
            "max": float(x.max())}


def _hist(ms: np.ndarray) -> dict:
    edges = PAUSE_BINS_MS
    return {f"{edges[i]}-{edges[i + 1]}": int(((ms >= edges[i]) & (ms < edges[i + 1])).sum())
            for i in range(len(edges) - 1)}


def _r(x, n: int = 3):
    return None if x is None or not math.isfinite(float(x)) else round(float(x), n)


# ------------------------------------------------------------------------------------------------------------ index
def calibrate(rows: list[dict]) -> dict:
    cal = {"features": FEATURES, "groups": GROUPS, "weights": WEIGHTS, "pool": [f"{r['label']}/{r['passage']}" for r in rows], "mu": {}, "sigma": {}}
    for f in FEATURES:
        xs = np.array([r["metrics"][f] for r in rows if r["metrics"].get(f) is not None])
        cal["mu"][f], cal["sigma"][f] = float(xs.mean()), float(xs.std(ddof=1)) or 1.0
    return cal


def index(metrics: dict, cal: dict) -> tuple[float | None, dict]:
    z = {}
    for f in cal["features"]:
        x = metrics.get(f)
        if x is not None:
            z[f] = round(float(np.clip((x - cal["mu"][f]) / cal["sigma"][f], -3, 3)), 3)
    groups, weights = cal["groups"], cal["weights"]
    if not all(any(f in z for f in fs) for fs in groups.values()):
        return None, z
    ei = sum(weights[g] * np.mean([z[f] for f in fs if f in z]) for g, fs in groups.items())
    return round(float(ei), 3), z


def anchors(rows: list[dict]) -> dict:
    by = {}
    for r in rows:
        if r["index"] is not None:
            by.setdefault(r["label"], []).append(r["index"])
    return {k: round(float(np.mean(v)), 3) for k, v in by.items() if k in ("human", "control_monotone")}


# ------------------------------------------------------------------------------------------------------------ main
def cmd_monotone(root: Path) -> None:
    src = root / "control_sentence"
    for d in sorted(p for p in src.iterdir() if (p / "meta.json").exists()):
        meta = json.loads((d / "meta.json").read_text())
        out = root / "control_monotone" / d.name
        out.mkdir(parents=True, exist_ok=True)
        joined = np.concatenate([decode(d / s["file"]) for s in meta["segments"]])
        med = float(np.nanmedian(f0_track(joined)[1]))
        for seg in meta["segments"]:
            snd = parselmouth.Sound(decode(d / seg["file"]).astype(np.float64), SR)
            manip = parselmouth.praat.call(snd, "To Manipulation", HOP_S, 60, 600)
            tier = parselmouth.praat.call(manip, "Extract pitch tier")
            parselmouth.praat.call(tier, "Remove points between", 0, snd.duration)
            parselmouth.praat.call(tier, "Add point", snd.duration / 2, med)
            parselmouth.praat.call([manip, tier], "Replace pitch tier")
            flat = parselmouth.praat.call(manip, "Get resynthesis (overlap-add)")
            encode_webm(np.asarray(flat.values[0], dtype=np.float32), SR, out / seg["file"])
        meta.update(label="control_monotone", voice="kokoro:af_heart (F0 flattened to median by Praat PSOLA)")
        (out / "meta.json").write_text(json.dumps(meta, indent=1))
        print(f"control_monotone/{d.name}: F0 flattened to {med:.1f} Hz")


ORDER = ["af_heart", "bf_emma", "am_michael", "bm_george", "control_sentence", "control_monotone", "human"]


def cmd_measure(root: Path, out: Path, md: Path | None, do_cal: bool) -> None:
    rows = []
    dirs = sorted((p for p in root.glob("*/*") if (p / "meta.json").exists()),
                  key=lambda p: (ORDER.index(p.parent.name) if p.parent.name in ORDER else 99, p.name))
    for d in dirs:
        wave, spans, meta = load_passage(d)
        metrics = measure_wave(wave)
        clips = []
        for seg, (a, b) in zip(meta["segments"], spans):
            cm = measure_wave(wave[a:b])
            clips.append({"file": seg["file"], "text": seg.get("text"),
                          **{k: cm[k] for k in ("duration_s", "f0_median_hz", "f0_std_st", "f0_range_st",
                                                "f0_slope_std", "energy_range_db", "pause_count", "ipu_count",
                                                "artic_rate", "rate_std")}})
        text = " ".join(s.get("text") or "" for s in meta["segments"])
        row = {"label": meta["label"], "voice": meta["voice"], "passage": meta["passage"], "metrics": metrics,
               "text_syllables": text_syllables(text) if text.strip() else None,
               "sentence_check": sentence_check(meta, spans, wave), "clips": clips}
        if meta.get("source"):
            row["source"] = {"url": meta["source"], "start_s": meta["start_s"], "description": meta["description"]}
        rows.append(row)
        print(f"{meta['label']}/{meta['passage']}: f0sd {metrics['f0_std_st']} st, range {metrics['f0_range_st']} st, "
              f"pauses {metrics['pause_count']}, nuclei {metrics['syllable_nuclei']} vs text {row['text_syllables']}",
              file=sys.stderr)
    if do_cal:
        cal = calibrate(rows)
    else:
        cal = json.loads(CALIBRATION.read_text())
    for r in rows:
        r["index"], r["z"] = index(r["metrics"], cal)
    if do_cal:
        cal["anchors"] = anchors(rows)
        CALIBRATION.write_text(json.dumps(cal, indent=1))
    a = cal.get("anchors", {})
    lo, hi = a.get("control_monotone"), a.get("human")
    for r in rows:
        r["ceiling_pct"] = (round(100 * (r["index"] - lo) / (hi - lo), 1)
                            if None not in (r["index"], lo, hi) and hi != lo else None)
    result = {"tool": "tools/expressiveness/prosody.py", "definition": (__doc__ or "").strip(),
              "calibration": cal, "passages": rows, "voices": summarise(rows)}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1))
    if md:
        md.write_text(table(rows, result["voices"]))
    print(table(rows, result["voices"]))


def summarise(rows: list[dict]) -> dict:
    out = {}
    for label in dict.fromkeys(r["label"] for r in rows):
        rs = [r for r in rows if r["label"] == label]
        s = {f: _r(np.mean([r["metrics"][f] for r in rs if r["metrics"].get(f) is not None]))
             for f in FEATURES + ["f0_median_hz", "pauses_per_min", "artic_rate"]}
        s["index"] = _r(np.mean([r["index"] for r in rs if r["index"] is not None]))
        s["ceiling_pct"] = _r(np.mean([r["ceiling_pct"] for r in rs if r["ceiling_pct"] is not None]), 1)
        out[label] = s
    return out


def table(rows: list[dict], voices: dict) -> str:
    h = ("| voice | passage | F0 med Hz | F0 sd st | F0 range st | slope sd st/s | F0 level sd st | energy p90-p10 dB "
         "| energy level sd dB | pauses | pause ms med (p10-p90) | ln-pause sd | syl/s | rate sd | EI | % of human |")
    lines = [h, "|" + "---|" * 16]
    for r in rows:
        m, p = r["metrics"], r["metrics"]["pause_ms"]
        pm = f"{p.get('median', 0):.0f} ({p.get('p10', 0):.0f}-{p.get('p90', 0):.0f})" if p.get("n") else "-"
        lines.append(f"| {r['label']} | {r['passage']} | {_f(m['f0_median_hz'], 0)} | {_f(m['f0_std_st'])} | "
                     f"{_f(m['f0_range_st'])} | {_f(m['f0_slope_std'])} | {_f(m['f0_level_std'], 2)} | "
                     f"{_f(m['energy_range_db'])} | {_f(m['energy_level_std'], 2)} | {m['pause_count']} | {pm} | "
                     f"{_f(m['pause_log_std'], 2)} | {_f(m['artic_rate'])} | {_f(m['rate_std'], 2)} | "
                     f"{_f(r['index'], 2)} | {_f(r['ceiling_pct'], 0)} |")
    lines += ["", "| voice (mean of 3 passages) | F0 sd st | F0 range st | slope sd | F0 level sd | energy dB | "
                  "energy level sd | ln-pause sd | rate sd | EI | % of human |", "|" + "---|" * 11]
    for k, s in voices.items():
        lines.append(f"| {k} | {_f(s['f0_std_st'])} | {_f(s['f0_range_st'])} | {_f(s['f0_slope_std'])} | "
                     f"{_f(s['f0_level_std'], 2)} | {_f(s['energy_range_db'])} | {_f(s['energy_level_std'], 2)} | "
                     f"{_f(s['pause_log_std'], 2)} | {_f(s['rate_std'], 2)} | "
                     f"{_f(s['index'], 2)} | {_f(s['ceiling_pct'], 0)} |")
    return "\n".join(lines) + "\n"


def _f(x, n: int = 1) -> str:
    return "-" if x is None else f"{x:.{n}f}"


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("monotone")
    m.add_argument("root", type=Path)
    s = sub.add_parser("measure")
    s.add_argument("root", type=Path)
    s.add_argument("--out", type=Path, required=True)
    s.add_argument("--md", type=Path)
    s.add_argument("--calibrate", action="store_true", help="recompute and freeze calibration.json from this run")
    a = ap.parse_args()
    if a.cmd == "monotone":
        cmd_monotone(a.root)
    else:
        cmd_measure(a.root, a.out, a.md, a.calibrate)


if __name__ == "__main__":
    main()
