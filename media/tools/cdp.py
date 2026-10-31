"""Minimal Chrome DevTools Protocol driver (headless Chrome, websocket-client)."""
from __future__ import annotations

import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

import websocket

SCRATCH = Path(os.environ.get("MEDIA_TMP", tempfile.gettempdir()))


def chrome_binary() -> str:
    if os.environ.get("CHROME"):
        return os.environ["CHROME"]
    for app in sorted(glob.glob("/Applications/Google Chrome*.app")) + sorted(
            glob.glob(str(Path.home() / "Applications/Google Chrome*.app"))):
        b = Path(app) / "Contents/MacOS" / Path(app).stem
        if b.exists():
            return str(b)
    for n in ("google-chrome", "chromium", "chromium-browser"):
        if shutil.which(n):
            return shutil.which(n)
    sys.exit("Chrome not found (set CHROME=/path/to/chrome)")


class Cdp:
    def __init__(self, ws_url: str):
        self.ws = websocket.create_connection(ws_url, timeout=300, suppress_origin=True, max_size=None)
        self.next_id = 0
        self.events: list[dict] = []

    def call(self, method: str, **params):
        self.next_id += 1
        mid = self.next_id
        self.ws.send(json.dumps({"id": mid, "method": method, "params": params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == mid:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})
            if "method" in msg:
                self.events.append(msg)

    def drain(self, timeout: float = 0.02) -> None:
        """Read pushed events (screencast frames etc.) for up to `timeout` seconds into self.events."""
        self.ws.settimeout(timeout)
        try:
            while True:
                msg = json.loads(self.ws.recv())
                if "method" in msg:
                    self.events.append(msg)
        except (websocket.WebSocketTimeoutException, TimeoutError):
            pass
        finally:
            self.ws.settimeout(300)

    def eval(self, expr: str):
        res = self.call("Runtime.evaluate", expression=expr, awaitPromise=True, returnByValue=True)
        if "exceptionDetails" in res:
            d = res["exceptionDetails"]
            raise RuntimeError(f"page exception in {expr[:80]}: {d.get('exception', {}).get('description') or d}")
        return res.get("result", {}).get("value")


class Chrome:
    """Launch headless Chrome with a throwaway profile; .cdp is the first page's session."""

    def __init__(self, width: int, height: int, extra: list[str] | None = None, headful: bool = False):
        SCRATCH.mkdir(parents=True, exist_ok=True)
        self.profile = Path(tempfile.mkdtemp(prefix="chrome-profile-", dir=SCRATCH))
        args = [chrome_binary(), f"--user-data-dir={self.profile}", "--remote-debugging-port=0", "--no-first-run",
                "--no-default-browser-check", "--autoplay-policy=no-user-gesture-required", "--mute-audio",
                "--disable-background-timer-throttling", "--disable-renderer-backgrounding",
                "--disable-backgrounding-occluded-windows", "--disable-extensions", "--disable-sync",
                "--hide-scrollbars", f"--window-size={width},{height}", *(extra or []), "about:blank"]
        if not headful:
            args.insert(1, "--headless=new")
        self.proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        pf = self.profile / "DevToolsActivePort"
        deadline = time.time() + 30
        while not (pf.exists() and pf.read_text().strip()):
            if time.time() > deadline or self.proc.poll() is not None:
                self.close()
                sys.exit("Chrome did not start")
            time.sleep(0.1)
        port = int(pf.read_text().split()[0])
        pages = []
        while not pages:
            pages = [p for p in json.loads(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list").read())
                     if p.get("type") == "page"]
            time.sleep(0.1)
        self.cdp = Cdp(pages[0]["webSocketDebuggerUrl"])
        self.cdp.call("Emulation.setDeviceMetricsOverride", width=width, height=height, deviceScaleFactor=1,
                      mobile=False)

    def close(self):
        try:
            self.proc.terminate()
            self.proc.wait(10)
        except Exception:
            self.proc.kill()
        shutil.rmtree(self.profile, ignore_errors=True)
