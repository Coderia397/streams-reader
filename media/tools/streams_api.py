"""Tiny client for the Streams Reader API (listen.streams.watch). Standard library only.

The passphrase is read from ~/.config/nolabs/app.env (PASSPHRASE=...) and never printed.
"""
from __future__ import annotations

import http.cookiejar
import json
import os
import time
import urllib.request
from pathlib import Path

BASE = os.environ.get("STREAMS_READER_URL", "https://listen.streams.watch")


def passphrase() -> str:
    env = Path.home() / ".config/nolabs/app.env"
    for line in env.read_text().splitlines():
        if line.startswith("PASSPHRASE="):
            return line.split("=", 1)[1].strip().strip('"').strip("'")
    raise SystemExit(f"PASSPHRASE missing in {env}")


class Api:
    def __init__(self, base: str = BASE):
        self.base = base.rstrip("/")
        self.jar = http.cookiejar.CookieJar()
        self.op = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.jar))

    def req(self, method: str, path: str, body=None, raw: bool = False):
        data = None if body is None else json.dumps(body).encode()
        r = urllib.request.Request(self.base + path, data=data, method=method)
        r.add_header("Origin", self.base)
        r.add_header("x-nolabs-api", "1")
        r.add_header("User-Agent", "streams-reader-media/1")
        if data is not None:
            r.add_header("Content-Type", "application/json")
        with self.op.open(r, timeout=120) as resp:
            b = resp.read()
            return b if raw else (json.loads(b) if b else None)

    def login(self) -> "Api":
        self.req("POST", "/api/session", {"passphrase": passphrase()})
        return self

    def cookies(self) -> list[dict]:
        return [{"name": c.name, "value": c.value, "domain": c.domain, "path": c.path, "secure": c.secure,
                 "httpOnly": True} for c in self.jar]

    def render(self, voice: str, texts: list[str], timeout_s: float = 600, emotions: list[str] | None = None) -> list[dict]:
        """Render texts with a voice in ONE request, then poll status (never resubmits). Returns the ready status
        items (key, audio{duration_ms, words}). Raises on failed or deferred (GPU budget) items."""
        items = [{"text": t, "boundary": "paragraph", **({"emotion": emotions[i]} if emotions else {})}
                 for i, t in enumerate(texts)]
        res = self.req("POST", "/api/render", {"voice": voice, "items": items, "priority": "interactive"})
        keys = [i["key"] for i in res["items"]]
        deadline = time.time() + timeout_s
        while True:
            st = self.req("GET", "/api/render/status?keys=" + ",".join(keys))["items"]
            states = [i["state"] for i in st]
            if all(s == "ready" for s in states):
                return st
            if any(s == "failed" for s in states):
                raise RuntimeError(f"render failed: {[i.get('error') for i in st]}")
            if any(s == "deferred" for s in states):
                raise RuntimeError(f"render deferred (GPU budget): {[i.get('error') for i in st]}")
            if time.time() > deadline:
                raise TimeoutError(f"render not ready: {states}")
            time.sleep(2)

    def audio(self, key: str) -> bytes:
        return self.req("GET", f"/api/audio/{key}", raw=True)
