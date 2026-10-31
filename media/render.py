#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Render a shot from the Streams Reader film studio.

    uv run --script media/render.py media/shots/promo.json                 # full render: WebM + MP4 + poster (+ VTT)
    uv run --script media/render.py media/shots/promo.json --stills 3,12.5 # PNG stills at those times
    uv run --script media/render.py media/shots/promo.json --range 0:5 --scale 0.5   # quick rough clip
    uv run --script media/render.py --spot media/spots/welcome.json        # transparent PNG + WebP at 512 and 1024
    uv run --script media/render.py --spots                                # every preset in media/spots/

Serves the repository on 127.0.0.1, opens media/scene/index.html in headless Chrome (GPU via ANGLE/Metal by default,
--swiftshader for software GL), calls studio.load(shot) and studio.frame(i) for every frame (time only advances per
frame, randomness is seeded, DPR is fixed at 1), captures each frame with Page.captureScreenshot and pipes it straight
into ffmpeg. No frame files are written. Options: --out DIR (default media/dist/<shot name>), --lang en (voice-over
language), --no-audio, --swiftshader, --headful.
"""
from __future__ import annotations

import argparse
import base64
import functools
import http.server
import json
import os
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

MEDIA = Path(__file__).resolve().parent
ROOT = MEDIA.parent
sys.path.insert(0, str(MEDIA / "tools"))
os.environ.setdefault("MEDIA_TMP", str(Path(tempfile.gettempdir()) / "streams-media"))
from cdp import Chrome  # noqa: E402
from ffmpeg import ffmpeg_exe  # noqa: E402
import audio as audio_mix  # noqa: E402


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def serve() -> tuple[socketserver.TCPServer, int]:
    handler = functools.partial(Quiet, directory=str(ROOT))
    srv = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def resolve_paths(shot: dict, lang: str = "en") -> dict:
    """Paths in shots are relative to media/; the page lives at /media/scene/, so prefix them for the browser.
    Overlay text_id/sub_id are looked up in media/vo/script.<lang>.json "cards"."""
    shot = json.loads(json.dumps(shot))
    sp = MEDIA / "vo" / f"script.{lang}.json"
    cards = json.loads(sp.read_text()).get("cards", {}) if sp.exists() else {}
    for o in shot.get("overlays", []):
        for key, field in (("text_id", "text"), ("sub_id", "sub")):
            if o.get(key) and o[key] in cards:
                o[field] = cards[o[key]]
    for seg in shot.get("segments", []):
        s = seg.get("screen")
        if s and s.get("clip") and not s["clip"].startswith(("/", "http")):
            s["clip"] = "/media/" + s["clip"]
    for o in shot.get("overlays", []):
        if o.get("logo") and not o["logo"].startswith(("/", "http")):
            o["logo"] = "/" + o["logo"]
    return shot


class Session:
    def __init__(self, shot: dict, args, transparent: bool = False):
        self.shot = resolve_paths(shot, getattr(args, "lang", "en"))
        if transparent:
            self.shot["transparent"] = True
        W, H = self.shot["resolution"]
        self.srv, port = serve()
        flags = ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] if args.swiftshader else [
            "--use-angle=metal", "--enable-gpu-rasterization", "--ignore-gpu-blocklist"]
        self.chrome = Chrome(W, H, flags, headful=args.headful)
        d = self.d = self.chrome.cdp
        d.call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=1, mobile=False)
        if transparent:
            d.call("Emulation.setDefaultBackgroundColorOverride", color={"r": 0, "g": 0, "b": 0, "a": 0})
        d.call("Runtime.enable")
        d.call("Page.navigate", url=f"http://127.0.0.1:{port}/media/scene/index.html")
        deadline = time.time() + 30
        while not d.eval("!!window.studioReady"):
            if time.time() > deadline:
                self.dump_errors()
                sys.exit("scene did not load")
            time.sleep(0.2)
        try:
            self.info = d.eval(f"studio.load({json.dumps(self.shot)})")
        except RuntimeError:
            self.dump_errors()
            raise
        print(f"loaded {self.shot.get('name')} {W}x{H} @{self.info['fps']} fps, {self.info['frames']} frames, GL: {self.info['gl']}",
              flush=True)

    def dump_errors(self):
        for ev in self.d.events:
            if ev["method"] in ("Runtime.exceptionThrown", "Runtime.consoleAPICalled"):
                print(json.dumps(ev["params"])[:800], file=sys.stderr)

    def grab(self, i: int, fmt: str = "jpeg", quality: int = 94) -> bytes:
        try:
            # wait for two animation frames so the compositor has presented this frame before the screenshot
            self.d.eval(f"studio.frame({i}).then(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))))")
        except RuntimeError:
            self.dump_errors()
            raise
        params = {"format": fmt, "fromSurface": True}
        if fmt == "jpeg":
            params["quality"] = quality
        return base64.b64decode(self.d.call("Page.captureScreenshot", **params)["data"])

    def close(self):
        self.chrome.close()
        self.srv.shutdown()


def encode_cmd(shot: dict, out: Path, name: str, fps: int, audio: Path | None, frames_from: int, scale: float) -> list[str]:
    enc = shot.get("encode", {})
    w, h = shot["resolution"]
    size = f"{int(w * scale) // 2 * 2}:{int(h * scale) // 2 * 2}:" if scale != 1 else ""
    # JPEG frames are full range; deliver limited-range BT.709 yuv420p
    vf = ["-vf", f"scale={size}out_range=tv:out_color_matrix=bt709:flags=lanczos,format=yuv420p",
          "-color_range", "tv", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"]
    cmd = [ffmpeg_exe(), "-y", "-loglevel", "error", "-f", "image2pipe", "-c:v", "mjpeg", "-framerate", str(fps), "-i", "-"]
    if audio:
        cmd += ["-ss", f"{frames_from / fps:.3f}", "-i", str(audio)]
    wm = enc.get("webm", {})
    codec = wm.get("codec", "av1")
    cmd += ["-map", "0:v"] + (["-map", "1:a"] if audio else []) + vf
    if codec == "av1":
        cmd += ["-c:v", "libsvtav1", "-crf", str(wm.get("crf", 34)), "-preset", str(wm.get("preset", 6)),
                "-g", str(wm.get("gop", fps * 5)), "-svtav1-params", "tune=0:film-grain=0"]
    else:
        cmd += ["-c:v", "libvpx-vp9", "-crf", str(wm.get("crf", 34)), "-b:v", wm.get("maxrate", "0"), "-row-mt", "1",
                "-deadline", "good", "-cpu-used", "3"]
    cmd += ["-pix_fmt", "yuv420p"]
    if audio:
        cmd += ["-c:a", "libopus", "-b:a", "128k"]
    cmd += ["-shortest"] if audio else []
    cmd += [str(out / f"{name}.webm")]
    mp = enc.get("mp4", {})
    cmd += ["-map", "0:v"] + (["-map", "1:a"] if audio else []) + vf
    cmd += ["-c:v", "libx264", "-preset", mp.get("preset", "slow"), "-crf", str(mp.get("crf", 20)), "-pix_fmt", "yuv420p",
            "-profile:v", "high", "-movflags", "+faststart", "-tune", "film"]
    if mp.get("maxrate"):
        cmd += ["-maxrate", mp["maxrate"], "-bufsize", mp.get("bufsize", mp["maxrate"])]
    if audio:
        cmd += ["-c:a", "aac", "-b:a", "160k", "-shortest"]
    cmd += [str(out / (f"{name}.master.mp4" if enc.get("deliver") else f"{name}.mp4"))]
    return cmd


def deliver_mp4(shot: dict, out: Path, name: str):
    """Web MP4s from the high-quality master: two-pass x264 at a fixed video bitrate per size, AAC 128k, faststart."""
    master = out / f"{name}.master.mp4"
    if not master.exists():
        return
    for d in shot["encode"]["deliver"]:
        dst = out / f"{name}{d.get('suffix', '')}.mp4"
        log = out / f".{name}{d.get('suffix', '')}-2pass"
        vf = ["-vf", f"scale=-2:{d['height']}:flags=lanczos,format=yuv420p"]
        common = ["-c:v", "libx264", "-preset", "slow", "-b:v", d["video_bitrate"], "-profile:v", "high", "-tune", "film",
                  "-color_range", "tv", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
                  "-passlogfile", str(log)]
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(master), *vf, *common, "-pass", "1", "-an",
                        "-f", "null", "-"], check=True)
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(master), *vf, *common, "-pass", "2",
                        "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart", str(dst)], check=True)
        for f in out.glob(f".{name}{d.get('suffix', '')}-2pass*"):
            f.unlink()
    master.unlink()


def render_video(shot: dict, args):
    name = shot.get("name", "shot")
    out = Path(args.out or MEDIA / "dist" / name)
    out.mkdir(parents=True, exist_ok=True)
    s = Session(shot, args)
    fps, n = s.info["fps"], s.info["frames"]
    a, b = 0, n
    if args.range:
        x, y = args.range.split(":")
        a, b = int(float(x) * fps), int(float(y) * fps)
    audio = None
    tmpd = Path(tempfile.mkdtemp(dir=os.environ["MEDIA_TMP"]))
    try:
        if shot.get("audio") and not args.no_audio:
            audio = audio_mix.build(shot, args.lang, tmpd)
            vtt = audio_mix.captions_vtt(shot, args.lang)
            if vtt:
                (out / f"{name}.{args.lang}.vtt").write_text(vtt)
        suffix = "" if (a, b) == (0, n) else f"-{a}-{b}"
        proc = subprocess.Popen(encode_cmd(shot, out, name + suffix, fps, audio, a, args.scale), stdin=subprocess.PIPE)
        poster_i = int(shot.get("poster_t", 0) * fps)
        t0 = time.time()
        for i in range(a, b):
            jpg = s.grab(i)
            proc.stdin.write(jpg)
            if i == poster_i or (i == a and not (a <= poster_i < b)):
                (out / f"{name}{suffix}.poster.jpg").write_bytes(jpg)
            if (i - a) % (fps * 2) == 0:
                el = time.time() - t0
                print(f"  frame {i - a}/{b - a}  {el:.0f}s elapsed, {(i - a + 1) / max(el, 1e-3):.1f} fps", flush=True)
        proc.stdin.close()
        proc.wait()
        print(f"render {(time.time() - t0):.0f}s for {b - a} frames")
    finally:
        s.close()
        shutil.rmtree(tmpd, ignore_errors=True)
    if shot.get("encode", {}).get("deliver") and not args.range:
        deliver_mp4(shot, out, name)
    # small poster: re-encode to a compact JPEG
    p = out / f"{name}{suffix}.poster.jpg"
    if p.exists():
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(p), "-q:v", "4", str(p)], check=True)
        # AVIF poster for the landing page (LCP image; budget 60 KB in note 15)
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "quiet", "-i", str(p), "-vf", "scale=out_range=tv,format=yuv420p",
                        "-c:v", "libsvtav1", "-crf", "38", "-preset", "6", "-frames:v", "1", str(p.with_suffix(".avif"))],
                       check=True)
    for f in sorted(out.glob(f"{name}{suffix}.*")):
        print(f"  {f}  {f.stat().st_size / 1e6:.2f} MB")


def render_stills(shot: dict, args):
    name = shot.get("name", "shot")
    out = Path(args.out or MEDIA / "dist" / name)
    out.mkdir(parents=True, exist_ok=True)
    s = Session(shot, args)
    try:
        for t in [float(x) for x in args.stills.split(",")]:
            png = s.grab(int(t * s.info["fps"]), fmt="png")
            p = out / f"{name}-{t:05.2f}s.png"
            p.write_bytes(png)
            print(f"  {p}  {len(png) / 1e6:.2f} MB")
    finally:
        s.close()


def render_spot(path: Path, args, session_cache: dict):
    preset = json.loads(path.read_text())
    name = preset.get("name", path.stem)
    out = Path(args.out or MEDIA / "dist" / "spots")
    out.mkdir(parents=True, exist_ok=True)
    shot = spot_shot(preset)
    s = Session(shot, args, transparent=True)
    try:
        png = s.grab(int(preset.get("t", 0.5) * 30), fmt="png")
    finally:
        s.close()
    src = out / f".{name}-src.png"
    src.write_bytes(png)
    for size in (1024, 512):
        sfx = "" if size == 1024 else "-512"
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(src), "-vf", f"scale={size}:{size}:flags=lanczos",
                        str(out / f"{name}{sfx}.png")], check=True)
        subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(src), "-vf", f"scale={size}:{size}:flags=lanczos",
                        "-c:v", "libwebp", "-quality", "88", "-lossless", "0", str(out / f"{name}{sfx}.webp")], check=True)
    src.unlink()
    print(f"  spot {name}: " + ", ".join(f"{p.name} {p.stat().st_size // 1024} KB" for p in sorted(out.glob(f"{name}*"))))


def spot_shot(preset: dict) -> dict:
    """A one-frame shot in the studio environment from a spot preset."""
    seg = {"start": 0, "end": 1, "environment": {"preset": "studio"}, "character": preset.get("character", {}),
           "camera": {"keys": [{"t": 0, **preset.get("camera", {"pos": [0, 1.0, 4.2], "target": [0, 0.75, 0], "fov": 26})}],
                      "handheld": 0, "focus": [{"t": 0, "on": "face"}]},
           "look": {"dof": {"on": False}}, "props": preset.get("props", [])}
    if preset.get("screen"):
        seg["screen"] = preset["screen"]
    return {"name": "spot-" + preset.get("name", "x"), "fps": 30, "duration": 1, "resolution": [1024, 1024], "seed": 7,
            "segments": [seg]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("shot", nargs="?")
    ap.add_argument("--out")
    ap.add_argument("--stills")
    ap.add_argument("--range")
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--lang", default="en")
    ap.add_argument("--no-audio", action="store_true")
    ap.add_argument("--swiftshader", action="store_true")
    ap.add_argument("--headful", action="store_true")
    ap.add_argument("--spot")
    ap.add_argument("--spots", action="store_true")
    args = ap.parse_args()
    if args.spots or args.spot:
        paths = sorted((MEDIA / "spots").glob("*.json")) if args.spots else [Path(args.spot)]
        for p in paths:
            render_spot(p, args, {})
        return
    shot = json.loads(Path(args.shot).read_text())
    if args.stills:
        render_stills(shot, args)
    else:
        render_video(shot, args)


if __name__ == "__main__":
    main()
