#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["imageio-ffmpeg"]
# ///
"""Cut the human ceiling references: 75 s excerpts of public-domain LibriVox readings (archive.org), one per passage
genre, re-encoded to WebM/Opus 32 kbps mono 24 kHz. Only the excerpt is downloaded (ffmpeg seeks over HTTP).

    uv run tools/expressiveness/fetch_human.py --out <dir>      # writes <dir>/human/<passage>/00.webm + meta.json
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import imageio_ffmpeg

IA = "https://archive.org/download"
# passage -> (archive.org item, file, start s, what it is)
REFERENCES = {
    "1-suspense": ("12_creepytales_1206_librivox", "creepytalesbypoe_01_poe_64kb.mp3", 540,
                   "Poe, The Tell-Tale Heart (LibriVox '12 Creepy Tales'), 9:00-10:15"),
    "2-dialogue": ("solo_pride_librivox", "prideandprejudice_01-04_austen_apc_64kb.mp3", 35,
                   "Austen, Pride and Prejudice ch. 1, Mr and Mrs Bennet (LibriVox version 2, solo), 0:35-1:50"),
    "3-reflective": ("walden_librivox", "walden_c05_64kb.mp3", 60,
                     "Thoreau, Walden ch. 5 'Solitude' (LibriVox), 1:00-2:15"),
}
LENGTH_S = 75


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    ff = imageio_ffmpeg.get_ffmpeg_exe()
    for passage, (item, name, start, what) in REFERENCES.items():
        d = a.out / "human" / passage
        if (d / "meta.json").exists():
            continue
        d.mkdir(parents=True, exist_ok=True)
        url = f"{IA}/{item}/{name}"
        subprocess.run([ff, "-hide_banner", "-loglevel", "error", "-y", "-ss", str(start), "-t", str(LENGTH_S),
                        "-i", url, "-ac", "1", "-ar", "24000", "-c:a", "libopus", "-b:a", "32k", str(d / "00.webm")],
                       check=True)
        (d / "meta.json").write_text(json.dumps({"voice": "human", "label": "human", "passage": passage,
                                                 "source": url, "start_s": start, "length_s": LENGTH_S,
                                                 "description": what,
                                                 "segments": [{"file": "00.webm", "words": []}]}, indent=1))
        print(f"human/{passage}: {what}")


if __name__ == "__main__":
    main()
