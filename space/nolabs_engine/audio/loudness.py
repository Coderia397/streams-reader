"""Loudness by static gain only (no limiter, no compressor), so the rise and fall inside a performance survives.

One gain moves the batch toward the target (EBU R128 integrated loudness of the batch as one programme); each segment
may then deviate from that batch gain by at most `max_correction_db` on its way to the target, so a whisper stays
quieter than a shout while consecutive clips still sit close together. Finally each segment is scaled down, if needed,
so its sample peak stays under the ceiling.

Segments are streamed out as they finish, so the batch loudness used for segment N is that of segments 0..N (the
programme so far, `Programme`), never of segments not yet rendered. The first segment is therefore levelled on its own.
"""
from __future__ import annotations

from collections.abc import Sequence

import numpy as np
import pyloudnorm
import scipy.signal

# pyloudnorm measures 400 ms blocks; anything shorter has no integrated loudness.
MIN_MEASURE_SECONDS = 0.4


def measure(wave: np.ndarray, sample_rate: int) -> float | None:
    """Integrated loudness in LUFS, or None for clips too short or too quiet to measure."""
    if wave.size < int(sample_rate * MIN_MEASURE_SECONDS) + 1:
        return None
    loudness = pyloudnorm.Meter(sample_rate).integrated_loudness(wave.astype(np.float64))
    return float(loudness) if np.isfinite(loudness) else None


def measure_blocks(wave: np.ndarray, sample_rate: int) -> float | None:
    """Integrated loudness in LUFS on the same complete-block meter as `Programme`, or None when nothing is
    measurable. The gain plan uses this on both sides (per clip and batch so far) so the two terms agree; `measure`
    stays for code that wants pyloudnorm's value."""
    programme = Programme(sample_rate)
    programme.add(wave)
    return programme.loudness()


# ITU-R BS.1770-4 gating: 400 ms blocks every 100 ms, absolute gate -70 LUFS, relative gate 10 LU under the mean.
BLOCK_S, HOP_S = 0.4, 0.1
ABSOLUTE_GATE_LUFS = -70.0
RELATIVE_GATE_LU = 10.0


class Programme:
    """Integrated loudness of clips appended one after another, as if measured on their concatenation, in time linear
    in the audio added. K-weighting runs with its filter state carried across clips and gating blocks span clip
    joins, so the value equals `measure_blocks(np.concatenate(clips))` exactly (up to float summation order).

    Follows ITU-R BS.1770 with complete 400 ms blocks only. pyloudnorm 0.1.1 (`measure`) counts
    round((n/fs - 0.4)/0.1) + 1 blocks, which adds a partial trailing block (summed over fewer samples, divided by the
    full block length) when more than 50 ms is left after the last full hop; there the two differ by up to about
    0.2 LU. Not thread-safe; feed it in segment order."""

    def __init__(self, sample_rate: int):
        self.sample_rate = sample_rate
        meter = pyloudnorm.Meter(sample_rate)
        self._stages = [(f.passband_gain, np.asarray(f.b, np.float64), np.asarray(f.a, np.float64))
                        for f in meter._filters.values()]
        self._state = [np.zeros(max(len(a), len(b)) - 1) for _, b, a in self._stages]
        self._block = int(round(BLOCK_S * sample_rate))
        self._hop = int(round(HOP_S * sample_rate))
        self._pending = np.zeros(0, np.float64)  # squared K-weighted samples from the next block start onwards
        self._blocks: list[float] = []  # mean square per gating block

    def add(self, wave: np.ndarray) -> None:
        x = np.asarray(wave, np.float64).reshape(-1)
        for i, (gain, b, a) in enumerate(self._stages):
            y, self._state[i] = scipy.signal.lfilter(b, a, x, zi=self._state[i])
            x = gain * y
        pending = np.concatenate([self._pending, np.square(x)])
        start = 0
        while start + self._block <= pending.size:
            self._blocks.append(float(pending[start:start + self._block].sum() / self._block))
            start += self._hop
        self._pending = pending[start:]

    def loudness(self) -> float | None:
        """Gated integrated loudness in LUFS, or None before the first full block or when everything is gated out."""
        if not self._blocks:
            return None
        z = np.asarray(self._blocks)
        with np.errstate(divide="ignore"):
            levels = -0.691 + 10.0 * np.log10(z)
        above = z[levels >= ABSOLUTE_GATE_LUFS]
        if above.size == 0:
            return None
        relative = -0.691 + 10.0 * np.log10(above.mean()) - RELATIVE_GATE_LU
        gated = z[(levels > relative) & (levels > ABSOLUTE_GATE_LUFS)]
        if gated.size == 0:
            return None
        value = -0.691 + 10.0 * np.log10(gated.mean())
        return float(value) if np.isfinite(value) else None


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
