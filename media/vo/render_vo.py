#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Render a voice-over script through the Streams Reader API (Kokoro, CPU tier; never Ming/GPU).

    uv run --script media/vo/render_vo.py [--lang en] [--voice kokoro:af_heart]
    uv run --script media/vo/render_vo.py --voice ming:US_Emma --set en-ming --gpu   # expressive, one GPU request

Reads media/vo/script.<lang>.json, writes media/assets/vo/<set>/<id>.webm (set defaults to the language) and an index
media/assets/vo/<set>/index.json with each clip's duration and word timings (ms, char offsets). All lines go in ONE
/api/render request. Expressive (ming:*) voices spend the shared daily GPU budget, so they need --gpu, use each line's
"emotion", and the script never resubmits: a deferred or failed item stops it.
"""
import argparse, json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "tools"))
from streams_api import Api  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("--lang", default="en")
ap.add_argument("--voice")
ap.add_argument("--set", help="output folder under media/assets/vo (default: the language)")
ap.add_argument("--gpu", action="store_true", help="allow one expressive (ming:*) render request")
a = ap.parse_args()
script = json.loads((HERE / f"script.{a.lang}.json").read_text())
voice = a.voice or script["voice"]
if not voice.startswith("kokoro:") and not a.gpu:
    sys.exit("expressive voices spend the GPU budget: pass --gpu to allow one request")
out = HERE.parent / "assets" / "vo" / (a.set or a.lang)
out.mkdir(parents=True, exist_ok=True)
api = Api().login()
lines = script["lines"]
emotions = [l.get("emotion", "neutral") for l in lines] if not voice.startswith("kokoro:") else None
st = api.render(voice, [l["text"] for l in lines], emotions=emotions, timeout_s=1500)
index = {"lang": a.lang, "voice": voice, "lines": {}}
if emotions:
    index["emotions"] = dict(zip([l["id"] for l in lines], emotions))
for l, s in zip(lines, st):
    (out / f"{l['id']}.webm").write_bytes(api.audio(s["key"]))
    index["lines"][l["id"]] = {"text": l["text"], "file": f"{l['id']}.webm", "key": s["key"],
                               "duration_ms": s["audio"]["duration_ms"], "words": s["audio"]["words"]}
    print(f"{l['id']:10s} {s['audio']['duration_ms']:6d} ms  {l['text']}")
(out / "index.json").write_text(json.dumps(index, indent=1))
