"""ffmpeg: WebM/Opus encoding of finished clips and decoding of uploaded recordings to mono PCM.

ffmpeg is preinstalled on Spaces (Debian build with libopus and libsoxr). `NOLABS_FFMPEG` overrides the binary, which
the tests use to point at a portable build.
"""
from __future__ import annotations

import functools
import os
import shutil
import subprocess
import tempfile

import numpy as np

CONTENT_TYPE = "audio/webm"
# notes/09: `-application audio -vbr on -frame_duration 20` for every profile.
OPUS_OPTIONS = ("-c:a", "libopus", "-vbr", "on", "-application", "audio", "-frame_duration", "20")
TIMEOUT_S = 120


class CodecError(RuntimeError):
    pass


def ffmpeg_exe() -> str:
    exe = os.environ.get("NOLABS_FFMPEG") or shutil.which("ffmpeg")
    if not exe:
        raise CodecError("ffmpeg is not installed")
    return exe


@functools.lru_cache(maxsize=4)
def has_soxr(exe: str) -> bool:
    """True when this ffmpeg was built with libsoxr. Builds without it fall back to ffmpeg's own resampler."""
    try:
        done = subprocess.run([exe, "-hide_banner", "-buildconf"], capture_output=True, check=False, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        return False
    return b"--enable-libsoxr" in done.stdout


def _resample_filter(source_rate: int, out_rate: int, soxr: bool) -> list[str]:
    if source_rate == out_rate:
        return []
    resampler = ":resampler=soxr:precision=28" if soxr else ""
    return ["-af", f"aresample={out_rate}{resampler}"]


def encode_command(exe: str, source_rate: int, out_rate: int, bitrate_kbps: int, out_path: str,
                   soxr: bool) -> list[str]:
    """ffmpeg arguments for float32 mono PCM on stdin to a WebM/Opus file. Output goes to a seekable file (not a pipe)
    so the muxer can write the duration and cues; `bitexact` keeps the same input producing the same bytes."""
    return [
        exe, "-hide_banner", "-loglevel", "error", "-nostdin", "-y",
        "-f", "f32le", "-ar", str(source_rate), "-ac", "1", "-i", "pipe:0",
        *_resample_filter(source_rate, out_rate, soxr),
        *OPUS_OPTIONS, "-b:a", f"{bitrate_kbps}k", "-ac", "1",
        "-map_metadata", "-1", "-fflags", "+bitexact", "-flags:a", "+bitexact",
        "-f", "webm", out_path,
    ]


def encode_webm(wave: np.ndarray, source_rate: int, out_rate: int, bitrate_kbps: int) -> bytes:
    exe = ffmpeg_exe()
    pcm = np.ascontiguousarray(wave, dtype="<f4").tobytes()
    with tempfile.TemporaryDirectory(prefix="enc-") as tmp:
        out_path = os.path.join(tmp, "clip.webm")
        cmd = encode_command(exe, source_rate, out_rate, bitrate_kbps, out_path, has_soxr(exe))
        _run(cmd, pcm)
        with open(out_path, "rb") as f:
            data = f.read()
    if not data:
        raise CodecError("ffmpeg wrote an empty file")
    return data


def decode_command(exe: str, in_path: str, out_rate: int, max_seconds: float, soxr: bool) -> list[str]:
    resampler = ["-af", "aresample=resampler=soxr:precision=28"] if soxr else []
    return [
        exe, "-hide_banner", "-loglevel", "error", "-nostdin",
        "-i", in_path, "-t", f"{max_seconds:g}", "-vn", "-ac", "1", *resampler, "-ar", str(out_rate),
        "-f", "f32le", "pipe:1",
    ]


def decode_mono(data: bytes, out_rate: int, max_seconds: float) -> np.ndarray:
    """Any audio file ffmpeg reads, as mono float32 at `out_rate`, cut at `max_seconds`."""
    exe = ffmpeg_exe()
    with tempfile.TemporaryDirectory(prefix="dec-") as tmp:
        in_path = os.path.join(tmp, "input")
        with open(in_path, "wb") as f:
            f.write(data)
        out = _run(decode_command(exe, in_path, out_rate, max_seconds, has_soxr(exe)), None)
    usable = len(out) - len(out) % 4
    return np.frombuffer(out[:usable], dtype="<f4").astype(np.float32)


def _run(cmd: list[str], stdin: bytes | None) -> bytes:
    try:
        done = subprocess.run(cmd, input=stdin, capture_output=True, check=False, timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        raise CodecError(f"ffmpeg timed out after {TIMEOUT_S} s") from None
    if done.returncode != 0:
        tail = done.stderr.decode(errors="replace").strip()[-400:]
        raise CodecError(f"ffmpeg failed ({done.returncode}): {tail}")
    return done.stdout
