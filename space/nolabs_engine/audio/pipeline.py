"""One render batch from raw model waveforms to encoded clips: trim, programme loudness, per-segment gain and peak
ceiling, boundary pause, resample + encode. Failures stay per item so one bad clip never loses the rest of the batch.
Clips are levelled in order as they arrive (`Stream`), so the first one can ship before the last one exists."""
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


@dataclass(frozen=True)
class Levelled:
    """A clip with its gain decided, ready to encode. Produced in segment order by `Stream.level`."""
    prepared: _Prepared
    boundary: str
    gain_db: float


class Stream:
    """One render batch processed segment by segment, so each clip can be encoded and uploaded while the next one is
    still being synthesised.

    `level` must be called in segment order (it extends the batch programme loudness); `finish` (resample + encode,
    the slow part) may run on any thread and in any order."""

    def __init__(self, profile: AudioProfile, encode: Encoder = codec.encode_webm):
        self.profile, self._encode = profile, encode
        self._programmes: dict[int, loudness.Programme] = {}

    def level(self, clip: RawClip | BaseException) -> Levelled | BaseException:
        if isinstance(clip, BaseException):
            return clip
        try:
            prepared = _prepare(clip)
        except Exception as e:  # noqa: BLE001 - reported per item
            return e
        if isinstance(prepared, BaseException):
            return prepared
        programme = self._programmes.setdefault(prepared.sample_rate, loudness.Programme(prepared.sample_rate))
        programme.add(prepared.wave)
        gain = loudness.plan_gains([prepared.loudness], programme.loudness(), self.profile.target_lufs,
                                   self.profile.max_correction_db)[0]
        return Levelled(prepared=prepared, boundary=clip.boundary, gain_db=gain)

    def finish(self, item: Levelled | BaseException) -> FinishedClip | BaseException:
        if isinstance(item, BaseException):
            return item
        try:
            return _finish(item.prepared, item.boundary, item.gain_db, self.profile, self._encode)
        except Exception as e:  # noqa: BLE001 - reported per item
            return e


def finish_batch(clips: Sequence[RawClip | BaseException], profile: AudioProfile,
                 encode: Encoder = codec.encode_webm) -> list[FinishedClip | BaseException]:
    """The whole batch in one thread, with exactly the levelling the streamed path applies. Entries that are already
    exceptions (synthesis failed) pass through unchanged; the result is aligned with `clips`."""
    stream = Stream(profile, encode)
    return [stream.finish(stream.level(clip)) for clip in clips]


def _prepare(clip: RawClip) -> _Prepared | BaseException:
    trimmed = silence.trim(silence.sanitise(clip.wave), clip.sample_rate)
    if trimmed.silent:
        return SilentOutput("the engine produced only silence")
    return _Prepared(wave=trimmed.wave, sample_rate=clip.sample_rate, lead_samples=trimmed.lead_samples,
                     loudness=loudness.measure_blocks(trimmed.wave, clip.sample_rate))


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
