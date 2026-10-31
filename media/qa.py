#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Rig and continuity QA for a shot: contact sheets of the whole timeline plus joint close-ups.

    uv run --script media/qa.py media/shots/promo.json --out DIR [--step 0.25] [--joints-every 2.0] [--only-joints]

Writes DIR/<shot>-timeline-NN.jpg (3x3 tiles of 640x360 frames, every --step seconds, time stamped) and
DIR/<shot>-joints-<t>.jpg (shoulder, elbow, wrist/hand, hip, knee, ankle close-ups at time t, both sides), and
DIR/<shot>-metrics.json with per-frame numbers the eye can miss: lowest sole height (feet floating / sinking while
standing or walking), hand-to-target error of the arm IK, elbow and knee hinge angles (negative = bending backwards).
Frames stream from Chrome into ffmpeg; nothing but the sheets is written.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

MEDIA = Path(__file__).resolve().parent
sys.path.insert(0, str(MEDIA))
sys.path.insert(0, str(MEDIA / "tools"))
import render as R  # noqa: E402
from ffmpeg import ffmpeg_exe  # noqa: E402

JOINT_VIEWS = [("shoulderR", [-60, 10, 0.75]), ("elbowR", [-80, 5, 0.6]), ("wristR", [-30, 15, 0.45]),
               ("wristR", [-120, 60, 0.6]), ("elbowL", [80, 5, 0.6]), ("wristL", [30, 15, 0.45]),
               ("hipR", [-90, 5, 0.8]), ("kneeR", [-90, 0, 0.7]), ("ankleR", [-90, 10, 0.55]),
               ("hips", [0, 10, 1.4]), ("kneeL", [90, 0, 0.7]), ("ankleL", [90, 10, 0.55])]


def tile(images: list[bytes], cols: int, rows: int, w: int, h: int, out: Path, labels: list[str]):
    """Tile JPEG frames into one sheet; each tile shows its index, labels go to a .txt next to the sheet."""
    font = "/System/Library/Fonts/Supplemental/Arial.ttf"
    vf = (f"scale={w}:{h},drawtext=fontfile={font}:text='%{{n}}':x=8:y=8:fontsize=22:fontcolor=white:box=1:"
          f"boxcolor=black@0.5,tile={cols}x{rows}:padding=2:color=black")
    p = subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-f", "image2pipe", "-c:v", "mjpeg", "-i", "-",
                        "-vf", vf, "-frames:v", "1", "-q:v", "3", str(out)], input=b"".join(images))
    if p.returncode:
        raise SystemExit("ffmpeg tile failed")
    out.with_suffix(".txt").write_text("\n".join(f"{i}: {l}" for i, l in enumerate(labels)) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("shot")
    ap.add_argument("--out", required=True)
    ap.add_argument("--step", type=float, default=0.25)
    ap.add_argument("--joints-every", type=float, default=2.0)
    ap.add_argument("--only-joints", action="store_true")
    ap.add_argument("--swiftshader", action="store_true")
    ap.add_argument("--headful", action="store_true")
    ap.add_argument("--lang", default="en")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    shot = json.loads(Path(a.shot).read_text())
    name = shot.get("name", "shot")
    shot["resolution"] = [960, 540] if shot["resolution"][0] >= shot["resolution"][1] else [540, 960]
    s = R.Session(shot, a)
    fps = s.info["fps"]
    dur = shot["duration"]
    metrics = []
    try:
        if not a.only_joints:
            frames, labels, sheet = [], [], 0
            t = 0.0
            while t < dur - 1e-6:
                i = int(round(t * fps))
                frames.append(s.grab(i, quality=85))
                metrics.append({"t": round(t, 2), **(s.d.eval("studio.qaMetrics()") or {})})
                labels.append(f"t={t:.2f}s")
                if len(frames) == 9:
                    tile(frames, 3, 3, 640, 360, out / f"{name}-timeline-{sheet:02d}.jpg", labels)
                    frames, labels, sheet = [], [], sheet + 1
                t += a.step
            if frames:
                while len(frames) < 9:
                    frames.append(frames[-1]); labels.append("(pad)")
                tile(frames, 3, 3, 640, 360, out / f"{name}-timeline-{sheet:02d}.jpg", labels)
        t = 0.5
        while t < dur:
            i = int(round(t * fps))
            shots, labels = [], []
            for joint, orbit in JOINT_VIEWS:
                s.d.eval(f"studio.qa = {json.dumps({'joint': joint, 'orbit': orbit})}")
                shots.append(s.grab(i, quality=85))
                labels.append(f"t={t:.2f}s {joint}")
            s.d.eval("studio.qa = null")
            tile(shots, 6, 2, 480, 270, out / f"{name}-joints-{t:05.2f}.jpg", labels)
            t += a.joints_every
    finally:
        s.close()
    (out / f"{name}-metrics.json").write_text(json.dumps(metrics, indent=0))
    bad = [m for m in metrics if m.get("issues")]
    print(f"{name}: {len(metrics)} sampled frames, {len(bad)} with metric issues")
    for m in bad[:60]:
        print(f"  t={m['t']}: {m['issues']}")


if __name__ == "__main__":
    main()
