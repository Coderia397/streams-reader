#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Player harness driver. Builds the harness wasm page, serves it on 127.0.0.1, launches headless Chrome, runs every
scenario in harness/scenarios/*.json over the DevTools protocol and checks the player against ground truth computed
from the media element's clock and the fixture durations. Writes harness/out/report.json and prints a summary.

    uv run --script crates/player/harness/run.py [--only NAME] [--no-build] [--headful] [--tmp DIR] [--out FILE]

Checks (ids used in the report):
  tracking      reported segment/position/word agree with ground truth (samples and events)
  seek          seek_by lands within 150 ms of its target (inside/outside the buffer, across boundaries, clamps)
  jump          jump_to/next/previous land on the segment start and report that segment
  timeline      each build appends its segments once, in order, end to end, with the fixture durations
  gapless       one contiguous buffered range from the playhead, no stalls, clock advances at the playback rate
  rapid         command bursts settle to the last command's target
  words         the word the UI shows (last event) matches the fixture timings within 120 ms of wall time and only
                moves forward during normal playback
  media_session setPositionState calls match the segment and position; the last one is current after each step
  ended         Ended / SleepFired / element `ended` fire exactly as often as the scenario expects
  errors        no player Error events or page errors unless the scenario expects them
"""

from __future__ import annotations

import argparse
import bisect
import functools
import glob
import http.server
import json
import os
import re
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from collections import defaultdict
from pathlib import Path

import websocket

HERE = Path(__file__).resolve().parent
PLAYER = HERE.parent
ROOT = PLAYER.parent.parent
DIST = HERE / "dist"

SEEK_TOL_MS = 150
TRACK_TOL_MS = 60
WORD_TOL_MS = 120
POS_STATE_TOL_MS = 150
DURATION_TOL_MS = 45  # two Opus frames: the container's length vs the reported duration
SETTLE_TIMEOUT_MS = 10_000
# Media Session actions the player installs (crates/player/src/web/session.rs).
SESSION_ACTIONS = ("play", "pause", "stop", "seekbackward", "seekforward", "previoustrack", "nexttrack", "seekto")
WORD_FAIL_BUDGET = 0.02  # share of samples (at least one per window) allowed outside the word tolerance: timer jitter


# ------------------------------------------------------------------------------------------------------------ build


def wasm_bindgen_bin() -> str:
    lock = (ROOT / "Cargo.lock").read_text()
    m = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', lock)
    version = m.group(1) if m else ""
    home = Path.home()
    candidates = [
        home / f"Library/Caches/dev.trunkrs.trunk/wasm-bindgen-{version}/wasm-bindgen",
        home / f"Library/Caches/worker-build/wasm-bindgen-x86_64-apple-darwin-{version}/wasm-bindgen",
        home / f"Library/Caches/worker-build/wasm-bindgen-aarch64-apple-darwin-{version}/wasm-bindgen",
        home / f".cache/trunk/wasm-bindgen-{version}/wasm-bindgen",
    ]
    for c in candidates:
        if c.exists():
            return str(c)
    found = shutil.which("wasm-bindgen")
    if found:
        out = subprocess.run([found, "--version"], capture_output=True, text=True).stdout
        if version in out:
            return found
    sys.exit(f"wasm-bindgen {version} not found (run `trunk build` in crates/web once, or cargo install it)")


def build() -> None:
    t = time.time()
    cmd = ["cargo", "build", "-p", "nolabs-player", "--example", "harness", "--target", "wasm32-unknown-unknown"]
    subprocess.run(cmd, cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    wasm = target / "wasm32-unknown-unknown/debug/examples/harness.wasm"
    pkg = DIST / "pkg"
    shutil.rmtree(pkg, ignore_errors=True)
    subprocess.run(
        [wasm_bindgen_bin(), "--target", "web", "--no-typescript", "--out-dir", str(pkg), "--out-name", "harness",
         str(wasm)],
        check=True,
    )
    for f in (HERE / "web").iterdir():
        shutil.copy(f, DIST / f.name)
    print(f"built harness in {time.time() - t:.1f} s", flush=True)


def ensure_fixtures() -> dict:
    manifest = HERE / "fixtures/manifest.json"
    if not manifest.exists():
        subprocess.run(["uv", "run", "--script", str(HERE / "fetch_fixtures.py")], check=True)
    return json.loads(manifest.read_text())


# ----------------------------------------------------------------------------------------------------------- server


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm",
                      ".js": "text/javascript", ".webm": "audio/webm"}

    def log_message(self, *args):  # noqa: D102
        pass

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def serve() -> tuple[socketserver.TCPServer, int]:
    handler = functools.partial(QuietHandler, directory=str(HERE))
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd, httpd.server_address[1]


# ----------------------------------------------------------------------------------------------------------- chrome


def chrome_binary() -> str:
    env = os.environ.get("CHROME")
    if env:
        return env
    apps = sorted(glob.glob("/Applications/Google Chrome*.app")) + sorted(
        glob.glob(str(Path.home() / "Applications/Google Chrome*.app")))
    for app in apps:
        name = Path(app).stem
        binary = Path(app) / "Contents/MacOS" / name
        if binary.exists():
            return str(binary)
    for name in ("google-chrome", "chromium", "chromium-browser"):
        if shutil.which(name):
            return shutil.which(name)  # type: ignore[return-value]
    sys.exit("Chrome not found (set CHROME=/path/to/chrome)")


class Cdp:
    def __init__(self, ws_url: str):
        self.ws = websocket.create_connection(ws_url, timeout=60, suppress_origin=True)
        self.next_id = 0

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

    def eval(self, expr: str):
        res = self.call("Runtime.evaluate", expression=expr, awaitPromise=True, returnByValue=True)
        if "exceptionDetails" in res:
            d = res["exceptionDetails"]
            raise RuntimeError(f"page exception in {expr[:80]}: {d.get('exception', {}).get('description') or d}")
        return res.get("result", {}).get("value")


def launch_chrome(tmp: Path, headful: bool):
    profile = Path(tempfile.mkdtemp(prefix="chrome-profile-", dir=tmp))
    args = [
        chrome_binary(), f"--user-data-dir={profile}", "--remote-debugging-port=0", "--no-first-run",
        "--no-default-browser-check", "--autoplay-policy=no-user-gesture-required", "--mute-audio",
        "--disable-background-timer-throttling", "--disable-renderer-backgrounding",
        "--disable-backgrounding-occluded-windows", "--disable-extensions", "--disable-sync",
        "--window-size=900,700", "about:blank",
    ]
    if not headful:
        args.insert(1, "--headless=new")
    proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    port_file = profile / "DevToolsActivePort"
    deadline = time.time() + 30
    while not (port_file.exists() and port_file.read_text().strip()):
        if time.time() > deadline or proc.poll() is not None:
            proc.kill()
            sys.exit("Chrome did not start")
        time.sleep(0.1)
    port = int(port_file.read_text().split()[0])
    pages = []
    while not pages:
        pages = [p for p in json.loads(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list").read())
                 if p.get("type") == "page"]
        time.sleep(0.1)
    version = json.loads(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version").read()).get("Browser")
    return proc, profile, Cdp(pages[0]["webSocketDebuggerUrl"]), version


# ------------------------------------------------------------------------------------------------------- scenarios


class Checks:
    """Per scenario check results: counts and failure evidence."""

    def __init__(self):
        self.data: dict[str, dict] = {}

    def record(self, check: str, ok: bool, evidence: dict | None = None, soft: bool = False):
        d = self.data.setdefault(check, {"checked": 0, "failed": 0, "failures": [], "notes": []})
        d["checked"] += 1
        if not ok:
            d["failed"] += 1
            if len(d["failures"]) < 12 and evidence is not None:
                d["failures"].append(evidence)
        elif soft and evidence is not None and len(d["notes"]) < 6:
            d["notes"].append(evidence)

    def note(self, check: str, evidence: dict):
        d = self.data.setdefault(check, {"checked": 0, "failed": 0, "failures": [], "notes": []})
        if len(d["notes"]) < 12:
            d["notes"].append(evidence)

    def verdicts(self) -> dict:
        return {k: ("fail" if v["failed"] else "pass") for k, v in self.data.items()}


class Scenario:
    def __init__(self, cdp: Cdp, base_url: str, manifest: dict, spec: dict):
        self.cdp, self.spec = cdp, spec
        self.segs = manifest["segments"]
        self.durs = [s["duration_ms"] for s in self.segs]
        self.cum = [0]
        for d in self.durs:
            self.cum.append(self.cum[-1] + d)
        self.total = self.cum[-1]
        self.base_url = base_url
        self.log: dict[str, list] = defaultdict(list)
        self.checks = Checks()
        self.pending: dict | None = None
        self.skip_ms = {"seekbackward": 15_000, "seekforward": 15_000}  # Media Session default, set by set_skip
        self.t0 = 0.0
        self.windows: list[tuple[float, float]] = []

    # ---- helpers

    def h(self, expr: str):
        return self.cdp.eval(f"window.__harness.{expr}")

    def drain(self):
        for key, items in (self.h("drain()") or {}).items():
            self.log[key].extend(items)

    def rel(self, t: float) -> float:
        return round(t - self.t0, 1)

    def appends(self, build: int, before: float | None = None) -> list[dict]:
        """Completed appends of a build, in order. A segment ends where the next one starts (the buffered end when
        that append began); the last one ends at the buffered end seen when it completed."""
        # An append whose SourceBuffer was removed as it finished (its build was replaced) has no readable end.
        apps = [a for a in self.log["appends"] if a["build"] == build and a.get("ok") == "ok" and a.get("end") is not None]
        for a, b in zip(apps, apps[1:] + [None]):
            a["stop"] = b["start"] if b is not None and b.get("start") is not None else a["end"]
        return [a for a in apps if before is None or a["t1"] <= before]

    def abs_of(self, rec: dict) -> float | None:
        """Document position (ms) of a record's currentTime: the segment whose appended audio holds the clock, plus
        the offset into it. Fixture durations are checked against the appended lengths by the timeline check."""
        apps = self.appends(rec["build"])
        ct = rec.get("ct")
        if not apps or ct is None or any(a["segment"] is None for a in apps):
            return None
        starts = [a["start"] for a in apps]
        i = max(0, bisect.bisect_right(starts, ct + 1e-9) - 1)
        a = apps[i]
        offset = (ct - a["start"]) * 1000.0
        if i == len(apps) - 1:
            offset = min(offset, (a["stop"] - a["start"]) * 1000.0)
        return self.cum[a["segment"]] + min(offset, self.durs[a["segment"]])

    def placed_ct(self, after: float, build: int) -> float | None:
        for s in self.log["samples"]:
            if s["t"] > after and s["build"] == build and s["snap"]["state"] not in ("Loading", "Idle") \
                    and not s["seeking"] and (s["ready"] or 0) >= 1:
                return s["ct"]
        return None

    def locate(self, abs_ms: float) -> tuple[int, float]:
        i = max(0, min(len(self.durs) - 1, bisect.bisect_right(self.cum, abs_ms) - 1))
        return i, abs_ms - self.cum[i]

    def word_at(self, seg: int, pos: float) -> int | None:
        starts = [w[0] for w in self.segs[seg]["words"]]
        i = bisect.bisect_right(starts, pos) - 1
        return i if i >= 0 else None

    def acceptable_words(self, seg: int, pos: float, tol: float) -> set:
        words = self.segs[seg]["words"]
        ok = set()
        for i, w in enumerate(words):
            nxt = words[i + 1][0] if i + 1 < len(words) else float("inf")
            if w[0] <= pos + tol and nxt > pos - tol:
                ok.add(i)
        if not words or words[0][0] > pos - tol:
            ok.add(None)
        return ok

    def near_boundary(self, seg: int, pos: float, tol: float) -> bool:
        return pos < tol or pos > self.durs[seg] - tol

    # ---- run

    def run(self) -> dict:
        cfg = self.spec.get("config", {})
        query = cfg.get("query", "")
        self.cdp.call("Page.navigate", url=f"{self.base_url}{'?' + query if query else ''}")
        deadline = time.time() + 20
        while True:
            try:
                if self.cdp.eval("window.__harness && window.__harness.ready.then(() => true)"):
                    break
            except RuntimeError:
                pass
            if time.time() > deadline:
                raise RuntimeError("harness page did not become ready")
            time.sleep(0.2)
        self.h(f"configure({json.dumps(cfg)})")
        self.t0 = self.h("state()")["t"]
        self.h("sampling(true, 50)")
        started = time.time()
        for step in self.spec["steps"]:
            self.step(step)
        time.sleep(0.2)
        self.h("sampling(false)")
        self.drain()
        self.final_checks()
        return {
            "name": self.spec["name"], "description": self.spec.get("description", ""),
            "seconds": round(time.time() - started, 1), "verdicts": self.checks.verdicts(), "checks": self.checks.data,
        }

    def step(self, step: dict):
        if "do" in step:
            self.command(step["do"], step.get("args", []), step)
        elif "action" in step:
            self.command(f"action:{step['action']}", [step.get("details", {})], step)
        elif "wait" in step:
            time.sleep(step["wait"] / 1000)
            self.drain()
        elif "settle" in step:
            self.settle(step.get("timeout", SETTLE_TIMEOUT_MS))
        elif "observe" in step:
            self.observe(step["observe"])
        elif "until" in step:
            self.until(step["until"], step.get("timeout", 30_000))
        elif "assert" in step:
            self.assert_state(step["assert"])
        elif "configure" in step:
            self.h(f"configure({json.dumps(step['configure'])})")
        else:
            raise ValueError(f"unknown step {step}")
        if "gap" in step:
            time.sleep(step["gap"] / 1000)

    # ---- commands and their expectations

    def command(self, name: str, args: list, step: dict):
        if name.startswith("action:"):
            entry = self.h(f"action({json.dumps(name[7:])}, {json.dumps(args[0])})")
        else:
            entry = self.h(f"cmd({json.dumps(name)}{''.join(', ' + json.dumps(a) for a in args)})")
        self.drain()
        unsettled = self.pending is not None
        if unsettled:
            base = self.pending["abs"]
        else:
            base = self.abs_of(entry)
            if base is None and entry.get("snap"):
                base = self.cum[entry["snap"]["segment"]] + entry["snap"]["segment_pos_ms"]
        kind = name[7:] if name.startswith("action:") else name
        exp: dict | None = None
        if kind in ("load", "reopen"):
            start = args[0] if args else 0
            exp = {"kind": "jump", "abs": self.cum[start], "segment": start}
        elif kind in ("seek_by", "seekforward", "seekbackward"):
            offered = None if kind == "seek_by" else args[0].get("seekOffset")
            delta = args[0] if kind == "seek_by" else (1 if kind == "seekforward" else -1) * (
                int(1000 * offered) if offered else self.skip_ms[kind])
            target = min(max(base + delta, 0), self.total)
            exp = {"kind": "seek", "abs": target, "segment": self.locate(target)[0], "delta": delta}
        elif kind == "seekto":
            seg = self.locate(base)[0]
            target = min(self.cum[seg] + 1000 * args[0]["seekTime"], self.cum[seg + 1])
            exp = {"kind": "seek", "abs": target, "segment": self.locate(target)[0]}
        elif kind == "jump_to":
            exp = {"kind": "jump", "abs": self.cum[args[0]], "segment": args[0]}
        elif kind in ("next", "nexttrack"):
            seg = self.locate(base)[0]
            if seg + 1 < len(self.durs):
                exp = {"kind": "jump", "abs": self.cum[seg + 1], "segment": seg + 1}
        elif kind in ("previous", "previoustrack"):
            seg, pos = self.locate(base)
            target = seg if pos > 3000 else max(seg - 1, 0)
            exp = {"kind": "jump", "abs": self.cum[target], "segment": target}
        elif kind == "set_rate":
            self.check_rate(args[0])
        elif kind == "set_skip":
            self.skip_ms = {"seekbackward": args[0] or 15_000, "seekforward": args[1] or 15_000}
        if step.get("expect") is False:
            exp = None
        if exp is not None:
            chain = (self.pending["chain"] if unsettled else []) + [kind]
            # While playing, the playhead moves on between the presses of a burst: allow for that time.
            drift = 0.0
            if unsettled and (entry.get("snap") or {}).get("state") == "Playing" and not entry.get("paused"):
                drift = self.pending.get("drift_ms", 0.0) + (entry["t"] - self.pending["t"]) * (entry.get("rate") or 1)
            exp.update({"drift_ms": drift})
            exp.update({"t": entry["t"], "chain": chain, "base": base, "cmd": name, "args": args,
                        "build_at_cmd": entry["build"], "state_at_cmd": (entry.get("snap") or {}).get("state")})
            self.pending = exp
        elif unsettled and kind not in ("play", "pause", "set_rate", "set_sleep", "set_skip", "toggle"):
            self.pending = None

    def check_rate(self, rate: float):
        time.sleep(0.05)
        st = self.h("state()")
        want = min(max(rate, 0.5), 3.0)
        ok = abs(st["rate"] - want) < 1e-6 and abs(st["snap"]["rate"] - want) < 1e-6
        self.checks.record("tracking", ok, {"t": self.rel(st["t"]), "what": "rate", "asked": rate,
                                            "element_rate": st["rate"], "snapshot_rate": st["snap"]["rate"]})

    def settle(self, timeout_ms: int):
        deadline = time.time() + timeout_ms / 1000
        stable = 0
        st = None
        while time.time() < deadline:
            st = self.h("state()")
            s = st["snap"]["state"] if st["snap"] else "Idle"
            placed = s in ("Playing", "Paused", "Ended", "Error") and not st["seeking"] and (
                s == "Ended" or (st["ready"] or 0) >= 2)
            stable = stable + 1 if placed else 0
            if stable >= 3:
                break
            time.sleep(0.1)
        self.drain()
        if self.pending is None:
            return
        exp, self.pending = self.pending, None
        if stable < 3:
            self.fail_expectation(exp, {"why": "did not settle", "state": st and st["snap"], "ct": st and st["ct"]})
            return
        self.judge(exp, st)

    def landing(self, exp: dict, st: dict) -> tuple[float | None, str]:
        """Media time where the last command put the playhead, and how it was found."""
        if st["snap"]["state"] == "Ended":
            return st["ct"], "ended"
        if st["paused"]:
            return st["ct"], "paused"
        for e in self.log["media"]:
            if e["t"] > exp["t"] and e["build"] == st["build"] and e["name"] == "seeked":
                return e["ct"], "seeked"
        ct = self.placed_ct(exp["t"], st["build"])
        if ct is not None:
            return ct, "first placed sample"
        return st["ct"] - max(0.0, (st["t"] - exp["t"]) / 1000 * st["rate"]), "extrapolated"

    def fail_expectation(self, exp: dict, evidence: dict):
        for check in self.checks_for(exp):
            self.checks.record(check, False, {"t": self.rel(exp["t"]), "cmd": exp["cmd"], "args": exp["args"],
                                              "chain": exp["chain"], **evidence})

    def checks_for(self, exp: dict) -> list[str]:
        out = ["seek" if exp["kind"] == "seek" else "jump"]
        if len(exp["chain"]) > 1 or exp.get("state_at_cmd") in ("Loading", "Buffering"):
            out.append("rapid")
        return out

    def judge(self, exp: dict, st: dict):
        ct, how = self.landing(exp, st)
        landed = self.abs_of({"ct": ct, "build": st["build"]})
        target = exp["abs"]
        rebuilt = st["build"] != exp["build_at_cmd"]
        ev = {"t": self.rel(exp["t"]), "cmd": exp["cmd"], "args": exp["args"], "chain": exp["chain"],
              "target_ms": round(target), "target_seg_pos": self.fmt(target), "base_ms": round(exp["base"] or 0),
              "rebuilt": rebuilt, "how": how, "state": st["snap"]["state"]}
        if landed is None:
            self.fail_expectation(exp, {**ev, "why": "no audio in the build"})
            return
        err = landed - target
        ev.update({"landed_ms": round(landed), "landed_seg_pos": self.fmt(landed), "error_ms": round(err)})
        ended_ok = target >= self.total - SEEK_TOL_MS and st["snap"]["state"] == "Ended"
        drift = exp.get("drift_ms", 0.0)
        if drift:
            ev["burst_drift_allowed_ms"] = round(drift)
        ok = -SEEK_TOL_MS <= err <= SEEK_TOL_MS + drift or ended_ok
        if exp["kind"] == "jump":
            # Reported segment right after landing: first Snapshot event after the command that is placed.
            snaps = [e for e in self.log["events"] if e["t"] > exp["t"] and e["type"] == "Snapshot"
                     and e["build"] == st["build"] and e["snapshot"]["state"] not in ("Loading",)]
            first = snaps[0]["snapshot"] if snaps else None
            ev["first_snapshot"] = first and {k: first[k] for k in ("state", "segment", "segment_pos_ms", "word")}
            changed = [e["segment"] for e in self.log["events"] if e["t"] > exp["t"] and e["type"] == "SegmentChanged"]
            ev["segment_changed"] = changed[:6]
            if first is None or first["segment"] != exp["segment"]:
                ok = False
                ev["why"] = "first snapshot after the jump reports another segment"
        for check in self.checks_for(exp):
            self.checks.record(check, ok, ev, soft=True)
        # The reported position at settle time against the clock (tracking) and the lock screen (media_session).
        self.check_state_tracking(st, "settle")
        self.check_position_current(st, "settle")

    def fmt(self, abs_ms: float) -> str:
        seg, pos = self.locate(abs_ms)
        return f"{seg}@{pos / 1000:.2f}s"

    # ---- observation

    def observe(self, ms: int):
        st = self.h("state()")
        start = st["t"]
        time.sleep(ms / 1000)
        st = self.h("state()")
        self.drain()
        end = st["t"]
        self.windows.append((start, end))
        self.window_checks(start, end)
        self.check_position_current(st, "observe end")

    def until(self, event_type: str, timeout_ms: int):
        deadline = time.time() + timeout_ms / 1000
        while time.time() < deadline:
            self.drain()
            if any(e["type"] == event_type for e in self.log["events"]):
                return
            time.sleep(0.1)
        self.checks.record("ended", False, {"why": f"no {event_type} within {timeout_ms} ms"})

    def assert_state(self, want: dict):
        """Scenario assertion on the player's state and the true playhead (reported under the `ended` check)."""
        st = self.h("state()")
        self.drain()
        snap = st["snap"]
        abs_ms = self.abs_of(st)
        seg, pos = self.locate(abs_ms) if abs_ms is not None else (None, None)
        problems = []
        if "state" in want and snap["state"] != want["state"]:
            problems.append(f"state {snap['state']}")
        if "segment" in want and (snap["segment"] != want["segment"] or seg != want["segment"]):
            problems.append(f"reported segment {snap['segment']}, playhead in {seg}")
        if "max_pos_ms" in want and pos is not None and pos > want["max_pos_ms"]:
            problems.append(f"playhead {pos:.0f} ms into the segment")
        if want.get("session"):
            # Every action handler installed and the metadata set: the lock screen drives this player.
            missing = sorted(set(SESSION_ACTIONS) - set(st["session"]["handlers"]))
            if missing:
                problems.append(f"no Media Session handler for {', '.join(missing)}")
            if not st["session"]["metadata"]:
                problems.append("no Media Session metadata")
        self.checks.record(want.get("check", "ended"), not problems, {
            "t": self.rel(st["t"]), "want": want, "problems": problems,
            "reported": {k: snap[k] for k in ("state", "segment", "segment_pos_ms")},
            "playhead": None if seg is None else f"{seg}@{pos:.0f}", "ct": st["ct"], "paused": st["paused"]})

    def check_state_tracking(self, st: dict, where: str):
        snap = st["snap"]
        abs_ms = self.abs_of(st)
        if abs_ms is None or snap["state"] in ("Loading", "Idle"):
            return
        seg, pos = self.locate(abs_ms)
        ok = (snap["segment"] == seg and abs(snap["segment_pos_ms"] - pos) <= TRACK_TOL_MS) or (
            self.near_boundary(seg, pos, TRACK_TOL_MS) and abs(self.cum[snap["segment"]] + snap["segment_pos_ms"] - abs_ms)
            <= TRACK_TOL_MS)
        if snap["state"] == "Ended":
            ok = ok or (snap["segment"] == len(self.durs) - 1)
        self.checks.record("tracking", ok, {"t": self.rel(st["t"]), "where": where, "ct": round(st["ct"], 3),
                                            "truth": f"{seg}@{pos:.0f}", "reported": f"{snap['segment']}@"
                                            f"{snap['segment_pos_ms']}", "state": snap["state"]})

    def check_position_current(self, st: dict, where: str):
        """The last setPositionState must describe now (Chrome extrapolates it only while playing)."""
        abs_ms = self.abs_of(st)
        calls = [c for c in self.log["position"] if c["t"] <= st["t"] and not c.get("cleared")]
        if abs_ms is None or not calls or st["snap"]["state"] in ("Loading", "Idle", "Ended"):
            return
        last = calls[-1]
        seg, pos = self.locate(abs_ms)
        playing = not st["paused"]
        shown = last["position"] * 1000 + ((st["t"] - last["t"]) * last["playbackRate"] if playing else 0)
        shown = min(shown, last["duration"] * 1000)
        ok = abs(shown - pos) <= max(POS_STATE_TOL_MS * 2, 300) and abs(last["duration"] * 1000 - self.durs[seg]) <= 60
        if not ok and seg > 0 and pos < 300:
            # Just crossed into a new segment: the update rides the next timeupdate (about 250 ms).
            prev = self.durs[seg - 1]
            ok = abs(last["duration"] * 1000 - prev) <= 60 and shown >= prev - 300
        self.checks.record("media_session", ok, {
            "t": self.rel(st["t"]), "where": where, "truth": f"{seg}@{pos:.0f}/{self.durs[seg]}",
            "lock_screen_shows": f"{shown:.0f}/{last['duration'] * 1000:.0f}", "last_call_age_ms": round(st["t"] - last["t"]),
            "playing": playing})

    def window_checks(self, start: float, end: float):
        samples = [s for s in self.log["samples"] if start <= s["t"] <= end]
        events = [e for e in self.log["events"] if e["type"] in ("Snapshot", "SegmentChanged")]
        playing = [s for s in samples if not s["paused"] and not s["seeking"] and s["snap"]["state"] == "Playing"]
        word_fail, word_total, worst = 0, 0, []
        for s in playing:
            abs_ms = self.abs_of(s)
            if abs_ms is None:
                continue
            seg, pos = self.locate(abs_ms)
            snap = s["snap"]
            # (1) the player's own mapping of the clock
            ok = (snap["segment"] == seg and abs(snap["segment_pos_ms"] - pos) <= TRACK_TOL_MS) or (
                self.near_boundary(seg, pos, TRACK_TOL_MS)
                and abs(self.cum[snap["segment"]] + snap["segment_pos_ms"] - abs_ms) <= TRACK_TOL_MS)
            if snap["segment"] == seg and ok:
                ok = snap["word"] == self.word_at(seg, snap["segment_pos_ms"])
            self.checks.record("tracking", ok, {"t": self.rel(s["t"]), "where": "sample", "ct": round(s["ct"], 3),
                                                "truth": f"{seg}@{pos:.0f} w{self.word_at(seg, pos)}",
                                                "reported": f"{snap['segment']}@{snap['segment_pos_ms']} w{snap['word']}"})
            # (7) what the UI shows: the last event before this instant
            ui_seg, ui_word = None, None
            for e in events:
                if e["t"] > s["t"]:
                    break
                if e["type"] == "SegmentChanged":
                    if e["segment"] != ui_seg:
                        ui_seg, ui_word = e["segment"], None
                else:
                    ui_seg, ui_word = e["snapshot"]["segment"], e["snapshot"]["word"]
            tol = WORD_TOL_MS * max(1.0, s["rate"] or 1.0)
            word_total += 1
            good = ui_seg == seg and ui_word in self.acceptable_words(seg, pos, tol)
            if not good and self.near_boundary(seg, pos, tol):
                if ui_seg == seg - 1 and pos < tol:
                    good = ui_word in self.acceptable_words(seg - 1, self.durs[seg - 1] + pos, tol)
                elif ui_seg == seg + 1 and pos > self.durs[seg] - tol:
                    good = ui_word in self.acceptable_words(seg + 1, pos - self.durs[seg], tol)
            if not good:
                word_fail += 1
                if len(worst) < 8:
                    worst.append({"t": self.rel(s["t"]), "truth": f"{seg}@{pos:.0f} w{self.word_at(seg, pos)}",
                                  "ui": f"{ui_seg} w{ui_word}", "rate": s["rate"]})
        if word_total:
            frac = word_fail / word_total
            self.checks.record("words", word_fail <= max(1, WORD_FAIL_BUDGET * word_total), {
                "window": [self.rel(start), self.rel(end)], "samples": word_total, "outside_tolerance": word_fail,
                "fraction": round(frac, 3), "examples": worst}, soft=True)
        # (7) monotonic: (segment, word) from events inside the window
        prev = None
        for e in self.log["events"]:
            if not (start <= e["t"] <= end) or e["type"] != "Snapshot" or e["snapshot"]["state"] != "Playing":
                continue
            key = (e["snapshot"]["segment"], -1 if e["snapshot"]["word"] is None else e["snapshot"]["word"])
            if prev is not None and key < prev:
                self.checks.record("words", False, {"why": "word went backwards during playback",
                                                    "t": self.rel(e["t"]), "from": prev, "to": key})
            prev = key
        # (5) gapless: contiguous buffer from the playhead to the last appended end, no stalls, clock at rate. Not on
        # the fallback path (?nomse=1): its buffer is one blob at a time and its appends are synthetic.
        if "nomse" in self.spec.get("config", {}).get("query", ""):
            return
        for s in playing:
            apps = self.appends(s["build"], before=s["t"])
            if not apps:
                continue
            last_end = apps[-1]["stop"]
            holding = [r for r in s["ranges"] if r[0] - 0.001 <= s["ct"] <= r[1] + 0.001]
            ok = bool(holding) and holding[0][1] >= last_end - 0.02
            self.checks.record("gapless", ok, {"t": self.rel(s["t"]), "ct": round(s["ct"], 3), "ranges": s["ranges"],
                                               "appended_to": round(last_end, 3)})
        stalls = [m for m in self.log["media"] if start <= m["t"] <= end and m["name"] in ("waiting", "stalled")]
        for m in stalls:
            ahead = max((r[1] - m["ct"] for r in m["ranges"] if r[0] <= m["ct"] <= r[1]), default=0)
            self.checks.record("gapless", ahead < 0.3, {"why": f"{m['name']} with audio buffered", "t": self.rel(m["t"]),
                                                       "ct": round(m["ct"], 3), "ranges": m["ranges"]})
        if len(playing) >= 10:
            a, b = playing[0], playing[-1]
            if a["build"] == b["build"]:
                wall = (b["t"] - a["t"]) / 1000
                media_s = b["ct"] - a["ct"]
                rate = b["rate"] or 1.0
                ratio = media_s / (wall * rate) if wall > 0 else 1
                self.checks.record("gapless", abs(ratio - 1) <= 0.06, {
                    "why": "media clock vs wall clock", "window": [self.rel(start), self.rel(end)],
                    "media_s": round(media_s, 3), "wall_s": round(wall, 3), "rate": rate, "ratio": round(ratio, 3)},
                    soft=True)

    # ---- whole-scenario checks

    def final_checks(self):
        L = self.log
        builds = sorted({a["build"] for a in L["appends"]} | {b["build"] for b in L["builds"]})
        build_t = {b["build"]: b["t"] for b in L["builds"]}
        for b in builds:
            apps = [a for a in L["appends"] if a["build"] == b]
            okapps = self.appends(b)
            segs = [a["segment"] for a in okapps]
            problems = []
            if len(set(segs)) != len(segs):
                problems.append("duplicate segment")
            if any(y != x + 1 for x, y in zip(segs, segs[1:])):
                problems.append("segments out of order or missing")
            if okapps and abs(okapps[0]["start"]) > 0.001:
                problems.append("first append does not start at 0")
            drift = []
            for prev, a in zip([None] + okapps, okapps):
                if prev is not None and abs(a["start"] - prev["end"]) > 0.001:
                    problems.append(f"append of {a['segment']} starts at {a['start']:.4f}, previous ended {prev['end']:.4f}")
                if a["segment"] is not None:
                    d = (a["stop"] - a["start"]) * 1000 - self.durs[a["segment"]]
                    drift.append(round(d, 1))
                    if abs(d) > DURATION_TOL_MS:
                        problems.append(f"segment {a['segment']} is {d:+.0f} ms against the fixture duration")
            nxt = build_t.get(b + 1)
            stale = [a["segment"] for a in apps if nxt is not None and a["t0"] > nxt]
            if stale:
                problems.append(f"appends to a replaced build: {stale}")
            failed = [a for a in apps if a.get("ok") not in ("ok", "abort", None) or (a.get("ok") == "abort" and nxt is None)]
            if failed:
                problems.append(f"{len(failed)} append(s) ended with {sorted({a.get('ok') for a in failed})}")
            self.checks.record("timeline", not problems, {
                "build": b, "segments": segs, "per_segment_delta_ms": drift[:30],
                "end_s": round(okapps[-1]["end"], 3) if okapps else None, "problems": problems[:10]}, soft=True)
        # provider asked twice for the same segment within one build
        # media session: each setPositionState against the clock at that moment
        for c in L["position"]:
            if c.get("cleared"):
                continue
            abs_ms = self.abs_of(c)
            if abs_ms is None:
                continue
            seg, pos = self.locate(abs_ms)
            ok = (abs(c["duration"] * 1000 - self.durs[seg]) <= 60 and abs(c["position"] * 1000 - pos) <= POS_STATE_TOL_MS
                  and abs(c["playbackRate"] - (c["rate"] or 1)) < 1e-6)
            if not ok and self.near_boundary(seg, pos, POS_STATE_TOL_MS):
                ok = True
            self.checks.record("media_session", ok, {
                "t": self.rel(c["t"]), "call": {k: round(c[k], 3) for k in ("duration", "position", "playbackRate")},
                "truth": f"{seg}@{pos:.0f}/{self.durs[seg]}", "element_rate": c["rate"]})
        # events that must fire a set number of times
        expect = self.spec.get("expect_events", {})
        counts = {
            "Ended": sum(1 for e in L["events"] if e["type"] == "Ended"),
            "SleepFired": sum(1 for e in L["events"] if e["type"] == "SleepFired"),
            "media_ended": sum(1 for m in L["media"] if m["name"] == "ended"),
        }
        for key, want in expect.items():
            if key in counts:
                self.checks.record("ended", counts[key] == want, {"event": key, "expected": want, "got": counts[key],
                                                                  "times": [self.rel(e["t"]) for e in L["events"]
                                                                            if e["type"] == key]})
        if "Ended" not in expect and counts["Ended"] > 1:
            self.checks.record("ended", False, {"event": "Ended", "got": counts["Ended"], "why": "more than once"})
        errors = [{"t": self.rel(e["t"]), "message": e["message"]} for e in L["events"] if e["type"] == "Error"]
        errors += [{"t": self.rel(e["t"]), "page": e["message"]} for e in L["errors"]]
        allowed = self.spec.get("allow_errors", 0)
        self.checks.record("errors", len(errors) <= allowed, {"errors": errors[:10], "allowed": allowed})
        self.checks.note("timeline", {"provider_calls": [(p["segment"], self.rel(p["t"])) for p in L["provider"]][:60]})


# ---------------------------------------------------------------------------------------------------------- main


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only", action="append", help="run only scenarios whose name contains this (repeatable)")
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--headful", action="store_true")
    ap.add_argument("--tmp", help="directory for the throwaway Chrome profile (default: system temp)")
    ap.add_argument("--out", default=str(HERE / "out/report.json"))
    ap.add_argument("--raw", action="store_true", help="also write every scenario's raw logs next to the report")
    args = ap.parse_args()

    started = time.time()
    manifest = ensure_fixtures()
    if not args.no_build or not (DIST / "pkg/harness.js").exists():
        build()
    for f in (HERE / "web").iterdir():
        shutil.copy(f, DIST / f.name)
    httpd, port = serve()
    tmp = Path(args.tmp) if args.tmp else Path(tempfile.gettempdir())
    tmp.mkdir(parents=True, exist_ok=True)
    proc, profile, cdp, version = launch_chrome(tmp, args.headful)
    base_url = f"http://127.0.0.1:{port}/dist/index.html"
    results, raw = [], {}
    try:
        cdp.call("Page.enable")
        cdp.call("Runtime.enable")
        files = sorted((HERE / "scenarios").glob("*.json"))
        for path in files:
            spec = json.loads(path.read_text())
            spec.setdefault("name", path.stem)
            if args.only and not any(o in spec["name"] for o in args.only):
                continue
            print(f"- {spec['name']} ...", end="", flush=True)
            sc = Scenario(cdp, base_url, manifest, spec)
            try:
                res = sc.run()
            except Exception as exc:  # noqa: BLE001 - one broken scenario must not hide the others
                res = {"name": spec["name"], "verdicts": {"harness": "fail"}, "checks": {"harness": {
                    "checked": 1, "failed": 1, "failures": [{"exception": repr(exc)}], "notes": []}}}
            results.append(res)
            if args.raw:
                raw[spec["name"]] = sc.log
            bad = [k for k, v in res["verdicts"].items() if v == "fail"]
            print(f" {res.get('seconds', 0)} s  " + ("ok" if not bad else "FAIL " + ", ".join(bad)), flush=True)
    finally:
        try:
            cdp.ws.close()
        except Exception:  # noqa: BLE001
            pass
        proc.terminate()
        try:
            proc.wait(5)
        except subprocess.TimeoutExpired:
            proc.kill()
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)

    summary: dict[str, str] = {}
    for res in results:
        for k, v in res["verdicts"].items():
            summary[k] = "fail" if v == "fail" or summary.get(k) == "fail" else "pass"
    report = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "seconds": round(time.time() - started, 1),
        "browser": version, "fixtures": {"voice": manifest.get("voice"), "segments": len(manifest["segments"]),
                                         "audio_ms": sum(s["duration_ms"] for s in manifest["segments"])},
        "summary": summary, "scenarios": results,
    }
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=1))
    if raw:
        out.with_name(out.stem + "-raw.json").write_text(json.dumps(raw))
    print(f"\n{'check':<14} result")
    for k in sorted(summary):
        print(f"{k:<14} {summary[k]}")
    print(f"\nreport: {out}  ({report['seconds']} s)")
    return 0 if all(v == "pass" for v in summary.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
