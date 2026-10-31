#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Render speed of the live Space, measured from the client side over Gradio's HTTP API (the Worker's path).

    HF_TOKEN=... uv run tools/bench/space_bench.py --runs 3                  # Kokoro, render_cpu, 1,000 chars
    HF_TOKEN=... uv run tools/bench/space_bench.py --engine ming --runs 1    # spends ZeroGPU quota (about 60 s)

For each run: POST /gradio_api/call/<api>, then read the SSE stream and time every event from the POST. Reports the time
to the first item result (first `generating` event that holds an EngineItemResult; for a Space without streaming it is
the `complete` event), total time to `complete`, the server's own elapsed_ms, audio seconds, and the real-time factor
(total s / audio s). Works against the old and the new engine, so run it before and after a deploy.

Writes go to R2 under fixed bench keys (workload.py), so repeated runs overwrite the same 4 clips, their sidecars and
`jobs/benchbenchbench0.json`. The token is read from the environment and never printed.
"""
from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import workload  # noqa: E402

DEFAULT_SPACE = "https://ibrahimar397-nolabs-engine.hf.space"


def sse_events(response):
    """Yield (event, data) pairs from a text/event-stream response."""
    event, data = "", []
    for raw in response:
        line = raw.decode("utf-8").rstrip("\r\n")
        if not line:
            if event or data:
                yield event, "\n".join(data)
            event, data = "", []
        elif line.startswith("event:"):
            event = line[6:].strip()
        elif line.startswith("data:"):
            data.append(line[5:].lstrip())
    if event or data:
        yield event, "\n".join(data)


def output(data: str) -> dict | None:
    """The one JSON string output of an event's data array, parsed; None when absent."""
    try:
        values = json.loads(data)
        return json.loads(values[0]) if isinstance(values, list) and values and isinstance(values[0], str) else None
    except (ValueError, IndexError):
        return None


def one_run(base: str, token: str, api: str, body: dict, timeout_s: float) -> dict:
    headers = {"authorization": f"Bearer {token}", "content-type": "application/json"}
    started = time.perf_counter()
    post = urllib.request.Request(f"{base}/gradio_api/call/{api}", method="POST", headers=headers,
                                  data=json.dumps({"data": [json.dumps(body, ensure_ascii=False)]}).encode())
    with urllib.request.urlopen(post, timeout=60) as res:
        event_id = json.load(res)["event_id"]
    submitted_s = time.perf_counter() - started
    get = urllib.request.Request(f"{base}/gradio_api/call/{api}/{event_id}",
                                 headers={**headers, "accept": "text/event-stream"})
    items: list[float] = []
    final = None
    with urllib.request.urlopen(get, timeout=timeout_s) as res:
        for event, data in sse_events(res):
            now = time.perf_counter() - started
            if event == "generating":
                value = output(data)
                if value is not None and "job_id" not in value:
                    items.append(now)
            elif event == "complete":
                final = output(data)
                complete_s = now
                break
            elif event == "error":
                raise SystemExit(f"{api} failed: {data}")
    if final is None:
        raise SystemExit("stream ended without a complete event")
    failed = [r for r in final["items"] if not r["ok"]]
    if failed:
        raise SystemExit(f"items failed: {failed}")
    audio_s = sum(r["duration_ms"] for r in final["items"]) / 1000
    return {"submit_s": round(submitted_s, 3), "first_item_s": round(items[0] if items else complete_s, 3),
            "streamed_items": len(items), "item_s": [round(t, 3) for t in items], "total_s": round(complete_s, 3),
            "server_elapsed_s": final["elapsed_ms"] / 1000, "gpu_s": final["gpu_ms"] / 1000,
            "audio_s": round(audio_s, 3), "rtf": round(complete_s / audio_s, 4)}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--space-url", default=os.environ.get("NOLABS_SPACE_URL", DEFAULT_SPACE))
    ap.add_argument("--token-env", default="HF_TOKEN", help="environment variable holding the HF token")
    ap.add_argument("--engine", choices=("kokoro", "ming"), default="kokoro")
    ap.add_argument("--voice", help="default af_heart (kokoro) or US_Emma (ming)")
    ap.add_argument("--chars", type=int, default=1000)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--label", default="live")
    ap.add_argument("--timeout", type=float, default=600.0)
    args = ap.parse_args()
    token = os.environ.get(args.token_env, "")
    if not token:
        raise SystemExit(f"set {args.token_env} to an HF token that can call the Space")
    voice = args.voice or ("af_heart" if args.engine == "kokoro" else "US_Emma")
    api = "render_cpu" if args.engine == "kokoro" else "render_gpu"
    body = workload.request(args.engine, voice, args.chars)
    runs = [one_run(args.space_url.rstrip("/"), token, api, body, args.timeout) for _ in range(args.runs)]
    print(json.dumps({
        "label": args.label, "space": args.space_url, "api": api, "voice": voice,
        "chars": sum(len(i["text"]) for i in body["items"]), "segments": len(body["items"]),
        "median_first_item_s": statistics.median(r["first_item_s"] for r in runs),
        "median_total_s": statistics.median(r["total_s"] for r in runs),
        "median_rtf": statistics.median(r["rtf"] for r in runs), "runs": runs,
    }, indent=2))


if __name__ == "__main__":
    main()
