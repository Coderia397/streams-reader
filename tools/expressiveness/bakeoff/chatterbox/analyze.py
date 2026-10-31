#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy>=1.26", "soundfile>=0.12"]
# ///
"""Summarise the Chatterbox bake-off: prosody index (from prosody.py measure), Whisper WER, UTMOS, duration checks,
RTF and VRAM per model. Writes a markdown table to --md and a JSON to --out.

    uv run tools/expressiveness/bakeoff/chatterbox/analyze.py --work W --audio A2 --base A \
        --prosody tools/expressiveness/bakeoff/chatterbox/results/chatterbox.json --md .../summary.md --out .../summary.json

Artefact checks per clip (raw model output, before the pipeline): speech seconds per text syllable against the
condition's median (> 1.6x = stretched or babbling, < 0.6x = truncated), ASR word errors, clipping (peak >= 0.99),
and the longest interior silence (> 1.2 s inside one clip is flagged).
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

import numpy as np
import soundfile as sf

NUM = {"0": "zero", "1": "one", "2": "two", "3": "three", "4": "four", "5": "five", "6": "six", "7": "seven",
       "8": "eight", "9": "nine", "10": "ten", "11": "eleven", "12": "twelve"}
FILLERS = {"uh", "um", "uhm", "hmm", "ah", "oh"}


def words(t: str) -> list[str]:
    t = re.sub(r"\[[^\]]*\]", " ", t.lower().replace("’", "'"))
    t = re.sub(r"(\d+)(:00)?", lambda m: " " + NUM.get(m.group(1), m.group(1)) + " ", t)
    t = re.sub(r"[^a-z0-9' ]+", " ", t)
    t = [w.strip("'") for w in t.split()]  # Whisper wraps quotes as "'...'": stray apostrophes are not words
    return [w for w in t if w and w not in FILLERS]


def edits(ref: list[str], hyp: list[str]) -> int:
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
            prev, d[j] = d[j], cur
    return d[len(hyp)]


def syllables(text: str) -> int:
    n = 0
    for w in re.findall(r"[a-z']+", text.lower()):
        n += max(1, len(re.findall(r"[aeiouy]+", re.sub(r"e$", "", w))))
    return n


def interior_silence_s(w: np.ndarray, sr: int) -> float:
    hop = int(0.01 * sr)
    n = w.size // hop
    if n < 10:
        return 0.0
    db = 20 * np.log10(np.sqrt(np.mean(w[: n * hop].reshape(n, hop) ** 2, axis=1)) + 1e-10)
    sil = db < np.percentile(db, 95) - 35
    idx = np.flatnonzero(~sil)
    if idx.size == 0:
        return 0.0
    sil = sil[idx[0]: idx[-1] + 1]
    best = run = 0
    for s in sil:
        run = run + 1 if s else 0
        best = max(best, run)
    return best * 0.01


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--audio", type=Path, required=True)
    ap.add_argument("--base", type=Path, required=True, help="baseline audio root (Kokoro + human meta)")
    ap.add_argument("--prosody", type=Path, required=True)
    ap.add_argument("--md", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    out = a.work / "out"
    conds = json.loads((a.work / "conditions.json").read_text())
    asr = json.loads((out / "asr.json").read_text()) if (out / "asr.json").exists() else {}
    mos = json.loads((out / "mos.json").read_text()) if (out / "mos.json").exists() else {}
    log = json.loads((out / "cb_log.json").read_text())
    pros = {(r["label"], r["passage"]): r for r in json.loads(a.prosody.read_text())["passages"]}
    rows, flags = [], []
    for c in conds:
        ref_w, hyp_w, err, ms, clip_rows = [], [], 0, [], []
        for it in c["items"]:
            k = f"{c['label']}/{c['passage']}/{it['file']}"
            f = out / "raw" / k
            if not f.exists():
                flags.append(f"{k}: missing")
                continue
            w, sr = sf.read(f, dtype="float32")
            r, h = words(it["text"]), words(asr.get(k, ""))
            e = edits(r, h)
            err += e
            ref_w += r
            if isinstance(mos.get(k), (int, float)):
                ms.append(mos[k])
            clip_rows.append({"key": k, "s_per_syl": w.size / sr / max(1, syllables(it["text"])),
                              "wer": e / max(1, len(r)), "peak": float(np.abs(w).max()),
                              "sil": interior_silence_s(w, sr), "asr": asr.get(k, ""), "text": it["spoken"]})
        med = float(np.median([x["s_per_syl"] for x in clip_rows])) if clip_rows else 0
        for x in clip_rows:
            why = []
            if x["s_per_syl"] > 1.6 * med:
                why.append(f"long {x['s_per_syl'] / med:.1f}x median")
            if x["s_per_syl"] < 0.6 * med:
                why.append(f"short {x['s_per_syl'] / med:.1f}x median")
            if x["wer"] >= 0.2:
                why.append(f"WER {x['wer']:.0%}")
            if x["peak"] >= 0.99:
                why.append("clipped")
            if x["sil"] > 1.2:
                why.append(f"silence {x['sil']:.1f} s")
            if why:
                flags.append(f"{x['key']}: {', '.join(why)} | said: {x['asr'][:120]!r} | text: {x['text'][:80]!r}")
        for label in [c["label"]] + ([c["label"] + "_dp"] if c["mode"] == "sentence" else []):
            p = pros.get((label, c["passage"]))
            rows.append({"label": label, "model": c["model"], "voice": c["voice"], "passage": c["passage"],
                         "ei": p and p["index"], "pct": p and p["ceiling_pct"],
                         "f0_median_hz": p and p["metrics"]["f0_median_hz"],
                         "wer": round(err / max(1, len(ref_w)), 3), "utmos": round(float(np.mean(ms)), 2) if ms else None,
                         "s_per_syl": round(float(np.mean([x["s_per_syl"] for x in clip_rows])), 3) if clip_rows else None,
                         "dir": str(a.audio / label / c["passage"])})
    # Baseline Kokoro and human with the same ASR / MOS.
    base = []
    for lab in ["af_heart", "am_michael", "bf_emma", "bm_george", "human"]:
        for d in sorted((a.base / lab).glob("*")):
            if not (d / "meta.json").exists():
                continue
            meta = json.loads((d / "meta.json").read_text())
            err = n = 0
            ms = []
            for s in meta["segments"]:
                k = f"{lab}/{d.name}/{s['file']}"
                if isinstance(mos.get(k), (int, float)):
                    ms.append(mos[k])
                if s.get("text"):
                    r = words(s["text"])
                    err += edits(r, words(asr.get(k, "")))
                    n += len(r)
            base.append({"label": lab, "passage": d.name, "wer": round(err / n, 3) if n else None,
                         "utmos": round(float(np.mean(ms)), 2) if ms else None})
    refs = {k: {"utmos": mos.get(v["file"]), "asr": asr.get(v["file"])} for k, v in
            json.loads((a.work / "dataset/refs.json").read_text()).items()}
    models = log.get("models", {})

    def agg(rs, key):
        v = [r[key] for r in rs if r[key] is not None]
        return round(float(np.mean(v)), 3) if v else None

    labels = list(dict.fromkeys(r["label"] for r in rows))
    lines = ["| condition | model | voice | EI 1/2/3 | % of human 1/2/3 | % mean | WER | UTMOS | s/syll |",
             "|---|---|---|---|---|---|---|---|---|"]
    summary = {}
    for lab in labels:
        rs = [r for r in rows if r["label"] == lab]
        s = {"model": rs[0]["model"], "voice": rs[0]["voice"], "pct": agg(rs, "pct"), "ei": agg(rs, "ei"),
             "wer": agg(rs, "wer"), "utmos": agg(rs, "utmos"), "s_per_syl": agg(rs, "s_per_syl")}
        summary[lab] = s
        ei = "/".join("-" if r["ei"] is None else f"{r['ei']:.2f}" for r in rs)
        pct = "/".join("-" if r["pct"] is None else f"{r['pct']:.0f}" for r in rs)
        pm = "-" if s["pct"] is None else f"{s['pct']:.0f}"
        wer = "-" if s["wer"] is None else f"{100 * s['wer']:.1f} %"
        lines.append(f"| {lab} | {s['model']} | {s['voice']} | {ei} | {pct} | {pm} | {wer} | {s['utmos']} | "
                     f"{s['s_per_syl']} |")
    lines += ["", "| baseline | passage | WER | UTMOS |", "|---|---|---|---|"]
    lines += [f"| {b['label']} | {b['passage']} | {b['wer']} | {b['utmos']} |" for b in base]
    lines += ["", "| model | load s | weights GB | peak VRAM GB | reserved GB | items | GPU s | audio s | RTF |",
              "|---|---|---|---|---|---|---|---|---|"]
    for k, m in models.items():
        lines.append(f"| {k} | {m.get('load_s')} | {m.get('weights_gb')} | {m.get('peak_vram_gb')} | "
                     f"{m.get('peak_reserved_gb')} | {m.get('items')} | {m.get('gpu_s')} | {m.get('audio_s')} | "
                     f"{m.get('rtf')} |")
    lines += ["", f"References: {json.dumps(refs, ensure_ascii=False)}", "", "Flags:"] + [f"- {f}" for f in flags]
    a.md.parent.mkdir(parents=True, exist_ok=True)
    a.md.write_text("\n".join(lines) + "\n")
    a.out.write_text(json.dumps({"rows": rows, "summary": summary, "baseline": base, "models": models,
                                 "refs": refs, "flags": flags, "errors": log.get("errors", [])}, indent=1))
    print("\n".join(lines))


if __name__ == "__main__":
    main()
