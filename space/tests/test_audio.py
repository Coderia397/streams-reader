import numpy as np
import pyloudnorm
import pytest
from conftest import tone

from nolabs_engine.audio import loudness, silence

SR = 24_000


def padded(speech: np.ndarray, lead_s: float, tail_s: float, noise: float = 0.0) -> np.ndarray:
    rng = np.random.default_rng(1)
    lead = (noise * rng.standard_normal(int(lead_s * SR))).astype(np.float32)
    tail = (noise * rng.standard_normal(int(tail_s * SR))).astype(np.float32)
    return np.concatenate([lead, speech, tail])


# ---------------------------------------------------------------------------------------------------------- silence


def test_trim_keeps_30ms_guard_and_reports_lead():
    wave = padded(tone(1.0, SR), 0.5, 0.7, noise=0.001)  # noise at -60 dBFS stays below the -45 dBFS threshold
    out = silence.trim(wave, SR)
    guard = int(0.03 * SR)
    assert out.lead_samples == int(0.5 * SR) - guard + 1  # first loud sample of the sine is index 1 (sin(0) = 0)
    assert abs(out.wave.size - (SR + 2 * guard)) <= 2


def test_trim_fades_both_ends_without_clicks():
    wave = padded(tone(0.5, SR, amplitude=0.8), 0.0, 0.0)
    wave[:] = 0.8  # a DC block would click at a hard cut
    out = silence.trim(wave, SR)
    fade = int(0.005 * SR)
    assert out.wave[0] < 0.8 / fade * 1.01 and out.wave[-1] < 0.8 / fade * 1.01
    assert np.all(np.diff(out.wave[:fade]) > 0)
    assert out.wave[fade + 1] == pytest.approx(0.8)


def test_trim_of_silence_is_empty():
    out = silence.trim(np.full(SR, 0.004, np.float32), SR)  # -48 dBFS
    assert out.silent and out.lead_samples == SR


def test_sanitise_zeroes_nan_and_flattens():
    out = silence.sanitise(np.array([[np.nan, np.inf], [-np.inf, 0.5]]))
    assert out.dtype == np.float32 and out.tolist() == [0.0, 0.0, 0.0, 0.5]


def test_append_pause_and_short_fade():
    wave = np.ones(4, np.float32)
    silence.fade(wave, 100)  # shortened to two samples per side
    assert wave.tolist() == [0.5, 1.0, 1.0, 0.5]
    out = silence.append_pause(np.ones(10, np.float32), SR, 250)
    assert out.size == 10 + 6000 and not out[10:].any()
    assert silence.append_pause(np.ones(4, np.float32), SR, 0).size == 4


# --------------------------------------------------------------------------------------------------------- loudness


def lufs(wave: np.ndarray) -> float:
    return pyloudnorm.Meter(SR).integrated_loudness(wave.astype(np.float64))


def test_measure_needs_400ms_and_signal():
    assert loudness.measure(tone(0.3, SR), SR) is None
    assert loudness.measure(np.zeros(SR, np.float32), SR) is None
    assert loudness.measure(tone(1.0, SR), SR) == pytest.approx(lufs(tone(1.0, SR)))


def test_plan_gains_clamps_per_segment_correction():
    gains = loudness.plan_gains([-30.0, -20.0, None], batch_loudness=-24.0, target_lufs=-16.0, max_correction_db=4.0)
    # batch gain +8 dB; segment 1 wants +14 (correction +6 → clamped +4); segment 2 wants +4 (correction -4).
    assert gains == pytest.approx([12.0, 4.0, 8.0])
    assert loudness.plan_gains([-20.0], None, -16.0, 2.0) == pytest.approx([2.0])


def test_apply_gain_respects_peak_ceiling_without_limiting():
    wave = tone(1.0, SR, amplitude=0.5)
    out, applied = loudness.apply_gain(wave, 12.0, -1.5)
    ceiling = 10 ** (-1.5 / 20)
    assert np.abs(out).max() == pytest.approx(ceiling, rel=1e-5)
    assert applied == pytest.approx(20 * np.log10(ceiling / 0.5), abs=1e-3)
    # Static scaling only: the waveform shape is unchanged.
    assert np.allclose(out / out.max(), wave / wave.max(), atol=1e-6)


def test_apply_gain_on_quiet_audio_hits_the_requested_gain():
    out, applied = loudness.apply_gain(tone(1.0, SR, amplitude=0.01), 6.0, -1.5)
    assert applied == pytest.approx(6.0) and np.abs(out).max() == pytest.approx(0.01 * 10 ** 0.3, rel=1e-4)


def speechlike(seconds: float, seed: int, level: float) -> np.ndarray:
    """Noise bursts with gaps, so gating has something to gate."""
    rng = np.random.default_rng(seed)
    wave = rng.normal(0, level, int(seconds * SR)).astype(np.float32)
    envelope = (np.sin(np.arange(wave.size) / SR * 2 * np.pi * 1.3) > -0.3).astype(np.float32)
    return wave * envelope


def test_programme_equals_measure_blocks_on_the_concatenation_for_random_clip_lengths():
    # Lengths are random to the sample, so block edges fall anywhere relative to clip joins and trailing remainders.
    rng = np.random.default_rng(22)
    checked = 0
    for case in range(25):
        clips = [speechlike(float(rng.uniform(0.3, 6.0)), 100 * case + k, float(rng.uniform(0.01, 0.3)))
                 for k in range(int(rng.integers(1, 6)))]
        programme = loudness.Programme(SR)
        for i, clip in enumerate(clips, start=1):
            programme.add(clip)
            expected = loudness.measure_blocks(np.concatenate(clips[:i]), SR)
            if expected is None:
                assert programme.loudness() is None
                continue
            assert programme.loudness() == pytest.approx(expected, abs=1e-9)
            checked += 1
    assert checked >= 50


def test_measure_blocks_equals_pyloudnorm_when_no_partial_block_is_left():
    # pyloudnorm adds a partial trailing block when more than 50 ms remain after the last full hop; at whole hops
    # (and up to 50 ms past them) both meters count the same complete blocks.
    for seconds, seed in ((3.0, 1), (0.7, 2), (5.34, 3), (2.04, 4)):
        wave = speechlike(seconds, seed, 0.1)
        assert loudness.measure_blocks(wave, SR) == pytest.approx(lufs(wave), abs=1e-9)
    assert loudness.measure_blocks(tone(0.3, SR), SR) is None
    assert loudness.measure_blocks(np.zeros(SR, np.float32), SR) is None


def test_programme_of_nothing_measurable_is_none():
    programme = loudness.Programme(SR)
    assert programme.loudness() is None
    programme.add(np.zeros(int(0.3 * SR), np.float32))  # shorter than one 400 ms block
    assert programme.loudness() is None
    programme.add(np.zeros(SR, np.float32))
    assert programme.loudness() is None  # every block under the -70 LUFS gate
    programme.add(tone(1.0, SR))
    assert programme.loudness() is not None
