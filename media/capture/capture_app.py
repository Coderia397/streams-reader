#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Capture the live Streams Reader app reading a document aloud, as a phone-screen texture clip.

    uv run --script media/capture/capture_app.py [media/capture/app-screen.json] [--explore]

Logs in (passphrase from ~/.config/nolabs/app.env, never printed), saves the document's reading progress, moves it to
`start_char`, opens /read/<id> in headless Chrome at 390x844 (DPR 2, mobile), optionally picks the voice whose row
contains `voice_label`, presses Play and records polled DPR-2 screenshots (about 7-14 fps; "mode": "screencast" gives
change-driven frames but only at 1x) for `seconds`. Frames are resampled to a
constant `fps` and piped straight into ffmpeg (VP9 WebM, no frames on disk). The audio the app played is rebuilt from
its media element and saved next to the clip
(<out>.narration.webm, recorded from the media element with MediaRecorder) with a sidecar <out>.json. Progress is restored afterwards.
"""
from __future__ import annotations

import base64, json, os, re, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "media/tools"))
os.environ.setdefault("MEDIA_TMP", str(Path(tempfile.gettempdir()) / "streams-media"))
from cdp import Chrome  # noqa: E402
from streams_api import Api, BASE  # noqa: E402
from ffmpeg import ffmpeg_exe  # noqa: E402

args = [a for a in sys.argv[1:] if not a.startswith("--")]
cfg = json.loads(Path(args[0] if args else ROOT / "media/capture/app-screen.json").read_text())
explore = "--explore" in sys.argv
W, H = cfg["viewport"]
DPR = cfg.get("dpr", 2)
out = ROOT / cfg["out"]
out.parent.mkdir(parents=True, exist_ok=True)

api = Api().login()
docs = api.req("GET", "/api/library")["documents"]
doc = next(d for d in docs if d["meta"]["title"] == cfg["doc_title"] or d["id"] == cfg.get("doc_id"))
saved = api.req("GET", f"/api/documents/{doc['id']}").get("progress")
api.req("PUT", f"/api/documents/{doc['id']}/progress",
        {"char_offset": cfg.get("start_char", 0), "updated_at_ms": int(time.time() * 1000)})

c = Chrome(W, H)
d = c.cdp
try:
    d.call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=DPR, mobile=True)
    d.call("Emulation.setTouchEmulationEnabled", enabled=True)
    d.call("Network.enable")
    d.call("Page.enable")
    for ck in api.cookies():
        d.call("Network.setCookie", name=ck["name"], value=ck["value"], url=BASE + "/", path="/", secure=True,
               httpOnly=True)
    # skip the first-run welcome page
    d.call("Page.navigate", url=BASE + "/settings")
    time.sleep(3)
    d.eval("localStorage.setItem('nolabs.welcomed','true')")
    d.call("Page.navigate", url=f"{BASE}/read/{doc['id']}")
    time.sleep(6)

    def click_text(sel: str, needle: str) -> bool:
        return d.eval(f"""(()=>{{const n={json.dumps(needle)}.toLowerCase();
          const el=[...document.querySelectorAll({json.dumps(sel)})].find(e=>((e.getAttribute('aria-label')||'')+' '+e.innerText).toLowerCase().includes(n));
          if(!el) return false; el.click(); return true}})()""")

    if cfg.get("voice_label"):
        # the voice chip shows the current voice's name; open it and pick the wanted voice
        voices = api.req("GET", "/api/voices")["voices"]
        names = [v["name"] for v in voices if v["id"].startswith("kokoro:")]
        opened = d.eval(f"""(()=>{{const names={json.dumps(names)};
          const el=[...document.querySelectorAll('button.tool')].find(b=>names.includes(b.innerText.trim()));
          if(!el) return false; el.click(); return true}})()""")
        time.sleep(1.5)
        picked = opened and click_text("button,[role=button],[role=option],[role=radio],li,label", cfg["voice_label"])
        time.sleep(1.0)
        d.eval("document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))")
        time.sleep(0.5)
        print(f"voice chip opened={opened} picked={picked}")
    if explore:
        b = d.call("Page.captureScreenshot", format="jpeg", quality=70)["data"]
        Path(os.environ["MEDIA_TMP"], "explore.jpg").write_bytes(base64.b64decode(b))
        print(d.eval("document.body.innerText.slice(0,500)"))
        raise SystemExit(0)

    d.events.clear()
    mode = cfg.get("mode", "screenshot")
    if mode == "screencast":
        d.call("Page.startScreencast", format="jpeg", quality=88, maxWidth=W * DPR, maxHeight=H * DPR,
               everyNthFrame=1)
    frames: list[tuple[float, bytes]] = []
    t0 = time.time()
    played = False
    rec_start = None

    def pump():
        for ev in d.events:
            if ev["method"] == "Page.screencastFrame":
                p = ev["params"]
                frames.append((p["metadata"]["timestamp"], base64.b64decode(p["data"])))
                d.ws.send(json.dumps({"id": 10**9 + p["sessionId"], "method": "Page.screencastFrameAck",
                                      "params": {"sessionId": p["sessionId"]}}))
        d.events.clear()

    # The app's own audio is recorded from its media element (captureStream + MediaRecorder), so the narration
    # is exactly what played, on the same clock as the frames.
    REC = """(()=>{const a=[...document.querySelectorAll('audio,video')].find(x=>!x.paused);
      if(!a) return null; const r=new MediaRecorder(a.captureStream(),{mimeType:'audio/webm;codecs=opus',audioBitsPerSecond:128000});
      window.__chunks=[]; r.ondataavailable=e=>window.__chunks.push(e.data); r.start(250); window.__rec=r;
      return (performance.timeOrigin+performance.now())/1000})()"""
    while time.time() - t0 < cfg["seconds"] + 1.0:
        if not played and time.time() - t0 > 0.6:
            played = click_text("button.play-btn,button", "play")
        elif played and rec_start is None:
            rec_start = d.eval(REC)
        if mode == "screencast":
            d.drain(0.03)
            pump()
        else:
            # polled screenshots: timestamp = midpoint of the capture call (wall clock)
            a = time.time()
            b64 = d.call("Page.captureScreenshot", format="jpeg", quality=85)["data"]
            frames.append(((a + time.time()) / 2, base64.b64decode(b64)))
    if mode == "screencast":
        d.call("Page.stopScreencast")
    pump()
    audio_b64 = d.eval("""new Promise(res=>{const r=window.__rec; if(!r) return res(null);
      r.onstop=()=>{const fr=new FileReader(); fr.onload=()=>res(fr.result.split(',')[1]);
      fr.readAsDataURL(new Blob(window.__chunks,{type:'audio/webm'}))}; r.stop()})""")
finally:
    c.close()
    if saved:
        api.req("PUT", f"/api/documents/{doc['id']}/progress",
                {"char_offset": saved["char_offset"], "updated_at_ms": int(time.time() * 1000)})

if not frames:
    sys.exit("no screencast frames")
# constant-fps resample: hold the latest frame at each output time
fps = cfg["fps"]
start = frames[0][0]
n = int(cfg["seconds"] * fps)
proc = subprocess.Popen([ffmpeg_exe(), "-y", "-loglevel", "error", "-f", "image2pipe", "-c:v", "mjpeg", "-framerate",
                         str(fps), "-i", "-", "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "34", "-row-mt", "1",
                         "-deadline", "good", "-cpu-used", "4", "-pix_fmt", "yuv420p", "-vf", "scale=out_range=tv:flags=lanczos,format=yuv420p", "-color_range", "tv", "-an", str(out)],
                        stdin=subprocess.PIPE)
j = 0
for i in range(n):
    t = start + i / fps
    while j + 1 < len(frames) and frames[j + 1][0] <= t:
        j += 1
    proc.stdin.write(frames[j][1])
proc.stdin.close()
proc.wait()

side = {"doc": doc["id"], "fps": fps, "seconds": cfg["seconds"], "frames_received": len(frames),
        "voice_label": cfg.get("voice_label")}
if audio_b64 and rec_start:
    delay_ms = max(0, int(round((rec_start - start) * 1000)))
    side["narration_delay_ms"] = delay_ms
    raw = Path(tempfile.mkstemp(suffix=".webm", dir=os.environ["MEDIA_TMP"])[1])
    raw.write_bytes(base64.b64decode(audio_b64))
    narr = out.with_suffix(".narration.webm")
    subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(raw), "-af",
                    f"adelay={delay_ms}:all=1,apad,atrim=0:{cfg['seconds']}", "-c:a", "libopus", "-b:a", "96k",
                    str(narr)], check=True)
    raw.unlink()
    side["narration"] = narr.name
out.with_suffix(".json").write_text(json.dumps(side, indent=1))
print(json.dumps(side, indent=1))
print(f"{out} {out.stat().st_size // 1024} KB")
