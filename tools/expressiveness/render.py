#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Render the test passages through the live API with Kokoro voices, exactly as the app would.

Segments come from passages/segments.json (made by segment/ with the app's own segmenter, Kokoro profile).
Login: POST /api/session with PASSPHRASE from ~/.config/nolabs/app.env (never printed); cookie __Host-nolabs_s.

    uv run tools/expressiveness/render.py --out <dir> [--voices kokoro:af_heart,...] [--control]

Writes <out>/<label>/<passage>/NN.webm plus <out>/<label>/<passage>/meta.json (segment text, boundary, sentence
ranges, duration_ms, word timings). <label> is the voice id without the engine prefix; --control also renders
af_heart with every boundary set to 'sentence' as <out>/control_sentence/. Refuses ming:* voices (GPU quota).
"""
from __future__ import annotations

import argparse
import http.cookiejar
import json
import time
import urllib.error
import urllib.request
from pathlib import Path

BASE = "https://listen.streams.watch"
HERE = Path(__file__).resolve().parent
DEFAULT_VOICES = ["kokoro:af_heart", "kokoro:bf_emma", "kokoro:am_michael", "kokoro:bm_george"]


def passphrase() -> str:
    env = Path.home() / ".config/nolabs/app.env"
    for line in env.read_text().splitlines():
        if line.startswith("PASSPHRASE="):
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    raise SystemExit(f"PASSPHRASE missing in {env}")


class Api:
    def __init__(self, base: str = BASE):
        self.base = base.rstrip("/")
        self.op = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def req(self, method: str, path: str, body=None, raw: bool = False):
        data = None if body is None else json.dumps(body).encode()
        r = urllib.request.Request(self.base + path, data=data, method=method)
        r.add_header("Origin", self.base)
        r.add_header("x-nolabs-api", "1")
        r.add_header("User-Agent", "nolabs-expressiveness/1")
        if data is not None:
            r.add_header("Content-Type", "application/json")
        for attempt in range(4):
            try:
                with self.op.open(r, timeout=120) as resp:
                    b = resp.read()
                    return b if raw else (json.loads(b) if b else None)
            except urllib.error.HTTPError as e:
                if e.code < 500 or attempt == 3:
                    raise RuntimeError(f"{method} {path}: HTTP {e.code} {e.read()[:300]!r}") from None
            except urllib.error.URLError:
                if attempt == 3:
                    raise
            time.sleep(2 * (attempt + 1))

    def login(self) -> "Api":
        self.req("POST", "/api/session", {"passphrase": passphrase()})
        return self

    def render(self, voice: str, items: list[dict], timeout_s: float = 900) -> list[dict]:
        if not voice.startswith("kokoro:"):
            raise SystemExit(f"refusing {voice}: this harness renders Kokoro only (no ZeroGPU spend)")
        res = self.req("POST", "/api/render", {"voice": voice, "items": items, "priority": "interactive"})
        keys = [i["key"] for i in res["items"]]
        deadline = time.time() + timeout_s
        while True:
            st = self.req("GET", "/api/render/status?keys=" + ",".join(keys))["items"]
            states = [i["state"] for i in st]
            if all(s == "ready" for s in states):
                return st
            if any(s in ("failed", "deferred") for s in states):
                raise RuntimeError(f"render {voice}: {states} {[i.get('error') for i in st]}")
            if time.time() > deadline:
                raise TimeoutError(f"render {voice} not ready: {states}")
            time.sleep(3)


def render_set(api: Api, voice: str, label: str, segments: dict, out: Path, boundary: str | None) -> None:
    for passage, segs in segments.items():
        d = out / label / passage
        if (d / "meta.json").exists():
            print(f"skip {label}/{passage} (exists)")
            continue
        items = [{"text": s["text"], "boundary": boundary or s["boundary"], "emotion": "neutral"} for s in segs]
        t0 = time.time()
        st = api.render(voice, items)
        d.mkdir(parents=True, exist_ok=True)
        meta = []
        for i, (s, item, status) in enumerate(zip(segs, items, st, strict=True)):
            (d / f"{i:02d}.webm").write_bytes(api.req("GET", f"/api/audio/{status['key']}", raw=True))
            audio = status.get("audio") or {}
            meta.append({"file": f"{i:02d}.webm", "key": status["key"], "text": s["text"], "boundary": item["boundary"],
                         "sentences": s["sentences"], "duration_ms": audio.get("duration_ms"),
                         "words": audio.get("words", [])})
        (d / "meta.json").write_text(json.dumps({"voice": voice, "label": label, "passage": passage,
                                                 "segments": meta}, indent=1))
        print(f"{label}/{passage}: {len(segs)} clips, {sum(m['duration_ms'] or 0 for m in meta) / 1000:.1f} s audio, "
              f"{time.time() - t0:.1f} s wall")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--voices", default=",".join(DEFAULT_VOICES))
    ap.add_argument("--segments", type=Path, default=HERE / "passages/segments.json")
    ap.add_argument("--control", action="store_true", help="also render af_heart with all boundaries 'sentence'")
    a = ap.parse_args()
    segments = json.loads(a.segments.read_text())
    api = Api().login()
    for voice in a.voices.split(","):
        render_set(api, voice, voice.split(":", 1)[1], segments, a.out, None)
    if a.control:
        render_set(api, "kokoro:af_heart", "control_sentence", segments, a.out, "sentence")


if __name__ == "__main__":
    main()
