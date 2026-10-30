"""Post-processing between a model's float waveform and the Ogg Opus file stored in R2.

Loudness is matched with one static gain per clip (EBU R128 integrated loudness), never with dynamic compression, so the
rise and fall inside a performance survives while consecutive clips sit at the same level.
"""
from __future__ import annotations

import subprocess
from dataclasses import dataclass

import numpy as np
import pyloudnorm

TARGET_LUFS = -18.0
PEAK_CEILING = 0.97
MIN_LOUDNESS_SECONDS = 0.5
LEAD_IN_SECONDS = 0.02


@dataclass(frozen=True)
class EncodedAudio:
    data: bytes
    duration_ms: int


def normalise(wave: np.ndarray, sample_rate: int) -> np.ndarray:
    """Mono float32 in [-1, 1] at TARGET_LUFS, peak-limited by scaling only."""
    wave = np.asarray(wave, dtype=np.float32).reshape(-1)
    wave = np.nan_to_num(wave, nan=0.0, posinf=0.0, neginf=0.0)
    if wave.size >= int(sample_rate * MIN_LOUDNESS_SECONDS):
        loudness = pyloudnorm.Meter(sample_rate).integrated_loudness(wave.astype(np.float64))
        if np.isfinite(loudness):
            wave = wave * np.float32(10.0 ** ((TARGET_LUFS - loudness) / 20.0))
    peak = float(np.abs(wave).max()) if wave.size else 0.0
    if peak > PEAK_CEILING:
        wave = wave * np.float32(PEAK_CEILING / peak)
    # A few milliseconds of fade-in removes the click some vocoders leave on the first sample.
    ramp = min(wave.size, int(sample_rate * LEAD_IN_SECONDS))
    if ramp:
        wave[:ramp] *= np.linspace(0.0, 1.0, ramp, dtype=np.float32)
    return wave


def encode_opus(wave: np.ndarray, sample_rate: int, bitrate_kbps: int) -> EncodedAudio:
    """Encode a normalised mono waveform as Ogg Opus with ffmpeg (preinstalled on Spaces)."""
    pcm = np.ascontiguousarray(wave, dtype=np.float32).tobytes()
    cmd = [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin",
        "-f", "f32le", "-ar", str(sample_rate), "-ac", "1", "-i", "pipe:0",
        "-c:a", "libopus", "-b:a", f"{bitrate_kbps}k", "-vbr", "on", "-application", "audio",
        "-frame_duration", "20", "-f", "ogg", "pipe:1",
    ]
    done = subprocess.run(cmd, input=pcm, capture_output=True, check=False)
    if done.returncode != 0 or not done.stdout:
        raise RuntimeError(f"ffmpeg failed ({done.returncode}): {done.stderr.decode(errors='replace')[-400:]}")
    return EncodedAudio(data=done.stdout, duration_ms=round(wave.size * 1000 / sample_rate))


def finish(wave: np.ndarray, sample_rate: int, bitrate_kbps: int) -> EncodedAudio:
    return encode_opus(normalise(wave, sample_rate), sample_rate, bitrate_kbps)
