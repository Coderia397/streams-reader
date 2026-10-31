# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Octave-jump check: share of adjacent voiced frames whose F0 jumps more than 4 st, per label."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import numpy as np
import prosody as P
for root in sys.argv[1:]:
    for lab in sorted(Path(root).iterdir()):
        if not lab.is_dir(): continue
        jumps, pairs, halves = 0, 0, 0
        for pas in sorted(lab.iterdir()):
            if not (pas / "meta.json").exists(): continue
            w, _, _ = P.load_passage(pas)
            _, hz = P.f0_track(w)
            st = 12 * np.log2(hz / np.nanmedian(hz))
            d = np.abs(np.diff(st)); ok = ~np.isnan(d)
            jumps += int((d[ok] > 4).sum()); pairs += int(ok.sum())
            halves += int((st[~np.isnan(st)] < -7).sum())
        if pairs: print(f"{lab.name:28s} jumps>4st {1000*jumps/pairs:5.1f}/1000  frames<-7st {halves}")
