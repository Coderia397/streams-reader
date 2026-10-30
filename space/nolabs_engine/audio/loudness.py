"""Loudness by static gain only (no limiter, no compressor), so the rise and fall inside a performance survives.

One gain moves the whole render batch toward the target (EBU R128 integrated loudness of the batch); each segment may
then deviate from that batch gain by at most `max_correction_db` on its way to the target, so a whisper stays quieter
than a shout while consecutive clips still sit close together. Finally each segment is scaled down, if needed, so its
sample peak stays under the ceiling.
"""
from __future__ import annotations

from collections.abc import Sequence

import numpy as np
import pyloudnorm

# pyloudnorm measures 400 ms blocks; anything shorter has no integrated loudness.
MIN_MEASURE_SECONDS = 0.4


def measure(wave: np.ndarray, sample_rate: int) -> float | None:
    """Integrated loudness in LUFS, or None for clips too short or too quiet to measure."""
    if wave.size < int(sample_rate * MIN_MEASURE_SECONDS) + 1:
        return None
    loudness = pyloudnorm.Meter(sample_rate).integrated_loudness(wave.astype(np.float64))
    return float(loudness) if np.isfinite(loudness) else None


def batch_gain(batch_loudness: float | None, target_lufs: float) -> float:
    return 0.0 if batch_loudness is None else target_lufs - batch_loudness


def plan_gains(loudness: Sequence[float | None], batch_loudness: float | None, target_lufs: float,
               max_correction_db: float) -> list[float]:
    """Gain in dB per segment: the batch gain plus a per-segment correction toward the target, clamped to
    ±max_correction_db. Segments without a measurement get the batch gain."""
    base = batch_gain(batch_loudness, target_lufs)
    gains = []
    for value in loudness:
        if value is None:
            gains.append(base)
            continue
        correction = (target_lufs - value) - base
        gains.append(base + float(np.clip(correction, -max_correction_db, max_correction_db)))
    return gains


def apply_gain(wave: np.ndarray, gain_db: float, peak_ceiling_dbfs: float) -> tuple[np.ndarray, float]:
    """Scale by `gain_db`, then lower the gain just enough for the sample peak to sit at the ceiling if it would pass
    it. Returns the scaled wave and the gain actually applied."""
    ceiling = 10.0 ** (peak_ceiling_dbfs / 20.0)
    gain = 10.0 ** (gain_db / 20.0)
    peak = float(np.abs(wave).max()) if wave.size else 0.0
    if peak * gain > ceiling:
        gain = ceiling / peak
    return (wave * np.float32(gain)).astype(np.float32), 20.0 * float(np.log10(gain)) if gain > 0 else 0.0
