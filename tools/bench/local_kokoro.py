#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10,<3.13"
# dependencies = [
#   "kokoro==0.9.4", "misaki[en]==0.9.4", "torch", "numpy==1.26.4", "transformers==4.46.3", "tokenizers==0.20.3",
#   "huggingface_hub==0.36.2", "pyloudnorm==0.1.1", "soundfile", "imageio-ffmpeg",
#   "en_core_web_sm @ https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl",
# ]
# ///
"""Kokoro render speed on this machine, through the Space's own Service code: real Kokoro (pinned kokoro 0.9.4 and
model commit), real ffmpeg encode, and a storage stub that sleeps like an R2 PUT.

    uv run tools/bench/local_kokoro.py --engine-dir space --label after
    uv run tools/bench/local_kokoro.py --engine-dir <copy of an older space/> --label before

Reports, per run and as medians: time to the first item result (what the Worker can act on; for an engine without
streaming it is the whole render), total time, audio seconds, and the real-time factor (render s / audio s, lower is
faster). Out-of-lexicon words get the BART fallback with the current engine, and are skipped with an older engine
unless `--espeak` is given (the benchmark passage has none). `--threads 2` matches the Space's 2 vCPU allowance. The
first Kokoro call is a warm-up and not counted.
"""
from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import workload  # noqa: E402


class SleepyStorage:
    """Accepts every write after `delay_s`, keeps nothing (an R2 PUT from a Space takes tens to hundreds of ms)."""

    def __init__(self, delay_s: float):
        self.delay_s = delay_s
        self.puts = 0

    def _put(self, *_args) -> None:
        time.sleep(self.delay_s)
        self.puts += 1

    put_audio = put_json = put_wav = _put

    def ping(self) -> bool:
        return True


def disable_espeak() -> None:
    """Make kokoro's espeak fallback fail softly. The espeakng-loader build for macOS x86_64 aborts the process when it
    looks for its data in its CI build folder (notes/19), and the Space no longer uses espeak for English."""
    import types

    import misaki

    stub = types.ModuleType("misaki.espeak")

    class EspeakFallback:
        def __init__(self, *_args, **_kwargs):
            raise RuntimeError("espeak disabled for this benchmark")

    stub.EspeakFallback = EspeakFallback
    sys.modules["misaki.espeak"] = stub
    misaki.espeak = stub


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--engine-dir", required=True, help="folder holding the nolabs_engine package (space/)")
    ap.add_argument("--label", default="run")
    ap.add_argument("--chars", type=int, default=1000)
    ap.add_argument("--voice", default="af_heart")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--upload-ms", type=float, default=150.0, help="simulated latency of each R2 PUT")
    ap.add_argument("--espeak", action="store_true", help="let kokoro load espeak-ng (works on Linux, aborts on macOS)")
    args = ap.parse_args()

    sys.path.insert(0, str(Path(args.engine_dir).resolve()))
    from nolabs_engine import runtime

    runtime.configure_threads(args.threads)
    import imageio_ffmpeg
    import torch

    os.environ.setdefault("NOLABS_FFMPEG", imageio_ffmpeg.get_ffmpeg_exe())
    if not args.espeak:
        disable_espeak()
    runtime.configure_torch(torch, args.threads)
    from nolabs_engine import contracts
    from nolabs_engine.engines.kokoro_engine import KokoroEngine
    from nolabs_engine.service import Service

    load_started = time.perf_counter()
    kokoro = KokoroEngine()
    load_s = time.perf_counter() - load_started
    kokoro.synthesize("A short warm-up sentence.", args.voice)

    storage = SleepyStorage(args.upload_ms / 1000)
    service = Service(storage, kokoro, None, statuses={"kokoro": "ready"})
    req = contracts.parse_engine_request(workload.request("kokoro", args.voice, args.chars))
    streams = hasattr(service, "stream_cpu")
    runs = []
    for _ in range(args.runs):
        started = time.perf_counter()
        item_s: list[float] = []
        if streams:
            for event in service.stream_cpu(req):
                if isinstance(event, contracts.EngineItemResult):
                    item_s.append(time.perf_counter() - started)
                else:
                    response = event
        else:
            response = service.render_cpu(req)
            item_s = [time.perf_counter() - started] * len(response.items)
        total_s = time.perf_counter() - started
        audio_s = sum(r.duration_ms for r in response.items if r.ok) / 1000
        if not all(r.ok for r in response.items):
            raise SystemExit(f"items failed: {[r.error for r in response.items if not r.ok]}")
        runs.append({"first_item_s": round(item_s[0], 3), "item_s": [round(t, 3) for t in item_s],
                     "total_s": round(total_s, 3), "audio_s": round(audio_s, 3), "rtf": round(total_s / audio_s, 4)})

    summary = {
        "label": args.label, "streaming": streams, "chars": req.total_chars(), "segments": len(req.items),
        "threads": args.threads, "upload_ms": args.upload_ms, "voice": args.voice, "load_s": round(load_s, 2),
        "torch": torch.__version__, "median_first_item_s": statistics.median(r["first_item_s"] for r in runs),
        "median_total_s": statistics.median(r["total_s"] for r in runs),
        "median_rtf": statistics.median(r["rtf"] for r in runs), "runs": runs,
    }
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
