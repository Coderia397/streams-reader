import numpy as np
import pyloudnorm
import pytest
from conftest import KOKORO_AUDIO, MING_AUDIO, tone

from nolabs_engine import contracts as c
from nolabs_engine.audio import pipeline, silence
from nolabs_engine.audio.pipeline import RawClip, SilentOutput

SR = 24_000


def profile(body: dict) -> c.AudioProfile:
    return c.parse_engine_request({
        "v": 1, "job_id": "7q9vbxz2k4m6n8p0", "engine": "kokoro", "voice": {"kind": "preset", "id": "af_heart"},
        "items": [{"key": "1" * 64, "text": "x", "mode": "kokoro", "object_key": f"audio/kokoro/{'1' * 64}.webm"}],
        "audio": body, "marker_key": "jobs/7q9vbxz2k4m6n8p0.json"}).audio


class Recorder:
    """Fake encoder: keeps the PCM it was given and returns it as bytes."""

    def __init__(self, fail_on: int | None = None):
        self.calls: list[tuple[np.ndarray, int, int, int]] = []
        self.fail_on = fail_on

    def __call__(self, wave, source_rate, out_rate, kbps):
        self.calls.append((wave, source_rate, out_rate, kbps))
        if self.fail_on is not None and len(self.calls) - 1 == self.fail_on:
            raise RuntimeError("encoder broke")
        return wave.tobytes()


def with_silence(wave: np.ndarray, lead_s: float, tail_s: float, sr: int = SR) -> np.ndarray:
    return np.concatenate([np.zeros(int(lead_s * sr), np.float32), wave, np.zeros(int(tail_s * sr), np.float32)])


def lufs(wave: np.ndarray, sr: int = SR) -> float:
    return pyloudnorm.Meter(sr).integrated_loudness(wave.astype(np.float64))


def test_batch_trims_levels_and_appends_pause_by_boundary():
    loud, quiet = tone(2.0, SR, amplitude=0.3), tone(2.0, SR, amplitude=0.03)
    clips = [RawClip(with_silence(loud, 0.4, 0.6), SR, "sentence"),
             RawClip(with_silence(quiet, 0.1, 0.2), SR, "section")]
    encoder = Recorder()
    out = pipeline.finish_batch(clips, profile(KOKORO_AUDIO), encoder)
    assert all(isinstance(o, pipeline.FinishedClip) for o in out)
    first, second = out
    guard = 0.03
    assert first.lead_ms == pytest.approx((0.4 - guard) * 1000, abs=1)
    assert first.speech_ms == pytest.approx((2.0 + 2 * guard) * 1000, abs=1)
    assert first.duration_ms == first.speech_ms + 250 and second.duration_ms == second.speech_ms + 1200
    # Encoder got source rate 24 kHz, profile output rate and bitrate.
    assert [(s, o, k) for _, s, o, k in encoder.calls] == [(SR, 24000, 32), (SR, 24000, 32)]
    speech = [encoder.calls[i][0][: int(o.speech_ms * SR / 1000)] for i, o in enumerate(out)]
    levels = [lufs(s) for s in speech]
    # Batch loudness is gated like any EBU R128 programme, so the clip 20 dB down barely counts; the loud clip needs
    # only a small correction and lands on the target, the quiet one gets the batch gain plus the 4 dB maximum.
    trimmed = [silence.trim(clip.wave, SR).wave for clip in clips]
    batch_gain = -16.0 - lufs(np.concatenate(trimmed))
    assert levels[0] == pytest.approx(-16.0, abs=0.2)
    assert second.gain_db == pytest.approx(batch_gain + 4.0, abs=0.01)
    assert levels[1] == pytest.approx(lufs(trimmed[1]) + batch_gain + 4.0, abs=0.2)
    ceiling = 10 ** (-1.5 / 20)
    assert all(np.abs(s).max() <= ceiling + 1e-6 for s in speech)
    # Pause is digital silence.
    assert not encoder.calls[1][0][len(speech[1]):].any()


def test_peak_ceiling_wins_over_loudness_target():
    # A crest-heavy clip: short loud click train on a quiet bed; reaching -16 LUFS would push the peak over -1.5 dBFS.
    wave = tone(2.0, SR, amplitude=0.02)
    wave[::2400] = 0.9
    out = pipeline.finish_batch([RawClip(wave, SR, "sentence")], profile(KOKORO_AUDIO), Recorder())
    assert out[0].gain_db < 1.0


def test_failures_pass_through_and_stay_aligned():
    failure = RuntimeError("synthesis failed")
    clips = [failure, RawClip(np.zeros(SR, np.float32), SR, "sentence"), RawClip(tone(1.0, SR), SR, "paragraph"),
             RawClip(tone(1.0, SR), SR, "sentence")]
    out = pipeline.finish_batch(clips, profile(KOKORO_AUDIO), Recorder(fail_on=1))
    assert out[0] is failure
    assert isinstance(out[1], SilentOutput)
    assert isinstance(out[2], pipeline.FinishedClip)
    assert isinstance(out[3], RuntimeError) and "encoder broke" in str(out[3])


def test_ming_profile_resamples_on_encode():
    sr = 44_100
    out = pipeline.finish_batch([RawClip(tone(1.0, sr), sr, "paragraph")], profile(MING_AUDIO), rec := Recorder())
    assert rec.calls[0][1:] == (sr, 48000, 56)
    assert out[0].duration_ms == out[0].speech_ms + 600


def test_nan_output_is_sanitised():
    wave = tone(1.0, SR)
    wave[100:200] = np.nan
    out = pipeline.finish_batch([RawClip(wave, SR, "sentence")], profile(KOKORO_AUDIO), rec := Recorder())
    assert isinstance(out[0], pipeline.FinishedClip) and np.isfinite(rec.calls[0][0]).all()
