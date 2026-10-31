#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Listening pack: one joined WebM/Opus clip per passage and condition, plus index.md with each clip's measured EI.

    uv run tools/expressiveness/make_pack.py --scratch <scratch>/expr --out <scratch>/expr/pack

<scratch> holds the renders made by render.py (audio/), ming_kaggle.py collect (audio_ming/) and the director runs
(s1/, g1/). EI values are read from tools/expressiveness/results (frozen note 20 calibration); nothing is re-measured.
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from prosody import decode, encode_webm  # noqa: E402

RES = Path(__file__).resolve().parent / "results"
PASSAGES = ["1-suspense", "2-dialogue", "3-reflective"]
HUMAN_S = 30.0
SR = 24_000  # listening rate; prosody.py measures at 16 kHz

# (slot, name, render dir under scratch, results file, label in results, description)
COMMON = [
    ("01", "kokoro-plain", "audio/af_heart", "baseline.json", "af_heart",
     "Kokoro af_heart as the app renders it today (live API, one call per paragraph segment, fixed 250/600 ms pauses)."),
    ("02", "kokoro-director", "s1/a_chars", "director-runs/p1.json", "a_chars",
     "Kokoro af_heart with the recommended director mapping (kokoro_map.RECOMMENDED: per-sentence calls, speed, "
     "director pauses, partner blend, character voices). Local Kokoro 0.9.4, seed 1."),
    ("03", "ming-emma-plain", "audio_ming/ming_emma_ip", "ming.json", "ming_emma_ip",
     "Ming US_Emma preset, one IP call per paragraph segment, as the app sends it (Kaggle T4, seed 1234)."),
    ("04", "ming-emma-emotions-tags", "audio_ming/ming_emma_directed", "ming.json", "ming_emma_directed",
     "Ming US_Emma per sentence with hand-placed emotion labels and paralinguistic tags (passages/directions.json)."),
]
BEST = {
    "1-suspense": ("05", "best-index-ming-george", "audio_ming/ming_george_ip", "ming.json", "ming_george_ip",
                   "Highest EI measured on this passage: Ming UK_George preset, paragraph calls. Listen for creak at "
                   "phrase ends; the score may be inflated by it (note 32)."),
    "2-dialogue": ("05", "best-index-kokoro-george-director", "g1/a_chars_george", "director-runs/pg1.json",
                   "a_chars_george",
                   "Highest EI measured on this passage outside the rejected in-call hook path: Kokoro bm_george with "
                   "the recommended director mapping and character voices (seed 1). Character pitch gaps inflate it."),
    "3-reflective": ("05", "best-index-ming-george", "audio_ming/ming_george_ip", "ming.json", "ming_george_ip",
                     "Highest EI measured on this passage: Ming UK_George preset, paragraph calls. Listen for creak "
                     "at phrase ends; the score may be inflated by it (note 32)."),
}
HUMAN = ("06", "human-reference", "audio/human", "baseline.json", "human",
         "LibriVox reader, a different text of the same genre, first 30 s. Ear calibration only; it is the EI ceiling.")


def load(d: Path) -> tuple[np.ndarray, dict]:
    meta = json.loads((d / "meta.json").read_text())
    return np.concatenate([decode(d / s["file"], sr=SR) for s in meta["segments"]]), meta


def ei(results: str, label: str, passage: str) -> tuple[float, float]:
    d = json.loads((RES / results).read_text())
    for p in d["passages"]:
        if p["label"] == label and p["passage"] == passage:
            return p["index"], p["ceiling_pct"]
    raise KeyError(f"{label}/{passage} not in {results}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--kbps", type=int, default=32)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    texts = Path(__file__).resolve().parent / "passages"
    lines = ["# Listening pack", "",
             "Clips per passage, same text in 01-05. EI = expressiveness index (note 20, frozen calibration); "
             "% = share of the way from the F0-flattened control (0) to the LibriVox readers (100). "
             f"Opus {a.kbps} kbps mono 24 kHz. Made by tools/expressiveness/make_pack.py.", "",
             "What to judge by ear (none of it has been listened to yet):", "",
             "- 01 vs 02: does the director version sound more like a reading, and do the voice blends and speed "
             "changes between sentences sound natural or like a different speaker?",
             "- 03 vs 04: do Ming's emotion labels and tags add anything you can hear?",
             "- 05 on suspense/reflective: is the UK_George creak at phrase ends audible? If yes, its EI is inflated.",
             "- 02 and 05 on dialogue: do the character voices help you follow who speaks?",
             "- Which clip would you keep listening to for an hour? Ranking 01-05 per passage is enough.", ""]
    total = 0
    for passage in PASSAGES:
        lines += [f"## {passage}", "", "| file | s | EI | % | what it is |", "|---|---|---|---|---|"]
        for slot, name, rdir, res, label, desc in COMMON + [BEST[passage], HUMAN]:
            wave, meta = load(a.scratch / rdir / passage)
            if label == "human":
                wave = wave[: int(HUMAN_S * SR)]
                desc += f" Source: {meta.get('description', '')}."
            peak = float(np.max(np.abs(wave))) if wave.size else 0.0
            if peak > 0.99:
                wave = wave * (0.99 / peak)
            fname = f"{passage}_{slot}_{name}.webm"
            encode_webm(wave, SR, a.out / fname, kbps=a.kbps)
            size = (a.out / fname).stat().st_size
            total += size
            idx, pct = ei(res, label, passage)
            lines.append(f"| {fname} | {wave.size / SR:.0f} | {idx:.2f} | {pct:.0f} | {desc} |")
        lines += ["", "Text:", "", "> " + (texts / f"{passage}.txt").read_text().strip().replace("\n\n", "\n>\n> "), ""]
    lines += [f"Total size: {total / 1e6:.2f} MB."]
    (a.out / "index.md").write_text("\n".join(lines) + "\n")
    print(f"{total / 1e6:.2f} MB in {a.out}")


if __name__ == "__main__":
    main()
