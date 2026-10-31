#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "soundfile>=0.12", "imageio-ffmpeg"]
# ///
"""Summarise the Fish bake-off: prosody index (prosody.py measure output), RTF and VRAM (kernel logs), Whisper WER and
tag leakage, UTMOS22, and signal checks per clip. Writes --md and --out.

    uv run tools/expressiveness/bakeoff/fish/analyze.py --work W --prosody results/fish.json \
        --compare <dir with score__<label>__<passage>__NN.webm> --md results/summary.md --out results/summary.json

Signal checks, on the raw 24 kHz model output (before the pipeline), the same functions as prosody.py:
- sec/syl: clip seconds per text syllable, against the median of the label (> 1.6x stretched or babbling, < 0.6x
  truncated);
- longest interior pause (> 1.2 s flagged) and peak (>= 0.99 = clipping);
- F0 jumps: share of consecutive voiced 10 ms frames whose F0 moves more than 7 semitones (octave errors, creak,
  glitches); and voiced share of speech frames (breathy / noisy output lowers it);
- spectral flatness (median over speech frames, 0-1; buzzy or noisy vocoding raises it) and the share of energy above
  6 kHz (muffled output lowers it).
--compare runs the same checks on reference clips (Kokoro af_heart and the LibriVox human excerpts) for scale.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

HERE = Path(__file__).resolve().parent
EXPR = HERE.parents[1]
sys.path.insert(0, str(EXPR))
import prosody  # noqa: E402

TAG_WORDS = {"emphasis", "slowly", "quickly", "fearful", "cheerful", "angry", "sad", "disgusted", "surprised",
             "inhale", "chuckle", "sigh", "hesitating", "slightly", "very"}
NUM = {"7": "seven", "11": "eleven", "2": "two"}


def norm_words(s: str) -> list[str]:
    s = re.sub(r"\[[^\]]*\]", " ", s.lower())
    s = s.replace("’", "'")
    words = re.findall(r"[a-z0-9']+", s)
    return [NUM.get(w, w) for w in words]


def wer(ref: list[str], hyp: list[str]) -> tuple[int, int]:
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
            prev, d[j] = d[j], cur
    return d[len(hyp)], len(ref)


def signal(wave: np.ndarray, sr: int) -> dict:
    w16 = wave if sr == prosody.SR else np.interp(np.linspace(0, wave.size - 1, int(wave.size * prosody.SR / sr)),
                                                  np.arange(wave.size), wave).astype(np.float32)
    db = prosody.rms_db(w16)
    mask = prosody.speech_mask(db)
    ipus, pauses = prosody.segment_ipus(mask)
    t, hz = prosody.f0_track(w16)
    voiced = np.isfinite(hz) & (hz > 0)
    st = 12 * np.log2(np.where(voiced, hz, 1.0))
    both = voiced[1:] & voiced[:-1]
    jumps = float(np.mean(np.abs(np.diff(st))[both] > 7)) if both.sum() else None
    fi = np.clip((t / prosody.HOP_S).astype(int), 0, mask.size - 1)
    sp = mask[fi]
    voiced_share = float(voiced[sp].mean()) if sp.sum() else None
    n = 1024
    frames = np.lib.stride_tricks.sliding_window_view(wave, n)[:: n // 2] * np.hanning(n)
    spec = np.abs(np.fft.rfft(frames, axis=1)) ** 2 + 1e-12
    e = 10 * np.log10(spec.sum(1))
    sel = e > e.max() - 30
    flat = np.exp(np.mean(np.log(spec[sel]), 1)) / np.mean(spec[sel], 1)
    freqs = np.fft.rfftfreq(n, 1 / sr)
    hf = spec[sel][:, freqs >= 6000].sum() / spec[sel].sum()
    inner = [(e2 - s) * prosody.HOP_S for s, e2 in pauses]
    return {"dur_s": round(wave.size / sr, 3), "peak": round(float(np.abs(wave).max()), 3),
            "max_pause_s": round(max(inner), 2) if inner else 0.0, "f0_jump_pct": None if jumps is None else
            round(100 * jumps, 2), "voiced_pct": None if voiced_share is None else round(100 * voiced_share, 1),
            "flatness": round(float(np.median(flat)), 4), "hf6k_pct": round(100 * float(hf), 2)}


def mean(xs):
    xs = [x for x in xs if x is not None]
    return round(float(np.mean(xs)), 3) if xs else None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--prosody", type=Path, required=True)
    ap.add_argument("--compare", type=Path)
    ap.add_argument("--md", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    logs = {s: json.loads(p.read_text()) for s in ("refs", "drama3", "s2pro", "asr")
            if (p := a.work / "out/logs" / f"{s}.json").exists()}
    jobs = json.loads((a.work / "jobs.json").read_text())
    texts = {}
    for it in jobs["items"]:
        texts[(it["voice"], it["cond"], it["passage"], it["file"])] = it
    pros = json.loads(a.prosody.read_text())
    tr = logs.get("asr", {}).get("transcripts", {})
    mos = logs.get("asr", {}).get("mos", {})
    rows: dict[str, dict] = {}
    clips = []
    for stage in ("drama3", "s2pro"):
        for i in logs.get(stage, {}).get("items", []):
            _, model, voice, cond = i["label"].split("_", 3)
            it = texts[(voice, cond, i["passage"], i["file"])]
            rel = f"{i['label']}/{i['passage']}/{i['file']}"
            c = {"label": i["label"], "passage": i["passage"], "file": i["file"], **{k: i.get(k) for k in (
                "audio_s", "gen_s", "decode_s", "rtf_total", "frames", "chunks", "error")}}
            raw = a.work / "out/raw" / rel
            if raw.exists():
                w, sr = sf.read(raw, dtype="float32")
                c.update(signal(w, sr))
                c["syllables"] = prosody.text_syllables(it["text"])
                c["sec_per_syl"] = round(c["dur_s"] / c["syllables"], 3)
            if rel in tr:
                ref, hyp = norm_words(it["text"]), norm_words(tr[rel])
                errs, n = wer(ref, hyp)
                leaked = sum(max(0, hyp.count(t) - ref.count(t)) for t in TAG_WORDS)
                c.update({"asr": tr[rel], "word_errors": errs, "ref_words": n, "tag_words_spoken": leaked})
            if isinstance(mos.get(rel), (int, float)):
                c["utmos"] = mos[rel]
            clips.append(c)
    for lab in dict.fromkeys(c["label"] for c in clips):
        cs = [c for c in clips if c["label"] == lab and "dur_s" in c]
        med = float(np.median([c["sec_per_syl"] for c in cs])) if cs else None
        for c in cs:
            c["stretch"] = round(c["sec_per_syl"] / med, 2)
        ok = [c for c in clips if c["label"] == lab and c.get("audio_s")]
        audio = sum(c["audio_s"] for c in ok)
        p = pros["voices"].get(lab, {})
        per_passage = {r["passage"]: {"ei": r["index"], "pct": r["ceiling_pct"]} for r in pros["passages"]
                       if r["label"] == lab}
        werr = sum(c.get("word_errors", 0) for c in cs)
        wref = sum(c.get("ref_words", 0) for c in cs)
        rows[lab] = {
            "items_ok": len(ok), "items_failed": sum(1 for c in clips if c["label"] == lab and c.get("error")),
            "audio_s": round(audio, 1),
            "rtf_gen": round(sum(c["gen_s"] for c in ok) / audio, 3) if audio else None,
            "rtf_total": round(sum(c["gen_s"] + c["decode_s"] for c in ok) / audio, 3) if audio else None,
            "ei": p.get("index"), "pct_human": p.get("ceiling_pct"), "per_passage": per_passage,
            "wer_pct": round(100 * werr / wref, 1) if wref else None,
            "tag_words_spoken": sum(c.get("tag_words_spoken", 0) for c in cs),
            "utmos": mean([c.get("utmos") for c in cs]),
            "stretched": [f"{c['passage']}/{c['file']}" for c in cs if c["stretch"] > 1.6],
            "truncated": [f"{c['passage']}/{c['file']}" for c in cs if c["stretch"] < 0.6],
            "long_pauses": [f"{c['passage']}/{c['file']} {c['max_pause_s']}s" for c in cs if c["max_pause_s"] > 1.2],
            "clipped": [f"{c['passage']}/{c['file']}" for c in cs if c["peak"] >= 0.99],
            "f0_jump_pct": mean([c["f0_jump_pct"] for c in cs]), "voiced_pct": mean([c["voiced_pct"] for c in cs]),
            "flatness": mean([c["flatness"] for c in cs]), "hf6k_pct": mean([c["hf6k_pct"] for c in cs]),
            "sec_per_syl": mean([c["sec_per_syl"] for c in cs]),
        }
    compare = {}
    if a.compare:
        for f in sorted(a.compare.glob("score__*.webm")):
            _, lab, passage, _ = f.stem.split("__")
            w = prosody.decode(f, 24_000)
            compare.setdefault(lab, []).append(signal(w, 24_000))
        compare = {lab: {k: mean([s[k] for s in ss]) for k in ("f0_jump_pct", "voiced_pct", "flatness", "hf6k_pct")}
                   for lab, ss in compare.items()}
    resources = {s: {k: logs[s].get(k) for k in ("devices", "download", "load_s", "memory_after_load", "memory",
                                                    "nvidia_smi_after_load", "nvidia_smi_now", "smoke", "int4_load",
                                                    "ok", "errors")} for s in ("drama3", "s2pro") if s in logs}
    out = {"labels": rows, "clips": clips, "compare": compare, "resources": resources,
           "refs": logs.get("refs", {}).get("refs"), "ref_mos": {k: v for k, v in mos.items() if k.startswith("ref_")}}
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(out, indent=1, ensure_ascii=False))
    lines = ["| label | EI | % of human | 1-suspense | 2-dialogue | 3-reflective | RTF gen | RTF total | WER % | "
             "tag words spoken | UTMOS | F0 jumps % | voiced % | flatness | >6 kHz % | flags |",
             "|" + "---|" * 16]
    for lab, r in rows.items():
        pp = [r["per_passage"].get(p, {}).get("pct") for p in ("1-suspense", "2-dialogue", "3-reflective")]
        flags = "; ".join(f"{k}: {', '.join(v)}" for k in ("stretched", "truncated", "long_pauses", "clipped")
                          if (v := r[k]))
        lines.append(f"| {lab} | {r['ei']} | {r['pct_human']} | " + " | ".join(str(x) for x in pp) +
                     f" | {r['rtf_gen']} | {r['rtf_total']} | {r['wer_pct']} | {r['tag_words_spoken']} | "
                     f"{r['utmos']} | {r['f0_jump_pct']} | {r['voiced_pct']} | {r['flatness']} | {r['hf6k_pct']} | "
                     f"{flags or '-'} |")
    if compare:
        lines += ["", "| reference | F0 jumps % | voiced % | flatness | >6 kHz % |", "|---|---|---|---|---|"]
        for lab, s in compare.items():
            lines.append(f"| {lab} | {s['f0_jump_pct']} | {s['voiced_pct']} | {s['flatness']} | {s['hf6k_pct']} |")
    a.md.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
