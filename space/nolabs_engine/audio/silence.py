"""Leading/trailing silence. Models leave uneven silence around each clip, which makes the rhythm at clip boundaries
uneven; trimming it and appending a fixed pause by boundary type gives every boundary the same, intended gap."""
from __future__ import annotations

from dataclasses import dataclass

import numpy as np

TRIM_THRESHOLD_DBFS = -45.0
GUARD_MS = 30
FADE_MS = 5


@dataclass(frozen=True)
class Trimmed:
    wave: np.ndarray
    # Samples removed from the start; word timings shift left by this much.
    lead_samples: int

    @property
    def silent(self) -> bool:
        return self.wave.size == 0


def sanitise(wave: np.ndarray) -> np.ndarray:
    """Mono float32 with NaN/inf zeroed (a vocoder glitch must not poison loudness maths)."""
    flat = np.asarray(wave, dtype=np.float32).reshape(-1)
    return np.nan_to_num(flat, nan=0.0, posinf=0.0, neginf=0.0)


def trim(wave: np.ndarray, sample_rate: int, threshold_dbfs: float = TRIM_THRESHOLD_DBFS, guard_ms: int = GUARD_MS,
         fade_ms: int = FADE_MS) -> Trimmed:
    """Cut everything before the first and after the last sample above `threshold_dbfs`, keeping `guard_ms` of the
    quiet part on each side so consonant onsets and release tails survive, then fade both ends over `fade_ms` so the cut
    never clicks. A clip with nothing above the threshold comes back empty."""
    threshold = 10.0 ** (threshold_dbfs / 20.0)
    loud = np.flatnonzero(np.abs(wave) > threshold)
    if loud.size == 0:
        return Trimmed(wave=np.zeros(0, np.float32), lead_samples=int(wave.size))
    guard = int(round(sample_rate * guard_ms / 1000))
    start = max(0, int(loud[0]) - guard)
    end = min(int(wave.size), int(loud[-1]) + 1 + guard)
    out = np.array(wave[start:end], dtype=np.float32, copy=True)
    fade(out, int(round(sample_rate * fade_ms / 1000)))
    return Trimmed(wave=out, lead_samples=start)


def fade(wave: np.ndarray, samples: int) -> None:
    """Linear fade-in and fade-out of `samples` each, in place; shortened for clips under two fades long."""
    n = min(samples, wave.size // 2)
    if n <= 0:
        return
    ramp = np.linspace(0.0, 1.0, n + 1, dtype=np.float32)[1:]
    wave[:n] *= ramp
    wave[-n:] *= ramp[::-1]


def append_pause(wave: np.ndarray, sample_rate: int, pause_ms: int) -> np.ndarray:
    pad = int(round(sample_rate * pause_ms / 1000))
    if pad <= 0:
        return wave
    return np.concatenate([wave, np.zeros(pad, dtype=np.float32)])
