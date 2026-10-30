"""One render batch from raw model waveforms to encoded clips: trim, batch loudness, per-segment gain and peak ceiling,
boundary pause, resample + encode. Failures stay per item so one bad clip never loses the rest of the batch."""
from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass

import numpy as np

from ..contracts import AudioProfile
from . import codec, loudness, silence

Encoder = Callable[[np.ndarray, int, int, int], bytes]


class SilentOutput(RuntimeError):
    """The model produced nothing above the trim threshold."""


@dataclass(frozen=True)
class RawClip:
    wave: np.ndarray
    sample_rate: int
    boundary: str


@dataclass(frozen=True)
class FinishedClip:
    data: bytes
    # Whole clip including the appended pause.
    duration_ms: int
    # Speech part only (after trimming, before the pause); word timings must end inside it.
    speech_ms: int
    # Time cut from the start; word timings shift left by this much.
    lead_ms: float
    gain_db: float


@dataclass(frozen=True)
class _Prepared:
    wave: np.ndarray
    sample_rate: int
    lead_samples: int
    loudness: float | None


def finish_batch(clips: Sequence[RawClip | BaseException], profile: AudioProfile,
                 encode: Encoder = codec.encode_webm) -> list[FinishedClip | BaseException]:
    """Process a batch. Entries that are already exceptions (synthesis failed) pass through unchanged; the result is
    aligned with `clips`."""
    prepared: list[_Prepared | BaseException] = [_prepare(c) if isinstance(c, RawClip) else c for c in clips]
    measured = [p for p in prepared if isinstance(p, _Prepared)]
    gains = loudness.plan_gains([p.loudness for p in measured], _batch_loudness(measured), profile.target_lufs,
                                profile.max_correction_db)
    gain_of = {id(p): g for p, g in zip(measured, gains, strict=True)}
    out: list[FinishedClip | BaseException] = []
    boundaries = [c.boundary if isinstance(c, RawClip) else "" for c in clips]
    for item, boundary in zip(prepared, boundaries, strict=True):
        if isinstance(item, BaseException):
            out.append(item)
            continue
        try:
            out.append(_finish(item, boundary, gain_of[id(item)], profile, encode))
        except Exception as e:  # noqa: BLE001 - reported per item
            out.append(e)
    return out


def _prepare(clip: RawClip) -> _Prepared | BaseException:
    trimmed = silence.trim(silence.sanitise(clip.wave), clip.sample_rate)
    if trimmed.silent:
        return SilentOutput("the engine produced only silence")
    return _Prepared(wave=trimmed.wave, sample_rate=clip.sample_rate, lead_samples=trimmed.lead_samples,
                     loudness=loudness.measure(trimmed.wave, clip.sample_rate))


def _batch_loudness(prepared: Sequence[_Prepared]) -> float | None:
    """Integrated loudness of the batch as one programme. Every clip of a batch comes from one engine, so they share a
    sample rate; a mixed batch falls back to the mean of the per-clip values."""
    if not prepared:
        return None
    rates = {p.sample_rate for p in prepared}
    if len(rates) == 1:
        return loudness.measure(np.concatenate([p.wave for p in prepared]), rates.pop())
    values = [p.loudness for p in prepared if p.loudness is not None]
    return float(np.mean(values)) if values else None


def _finish(item: _Prepared, boundary: str, gain_db: float, profile: AudioProfile, encode: Encoder) -> FinishedClip:
    wave, applied = loudness.apply_gain(item.wave, gain_db, profile.peak_ceiling_dbfs)
    speech_samples = wave.size
    wave = silence.append_pause(wave, item.sample_rate, profile.pauses.for_boundary(boundary))
    data = encode(wave, item.sample_rate, profile.sample_rate, profile.bitrate_kbps)
    return FinishedClip(
        data=data,
        duration_ms=int(round(wave.size * 1000 / item.sample_rate)),
        speech_ms=int(round(speech_samples * 1000 / item.sample_rate)),
        lead_ms=item.lead_samples * 1000 / item.sample_rate,
        gain_db=applied,
    )
