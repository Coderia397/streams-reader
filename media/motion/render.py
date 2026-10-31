#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["websocket-client>=1.8,<2"]
# ///
"""Render a Streams Reader product-motion timeline (media/motion/timelines/*.json).

    uv run --script media/motion/render.py media/motion/timelines/reel.json             # WebM + MP4 + poster (+ VTT)
    uv run --script media/motion/render.py media/motion/timelines/reel-portrait.json --lang en
    uv run --script media/motion/render.py media/motion/timelines/loop-*.json          # several in a row
    uv run --script media/motion/render.py media/motion/timelines/reel.json --stills 3,9.5   # PNG stills
    uv run --script media/motion/render.py media/motion/timelines/reel.json --range 8:14 --scale 0.5   # rough clip
    uv run --script media/motion/render.py --qa media/motion/dist/reel/reel.mp4        # contact sheets every 0.5 s

A timeline may "extends" another (deep merge, lists replaced). Copy comes from strings/<lang>.json (missing keys fall
back to English). The "reading" track names a voice and a span of assets/reading/<voice>.mp3 (Kokoro renders of the
site's demo passage with per-word timings); it drives both the word highlight and the narration in the mix.
Frames: headless Chrome steps the page with motion.frame(i) (time is a pure input, fonts are awaited, nothing animates
by itself), Page.captureScreenshot JPEGs are piped into ffmpeg; no frame files are written. Options: --out DIR
(default media/motion/dist/<name>), --blur N (N sub-frames per frame, 180 degree shutter), --swiftshader, --headful.
"""
from __future__ import annotations

import argparse, array, base64, functools, http.server, json, os, re, shutil, socketserver, subprocess, sys, tempfile
import threading, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
MEDIA = HERE.parent
ROOT = MEDIA.parent
sys.path.insert(0, str(MEDIA / "tools"))
os.environ.setdefault("MEDIA_TMP", str(Path(tempfile.gettempdir()) / "streams-media"))
Path(os.environ["MEDIA_TMP"]).mkdir(parents=True, exist_ok=True)
from cdp import Chrome  # noqa: E402
from ffmpeg import ffmpeg_exe  # noqa: E402

DEMO = HERE / "assets/reading"  # Kokoro renders of the site demo passage, copied from crates/site (see README)


# ---------------------------------------------------------------- timeline
def merge(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        out = dict(a)
        for k, v in b.items():
            out[k] = merge(a[k], v) if k in a else v
        return out
    return b


def load_timeline(path: Path) -> dict:
    tl = json.loads(path.read_text())
    if tl.get("extends"):
        ext = Path(tl.pop("extends"))
        base = load_timeline(ext if ext.is_absolute() else path.parent / ext)
        tl = merge(base, tl)
    return tl


def load_strings(lang: str) -> dict:
    en = json.loads((HERE / "strings/en.json").read_text())
    p = HERE / f"strings/{lang}.json"
    return merge(en, json.loads(p.read_text())) if p.exists() and lang != "en" else en


def reading_words(tl: dict) -> list[dict]:
    """Word highlight schedule in reel time from the reading segments."""
    out = []
    for seg in tl.get("reading", []):
        words = json.loads((DEMO / f"{seg['voice']}.json").read_text())["words"]
        tempo = seg.get("tempo", 1.0)
        span = [(i, w) for i, w in enumerate(words) if w["s"] >= seg["from"] - 1e-3 and w["e"] <= seg["to"] + 0.05]
        for j, (i, w) in enumerate(span):
            out.append({"i": i, "s": round(seg["at"] + (w["s"] - seg["from"]) / tempo, 4),
                        "e": round(seg["at"] + (w["e"] - seg["from"]) / tempo, 4), "last": j == len(span) - 1})
    out.sort(key=lambda w: w["s"])
    return out


# ---------------------------------------------------------------- audio
def run_ff(args: list[str], **kw):
    return subprocess.run([ffmpeg_exe(), "-y", "-hide_banner", "-loglevel", "error", *args], check=True, **kw)


def narration(tl: dict, tmpd: Path) -> Path | None:
    segs = tl.get("reading", [])
    if not segs:
        return None
    dur = float(tl["duration"])
    inputs, chains = [], []
    for n, seg in enumerate(segs):
        inputs += ["-i", str(DEMO / f"{seg['voice']}.mp3")]
        a, b = max(0.0, seg["from"] - 0.06), seg["to"] + 0.12
        tempo = seg.get("tempo", 1.0)
        delay = int(round((seg["at"] - (seg["from"] - a) / tempo) * 1000))
        tchain = f",atempo={tempo}" if abs(tempo - 1) > 1e-3 else ""
        fl = (b - a) / tempo
        chains.append(f"[{n}:a]aresample=48000,aformat=channel_layouts=stereo,atrim={a}:{b},asetpts=PTS-STARTPTS{tchain},"
                      f"afade=t=in:d=0.04,afade=t=out:st={max(0, fl - 0.08):.3f}:d=0.08,volume={seg.get('gain_db', 0)}dB,"
                      f"adelay={max(0, delay)}:all=1[r{n}]")
    mix = "".join(f"[r{n}]" for n in range(len(segs)))
    chains.append(f"{mix}amix=inputs={len(segs)}:normalize=0:duration=longest,apad,atrim=0:{dur}[out]")
    out = tmpd / "narration.wav"
    run_ff([*inputs, "-filter_complex", ";".join(chains), "-map", "[out]", "-ar", "48000", "-ac", "2", str(out)])
    return out


def envelope(wav: Path, rate: int) -> list[float]:
    """RMS per 1/rate s of a WAV (mono 16-bit decode), mapped to 0..1 with a soft knee."""
    raw = subprocess.run([ffmpeg_exe(), "-hide_banner", "-loglevel", "error", "-i", str(wav), "-ac", "1", "-ar", "24000",
                          "-f", "s16le", "-"], capture_output=True, check=True).stdout
    a = array.array("h"); a.frombytes(raw)
    hop = 24000 // rate
    vals = []
    for i in range(0, len(a) - hop + 1, hop):
        seg = a[i:i + hop]
        vals.append((sum(x * x for x in seg) / hop) ** 0.5 / 32768)
    peak = max(vals) if vals and max(vals) > 0 else 1
    sm, out = 0.0, []
    for v in vals:
        x = (v / peak) ** 0.6
        sm = x if x > sm else sm * 0.82 + x * 0.18   # fast attack, soft release
        out.append(round(sm, 3))
    return out


def mix(tl: dict, narr: Path | None, tmpd: Path) -> Path | None:
    A = tl.get("audio")
    if not A:
        return None
    dur = float(tl["duration"])
    M = A["music"]
    inputs = ["-i", str(MEDIA / M["file"])]
    fo = M.get("fade_out", 2.5)
    f = [f"[0:a]aresample=48000,aformat=channel_layouts=stereo,atrim={M.get('in', 0)}:{M.get('in', 0) + dur},asetpts=PTS-STARTPTS,"
         f"afade=t=in:d={M.get('fade_in', 1.0)},afade=t=out:st={dur - fo}:d={fo},volume={M.get('gain_db', 0)}dB,apad,atrim=0:{dur}[mus]"]
    if narr:
        inputs += ["-i", str(narr)]
        f.append(f"[1:a]volume={A.get('voice_db', 0)}dB,asplit=2[vk][vm]")
        f.append(f"[mus][vk]sidechaincompress=threshold={M.get('duck_threshold', 0.02)}:ratio={M.get('duck_ratio', 5)}:"
                 f"attack=40:release=500:makeup=1[duck]")
        f.append("[vm][duck]amix=inputs=2:normalize=0:duration=first[mix]")
    else:
        f.append("[mus]anull[mix]")
    raw = tmpd / "mix_raw.wav"
    run_ff([*inputs, "-filter_complex", ";".join(f), "-map", "[mix]", "-ar", "48000", "-ac", "2", str(raw)])
    I, TP = A.get("loudness", -16), A.get("true_peak", -1.5)
    meas = subprocess.run([ffmpeg_exe(), "-hide_banner", "-i", str(raw), "-af", f"loudnorm=I={I}:TP={TP}:LRA=11:print_format=json",
                           "-f", "null", "-"], capture_output=True, text=True).stderr
    js = json.loads(re.search(r"\{[^{}]*\"input_i\"[^{}]*\}", meas, re.S).group(0))
    out = tmpd / "mix.wav"
    run_ff(["-i", str(raw), "-af", f"loudnorm=I={I}:TP={TP}:LRA=11:measured_I={js['input_i']}:measured_TP={js['input_tp']}:"
            f"measured_LRA={js['input_lra']}:measured_thresh={js['input_thresh']}:offset={js['target_offset']}:linear=true,"
            f"aresample=48000", "-ar", "48000", str(out)])
    return out


def loudness(path: Path) -> str:
    err = subprocess.run([ffmpeg_exe(), "-hide_banner", "-i", str(path), "-af", "ebur128=peak=true", "-f", "null", "-"],
                         capture_output=True, text=True).stderr
    i = re.findall(r"I:\s+(-?[\d.]+) LUFS", err)
    tp = re.findall(r"Peak:\s+(-?[\d.]+) dBFS", err)
    return f"{i[-1] if i else '?'} LUFS, true peak {tp[-1] if tp else '?'} dBFS"


def captions(tl: dict, strings: dict) -> str | None:
    words = tl.get("words") or []
    if not words:
        return None
    text = strings["doc"]["paragraphs"][0].split(" ")
    cues, cur = [], None
    for w in words:
        if cur and w["s"] - cur[1] < 0.9:
            cur[1] = w["e"]; cur[2].append(text[w["i"]])
        else:
            if cur: cues.append(cur)
            cur = [w["s"], w["e"], [text[w["i"]]]]
    if cur: cues.append(cur)
    ts = lambda s: f"{int(s // 3600):02d}:{int(s % 3600 // 60):02d}:{s % 60:06.3f}"
    body = "\n".join(f"{i + 1}\n{ts(a)} --> {ts(b + 0.3)}\n{' '.join(t)}\n" for i, (a, b, t) in enumerate(cues))
    return f"WEBVTT\nKind: captions\nLanguage: {strings.get('lang', 'en')}\n\n{body}"


# ---------------------------------------------------------------- browser
class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


class Session:
    def __init__(self, tl: dict, args):
        W, H = tl["size"]
        handler = functools.partial(Quiet, directory=str(ROOT))
        self.srv = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
        self.srv.daemon_threads = True
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        port = self.srv.server_address[1]
        flags = ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] if args.swiftshader else [
            "--use-angle=metal", "--enable-gpu-rasterization", "--ignore-gpu-blocklist"]
        flags += ["--force-color-profile=srgb", "--font-render-hinting=none"]
        self.chrome = Chrome(W, H, flags, headful=args.headful)
        d = self.d = self.chrome.cdp
        self.ss = tl.get("supersample", 2)
        d.call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=self.ss, mobile=False)
        d.call("Emulation.setEmulatedMedia", features=[{"name": "prefers-color-scheme", "value": "dark" if tl.get("theme", "ink") != "paper" else "light"},
                                                        {"name": "prefers-reduced-motion", "value": "no-preference"}])
        d.call("Runtime.enable")
        d.call("Page.navigate", url=f"http://127.0.0.1:{port}/media/motion/index.html")
        deadline = time.time() + 30
        while not d.eval("!!window.motionReady"):
            if time.time() > deadline:
                self.dump(); sys.exit("motion page did not load")
            time.sleep(0.1)
        try:
            self.info = d.eval(f"motion.load({json.dumps(tl)})")
        except RuntimeError:
            self.dump(); raise

    def dump(self):
        for ev in self.d.events:
            if ev["method"] in ("Runtime.exceptionThrown", "Runtime.consoleAPICalled"):
                print(json.dumps(ev["params"])[:600], file=sys.stderr)

    def grab(self, t: float, fmt="jpeg", q=93) -> bytes:
        try:
            self.d.eval(f"motion.seek({t:.6f}).then(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))))")
        except RuntimeError:
            self.dump(); raise
        p = {"format": fmt, "fromSurface": True}
        if fmt == "jpeg":
            p["quality"] = q
        return base64.b64decode(self.d.call("Page.captureScreenshot", **p)["data"])

    def close(self):
        self.chrome.close(); self.srv.shutdown()


# ---------------------------------------------------------------- encode
COLOR = ["-color_range", "tv", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"]


def size_of(p: Path) -> str:
    return f"{p.stat().st_size / 1e6:.2f} MB"


def deliver(tl: dict, master: Path, audio: Path | None, out: Path, name: str):
    enc = tl.get("encode", {})
    dur = float(tl["duration"])
    vf = "scale=out_range=tv:out_color_matrix=bt709:flags=lanczos,format=yuv420p"
    if enc.get("scale"):
        vf = f"scale={enc['scale']}:flags=lanczos," + vf
    ain = ["-i", str(audio)] if audio else []
    amap = ["-map", "1:a"] if audio else []
    budget = enc.get("max_bytes")
    # WebM (AV1)
    crf = enc.get("webm_crf", 32)
    while True:
        run_ff(["-i", str(master), *ain, "-map", "0:v", *amap, "-vf", vf, *COLOR, "-c:v", "libsvtav1", "-crf", str(crf),
                "-preset", str(enc.get("preset", 5)), "-g", str(int(tl["fps"] * 4)), "-svtav1-params", "tune=0:film-grain=0",
                *(["-c:a", "libopus", "-b:a", "128k"] if audio else ["-an"]), "-shortest" if audio else "-y",
                str(out / f"{name}.webm")])
        if not budget or (out / f"{name}.webm").stat().st_size <= budget or crf >= 55:
            break
        crf += 3
    # MP4 (H.264): CRF, or two-pass at a bitrate that fits the budget
    mp4 = out / f"{name}.mp4"
    x264 = ["-c:v", "libx264", "-preset", "slow", "-profile:v", "high", "-pix_fmt", "yuv420p", "-tune", "animation"]
    if budget:
        kbps = int(budget * 8 / dur / 1000 * 0.93)
        log = Path(os.environ["MEDIA_TMP"]) / f"{name}-2pass"
        common = ["-i", str(master), "-vf", vf, *COLOR, *x264, "-b:v", f"{kbps}k", "-passlogfile", str(log)]
        run_ff([*common, "-pass", "1", "-an", "-f", "null", "-"])
        run_ff([*common, "-pass", "2", "-an", "-movflags", "+faststart", str(mp4)])
        for f in log.parent.glob(log.name + "*"):
            f.unlink()
    else:
        run_ff(["-i", str(master), *ain, "-map", "0:v", *amap, "-vf", vf, *COLOR, *x264, "-crf", str(enc.get("mp4_crf", 19)),
                "-maxrate", enc.get("maxrate", "9M"), "-bufsize", "18M",
                *(["-c:a", "aac", "-b:a", "160k", "-shortest"] if audio else ["-an"]), "-movflags", "+faststart", str(mp4)])
    # posters
    pt = tl.get("poster_t", 0)
    jpg = out / f"{name}.poster.jpg"
    run_ff(["-ss", f"{pt}", "-i", str(master), "-frames:v", "1", "-vf", (f"scale={enc['scale']}:flags=lanczos" if enc.get("scale") else "null"),
            "-q:v", "3", str(jpg)])
    run_ff(["-i", str(jpg), "-vf", "scale=out_range=tv,format=yuv420p", "-c:v", "libsvtav1", "-crf", "36", "-preset", "6",
            "-frames:v", "1", str(jpg.with_suffix(".avif"))])
    run_ff(["-i", str(jpg), "-c:v", "libwebp", "-quality", "82", str(jpg.with_suffix(".webp"))])


def render(path: Path, args):
    tl = load_timeline(path)
    name = tl.get("name", path.stem)
    strings = load_strings(args.lang)
    tl["strings"] = strings
    if args.lang != "en":
        name += f"-{args.lang}"
    out = Path(args.out or HERE / "dist" / name)
    out.mkdir(parents=True, exist_ok=True)
    tmpd = Path(tempfile.mkdtemp(dir=os.environ["MEDIA_TMP"]))
    t_start = time.time()
    try:
        tl["words"] = reading_words(tl)
        narr = narration(tl, tmpd)
        tl["env"] = envelope(narr, 60) if narr else []
        tl["env_rate"] = 60
        fps = tl["fps"]
        n = int(round(tl["duration"] * fps))
        s = Session(tl, args)
        print(f"{name}: {tl['size'][0]}x{tl['size'][1]} @{fps} fps, {n} frames, {tl['duration']} s", flush=True)
        try:
            if args.stills:
                for t in [float(x) for x in args.stills.split(",")]:
                    p = out / f"{name}-{t:05.2f}s.png"
                    p.write_bytes(s.grab(t, "png"))
                    if s.ss != 1:
                        run_ff(["-i", str(p), "-vf", f"scale={tl['size'][0]}:{tl['size'][1]}:flags=lanczos", str(p) + ".png"])
                        os.replace(str(p) + ".png", p)
                    print(f"  {p}")
                return
            a, b = 0, n
            if args.range:
                x, y = args.range.split(":"); a, b = int(float(x) * fps), int(float(y) * fps)
            N = max(1, args.blur or tl.get("motion_blur", 1))
            W, H = tl["size"]
            size = f"scale={int(W * args.scale) // 2 * 2}:{int(H * args.scale) // 2 * 2}:flags=lanczos,"
            vf = (f"tmix=frames={N},select='eq(mod(n\\,{N})\\,{N - 1})',setpts=N/({fps}*TB)," if N > 1 else "") + size + "format=yuv444p"
            master = tmpd / "master.mkv"
            proc = subprocess.Popen([ffmpeg_exe(), "-y", "-loglevel", "error", "-f", "image2pipe", "-c:v", "mjpeg",
                                     "-framerate", str(fps * N), "-i", "-", "-vf", vf, "-c:v", "libx264", "-preset", "veryfast",
                                     "-crf", "8", "-r", str(fps), str(master)], stdin=subprocess.PIPE)
            t0 = time.time()
            shutter = tl.get("shutter", 0.5)
            for i in range(a, b):
                for k in range(N):
                    t = (i - (shutter * (N - 1 - k) / max(1, N - 1) if N > 1 else 0)) / fps
                    proc.stdin.write(s.grab(max(0.0, t)))
                if (i - a) % (fps * 2) == 0:
                    el = time.time() - t0
                    print(f"  frame {i - a}/{b - a}  {el:.0f}s, {(i - a + 1) / max(el, 1e-3):.1f} fps", flush=True)
            proc.stdin.close(); proc.wait()
            render_s = time.time() - t0
        finally:
            s.close()
        audio = None
        if not args.no_audio and tl.get("audio") and (a, b) == (0, n):
            audio = mix(tl, narr, tmpd)
            print(f"  mix: {loudness(audio)}")
        if (a, b) != (0, n) or args.scale != 1:
            name += f"-{a}-{b}" if (a, b) != (0, n) else "-draft"
            tl["poster_t"] = 0
        deliver(tl, master, audio, out, name)
        if not args.range:
            vtt = captions(tl, strings)
            if vtt:
                (out / f"{name}.{strings.get('lang', 'en')}.vtt").write_text(vtt)
        print(f"  render {render_s:.0f} s for {(b - a) * N} captures ({(b - a) / max(render_s, 1e-3):.1f} frames/s), "
              f"total {time.time() - t_start:.0f} s")
        for f in sorted(out.glob(f"{name}.*")):
            print(f"  {f.relative_to(ROOT)}  {size_of(f)}")
    finally:
        shutil.rmtree(tmpd, ignore_errors=True)


def qa(video: Path, every: float, dest: Path | None):
    """Contact sheets of frames every `every` seconds (3x3 per sheet) for review."""
    dest = dest or Path(os.environ["MEDIA_TMP"]) / "motion-qa" / video.stem
    dest.mkdir(parents=True, exist_ok=True)
    for f in dest.glob("*.jpg"):
        f.unlink()
    run_ff(["-i", str(video), "-vf", f"fps=1/{every},scale=640:-2:flags=lanczos,drawtext=text='%{{pts\\:hms}}':x=8:y=8:"
            "fontcolor=white:fontsize=18:box=1:boxcolor=black@0.5,tile=3x3:padding=6:margin=6", "-q:v", "3",
            str(dest / "sheet-%02d.jpg")])
    print("\n".join(str(p) for p in sorted(dest.glob("*.jpg"))))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("timelines", nargs="*")
    ap.add_argument("--out"); ap.add_argument("--lang", default="en")
    ap.add_argument("--stills"); ap.add_argument("--range"); ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--blur", type=int, default=0); ap.add_argument("--no-audio", action="store_true")
    ap.add_argument("--swiftshader", action="store_true"); ap.add_argument("--headful", action="store_true")
    ap.add_argument("--qa"); ap.add_argument("--every", type=float, default=0.5)
    args = ap.parse_args()
    if args.qa:
        return qa(Path(args.qa), args.every, Path(args.out) if args.out else None)
    for p in args.timelines:
        render(Path(p), args)


if __name__ == "__main__":
    main()
