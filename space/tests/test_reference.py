import numpy as np
import pytest

from nolabs_engine import reference as r
from nolabs_engine.errors import InputError

SR = r.SAMPLE_RATE
CHARS_PER_S = 14.0


def spoken_line(text: str, rng: np.random.Generator, amplitude: float = 0.2) -> np.ndarray:
    """Fake speech for one line: 'words' of tone with 60 ms gaps (shorter than a line pause), at a reading pace."""
    words = text.split()
    total_s = len(text) / CHARS_PER_S
    word_s = total_s / len(words) - 0.06
    parts = []
    for i, _ in enumerate(words):
        t = np.arange(int(word_s * SR)) / SR
        parts.append((amplitude * np.sin(2 * np.pi * (180 + 20 * i) * t)).astype(np.float32))
        parts.append(np.zeros(int(0.06 * SR), np.float32))
    return np.concatenate(parts[:-1]) + (0.0005 * rng.standard_normal(sum(p.size for p in parts[:-1]))).astype(
        np.float32)


def recording(lines: list[str], pause_s: float = 0.7, lead_s: float = 0.8, seed: int = 3,
              amplitudes: list[float] | None = None) -> np.ndarray:
    rng = np.random.default_rng(seed)
    noise = lambda s: (0.0005 * rng.standard_normal(int(s * SR))).astype(np.float32)  # noqa: E731 - -66 dBFS room
    parts = [noise(lead_s)]
    for i, line in enumerate(lines):
        parts.append(spoken_line(line, rng, (amplitudes or [0.2] * len(lines))[i]))
        parts.append(noise(pause_s))
    return np.concatenate(parts)


SCRIPT = [
    "The morning light crept slowly over the old stone wall.",          # 55 chars ≈ 3.9 s
    "She laughed, surprised that anyone had remembered her name.",      # 59 ≈ 4.2 s
    "Nobody answered when I knocked on the heavy wooden door.",         # 56 ≈ 4.0 s
    "Please, just tell me what happened last night.",                   # 46 ≈ 3.3 s
    "The rain kept falling until the river burst its banks.",           # 54 ≈ 3.9 s
    "We should leave now, before it gets dark.",                        # 41 ≈ 2.9 s
]


def test_splits_lines_and_keeps_whole_lines_for_10_to_18_seconds():
    wave = recording(SCRIPT)
    cut = r.prepare(wave, SR, "\n".join(SCRIPT))
    seconds = cut.wave.size / SR
    assert r.MIN_SPAN_S <= seconds <= r.MAX_SPAN_S + 2 * r.GUARD_MS / 1000
    kept = SCRIPT[cut.first_line:cut.last_line + 1]
    assert cut.transcript == " ".join(kept)
    # Longest fitting run of whole lines wins.
    assert len(kept) == 4 and cut.first_line == 0


def test_line_spans_follow_the_pauses_between_lines():
    wave = recording(SCRIPT[:3], pause_s=0.5, lead_s=1.0)
    spans = r.line_spans(r.speech_mask(r.frame_db(wave, SR)), 3)
    starts_s = [s * r.FRAME_MS / 1000 for s, _ in spans]
    expected, at = [], 1.0
    for line in SCRIPT[:3]:
        expected.append(at)
        at += len(line) / CHARS_PER_S - 0.06 + 0.5
    assert starts_s == pytest.approx(expected, abs=0.05)


def test_output_is_levelled_faded_and_under_the_ceiling():
    cut = r.prepare(recording(SCRIPT), SR, "\n".join(SCRIPT))
    assert np.abs(cut.wave).max() <= 10 ** (r.PEAK_CEILING_DBFS / 20) + 1e-6
    assert abs(cut.wave[0]) < 1e-3 and abs(cut.wave[-1]) < 1e-3


def test_avoids_clipped_lines_when_a_clean_window_exists():
    amplitudes = [1.0, 0.2, 0.2, 0.2, 0.2, 0.2]  # first line clips
    wave = recording(SCRIPT, amplitudes=amplitudes)
    cut = r.prepare(wave, SR, "\n".join(SCRIPT))
    assert cut.first_line >= 1


def test_blank_script_lines_are_ignored_and_whitespace_collapsed():
    assert r.split_lines("  one   two \n\n   \nthree\r\n") == ["one two", "three"]


def test_too_little_speech_is_an_input_error():
    lines = SCRIPT[:2]
    with pytest.raises(InputError, match="at least 10 s"):
        r.prepare(recording(lines), SR, "\n".join(lines))


def test_missing_pauses_are_an_input_error():
    wave = recording(SCRIPT[:3])
    with pytest.raises(InputError, match="script has 8"):
        r.prepare(wave, SR, "\n".join(SCRIPT[:3] + ["a", "b", "c", "d", "e"]))


def test_noise_only_is_an_input_error():
    rng = np.random.default_rng(0)
    with pytest.raises(InputError, match="no clear pauses"):
        r.prepare((0.1 * rng.standard_normal(20 * SR)).astype(np.float32), SR, "one\ntwo")


def test_misaligned_script_is_rejected():
    # The recording has six lines but the script's lines are far too long for their audio (rate check fails).
    long_lines = [line * 6 for line in SCRIPT]
    with pytest.raises(InputError, match="could not find"):
        r.prepare(recording(SCRIPT), SR, "\n".join(long_lines))


def test_choose_window_relaxes_to_20_seconds_for_long_lines():
    frames_per_s = 1000 // r.FRAME_MS
    spans = [(0, 19 * frames_per_s)]
    lines = ["x" * int(19 * CHARS_PER_S)]
    assert r.choose_window(spans, lines, np.zeros(spans[0][1], int)) == (0, 0)
