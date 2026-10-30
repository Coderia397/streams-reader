"""Choosing the reference clip for a cloned voice from the user's guided recording.

The user reads a script line by line, pausing between lines, so the pauses between lines are the longest silences in
the recording. Splitting at the `lines - 1` longest interior pauses aligns the audio with the script without ASR; the
kept span is the best run of whole lines lasting 10–18 s (Ming conditions on about 20 s at most, and a span that ends
mid-line would no longer match its transcript).
"""
from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np

from .audio import loudness, silence
from .errors import InputError

SAMPLE_RATE = 24_000
MIN_SPAN_S = 10.0
MAX_SPAN_S = 18.0
# Used only when no run of whole lines fits 10–18 s (long lines); still under Ming's ~20 s prompt limit.
RELAXED_MAX_SPAN_S = 20.0
FRAME_MS = 20
MIN_PAUSE_MS = 200
# Speech bursts shorter than this inside a pause are clicks or breaths, not words.
MIN_BURST_MS = 60
# Kept on each side of the span, inside the pause (at most half of the shortest pause).
GUARD_MS = 100
FADE_MS = 10
# Plausible reading rates; a line outside them means the split did not line up with the script.
MIN_CHARS_PER_S = 4.0
MAX_CHARS_PER_S = 30.0
MIN_DYNAMIC_RANGE_DB = 12.0
CLIP_LEVEL = 0.999
TARGET_LUFS = -20.0
PEAK_CEILING_DBFS = -1.5


@dataclass(frozen=True)
class ReferenceCut:
    wave: np.ndarray
    transcript: str
    first_line: int
    last_line: int


def split_lines(transcript: str) -> list[str]:
    """Script lines, whitespace collapsed, blank lines dropped."""
    return [" ".join(line.split()) for line in transcript.splitlines() if line.strip()]


def frame_db(wave: np.ndarray, sample_rate: int, frame_ms: int = FRAME_MS) -> np.ndarray:
    """RMS level per non-overlapping frame in dBFS (floor -120)."""
    size = max(1, sample_rate * frame_ms // 1000)
    frames = wave[: wave.size - wave.size % size].reshape(-1, size).astype(np.float64)
    rms = np.sqrt(np.mean(frames * frames, axis=1)) if frames.size else np.zeros(0)
    return 20.0 * np.log10(np.maximum(rms, 1e-6))


def speech_mask(db: np.ndarray) -> np.ndarray:
    """Frames that hold speech: above a threshold placed 30 % of the way from the noise floor (10th percentile) to the
    speech level (90th percentile), with bursts shorter than MIN_BURST_MS removed."""
    if db.size == 0:
        return np.zeros(0, dtype=bool)
    noise, speech = np.percentile(db, 10), np.percentile(db, 90)
    if speech - noise < MIN_DYNAMIC_RANGE_DB:
        raise InputError("the recording has no clear pauses between lines (too noisy, too quiet or no pauses); "
                         "record again in a quiet room and pause after each line")
    mask = db > noise + 0.3 * (speech - noise)
    min_burst = max(1, MIN_BURST_MS // FRAME_MS)
    for start, end in _runs(mask):
        if end - start < min_burst:
            mask[start:end] = False
    return mask


def _runs(mask: np.ndarray) -> list[tuple[int, int]]:
    """[start, end) of each run of True."""
    padded = np.concatenate([[False], mask, [False]]).astype(np.int8)
    edges = np.flatnonzero(np.diff(padded))
    return [(int(a), int(b)) for a, b in zip(edges[::2], edges[1::2], strict=True)]


def line_spans(mask: np.ndarray, n_lines: int) -> list[tuple[int, int]]:
    """Frame range [start, end) of each line: speech split at the `n_lines - 1` longest interior pauses."""
    speech = _runs(mask)
    if not speech:
        raise InputError("no speech found in the recording")
    first, last = speech[0][0], speech[-1][1]
    min_pause = max(1, MIN_PAUSE_MS // FRAME_MS)
    pauses = [(start, end) for start, end in _runs(~mask[first:last]) if end - start >= min_pause]
    pauses = [(start + first, end + first) for start, end in pauses]
    if len(pauses) < n_lines - 1:
        raise InputError(f"found {len(pauses) + 1} spoken lines but the script has {n_lines}; "
                         "pause briefly after each line and read every line")
    # Longest first; on equal length the earlier pause wins, so the choice is deterministic.
    chosen = sorted(sorted(pauses, key=lambda p: (-(p[1] - p[0]), p[0]))[: n_lines - 1])
    bounds = [first] + [b for pause in chosen for b in pause] + [last]
    return [(bounds[i], bounds[i + 1]) for i in range(0, len(bounds), 2)]


def choose_window(spans: Sequence[tuple[int, int]], lines: Sequence[str], clipped: np.ndarray,
                  frame_ms: int = FRAME_MS) -> tuple[int, int]:
    """Best run of whole lines (first, last inclusive): 10–18 s long (20 s if nothing fits), every line read at a
    plausible rate, then fewest clipped frames, then longest, then earliest."""
    seconds = frame_ms / 1000.0
    plausible = [MIN_CHARS_PER_S <= len(line) / max(seconds * (e - s), 1e-3) <= MAX_CHARS_PER_S
                 for (s, e), line in zip(spans, lines, strict=True)]
    clip_prefix = np.concatenate([[0], np.cumsum(clipped)])
    for upper in (MAX_SPAN_S, RELAXED_MAX_SPAN_S):
        best: tuple[tuple[int, float, int], tuple[int, int]] | None = None
        for i in range(len(spans)):
            for j in range(i, len(spans)):
                if not plausible[j]:
                    break
                duration = (spans[j][1] - spans[i][0]) * seconds
                if duration > upper:
                    break
                if duration < MIN_SPAN_S:
                    continue
                clips = int(clip_prefix[spans[j][1]] - clip_prefix[spans[i][0]])
                score = (clips, -duration, i)
                if best is None or score < best[0]:
                    best = (score, (i, j))
        if best is not None:
            return best[1]
    total = sum(e - s for s, e in spans) * seconds
    if total < MIN_SPAN_S:
        raise InputError(f"the recording has {total:.1f} s of speech; at least {MIN_SPAN_S:.0f} s is needed")
    raise InputError("could not find 10 to 18 seconds of whole lines that match the script; "
                     "read each line once, at a natural pace, with a short pause after it")


def prepare(wave: np.ndarray, sample_rate: int, transcript: str) -> ReferenceCut:
    """Cut, level and fade the best span. The transcript of the kept lines is joined with single spaces."""
    lines = split_lines(transcript)
    if not lines:
        raise InputError("the transcript is empty")
    wave = silence.sanitise(wave)
    if wave.size:
        wave = wave - np.float32(wave.mean())  # phone mics can carry a DC offset that skews levels and fades
    db = frame_db(wave, sample_rate)
    spans = line_spans(speech_mask(db), len(lines))
    size = sample_rate * FRAME_MS // 1000
    clipped = frame_clip_counts(wave, size, len(db))
    first, last = choose_window(spans, lines, clipped)
    guard = sample_rate * GUARD_MS // 1000
    start = max(0, spans[first][0] * size - guard)
    end = min(wave.size, spans[last][1] * size + guard)
    cut = np.array(wave[start:end], dtype=np.float32, copy=True)
    silence.fade(cut, sample_rate * FADE_MS // 1000)
    level = loudness.measure(cut, sample_rate)
    gain = 0.0 if level is None else TARGET_LUFS - level
    cut, _ = loudness.apply_gain(cut, gain, PEAK_CEILING_DBFS)
    return ReferenceCut(wave=cut, transcript=" ".join(lines[first:last + 1]), first_line=first, last_line=last)


def frame_clip_counts(wave: np.ndarray, frame_size: int, n_frames: int) -> np.ndarray:
    """Samples at full scale per frame (a clipped line is a poor voice reference)."""
    frames = np.abs(wave[: n_frames * frame_size]).reshape(n_frames, frame_size) if n_frames else np.zeros((0, 1))
    return (frames >= CLIP_LEVEL).sum(axis=1)
