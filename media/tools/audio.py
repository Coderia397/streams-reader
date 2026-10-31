"""Sound mix for a shot: voice-over clips (by line id, per language), the app's own narration, and a music bed ducked
under the voices with sidechaincompress, normalised to -16 LUFS integrated / -1.5 dBTP (two-pass loudnorm).

Shot JSON:
  "audio": {
    "music": {"file": "assets/music/bed.ogg", "gain_db": -3, "in": 0, "fade_in": 1.5, "fade_out": 3},
    "vo": [{"id": "late", "at": 1.2, "gain_db": 0}],
    "narration": [{"file": "assets/screen/app-alice.narration.webm", "at": 10, "in": 2.0, "dur": 6, "gain_db": -2,
                   "fade": 0.4}],
    "loudness": -16, "true_peak": -1.5
  }
"""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

from ffmpeg import ffmpeg_exe

MEDIA = Path(__file__).resolve().parents[1]


def vo_set(shot: dict, lang: str) -> str:
    """Voice-over folder: audio.vo_set (e.g. "{lang}-ming" for the expressive take) if rendered, else the Kokoro set."""
    want = shot.get("audio", {}).get("vo_set", "{lang}").replace("{lang}", lang)
    return want if (MEDIA / "assets" / "vo" / want / "index.json").exists() else lang


def vo_index(name: str) -> dict:
    p = MEDIA / "assets" / "vo" / name / "index.json"
    if not p.exists():
        raise SystemExit(f"no voice-over set '{name}': run media/vo/render_vo.py")
    return json.loads(p.read_text())


def build(shot: dict, lang: str, tmpd: Path) -> Path:
    A = shot["audio"]
    dur = float(shot["duration"])
    inputs: list[str] = []
    voice_chains: list[str] = []
    n = 0
    vset = vo_set(shot, lang)
    idx = vo_index(vset) if A.get("vo") else None
    print(f"voice-over set: {vset} ({idx['voice'] if idx else '-'})")
    for v in A.get("vo", []):
        line = idx["lines"][v["id"]]
        inputs += ["-i", str(MEDIA / "assets" / "vo" / vset / line["file"])]
        ms = int(v["at"] * 1000)
        voice_chains.append(f"[{n}:a]aresample=48000,aformat=channel_layouts=stereo,volume={v.get('gain_db', 0)}dB,"
                            f"adelay={ms}:all=1[v{n}]")
        n += 1
    for nar in A.get("narration", []):
        inputs += ["-i", str(MEDIA / nar["file"])]
        ms = int(nar["at"] * 1000)
        f = nar.get("fade", 0.4)
        voice_chains.append(
            f"[{n}:a]aresample=48000,aformat=channel_layouts=stereo,atrim={nar.get('in', 0)}:{nar.get('in', 0) + nar['dur']},"
            f"asetpts=PTS-STARTPTS,afade=t=in:d={f},afade=t=out:st={max(0, nar['dur'] - f)}:d={f},"
            f"volume={nar.get('gain_db', 0)}dB,adelay={ms}:all=1[v{n}]")
        n += 1
    voices = [f"[v{i}]" for i in range(n)]
    filt = voice_chains[:]
    if voices:
        filt.append("".join(voices) + f"amix=inputs={len(voices)}:normalize=0:duration=longest,apad,atrim=0:{dur}[voice]")
    else:
        filt.append(f"anullsrc=r=48000:cl=stereo,atrim=0:{dur}[voice]")
    M = A.get("music")
    if M:
        inputs += ["-i", str(MEDIA / M["file"])]
        m = n
        fo = M.get("fade_out", 3)
        filt.append(f"[{m}:a]aresample=48000,aformat=channel_layouts=stereo,atrim={M.get('in', 0)}:{M.get('in', 0) + dur},"
                    f"asetpts=PTS-STARTPTS,afade=t=in:d={M.get('fade_in', 1.5)},afade=t=out:st={dur - fo}:d={fo},"
                    f"volume={M.get('gain_db', -3)}dB,apad,atrim=0:{dur}[mus]")
        filt.append("[voice]asplit=2[vk][vm]")
        filt.append(f"[mus][vk]sidechaincompress=threshold={M.get('duck_threshold', 0.03)}:ratio={M.get('duck_ratio', 6)}:"
                    f"attack=30:release=600:makeup=1[duck]")
        filt.append("[vm][duck]amix=inputs=2:normalize=0:duration=first[mix]")
    else:
        filt.append("[voice]anull[mix]")
    raw = tmpd / "mix_raw.wav"
    subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", *inputs, "-filter_complex", ";".join(filt), "-map", "[mix]",
                    "-ar", "48000", "-ac", "2", str(raw)], check=True)
    I, TP = A.get("loudness", -16), A.get("true_peak", -1.5)
    meas = subprocess.run([ffmpeg_exe(), "-hide_banner", "-i", str(raw), "-af", f"loudnorm=I={I}:TP={TP}:LRA=11:print_format=json",
                           "-f", "null", "-"], capture_output=True, text=True).stderr
    js = json.loads(re.search(r"\{[^{}]*\"input_i\"[^{}]*\}", meas, re.S).group(0))
    out = tmpd / "mix.wav"
    subprocess.run([ffmpeg_exe(), "-y", "-loglevel", "error", "-i", str(raw), "-af",
                    f"loudnorm=I={I}:TP={TP}:LRA=11:measured_I={js['input_i']}:measured_TP={js['input_tp']}:"
                    f"measured_LRA={js['input_lra']}:measured_thresh={js['input_thresh']}:offset={js['target_offset']}:"
                    f"linear=true,aresample=48000", "-ar", "48000", str(out)], check=True)
    raw.unlink()
    return out


def _ts(s: float) -> str:
    h, rem = divmod(s, 3600)
    m, sec = divmod(rem, 60)
    return f"{int(h):02d}:{int(m):02d}:{sec:06.3f}"


def captions_vtt(shot: dict, lang: str) -> str | None:
    A = shot.get("audio", {})
    if not A.get("vo"):
        return None
    idx = vo_index(vo_set(shot, lang))
    cues = []
    for v in A["vo"]:
        line = idx["lines"][v["id"]]
        cues.append((v["at"], v["at"] + line["duration_ms"] / 1000, line["text"]))
    for c in shot.get("captions", []):
        cues.append((c["start"], c["end"], c["text"]))
    cues.sort()
    body = "\n".join(f"{i + 1}\n{_ts(a)} --> {_ts(b)}\n{t}\n" for i, (a, b, t) in enumerate(cues))
    return f"WEBVTT\nKind: captions\nLanguage: {lang}\n\n{body}"
