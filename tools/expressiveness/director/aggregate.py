#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Combine per-seed prosody results (prosody.py measure, one root per seed) with artefacts.py and asr_check.py output
into one table: EI mean and sd over seeds, % of the human ceiling, feature means, artefacts, WER.

    uv run tools/expressiveness/director/aggregate.py --prosody s1.json s2.json s3.json --artefacts a1.json ...
        --asr w1.json ... --out results/director.json --md results/director.md
"""
from __future__ import annotations

import argparse
import json
import statistics as st
import sys
from pathlib import Path

FEATURES = ["f0_std_st", "f0_range_st", "f0_slope_std", "f0_level_std", "energy_range_db", "energy_level_std",
            "pause_log_std", "rate_std"]
PASSAGES = ["1-suspense", "2-dialogue", "3-reflective"]


def mean(xs):
    xs = [x for x in xs if x is not None]
    return st.mean(xs) if xs else None


def sd(xs):
    xs = [x for x in xs if x is not None]
    return st.stdev(xs) if len(xs) > 1 else None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--prosody", nargs="+", type=Path, required=True)
    ap.add_argument("--artefacts", nargs="*", type=Path, default=[])
    ap.add_argument("--asr", nargs="*", type=Path, default=[])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--md", type=Path)
    a = ap.parse_args()
    runs = [json.loads(p.read_text()) for p in a.prosody]
    labels = list(dict.fromkeys(r["label"] for run in runs for r in run["passages"]))
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from kokoro_map import CONDITIONS
    order = [c.name for c in CONDITIONS]
    labels.sort(key=lambda x: (order.index(x.split("_george")[0]) if x.split("_george")[0] in order else 99, x))
    rows = {}
    for lab in labels:
        seed_ei, seed_pct, per_passage, feats = [], [], {p: [] for p in PASSAGES}, {f: [] for f in FEATURES}
        for run in runs:
            rs = [r for r in run["passages"] if r["label"] == lab]
            if not rs:
                continue
            seed_ei.append(mean([r["index"] for r in rs]))
            seed_pct.append(mean([r["ceiling_pct"] for r in rs]))
            for r in rs:
                per_passage[r["passage"]].append(r["index"])
                for f in FEATURES:
                    feats[f].append(r["metrics"].get(f))
        rows[lab] = {"seeds": len(seed_ei), "ei": mean(seed_ei), "ei_sd_over_seeds": sd(seed_ei),
                     "pct_human": mean(seed_pct), "per_passage_ei": {p: mean(v) for p, v in per_passage.items()},
                     "features": {f: mean(v) for f, v in feats.items()}}
    for p in a.artefacts:
        for lab, v in json.loads(p.read_text()).items():
            if lab in rows:
                rows[lab].setdefault("_art", []).append(v["mean"])
    for p in a.asr:
        for lab, v in json.loads(p.read_text()).items():
            if lab in rows:
                rows[lab].setdefault("_asr", []).append(v)
    for lab, r in rows.items():
        arts = r.pop("_art", [])
        if arts:
            r["artefacts"] = {k: mean([x[k] for x in arts]) for k in arts[0]}
        asr = r.pop("_asr", [])
        if asr:
            r["wer"] = sum(x["errors"] for x in asr) / max(1, sum(x["words"] for x in asr))
    base = rows.get("k_plain", {}).get("ei")
    out = {"tool": "tools/expressiveness/director/aggregate.py", "inputs": [str(p) for p in a.prosody], "rows": rows}
    a.out.write_text(json.dumps(out, indent=1))
    lines = ["| condition | EI (sd over seeds) | Δ vs k_plain (same narrator) | % of human | suspense | dialogue | reflective | "
             "F0 level sd | slope sd | energy level sd | ln-pause sd | rate sd | joins: timbre jump dB | "
             "pitch breaks/min | clipped | WER % |", "|" + "---|" * 16]

    def f(x, n=2):
        return "-" if x is None else f"{x:.{n}f}"
    for lab, r in rows.items():
        ft, art, pp = r["features"], r.get("artefacts", {}), r["per_passage_ei"]
        sfx = lab[len(lab.split("_george")[0]):]
        b0 = rows.get("k_plain" + sfx, {}).get("ei", base)
        d = None if b0 is None or r["ei"] is None else r["ei"] - b0
        lines.append(
            f"| {lab} | {f(r['ei'])} ({f(r['ei_sd_over_seeds'])}) | {('+' if d and d > 0 else '') + f(d)} | "
            f"{f(r['pct_human'], 0)} | {f(pp['1-suspense'])} | {f(pp['2-dialogue'])} | {f(pp['3-reflective'])} | "
            f"{f(ft['f0_level_std'])} | {f(ft['f0_slope_std'], 1)} | {f(ft['energy_level_std'])} | "
            f"{f(ft['pause_log_std'])} | {f(ft['rate_std'])} | {f(art.get('timbre_jump_db'), 1)} | "
            f"{f(art.get('pitch_breaks_per_min'), 1)} | {f(art.get('clipped'), 0)} | "
            f"{f(None if r.get('wer') is None else 100 * r['wer'], 1)} |")
    md = "\n".join(lines) + "\n"
    if a.md:
        a.md.write_text(md)
    print(md)


if __name__ == "__main__":
    main()
